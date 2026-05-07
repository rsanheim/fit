use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use crate::repo::repo_display_name;
use crate::trace::{RepoTraceSample, TraceSink};

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

/// URL scheme to force for git operations
#[derive(Clone, Copy)]
pub enum UrlScheme {
    /// Force SSH: git@github.com:user/repo
    Ssh,
    /// Force HTTPS: https://github.com/user/repo
    Https,
}

/// Format repo name with fixed width: truncate long names, pad short ones
fn compute_name_width(repos: &[PathBuf], display_root: &Path) -> usize {
    let mut max_len = 0usize;
    for repo in repos {
        let name = repo_display_name(repo, display_root);
        max_len = max_len.max(name.len());
    }

    let capped = max_len.min(MAX_REPO_NAME_WIDTH_CAP);
    capped.max(MIN_REPO_NAME_WIDTH)
}

/// Format repo name with fixed width: truncate long names, pad short ones
fn format_repo_name(name: &str, width: usize) -> String {
    let display_name = if name.len() > width {
        if width <= 4 {
            name.chars().take(width).collect()
        } else {
            format!("{}-...", &name[..width - 4])
        }
    } else {
        name.to_string()
    };
    format!("[{:<width$}]", display_name, width = width)
}

/// Cross-cutting options that apply to every git invocation in a run.
#[derive(Clone, Copy)]
pub struct GitInvocationOptions {
    pub url_scheme: Option<UrlScheme>,
}

/// Execution context holding configuration for running git commands
pub struct ExecutionContext {
    dry_run: bool,
    url_scheme: Option<UrlScheme>,
    max_connections: usize,
    display_root: PathBuf,
    trace: TraceSink,
}

impl ExecutionContext {
    pub fn new(
        dry_run: bool,
        url_scheme: Option<UrlScheme>,
        max_connections: usize,
        display_root: PathBuf,
        trace: TraceSink,
    ) -> Self {
        Self {
            dry_run,
            url_scheme,
            max_connections,
            display_root,
            trace,
        }
    }

    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    pub fn git_invocation_options(&self) -> GitInvocationOptions {
        GitInvocationOptions {
            url_scheme: self.url_scheme,
        }
    }

    pub fn max_connections(&self) -> usize {
        self.max_connections
    }

    pub fn display_root(&self) -> &std::path::Path {
        &self.display_root
    }

    pub fn trace_enabled(&self) -> bool {
        self.trace.enabled()
    }

