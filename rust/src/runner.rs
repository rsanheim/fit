use anyhow::Result;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use crossterm::tty::IsTty;

use crate::printer::{RepoRow, StreamPrinter, TtyPrinter};
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
const MIN_ID_WIDTH: usize = 3;

fn compute_repo_id_width(repo_count: usize) -> usize {
    repo_count.max(1).to_string().len().max(MIN_ID_WIDTH)
}

/// URL scheme to force for git operations
#[derive(Clone, Copy)]
pub enum UrlScheme {
    /// Force SSH: git@github.com:user/repo
    Ssh,
    /// Force HTTPS: https://github.com/user/repo
    Https,
}

/// Compute the max display name width across all repos, clamped to bounds.
fn compute_name_width(repos: &[PathBuf], display_root: &Path) -> usize {
    let mut max_len = 0usize;
    for repo in repos {
        let name = repo_display_name(repo, display_root);
        max_len = max_len.max(name.len());
    }

    let capped = max_len.min(MAX_REPO_NAME_WIDTH_CAP);
    capped.max(MIN_REPO_NAME_WIDTH)
}

/// Build RepoRow descriptors for all repos (sorted order, 1-indexed).
fn build_repo_rows(repos: &[PathBuf], display_root: &Path) -> Vec<RepoRow> {
    let name_width = compute_name_width(repos, display_root);
    let id_width = compute_repo_id_width(repos.len());
    repos
        .iter()
        .enumerate()
        .map(|(i, repo)| RepoRow {
            idx: i + 1,
            name: repo_display_name(repo, display_root),
            id_width,
            name_width,
        })
        .collect()
}

