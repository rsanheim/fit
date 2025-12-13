use anyhow::Result;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};

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
}

impl ExecutionContext {
    pub fn new(dry_run: bool, url_scheme: Option<UrlScheme>) -> Self {
        Self {
            dry_run,
            url_scheme,
        }
    }

    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    pub fn url_scheme(&self) -> Option<UrlScheme> {
        self.url_scheme
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

/// A spawned git process with its associated repo info
struct SpawnedCommand {
    repo_path: PathBuf,
    child: Result<Child, std::io::Error>,
}

/// Run commands in parallel across all repos using spawn-first pattern.
/// All git processes are spawned immediately, then results are collected
/// and printed in deterministic (repo) order.
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

    Ok(())
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
