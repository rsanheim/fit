use anyhow::Result;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::Duration;

use crate::repo::repo_name;

const MAX_REPO_NAME_WIDTH: usize = 24;

/// URL scheme to force for git operations
#[derive(Clone, Copy)]
pub enum UrlScheme {
    /// Force SSH: git@github.com:user/repo
    Ssh,
    /// Force HTTPS: https://github.com/user/repo
    Https,
}

/// Format repo name with fixed width: truncate long names, pad short ones
fn format_repo_name(name: &str) -> String {
    let display_name = if name.len() > MAX_REPO_NAME_WIDTH {
        format!("{}-...", &name[..MAX_REPO_NAME_WIDTH - 4])
    } else {
        name.to_string()
    };
    format!("[{:<width$}]", display_name, width = MAX_REPO_NAME_WIDTH)
}

/// Execution context holding configuration for running git commands
pub struct ExecutionContext {
    dry_run: bool,
    url_scheme: Option<UrlScheme>,
    max_connections: usize,
}

impl ExecutionContext {
    pub fn new(dry_run: bool, url_scheme: Option<UrlScheme>, max_connections: usize) -> Self {
        Self {
            dry_run,
            url_scheme,
            max_connections,
        }
    }

    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    pub fn url_scheme(&self) -> Option<UrlScheme> {
        self.url_scheme
    }

    pub fn max_connections(&self) -> usize {
        self.max_connections
    }
}

/// A git command ready to be executed against a repository
pub struct GitCommand {
    pub repo_path: PathBuf,
    pub args: Vec<String>,
}

impl GitCommand {
    pub fn new(repo_path: PathBuf, args: Vec<String>) -> Self {
        Self { repo_path, args }
    }

    /// Spawn the git command without waiting for completion.
    /// Returns immediately with a Child process handle.
    pub fn spawn(&self, url_scheme: Option<UrlScheme>) -> std::io::Result<Child> {
        let mut cmd = Command::new("git");

        // Inject URL scheme override if specified (must come before other args)
        if let Some(scheme) = url_scheme {
            match scheme {
                UrlScheme::Ssh => {
                    cmd.arg("-c")
                        .arg("url.git@github.com:.insteadOf=https://github.com/");
                }
                UrlScheme::Https => {
                    cmd.arg("-c")
                        .arg("url.https://github.com/.insteadOf=git@github.com:");
                }
            }
        }

        cmd.arg("-C")
            .arg(&self.repo_path)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("GIT_TERMINAL_PROMPT", "0")
            .spawn()
    }

    /// Build the full command string for display (used in dry-run)
    pub fn command_string_with_scheme(&self, url_scheme: Option<UrlScheme>) -> String {
        let scheme_args = match url_scheme {
            Some(UrlScheme::Ssh) => "-c \"url.git@github.com:.insteadOf=https://github.com/\" ",
            Some(UrlScheme::Https) => "-c \"url.https://github.com/.insteadOf=git@github.com:\" ",
            None => "",
        };
        format!(
            "git {}-C {} {}",
            scheme_args,
            self.repo_path.display(),
            self.args.join(" ")
        )
    }
}

/// Trait for formatting command output into one line
pub trait OutputFormatter: Sync {
    fn format(&self, output: &Output) -> String;
}

/// A spawned git process with its associated repo info (used by unlimited mode)
struct SpawnedCommand {
    repo_path: PathBuf,
    child: Result<Child, std::io::Error>,
}

/// An active git process being tracked in limited mode
struct ActiveProcess {
    index: usize,
    repo_path: PathBuf,
    child: Child,
}

/// Completed process output waiting to be printed in order
struct CompletedOutput {
    index: usize,
    repo_path: PathBuf,
    output: Result<Output, std::io::Error>,
}

/// Run commands in parallel across all repos.
/// Respects max_connections limit if set, otherwise spawns all immediately.
pub fn run_parallel<F>(
    ctx: &ExecutionContext,
    repos: &[PathBuf],
    build_command: F,
    formatter: &dyn OutputFormatter,
) -> Result<()>
where
    F: Fn(&PathBuf) -> GitCommand,
{
    let url_scheme = ctx.url_scheme();

    // Handle dry-run mode separately
    if ctx.is_dry_run() {
        for repo in repos {
            let cmd = build_command(repo);
            println!("{}", cmd.command_string_with_scheme(url_scheme));
        }
        return Ok(());
    }

    let max_conn = ctx.max_connections();

    // Use unlimited (spawn-all) when limit is 0 or >= repo count
    if max_conn == 0 || max_conn >= repos.len() {
        run_parallel_unlimited(repos, &build_command, formatter, url_scheme)
    } else {
        run_parallel_limited(repos, &build_command, formatter, url_scheme, max_conn)
    }
}

/// Original spawn-first pattern: spawn all processes immediately, wait in order.
fn run_parallel_unlimited<F>(
    repos: &[PathBuf],
    build_command: &F,
    formatter: &dyn OutputFormatter,
    url_scheme: Option<UrlScheme>,
) -> Result<()>
where
    F: Fn(&PathBuf) -> GitCommand,
{
    // Phase 1: Spawn all git processes immediately (non-blocking)
    let spawned: Vec<SpawnedCommand> = repos
        .iter()
        .map(|repo| {
            let cmd = build_command(repo);
            SpawnedCommand {
                repo_path: repo.clone(),
                child: cmd.spawn(url_scheme),
            }
        })
        .collect();

    // Phase 2: Wait for each process and print results in order
    for spawned_cmd in spawned {
        print_spawned_result(spawned_cmd, formatter);
    }

    Ok(())
}