/// Format a git command result into a single status string.
pub fn format_status(
    result: &Result<Output, std::io::Error>,
    formatter: &dyn OutputFormatter,
) -> String {
    match result {
        Ok(output) => formatter.format(output),
        Err(e) => format!("ERROR: {}", e),
    }
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

    pub fn url_scheme(&self) -> Option<UrlScheme> {
        self.url_scheme
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

enum RepoEvent {
    Started { _idx: usize },
    Completed {
        idx: usize,
        result: Result<Output, std::io::Error>,
        trace: Option<RepoTraceSample>,
    },
}

/// Run commands in parallel across all repos with streaming output.
///
/// Repos are discovered and sorted deterministically up front. Results are then
/// printed in completion order with stable repo IDs derived from that sorted list.
///
/// TTY mode: completion-order lines with a sticky progress footer (crossterm).
/// Non-TTY mode: plain completion-order lines without ANSI escapes.
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
    let url_scheme = ctx.url_scheme();
    let trace_enabled = ctx.trace_enabled();

    if ctx.is_dry_run() {
        for repo in repos {
            let cmd = build_command(repo);
            println!("{}", cmd.command_string_with_scheme(url_scheme));
        }
        return Ok(());
    }

    let rows = build_repo_rows(repos, ctx.display_root());
    let run_started_at = Instant::now();

    let max_workers = ctx.max_connections();

    let semaphore = if max_workers > 0 && max_workers < repos.len() {
        Some(Arc::new(Semaphore::new(max_workers)))
    } else {
        None
    };

    let mut first_exit_ms: Option<u128> = None;
    let mut first_print_ms: Option<u128> = None;
    let mut delayed_repos: usize = 0;
    let mut max_ordered_wait_ms: u128 = 0;

    let (tx, rx) = mpsc::channel();

    let stdout = io::stdout();
    let is_tty = stdout.is_tty();

    std::thread::scope(|s| -> Result<()> {
        for (idx, repo) in repos.iter().enumerate() {
            let tx = tx.clone();
            let cmd = build_command(repo);
            let sem = semaphore.clone();

            s.spawn(move || {
                if let Some(ref sem) = sem {
                    sem.acquire();
                }

                let _ = tx.send(RepoEvent::Started { _idx: idx });

                let start_ms = if trace_enabled {
                    Some(run_started_at.elapsed().as_millis())
                } else {
                    None
                };
                let spawn_result = cmd.spawn(url_scheme);
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

                let _ = tx.send(RepoEvent::Completed {
                    idx,
                    result,
                    trace: trace_sample,
                });
            });
        }
        drop(tx);

        if is_tty {
            let mut writer = stdout.lock();
            let mut printer = TtyPrinter::new(&mut writer, repos.len(), run_started_at);

            for event in &rx {
                match event {
                    RepoEvent::Started { .. } => {
                        printer.mark_started();
                    }
                    RepoEvent::Completed { idx, ref result, trace } => {
                        let status_text = format_status(result, formatter);
                        printer.print_result(&rows[idx], &status_text);

                        if let Some(sample) = trace {
                            emit_trace(
                                ctx,
                                idx,
                                &rows[idx].name,
                                sample,
                                run_started_at,
                                &mut first_exit_ms,
                                &mut first_print_ms,
                                &mut delayed_repos,
                                &mut max_ordered_wait_ms,
                            )?;
                        }
                    }
                }
            }
            printer.finish();
        } else {
            let mut writer = stdout.lock();
            let mut printer = StreamPrinter::new(&mut writer);

            for event in &rx {
                if let RepoEvent::Completed { idx, ref result, trace } = event {
                    let status_text = format_status(result, formatter);
                    printer.print_result(&rows[idx], &status_text);

                    if let Some(sample) = trace {
                        emit_trace(
                            ctx,
                            idx,
                            &rows[idx].name,
                            sample,
                            run_started_at,
                            &mut first_exit_ms,
                            &mut first_print_ms,
                            &mut delayed_repos,
                            &mut max_ordered_wait_ms,
                        )?;
                    }
                }
            }
            printer.finish();
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

fn emit_trace(
    ctx: &mut ExecutionContext,
    idx: usize,
    repo_name: &str,
    sample: RepoTraceSample,
    run_started_at: Instant,
    first_exit_ms: &mut Option<u128>,
    first_print_ms: &mut Option<u128>,
    delayed_repos: &mut usize,
    max_ordered_wait_ms: &mut u128,
) -> Result<()> {
    let printed_ms = run_started_at.elapsed().as_millis();
    let ordered_wait_ms = sample.ordered_wait_ms(printed_ms);
    *first_exit_ms = Some(first_exit_ms.map_or(sample.exit_ms, |current| current.min(sample.exit_ms)));
    *first_print_ms = Some(first_print_ms.map_or(printed_ms, |current| current.min(printed_ms)));
    if ordered_wait_ms > 0 {
        *delayed_repos += 1;
    }
    *max_ordered_wait_ms = (*max_ordered_wait_ms).max(ordered_wait_ms);
    ctx.trace_mut().emit_repo(idx, repo_name, sample, printed_ms)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_compute_repo_id_width_minimum() {
        assert_eq!(compute_repo_id_width(1), 3);
        assert_eq!(compute_repo_id_width(98), 3);
        assert_eq!(compute_repo_id_width(1234), 4);
    }

    #[test]
    fn test_build_repo_rows() {
        let root = PathBuf::from("/workspace");
        let repos = vec![root.join("alpha"), root.join("beta")];
        let rows = build_repo_rows(&repos, &root);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].idx, 1);
        assert_eq!(rows[0].name, "alpha");
        assert_eq!(rows[1].idx, 2);
        assert_eq!(rows[1].name, "beta");
        // Labels should have stable IDs
        assert!(rows[0].label().starts_with("[001 "));
        assert!(rows[1].label().starts_with("[002 "));
    }

    #[test]
    fn test_format_status_success() {
        struct TestFormatter;
        impl OutputFormatter for TestFormatter {
            fn format(&self, _output: &Output) -> String {
                "clean".to_string()
            }
        }
        let output = Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        assert_eq!(format_status(&Ok(output), &TestFormatter), "clean");
    }

    #[test]
    fn test_format_status_error() {
        struct TestFormatter;
        impl OutputFormatter for TestFormatter {
            fn format(&self, _output: &Output) -> String {
                unreachable!()
            }
        }
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "git not found");
        let result = format_status(&Err(err), &TestFormatter);
        assert!(result.starts_with("ERROR:"));
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
