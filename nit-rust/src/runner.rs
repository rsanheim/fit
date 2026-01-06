use anyhow::Result;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Condvar, Mutex};

use crate::repo::repo_name;

/// Simple counting semaphore using stdlib primitives.
/// Allows limiting concurrent operations to N at a time.
struct Semaphore {
    count: Mutex<usize>,
    cond: Condvar,
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        Semaphore {
            count: Mutex::new(permits),
            cond: Condvar::new(),
        }
    }

    /// Acquire a permit, blocking if none available.
    fn acquire(&self) {
        let mut count = self.count.lock().unwrap();
        while *count == 0 {
            count = self.cond.wait(count).unwrap();
        }
        *count -= 1;
    }

    /// Release a permit, waking one waiting thread.
    fn release(&self) {
        let mut count = self.count.lock().unwrap();
        *count += 1;
        self.cond.notify_one();
    }
}

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
    pub fn spawn(&self, url_scheme: Option<UrlScheme>) -> std::io::Result<std::process::Child> {
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

/// Run commands in parallel across all repos.
///
/// Uses thread-per-process pattern with `wait_with_output()` which is deadlock-safe
/// (stdlib internally spawns threads to drain stdout/stderr concurrently).
///
/// Respects max_connections limit via channel-as-semaphore pattern.
pub fn run_parallel<F>(
    ctx: &ExecutionContext,
    repos: &[PathBuf],
    build_command: F,
    formatter: &dyn OutputFormatter,
) -> Result<()>
where
    F: Fn(&PathBuf) -> GitCommand + Sync,
{
    let url_scheme = ctx.url_scheme();

    // Handle dry-run mode
    if ctx.is_dry_run() {
        for repo in repos {
            let cmd = build_command(repo);
            println!("{}", cmd.command_string_with_scheme(url_scheme));
        }
        return Ok(());
    }

    let max_workers = ctx.max_connections();

    // Determine whether to use concurrency limiting
    // Skip semaphore when unlimited (0) or when workers >= repos
    let use_semaphore = max_workers > 0 && max_workers < repos.len();

    // Spawn threads, collect results with indices for ordered output
    let results: Vec<(usize, PathBuf, Result<Output, std::io::Error>)> = if use_semaphore {
        run_with_semaphore(repos, &build_command, url_scheme, max_workers)
    } else {
        run_unlimited(repos, &build_command, url_scheme)
    };

    // Sort by index and print in discovery order
    let mut sorted = results;
    sorted.sort_by_key(|(idx, _, _)| *idx);

    for (_, repo_path, result) in sorted {
        print_result(&repo_path, &result, formatter);
    }

    Ok(())
}

/// Run with concurrency limiting via semaphore
fn run_with_semaphore<F>(
    repos: &[PathBuf],
    build_command: &F,
    url_scheme: Option<UrlScheme>,
    max_workers: usize,
) -> Vec<(usize, PathBuf, Result<Output, std::io::Error>)>
where
    F: Fn(&PathBuf) -> GitCommand + Sync,
{
    let semaphore = Arc::new(Semaphore::new(max_workers));

    std::thread::scope(|s| {
        let handles: Vec<_> = repos
            .iter()
            .enumerate()
            .map(|(idx, repo)| {
                let cmd = build_command(repo);
                let sem = Arc::clone(&semaphore);
                s.spawn(move || {
                    // Acquire permit (blocks if max_workers processes already running)
                    sem.acquire();

                    let result = cmd
                        .spawn(url_scheme)
                        .and_then(|child| child.wait_with_output());

                    // Release permit for next thread
                    sem.release();

                    (idx, repo.clone(), result)
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect()
    })
}

/// Run unlimited: spawn all processes immediately
fn run_unlimited<F>(
    repos: &[PathBuf],
    build_command: &F,
    url_scheme: Option<UrlScheme>,
) -> Vec<(usize, PathBuf, Result<Output, std::io::Error>)>
where
    F: Fn(&PathBuf) -> GitCommand + Sync,
{
    std::thread::scope(|s| {
        let handles: Vec<_> = repos
            .iter()
            .enumerate()
            .map(|(idx, repo)| {
                let cmd = build_command(repo);
                s.spawn(move || {
                    let result = cmd
                        .spawn(url_scheme)
                        .and_then(|child| child.wait_with_output());
                    (idx, repo.clone(), result)
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect()
    })
}

/// Print result for a single repository
fn print_result(
    repo_path: &std::path::Path,
    result: &Result<Output, std::io::Error>,
    formatter: &dyn OutputFormatter,
) {
    let name = repo_name(repo_path);
    let output_line = match result {
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

    /// Test that large output (>64KB) doesn't cause pipe buffer deadlock.
    /// wait_with_output() internally spawns threads to drain pipes, so this should complete.
    #[test]
    fn test_large_output_no_deadlock() {
        use std::process::Stdio;
        use std::time::{Duration, Instant};

        let start = Instant::now();
        let timeout = Duration::from_secs(5);

        // Spawn a process that outputs 100KB (more than 64KB pipe buffer)
        let child = Command::new("head")
            .args(["-c", "100000", "/dev/zero"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn head command");

        // wait_with_output handles pipe draining internally - no deadlock
        let output = child.wait_with_output().expect("Failed to wait for output");

        // Verify we got all the output
        assert_eq!(
            output.stdout.len(),
            100000,
            "Expected 100000 bytes, got {}",
            output.stdout.len()
        );

        // Verify it didn't take suspiciously long (would indicate near-deadlock)
        assert!(
            start.elapsed() < timeout,
            "Test took too long - possible deadlock: {:?}",
            start.elapsed()
        );
    }
}