/// Sliding window pattern: maintain at most max_conn active processes.
fn run_parallel_limited<F>(
    repos: &[PathBuf],
    build_command: &F,
    formatter: &dyn OutputFormatter,
    url_scheme: Option<UrlScheme>,
    max_conn: usize,
) -> Result<()>
where
    F: Fn(&PathBuf) -> GitCommand,
{
    let mut next_to_spawn = 0;
    let mut next_to_print = 0;
    let mut active: Vec<ActiveProcess> = Vec::with_capacity(max_conn);
    let mut completed: Vec<CompletedOutput> = Vec::new();

    // Initial burst: spawn up to max_conn
    while next_to_spawn < repos.len() && active.len() < max_conn {
        spawn_process(
            repos,
            build_command,
            url_scheme,
            next_to_spawn,
            &mut active,
            &mut completed,
        );
        next_to_spawn += 1;
    }

    // Main loop: poll active processes, spawn new ones, print completed in order
    while !active.is_empty() || next_to_print < repos.len() {
        // Check each active process with try_wait
        let mut i = 0;
        while i < active.len() {
            match active[i].child.try_wait() {
                Ok(Some(status)) => {
                    // Process finished - remove from active and collect output
                    let mut proc = active.swap_remove(i);
                    let output = collect_child_output(&mut proc.child, status);
                    completed.push(CompletedOutput {
                        index: proc.index,
                        repo_path: proc.repo_path,
                        output: Ok(output),
                    });
                    // Don't increment i - swap_remove moved last element here
                }
                Ok(None) => {
                    // Still running
                    i += 1;
                }
                Err(e) => {
                    // Error checking status - treat as failed
                    let proc = active.swap_remove(i);
                    completed.push(CompletedOutput {
                        index: proc.index,
                        repo_path: proc.repo_path,
                        output: Err(e),
                    });
                }
            }
        }

        // Spawn new processes if we have capacity
        while next_to_spawn < repos.len() && active.len() < max_conn {
            spawn_process(
                repos,
                build_command,
                url_scheme,
                next_to_spawn,
                &mut active,
                &mut completed,
            );
            next_to_spawn += 1;
        }

        // Print any completed outputs that are ready (in order)
        while let Some(pos) = completed.iter().position(|c| c.index == next_to_print) {
            let c = completed.swap_remove(pos);
            print_completed_output(&c, formatter);
            next_to_print += 1;
        }

        // If all printed, we're done
        if next_to_print >= repos.len() {
            break;
        }

        // Small sleep to avoid busy-waiting
        if !active.is_empty() {
            thread::sleep(Duration::from_millis(5));
        }
    }

    Ok(())
}

/// Spawn a single git process, adding to active or completed list.
fn spawn_process<F>(
    repos: &[PathBuf],
    build_command: &F,
    url_scheme: Option<UrlScheme>,
    index: usize,
    active: &mut Vec<ActiveProcess>,
    completed: &mut Vec<CompletedOutput>,
) where
    F: Fn(&PathBuf) -> GitCommand,
{
    let repo = &repos[index];
    let cmd = build_command(repo);
    match cmd.spawn(url_scheme) {
        Ok(child) => {
            active.push(ActiveProcess {
                index,
                repo_path: repo.clone(),
                child,
            });
        }
        Err(e) => {
            // Spawn failed - store error result immediately
            completed.push(CompletedOutput {
                index,
                repo_path: repo.clone(),
                output: Err(e),
            });
        }
    }
}

/// Collect stdout/stderr from a child after try_wait returned Some.
fn collect_child_output(child: &mut Child, status: std::process::ExitStatus) -> Output {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    if let Some(ref mut out) = child.stdout {
        let _ = out.read_to_end(&mut stdout);
    }
    if let Some(ref mut err) = child.stderr {
        let _ = err.read_to_end(&mut stderr);
    }

    Output {
        status,
        stdout,
        stderr,
    }
}

/// Print result from a SpawnedCommand (used by unlimited mode).
fn print_spawned_result(spawned_cmd: SpawnedCommand, formatter: &dyn OutputFormatter) {
    let name = repo_name(&spawned_cmd.repo_path);
    let output_line = match spawned_cmd.child {
        Ok(child) => match child.wait_with_output() {
            Ok(output) => {
                let formatted = formatter.format(&output);
                format!("{} {}", format_repo_name(&name), formatted)
            }
            Err(e) => format!("{} ERROR: {}", format_repo_name(&name), e),
        },
        Err(e) => format!("{} ERROR: spawn failed: {}", format_repo_name(&name), e),
    };
    println!("{}", output_line);
}

/// Print a CompletedOutput (used by limited mode).
fn print_completed_output(c: &CompletedOutput, formatter: &dyn OutputFormatter) {
    let name = repo_name(&c.repo_path);
    let output_line = match &c.output {
        Ok(output) => {
            let formatted = formatter.format(output);
            format!("{} {}", format_repo_name(&name), formatted)
        }
        Err(e) => format!("{} ERROR: {}", format_repo_name(&name), e),
    };
    println!("{}", output_line);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_repo_name_short() {
        let result = format_repo_name("my-repo");
        assert_eq!(result, "[my-repo                 ]");
        assert_eq!(result.len(), 26); // [ + 24 + ]
    }

    #[test]
    fn test_format_repo_name_exact_length() {
        let result = format_repo_name("exactly-twenty-four-chr");
        assert_eq!(result.len(), 26);
    }

    #[test]
    fn test_format_repo_name_truncated() {
        let result = format_repo_name("this-is-a-very-long-repository-name");
        assert_eq!(result, "[this-is-a-very-long--...]");
        assert_eq!(result.len(), 26);
    }
}
