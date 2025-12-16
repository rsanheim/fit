use anyhow::Result;
use std::path::PathBuf;
use std::process::Output;

use crate::runner::{run_parallel, ExecutionContext, GitCommand, OutputFormatter};

struct StatusFormatter;

impl OutputFormatter for StatusFormatter {
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

        // Parse porcelain output to count file states
        let mut modified = 0;
        let mut added = 0;
        let mut deleted = 0;
        let mut untracked = 0;
        let mut renamed = 0;

        for line in stdout.lines() {
            if line.len() < 2 {
                continue;
            }

            let index_status = line.chars().next().unwrap_or(' ');
            let worktree_status = line.chars().nth(1).unwrap_or(' ');

            // Untracked files
            if index_status == '?' {
                untracked += 1;
                continue;
            }

            // Check index status (staged changes)
            match index_status {
                'M' => modified += 1,
                'A' => added += 1,
                'D' => deleted += 1,
                'R' => renamed += 1,
                _ => {}
            }

            // Check worktree status (unstaged changes) - only if not already counted
            if index_status == ' ' {
                match worktree_status {
                    'M' => modified += 1,
                    'D' => deleted += 1,
                    _ => {}
                }
            }
        }

        // Build human-readable summary
        if modified == 0 && added == 0 && deleted == 0 && untracked == 0 && renamed == 0 {
            return "clean".to_string();
        }

        let mut parts = Vec::new();

        if modified > 0 {
            parts.push(format!("{} modified", modified));
        }
        if added > 0 {
            parts.push(format!("{} added", added));
        }
        if deleted > 0 {
            parts.push(format!("{} deleted", deleted));
        }
        if renamed > 0 {
            parts.push(format!("{} renamed", renamed));
        }
        if untracked > 0 {
            parts.push(format!("{} untracked", untracked));
        }

        parts.join(", ")
    }
}

pub fn run(ctx: &ExecutionContext, repos: &[PathBuf], extra_args: &[String]) -> Result<()> {
    let formatter = StatusFormatter;

    run_parallel(
        ctx,
        repos,
        |repo| {
            // Always use --porcelain for machine-readable output
            let mut args = vec!["status".to_string(), "--porcelain".to_string()];
            args.extend(extra_args.iter().cloned());
            GitCommand::new(repo.clone(), args)
        },
        &formatter,
    )
}
