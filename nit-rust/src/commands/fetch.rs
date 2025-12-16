use anyhow::Result;
use std::path::PathBuf;
use std::process::Output;

use crate::runner::{run_parallel, ExecutionContext, GitCommand, OutputFormatter};

struct FetchFormatter;

impl OutputFormatter for FetchFormatter {
    fn format(&self, output: &Output) -> String {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            let error_line = stderr
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("unknown error");
            return format!("ERROR: {}", error_line);
        }

        // git fetch writes progress to stderr, actual updates to stdout
        // If stdout is empty, nothing was fetched
        let stdout_content: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
        let stderr_content: Vec<&str> = stderr
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("From"))
            .collect();

        if stdout_content.is_empty() && stderr_content.is_empty() {
            return "no new commits".to_string();
        }

        // Count branches/tags updated from stdout
        let updates: Vec<&str> = stdout
            .lines()
            .filter(|l| l.contains("->") || l.contains("[new"))
            .collect();

        if !updates.is_empty() {
            let branch_count = updates.iter().filter(|l| !l.contains("[new tag]")).count();
            let tag_count = updates.iter().filter(|l| l.contains("[new tag]")).count();

            let mut parts = Vec::new();
            if branch_count > 0 {
                parts.push(format!(
                    "{} branch{}",
                    branch_count,
                    if branch_count == 1 { "" } else { "es" }
                ));
            }
            if tag_count > 0 {
                parts.push(format!(
                    "{} tag{}",
                    tag_count,
                    if tag_count == 1 { "" } else { "s" }
                ));
            }

            if !parts.is_empty() {
                return format!("{} updated", parts.join(", "));
            }
        }

        // Fallback
        "fetched".to_string()
    }
}

pub fn run(ctx: &ExecutionContext, repos: &[PathBuf], extra_args: &[String]) -> Result<()> {
    let formatter = FetchFormatter;

    run_parallel(
        ctx,
        repos,
        |repo| {
            let mut args = vec!["fetch".to_string()];
            args.extend(extra_args.iter().cloned());
            GitCommand::new(repo.clone(), args)
        },
        &formatter,
    )
}
