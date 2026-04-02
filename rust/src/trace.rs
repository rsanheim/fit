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
        first_exit_ms: Option<u128>,
        first_print_ms: Option<u128>,
        delayed_repos: usize,
        max_ordered_wait_ms: u128,
        total_ms: u128,
    ) -> io::Result<()> {
        self.write_line(&format!(
            concat!(
                "git-all-trace phase=summary repos={repo_count} ",
                "first_exit_ms={first_exit_ms} first_print_ms={first_print_ms} ",
                "delayed_repos={delayed_repos} max_ordered_wait_ms={max_ordered_wait_ms} total_ms={total_ms}"
            ),
            repo_count = repo_count,
            first_exit_ms = first_exit_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "na".to_string()),
            first_print_ms = first_print_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "na".to_string()),
            delayed_repos = delayed_repos,
            max_ordered_wait_ms = max_ordered_wait_ms,
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

fn parse_trace_env(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "off" | "no"
    )
}

#[cfg(test)]
mod tests {
    use super::parse_trace_env;

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
}
