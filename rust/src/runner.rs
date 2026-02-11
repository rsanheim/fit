use anyhow::Result;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};

use crate::repo::repo_display_name;

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

const MIN_REPO_NAME_WIDTH: usize = 4;
const MAX_REPO_NAME_WIDTH_CAP: usize = 48;
const MAX_BRANCH_WIDTH_CAP: usize = 14;

/// URL scheme to force for git operations
#[derive(Clone, Copy)]
pub enum UrlScheme {
    /// Force SSH: git@github.com:user/repo
    Ssh,
    /// Force HTTPS: https://github.com/user/repo
    Https,
}

/// Compute the display width for the repo name column
fn compute_name_width(repos: &[PathBuf], display_root: &Path) -> usize {
    let mut max_len = 0usize;
    for repo in repos {
        let name = repo_display_name(repo, display_root);
        max_len = max_len.max(name.len());
    }

    let capped = max_len.min(MAX_REPO_NAME_WIDTH_CAP);
    capped.max(MIN_REPO_NAME_WIDTH)
}

/// Format a value into a fixed-width column: truncate with `...`, pad short values
fn format_column(value: &str, width: usize) -> String {
    if value.len() > width {
        if width <= 3 {
            value.chars().take(width).collect()
        } else {
            let truncated: String = value.chars().take(width - 3).collect();
            format!("{truncated}...")
        }
    } else {
        format!("{value:<width$}")
    }
}

/// Resolve the current branch name for a repository
fn resolve_branch(repo_path: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if branch == "HEAD" {
                "HEAD (detached)".to_string()
            } else {
                branch
            }
        }
        _ => "unknown".to_string(),
    }
}

/// Execution context holding configuration for running git commands
pub struct ExecutionContext {
    dry_run: bool,
    url_scheme: Option<UrlScheme>,
    max_connections: usize,
    display_root: PathBuf,
}