    pub fn trace_mut(&mut self) -> &mut TraceSink {
        &mut self.trace
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
    pub fn spawn(&self, opts: GitInvocationOptions) -> std::io::Result<std::process::Child> {
        let mut cmd = Command::new("git");

        // Inject URL scheme override if specified (must come before other args)
        if let Some(scheme) = opts.url_scheme {
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
    pub fn command_string(&self, opts: GitInvocationOptions) -> String {
        let scheme_args = match opts.url_scheme {
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

/// Run commands in parallel across all repos with streaming output.
///
/// Results are printed in alphabetical order (repos are pre-sorted) as soon as
/// contiguous results are available. Uses head-of-line blocking: if repo "aaa"
/// is slow, "bbb" and "ccc" won't print until "aaa" completes.
///
/// Uses thread-per-process pattern with `wait_with_output()` which is deadlock-safe
/// (stdlib internally spawns threads to drain stdout/stderr concurrently).
pub fn run_parallel<F>(
    ctx: &mut ExecutionContext,
    repos: &[PathBuf],
    build_command: F,
    formatter: &dyn OutputFormatter,
) -> Result<()>
where
    F: Fn(&PathBuf) -> GitCommand + Sync,
{
    let opts = ctx.git_invocation_options();
    let trace_enabled = ctx.trace_enabled();

    if ctx.is_dry_run() {
        for repo in repos {
            let cmd = build_command(repo);
            println!("{}", cmd.command_string(opts));
        }
        return Ok(());
    }

    let name_width = compute_name_width(repos, ctx.display_root());
    let run_started_at = Instant::now();

    let max_workers = ctx.max_connections();

    let semaphore = if max_workers > 0 && max_workers < repos.len() {
        Some(Arc::new(Semaphore::new(max_workers)))
    } else {
        None
    };

    let mut results: Vec<
        Option<(
            PathBuf,
            Result<Output, std::io::Error>,
            Option<RepoTraceSample>,
        )>,
    > = (0..repos.len()).map(|_| None).collect();
    let mut next_to_print: usize = 0;
    let mut first_exit_ms: Option<u128> = None;
    let mut first_print_ms: Option<u128> = None;
    let mut delayed_repos: usize = 0;
    let mut max_ordered_wait_ms: u128 = 0;

    let (tx, rx) = mpsc::channel();

    std::thread::scope(|s| -> Result<()> {
        for (idx, repo) in repos.iter().enumerate() {
            let tx = tx.clone();
            let cmd = build_command(repo);
            let repo = repo.clone();
            let sem = semaphore.clone();

            s.spawn(move || {
                if let Some(ref sem) = sem {
                    sem.acquire();
                }

                let start_ms = if trace_enabled {
                    Some(run_started_at.elapsed().as_millis())
                } else {
                    None
                };
                let spawn_result = cmd.spawn(opts);
                let spawn_ms = if trace_enabled {
                    Some(run_started_at.elapsed().as_millis())
                } else {
                    None
                };
                let result = match spawn_result {
                    Ok(child) => child.wait_with_output(),
                    Err(err) => Err(err),
                };
                let trace_sample = if trace_enabled {
                    let exit_ms = run_started_at.elapsed().as_millis();
                    Some(match &result {
                        Ok(output) => RepoTraceSample {
                            start_ms: start_ms.expect("trace enabled start_ms"),
                            spawn_ms: spawn_ms.expect("trace enabled spawn_ms"),
                            exit_ms,
                            stdout_bytes: output.stdout.len(),
                            stderr_bytes: output.stderr.len(),
                            success: output.status.success(),
                        },
                        Err(_) => RepoTraceSample {
                            start_ms: start_ms.expect("trace enabled start_ms"),
                            spawn_ms: spawn_ms.expect("trace enabled spawn_ms"),
                            exit_ms,
                            stdout_bytes: 0,
                            stderr_bytes: 0,
                            success: false,
                        },
                    })
                } else {
                    None
                };

                if let Some(ref sem) = sem {
                    sem.release();
                }

                let _ = tx.send((idx, repo, result, trace_sample));
            });
        }
        drop(tx);

        for (idx, repo, result, trace_sample) in rx {
            results[idx] = Some((repo, result, trace_sample));

            while next_to_print < results.len() {
                if let Some((ref repo_path, ref res, sample)) = results[next_to_print] {
                    print_result(repo_path, res, formatter, ctx.display_root(), name_width);
                    if let Some(sample) = sample {
                        let printed_ms = run_started_at.elapsed().as_millis();
                        let ordered_wait_ms = sample.ordered_wait_ms(printed_ms);
                        let repo_name = repo_display_name(repo_path, ctx.display_root());
                        first_exit_ms = Some(
                            first_exit_ms
                                .map_or(sample.exit_ms, |current| current.min(sample.exit_ms)),
                        );
                        first_print_ms = Some(
                            first_print_ms.map_or(printed_ms, |current| current.min(printed_ms)),
                        );
                        if ordered_wait_ms > 0 {
                            delayed_repos += 1;
                        }
                        max_ordered_wait_ms = max_ordered_wait_ms.max(ordered_wait_ms);
                        ctx.trace_mut()
                            .emit_repo(next_to_print, &repo_name, sample, printed_ms)?;
                    }
                    next_to_print += 1;
                } else {
                    break;
                }
            }
        }
        Ok(())
    })?;

    ctx.trace_mut().emit_summary(
        repos.len(),
        first_exit_ms,
        first_print_ms,
        delayed_repos,
        max_ordered_wait_ms,
        run_started_at.elapsed().as_millis(),
    )?;

    Ok(())
}

/// Print result for a single repository
fn print_result(
    repo_path: &std::path::Path,
    result: &Result<Output, std::io::Error>,
    formatter: &dyn OutputFormatter,
    display_root: &std::path::Path,
    name_width: usize,
) {
    let name = repo_display_name(repo_path, display_root);
    let output_line = match result {
        Ok(output) => {
            let formatted = formatter.format(output);
            format!("{} {}", format_repo_name(&name, name_width), formatted)
        }
        Err(e) => format!("{} ERROR: {}", format_repo_name(&name, name_width), e),
    };
    println!("{}", output_line);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_repo_name_short() {
        let result = format_repo_name("my-repo", 24);
        assert_eq!(result, "[my-repo                 ]");
        assert_eq!(result.len(), 26); // [ + 24 + ]
    }

    #[test]
    fn test_format_repo_name_exact_length() {
        let result = format_repo_name("exactly-twenty-four-chr", 24);
        assert_eq!(result.len(), 26);
    }

    #[test]
    fn test_format_repo_name_truncated() {
        let result = format_repo_name("this-is-a-very-long-repository-name", 24);
        assert_eq!(result, "[this-is-a-very-long--...]");
        assert_eq!(result.len(), 26);
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
