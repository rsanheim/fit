use anyhow::Result;
use crossterm::terminal::size as terminal_size;
use crossterm::tty::IsTty;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use crate::printer::{PlainPrinter, Printer, RepoRow, TtyTablePrinter};
use crate::repo::repo_display_name;
use crate::trace::{RepoTraceSample, TraceSink, TraceSummary};

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
const DEFAULT_TERMINAL_COLUMNS: usize = 80;
const DEFAULT_TERMINAL_ROWS: usize = 24;

type RepoCompletion = (PathBuf, Option<RepoTraceSample>);

enum RepoEvent {
    Started {
        idx: usize,
    },
    Completed {
        idx: usize,
        repo: PathBuf,
        result: Result<Output, std::io::Error>,
        trace_sample: Option<RepoTraceSample>,
    },
}

/// URL scheme to force for git operations
#[derive(Clone, Copy)]
pub enum UrlScheme {
    /// Force SSH: git@github.com:user/repo
    Ssh,
    /// Force HTTPS: https://github.com/user/repo
    Https,
}

fn compute_name_width(repos: &[PathBuf], display_root: &Path) -> usize {
    let mut max_len = 0usize;
    for repo in repos {
        let name = repo_display_name(repo, display_root);
        max_len = max_len.max(name.len());
    }

    let capped = max_len.min(MAX_REPO_NAME_WIDTH_CAP);
    capped.max(MIN_REPO_NAME_WIDTH)
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

    fn format_result(&self, result: &Result<Output, std::io::Error>) -> String {
        match result {
            Ok(output) => self.format(output),
            Err(e) => format!("ERROR: {}", e),
        }
    }
}

fn emit_traces_for_printed_rows(
    ctx: &mut ExecutionContext,
    completions: &[Option<RepoCompletion>],
    printed_indices: &[usize],
    printed_ms: u128,
    summary: &mut TraceSummary,
) -> Result<()> {
    for idx in printed_indices {
        let Some((repo_path, sample)) = &completions[*idx] else {
            continue;
        };
        if let Some(sample) = sample {
            summary.record(sample, printed_ms);
            let repo_name = repo_display_name(repo_path, ctx.display_root());
            ctx.trace_mut()
                .emit_repo(*idx, &repo_name, *sample, printed_ms)?;
        }
    }
    Ok(())
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
    let url_scheme = ctx.url_scheme();
    let trace_enabled = ctx.trace_enabled();

    if ctx.is_dry_run() {
        for repo in repos {
            let cmd = build_command(repo);
            println!("{}", cmd.command_string_with_scheme(url_scheme));
        }
        return Ok(());
    }

    let name_width = compute_name_width(repos, ctx.display_root());
    let run_started_at = Instant::now();
    let mut rows: Vec<RepoRow> = repos
        .iter()
        .map(|repo| RepoRow::pending(repo_display_name(repo, ctx.display_root())))
        .collect();
    let stdout = std::io::stdout();
    let is_tty = stdout.is_tty();
    let (terminal_columns, terminal_rows) = if is_tty {
        terminal_size()
            .map(|(columns, rows)| (columns as usize, rows as usize))
            .unwrap_or((DEFAULT_TERMINAL_COLUMNS, DEFAULT_TERMINAL_ROWS))
    } else {
        (0, 0)
    };
    let stdout = stdout.lock();
    let mut printer: Box<dyn Printer + '_> = if is_tty {
        Box::new(TtyTablePrinter::new(
            stdout,
            terminal_rows,
            terminal_columns,
            name_width,
        ))
    } else {
        Box::new(PlainPrinter::new(stdout, name_width))
    };
    printer.start(&rows)?;

    let max_workers = ctx.max_connections();

    let semaphore = if max_workers > 0 && max_workers < repos.len() {
        Some(Arc::new(Semaphore::new(max_workers)))
    } else {
        None
    };

    let mut completions: Vec<Option<RepoCompletion>> = (0..repos.len()).map(|_| None).collect();
    let mut summary = TraceSummary::default();

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
                let _ = tx.send(RepoEvent::Started { idx });

                let start_ms = run_started_at.elapsed().as_millis();
                let spawn_result = cmd.spawn(url_scheme);
                let spawn_ms = run_started_at.elapsed().as_millis();
                let result = match spawn_result {
                    Ok(child) => child.wait_with_output(),
                    Err(err) => Err(err),
                };

                let trace_sample = trace_enabled.then(|| {
                    let (stdout_bytes, stderr_bytes, success) = match &result {
                        Ok(output) => (
                            output.stdout.len(),
                            output.stderr.len(),
                            output.status.success(),
                        ),
                        Err(_) => (0, 0, false),
                    };
                    RepoTraceSample {
                        start_ms,
                        spawn_ms,
                        exit_ms: run_started_at.elapsed().as_millis(),
                        stdout_bytes,
                        stderr_bytes,
                        success,
                    }
                });

                if let Some(ref sem) = sem {
                    sem.release();
                }

                let _ = tx.send(RepoEvent::Completed {
                    idx,
                    repo,
                    result,
                    trace_sample,
                });
            });
        }
        drop(tx);

        for event in rx {
            match event {
                RepoEvent::Started { idx } => {
                    rows[idx].mark_running();
                    let elapsed_ms = run_started_at.elapsed().as_millis();
                    let _ = printer.update_row(&rows, idx, elapsed_ms)?;
                }
                RepoEvent::Completed {
                    idx,
                    repo,
                    result,
                    trace_sample,
                } => {
                    rows[idx].mark_finished(formatter.format_result(&result));
                    completions[idx] = Some((repo, trace_sample));
                    let elapsed_ms = run_started_at.elapsed().as_millis();
                    let printed = printer.update_row(&rows, idx, elapsed_ms)?;
                    emit_traces_for_printed_rows(
                        ctx,
                        &completions,
                        &printed,
                        elapsed_ms,
                        &mut summary,
                    )?;
                }
            }
        }
        Ok(())
    })?;

    let total_ms = run_started_at.elapsed().as_millis();
    let printed = printer.complete(&rows, total_ms)?;
    emit_traces_for_printed_rows(ctx, &completions, &printed, total_ms, &mut summary)?;
    ctx.trace_mut()
        .emit_summary(repos.len(), &summary, total_ms)?;

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
