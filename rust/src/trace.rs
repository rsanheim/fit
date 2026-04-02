use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct TraceConfig {
    destination: Option<TraceDestination>,
}

#[derive(Clone)]
enum TraceDestination {
    Stderr,
    File(Arc<Mutex<BufWriter<File>>>),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RepoTraceSample {
    pub start_ms: u128,
    pub spawn_ms: Option<u128>,
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
        self.exit_ms
            .saturating_sub(self.spawn_ms.unwrap_or(self.start_ms))
    }
}

impl TraceConfig {
    pub fn from_env() -> io::Result<Self> {
        let trace_path = std::env::var_os("GIT_ALL_TRACE_FILE")
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty());
        let enabled = std::env::var("GIT_ALL_TRACE")
            .map(|value| parse_trace_env(&value))
            .unwrap_or(false)
            || trace_path.is_some();

        if !enabled {
            return Ok(Self::default());
        }

        let destination = match trace_path {
            Some(path) => Some(TraceDestination::File(Arc::new(Mutex::new(
                BufWriter::new(File::create(path)?),
            )))),
            None => Some(TraceDestination::Stderr),
        };

        Ok(Self { destination })
    }

    pub fn enabled(&self) -> bool {
        self.destination.is_some()
    }

    pub fn emit_scan(
        &self,
        command: &str,
        root: &Path,
        repo_count: usize,
        workers: usize,
        scan_ms: u128,
    ) {
        self.write_line(&format!(
            "git-all-trace phase=scan command={command:?} root={:?} repos={repo_count} workers={workers} scan_ms={scan_ms}",
            root.to_string_lossy()
        ));
    }

    pub fn emit_repo(
        &self,
        idx: usize,
        repo_name: &str,
        sample: RepoTraceSample,
        printed_ms: u128,
    ) {
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
            spawn_ms = sample.spawn_ms.unwrap_or(sample.start_ms),
            exit_ms = sample.exit_ms,
            printed_ms = printed_ms,
            run_ms = sample.run_ms(),
            ordered_wait_ms = sample.ordered_wait_ms(printed_ms),
            stdout_bytes = sample.stdout_bytes,
            stderr_bytes = sample.stderr_bytes,
            success = sample.success,
        ));
    }

    pub fn emit_summary(
        &self,
        repo_count: usize,
        first_exit_ms: Option<u128>,
        first_print_ms: Option<u128>,
        delayed_repos: usize,
        max_ordered_wait_ms: u128,
        total_ms: u128,
    ) {
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
        ));
    }

    fn write_line(&self, line: &str) {
        match &self.destination {
            None => {}
            Some(TraceDestination::Stderr) => eprintln!("{line}"),
            Some(TraceDestination::File(writer)) => {
                if let Ok(mut writer) = writer.lock() {
                    let _ = writeln!(writer, "{line}");
                }
            }
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