impl ExecutionContext {
    pub fn new(
        dry_run: bool,
        url_scheme: Option<UrlScheme>,
        max_connections: usize,
        display_root: PathBuf,
    ) -> Self {
        Self {
            dry_run,
            url_scheme,
            max_connections,
            display_root,
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

    pub fn display_root(&self) -> &std::path::Path {
        &self.display_root
    }
}

/// A git command ready to be executed against a repository
pub struct GitCommand {
    pub repo_path: PathBuf,
    /// Global git options placed before `-C` (e.g., `--no-optional-locks`)
    pub global_args: Vec<String>,
    /// Subcommand and its arguments placed after `-C <repo>`
    pub args: Vec<String>,
}

impl GitCommand {
    pub fn new(repo_path: PathBuf, args: Vec<String>) -> Self {
        Self {
            repo_path,
            global_args: Vec::new(),
            args,
        }
    }

    pub fn with_global_args(
        repo_path: PathBuf,
        global_args: Vec<String>,
        args: Vec<String>,
    ) -> Self {
        Self {
            repo_path,
            global_args,
            args,
        }
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

        // Global git options before -C
        cmd.args(&self.global_args);

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
        let global = if self.global_args.is_empty() {
            String::new()
        } else {
            format!("{} ", self.global_args.join(" "))
        };
        format!(
            "git {}{}-C {} {}",
            scheme_args,
            global,
            self.repo_path.display(),
            self.args.join(" ")
        )
    }
}

/// Result of formatting command output for display
pub struct FormattedResult {
    pub branch: String,
    pub message: String,
}

impl FormattedResult {
    /// Create a result with only a message, leaving branch empty.
    /// Used by formatters (fetch, pull) that don't extract branch info from output.
    pub fn message_only(message: String) -> Self {
        Self {
            branch: String::new(),
            message,
        }
    }
}

/// Trait for formatting command output into a structured result
pub trait OutputFormatter: Sync {
    fn format(&self, output: &Output) -> FormattedResult;
}

/// Run commands in parallel across all repos with streaming output.
///
/// Results are printed progressively using head-of-line blocking: as each result
/// arrives, all contiguous results from the front are printed immediately.
/// Uses a fixed branch column width so output can stream without waiting for all results.
/// Output is printed in alphabetical order (repos are pre-sorted).
///
/// Uses thread-per-process pattern with `wait_with_output()` which is deadlock-safe
/// (stdlib internally spawns threads to drain stdout/stderr concurrently).
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

    if ctx.is_dry_run() {
        for repo in repos {
            let cmd = build_command(repo);
            println!("{}", cmd.command_string_with_scheme(url_scheme));
        }
        return Ok(());
    }

    let name_width = compute_name_width(repos, ctx.display_root());
    let branch_width = MAX_BRANCH_WIDTH_CAP;

    let max_workers = ctx.max_connections();

    let semaphore = if max_workers > 0 && max_workers < repos.len() {
        Some(Arc::new(Semaphore::new(max_workers)))
    } else {
        None
    };

    let (tx, rx) = mpsc::channel();

    std::thread::scope(|s| {
        for (idx, repo) in repos.iter().enumerate() {
            let tx = tx.clone();
            let cmd = build_command(repo);
            let repo = repo.clone();
            let sem = semaphore.clone();

            s.spawn(move || {
                if let Some(ref sem) = sem {
                    sem.acquire();
                }

                let branch = resolve_branch(&repo);
                let result = cmd.spawn(url_scheme).and_then(|c| c.wait_with_output());

                if let Some(ref sem) = sem {
                    sem.release();
                }

                let _ = tx.send((idx, repo, branch, result));
            });
        }
        drop(tx);

        let mut results: Vec<Option<(PathBuf, String, Result<Output, std::io::Error>)>> =
            (0..repos.len()).map(|_| None).collect();
        let mut next_to_print = 0;

        for (idx, repo, branch, result) in rx {
            results[idx] = Some((repo, branch, result));

            while next_to_print < results.len() && results[next_to_print].is_some() {
                let (repo_path, pre_branch, output_result) =
                    results[next_to_print].take().unwrap();
                let name = repo_display_name(&repo_path, ctx.display_root());

                let (branch, message) = match output_result {
                    Ok(ref output) => {
                        let fr = formatter.format(output);
                        let branch = if fr.branch.is_empty() {
                            pre_branch
                        } else {
                            fr.branch
                        };
                        (branch, fr.message)
                    }
                    Err(ref e) => (pre_branch, format!("ERROR: {e}")),
                };

                println!(
                    "{} | {} | {}",
                    format_column(&name, name_width),
                    format_column(&branch, branch_width),
                    message
                );
                next_to_print += 1;
            }
        }
    });

    Ok(())
}

/// Run passthrough commands across all repos, preserving git stdout/stderr output.
pub fn run_passthrough<F>(
    ctx: &ExecutionContext,
    repos: &[PathBuf],
    build_command: F,
) -> Result<()>
where
    F: Fn(&PathBuf) -> GitCommand + Sync,
{
    let url_scheme = ctx.url_scheme();

    if ctx.is_dry_run() {
        for repo in repos {
            let cmd = build_command(repo);
            println!("{}", cmd.command_string_with_scheme(url_scheme));
        }
        return Ok(());
    }

    let max_workers = ctx.max_connections();
    let semaphore = if max_workers > 0 && max_workers < repos.len() {
        Some(Arc::new(Semaphore::new(max_workers)))
    } else {
        None
    };

    let (tx, rx) = mpsc::channel();

    std::thread::scope(|s| {
        for (idx, repo) in repos.iter().enumerate() {
            let tx = tx.clone();
            let cmd = build_command(repo);
            let repo = repo.clone();
            let sem = semaphore.clone();

            s.spawn(move || {
                if let Some(ref sem) = sem {
                    sem.acquire();
                }

                let result = cmd.spawn(url_scheme).and_then(|c| c.wait_with_output());

                if let Some(ref sem) = sem {
                    sem.release();
                }

                let _ = tx.send((idx, repo, result));
            });
        }
        drop(tx);

        let mut results: Vec<Option<(PathBuf, Result<Output, std::io::Error>)>> =
            (0..repos.len()).map(|_| None).collect();
        let mut next_to_print = 0;

        for (idx, repo, result) in rx {
            results[idx] = Some((repo, result));

            while next_to_print < results.len() && results[next_to_print].is_some() {
                let (repo, result) = results[next_to_print].take().unwrap();
                match result {
                    Ok(output) => {
                        let _ = std::io::stdout().write_all(&output.stdout);
                        let _ = std::io::stderr().write_all(&output.stderr);
                    }
                    Err(err) => {
                        let _ = writeln!(
                            std::io::stderr(),
                            "git-all: failed to run git in {}: {}",
                            repo_display_name(&repo, ctx.display_root()),
                            err
                        );
                    }
                }
                next_to_print += 1;
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_column_short() {
        let result = format_column("my-repo", 24);
        assert_eq!(result, "my-repo                 ");
        assert_eq!(result.len(), 24);
    }

    #[test]
    fn test_format_column_exact_length() {
        let result = format_column("exactly-twenty-four-chr!", 24);
        assert_eq!(result, "exactly-twenty-four-chr!");
        assert_eq!(result.len(), 24);
    }

    #[test]
    fn test_format_column_truncated() {
        let result = format_column("this-is-a-very-long-repository-name", 24);
        assert_eq!(result, "this-is-a-very-long-r...");
        assert_eq!(result.len(), 24);
    }

    #[test]
    fn test_compute_name_width_caps_and_min() {
        let root = PathBuf::from("/workspace");
        let repos = vec![
            root.join("a"),
            root.join("short"),
            root.join("this-is-a-very-long-repository-name-that-exceeds-cap"),
        ];
        let width = compute_name_width(&repos, &root);
        assert_eq!(width, MAX_REPO_NAME_WIDTH_CAP);

        let tiny = vec![root.join("a")];
        let tiny_width = compute_name_width(&tiny, &root);
        assert_eq!(tiny_width, MIN_REPO_NAME_WIDTH);
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
