use anyhow::Result;
use std::path::PathBuf;
use std::process::Output;

use crate::runner::{run_parallel, ExecutionContext, FormattedResult, GitCommand, OutputFormatter};

struct PullFormatter;

impl OutputFormatter for PullFormatter {
    fn format(&self, output: &Output) -> FormattedResult {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return FormattedResult::message_only(
                stderr.lines().next().unwrap_or("unknown error").to_string(),
            );
        }

        // Check for "Already up to date"
        if stdout.contains("Already up to date") {
            return FormattedResult::message_only("Already up to date".to_string());
        }

        // Try to extract summary from stdout (e.g., "3 files changed, 10 insertions(+), 5 deletions(-)")
        if let Some(summary_line) = stdout.lines().find(|l| l.contains("files changed")) {
            return FormattedResult::message_only(summary_line.trim().to_string());
        }

        // Check for fast-forward or merge info in stdout
        if let Some(line) = stdout
            .lines()
            .find(|l| l.contains("..") || l.contains("Updating"))
        {
            return FormattedResult::message_only(line.trim().to_string());
        }

        // Fallback: first non-empty line of stdout, or stderr
        let message = stdout
            .lines()
            .chain(stderr.lines())
            .find(|l| !l.trim().is_empty())
            .unwrap_or("completed")
            .trim()
            .to_string();

        FormattedResult::message_only(message)
    }
}

pub fn run(ctx: &ExecutionContext, repos: &[PathBuf], extra_args: &[String]) -> Result<()> {
    let formatter = PullFormatter;

    run_parallel(
        ctx,
        repos,
        |repo| {
            let mut args = vec!["pull".to_string()];
            args.extend(extra_args.iter().cloned());
            GitCommand::new(repo.clone(), args)
        },
        &formatter,
    )
}
