use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

pub enum TraceSink {
    Disabled,
    Stderr,
    File(BufWriter<File>),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RepoTraceSample {
    pub start_ms: u128,
    pub spawn_ms: u128,
    pub exit_ms: u128,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub success: bool,
}

impl RepoTraceSample {
    pub fn ordered_wait_ms(self, printed_ms: u128) -> u128 {
        printed_ms.saturating_sub(self.exit_ms)
    }

    pub fn run_ms(self) -> u128 {
        self.exit_ms.saturating_sub(self.spawn_ms)
    }
}

#[derive(Default)]
pub struct TraceSummary {
    pub first_exit_ms: Option<u128>,
    pub first_print_ms: Option<u128>,
    pub delayed_repos: usize,
    pub max_ordered_wait_ms: u128,
}

impl TraceSummary {
    pub fn record(&mut self, sample: &RepoTraceSample, printed_ms: u128) {
        let ordered_wait_ms = sample.ordered_wait_ms(printed_ms);
        self.first_exit_ms = Some(
            self.first_exit_ms
                .map_or(sample.exit_ms, |current| current.min(sample.exit_ms)),
        );
        self.first_print_ms = Some(
            self.first_print_ms
                .map_or(printed_ms, |current| current.min(printed_ms)),
        );
        if ordered_wait_ms > 0 {
            self.delayed_repos += 1;
        }
        self.max_ordered_wait_ms = self.max_ordered_wait_ms.max(ordered_wait_ms);
    }
}

impl TraceSink {
    pub fn from_env() -> io::Result<Self> {
        let trace_path = std::env::var_os("GIT_ALL_TRACE_FILE")
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty());
        let enabled = std::env::var("GIT_ALL_TRACE")
            .map(|value| parse_trace_env(&value))
            .unwrap_or(false)
            || trace_path.is_some();

        if !enabled {
            return Ok(Self::Disabled);
        }

        match trace_path {
            Some(path) => Ok(Self::File(BufWriter::new(File::create(path)?))),
            None => Ok(Self::Stderr),
        }
    }

    pub fn enabled(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub fn emit_scan(
        &mut self,
        command: &str,
        root: &std::path::Path,
        repo_count: usize,
        workers: usize,
        scan_ms: u128,
    ) -> io::Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        self.write_line(&format!(
            "git-all-trace phase=scan command={command:?} root={:?} repos={repo_count} workers={workers} scan_ms={scan_ms}",
            root.to_string_lossy()
        ))
    }

    pub fn emit_repo(
        &mut self,
        idx: usize,
        repo_name: &str,
        sample: RepoTraceSample,
        printed_ms: u128,
    ) -> io::Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        self.write_line(&format!(
            concat!(
                "git-all-trace phase=repo idx={idx} repo={repo_name:?} ",
                "start_ms={start_ms} spawn_ms={spawn_ms} exit_ms={exit_ms} ",
                "printed_ms={printed_ms} run_ms={run_ms} ordered_wait_ms={ordered_wait_ms} ",
                "stdout_bytes={stdout_bytes} stderr_bytes={stderr_bytes} success={success}"
            ),
            idx = idx,
            repo_name = repo_name,
            start_ms = sample.start_ms,
            spawn_ms = sample.spawn_ms,
            exit_ms = sample.exit_ms,
            printed_ms = printed_ms,
            run_ms = sample.run_ms(),
            ordered_wait_ms = sample.ordered_wait_ms(printed_ms),
            stdout_bytes = sample.stdout_bytes,
            stderr_bytes = sample.stderr_bytes,
            success = sample.success,
        ))
    }

    pub fn emit_summary(
        &mut self,
        repo_count: usize,
        summary: &TraceSummary,
        total_ms: u128,
    ) -> io::Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        self.write_line(&format!(
            concat!(
                "git-all-trace phase=summary repos={repo_count} ",
                "first_exit_ms={first_exit_ms} first_print_ms={first_print_ms} ",
                "delayed_repos={delayed_repos} max_ordered_wait_ms={max_ordered_wait_ms} total_ms={total_ms}"
            ),
            repo_count = repo_count,
            first_exit_ms = optional_ms(summary.first_exit_ms),
            first_print_ms = optional_ms(summary.first_print_ms),
            delayed_repos = summary.delayed_repos,
            max_ordered_wait_ms = summary.max_ordered_wait_ms,
            total_ms = total_ms,
        ))
    }

    fn write_line(&mut self, line: &str) -> io::Result<()> {
        match self {
            Self::Disabled => Ok(()),
            Self::Stderr => {
                let mut stderr = io::stderr().lock();
                writeln!(stderr, "{line}")
            }
            Self::File(writer) => writeln!(writer, "{line}"),
        }
    }
}

fn optional_ms(value: Option<u128>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "na".to_string())
}

fn parse_trace_env(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "off" | "no"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_trace_env_true_values() {
        for value in ["1", "true", "TRUE", "yes", "on", "anything"] {
            assert!(parse_trace_env(value), "{value} should enable tracing");
        }
    }

    #[test]
    fn test_parse_trace_env_false_values() {
        for value in ["", "0", "false", "FALSE", "off", "no"] {
            assert!(!parse_trace_env(value), "{value} should disable tracing");
        }
    }

    #[test]
    fn trace_summary_records_first_and_max_ordered_wait() {
        let mut summary = TraceSummary::default();
        summary.record(
            &RepoTraceSample {
                exit_ms: 100,
                ..Default::default()
            },
            200,
        );
        summary.record(
            &RepoTraceSample {
                exit_ms: 50,
                ..Default::default()
            },
            210,
        );
        summary.record(
            &RepoTraceSample {
                exit_ms: 300,
                ..Default::default()
            },
            300,
        );

        assert_eq!(summary.first_exit_ms, Some(50));
        assert_eq!(summary.first_print_ms, Some(200));
        assert_eq!(summary.delayed_repos, 2);
        assert_eq!(summary.max_ordered_wait_ms, 160);
    }
}
