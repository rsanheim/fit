use anyhow::Result;
use std::path::PathBuf;
use std::process::Output;

use crate::runner::{run_parallel, ExecutionContext, FormattedResult, GitCommand, OutputFormatter};

struct StatusFormatter;

/// Parse the `## branch...remote [ahead N, behind M]` header line from porcelain -b output.
/// Returns (branch_name, ahead_count, behind_count).
fn parse_branch_line(line: &str) -> (String, usize, usize) {
    let mut ahead = 0usize;
    let mut behind = 0usize;

    if !line.starts_with("## ") {
        return (String::new(), ahead, behind);
    }

    let content = &line[3..];

    if content.starts_with("HEAD (no branch)") {
        return ("HEAD (detached)".to_string(), ahead, behind);
    }

    if content.starts_with("No commits yet on ") {
        return (content[18..].to_string(), ahead, behind);
    }

    if content.starts_with("Initial commit on ") {
        return (content[18..].to_string(), ahead, behind);
    }

    let (branch_part, tracking_info) = if let Some(dots_pos) = content.find("...") {
        (&content[..dots_pos], &content[dots_pos + 3..])
    } else {
        (content.trim(), "")
    };

    let branch = branch_part.to_string();

    if let Some(bracket_start) = tracking_info.find('[') {
        if let Some(bracket_end) = tracking_info.find(']') {
            let info = &tracking_info[bracket_start + 1..bracket_end];
            for part in info.split(',') {
                let part = part.trim();
                if let Some(n) = part.strip_prefix("ahead ") {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix("behind ") {
                    behind = n.parse().unwrap_or(0);
                }
            }
        }
    }

    (branch, ahead, behind)
}

impl OutputFormatter for StatusFormatter {
    fn format(&self, output: &Output) -> FormattedResult {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return FormattedResult::message_only(
                stderr.lines().next().unwrap_or("unknown error").to_string(),
            );
        }

        let mut branch = String::new();
        let mut ahead = 0usize;
        let mut behind = 0usize;

        let mut modified = 0;
        let mut added = 0;
        let mut deleted = 0;
        let mut untracked = 0;
        let mut renamed = 0;

        for line in stdout.lines() {
            if line.starts_with("## ") {
                let (b, a, bh) = parse_branch_line(line);
                branch = b;
                ahead = a;
                behind = bh;
                continue;
            }

            if line.len() < 2 {
                continue;
            }

            let index_status = line.chars().next().unwrap_or(' ');
            let worktree_status = line.chars().nth(1).unwrap_or(' ');

            if index_status == '?' {
                untracked += 1;
                continue;
            }

            match index_status {
                'M' => modified += 1,
                'A' => added += 1,
                'D' => deleted += 1,
                'R' => renamed += 1,
                _ => {}
            }

            // Worktree status only counted when index status is a space (no staged change)
            if index_status == ' ' {
                match worktree_status {
                    'M' => modified += 1,
                    'D' => deleted += 1,
                    _ => {}
                }
            }
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

        let has_file_changes = !parts.is_empty();

        if ahead > 0 {
            parts.push(format!("{} ahead", ahead));
        }
        if behind > 0 {
            parts.push(format!("{} behind", behind));
        }

        let message = if parts.is_empty() {
            "clean".to_string()
        } else if !has_file_changes {
            format!("clean, {}", parts.join(", "))
        } else {
            parts.join(", ")
        };

        FormattedResult { branch, message }
    }
}

pub fn run(ctx: &ExecutionContext, repos: &[PathBuf], extra_args: &[String]) -> Result<()> {
    let formatter = StatusFormatter;

    run_parallel(
        ctx,
        repos,
        |repo| {
            let global_args = vec!["--no-optional-locks".to_string()];
            let mut args = vec![
                "status".to_string(),
                "--porcelain".to_string(),
                "-b".to_string(),
            ];
            args.extend(extra_args.iter().cloned());
            GitCommand::with_global_args(repo.clone(), global_args, args)
        },
        &formatter,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    fn make_output(stdout: &str, stderr: &str, success: bool) -> Output {
        Output {
            status: ExitStatus::from_raw(if success { 0 } else { 256 }),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn test_parse_branch_simple() {
        let (branch, ahead, behind) = parse_branch_line("## main");
        assert_eq!(branch, "main");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_parse_branch_with_tracking() {
        let (branch, ahead, behind) = parse_branch_line("## main...origin/main");
        assert_eq!(branch, "main");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_parse_branch_ahead() {
        let (branch, ahead, behind) = parse_branch_line("## main...origin/main [ahead 2]");
        assert_eq!(branch, "main");
        assert_eq!(ahead, 2);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_parse_branch_behind() {
        let (branch, ahead, behind) = parse_branch_line("## main...origin/main [behind 3]");
        assert_eq!(branch, "main");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 3);
    }

    #[test]
    fn test_parse_branch_diverged() {
        let (branch, ahead, behind) =
            parse_branch_line("## main...origin/main [ahead 2, behind 3]");
        assert_eq!(branch, "main");
        assert_eq!(ahead, 2);
        assert_eq!(behind, 3);
    }

    #[test]
    fn test_parse_branch_detached() {
        let (branch, ahead, behind) = parse_branch_line("## HEAD (no branch)");
        assert_eq!(branch, "HEAD (detached)");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_clean_repo() {
        let formatter = StatusFormatter;
        let output = make_output("## main\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.branch, "main");
        assert_eq!(result.message, "clean");
    }

    #[test]
    fn test_one_unstaged_modification() {
        let formatter = StatusFormatter;
        let output = make_output("## main\n M file.txt\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.branch, "main");
        assert_eq!(result.message, "1 modified");
    }

    #[test]
    fn test_one_staged_modification() {
        let formatter = StatusFormatter;
        let output = make_output("## main\nM  file.txt\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.message, "1 modified");
    }

    #[test]
    fn test_mm_counts_once() {
        let formatter = StatusFormatter;
        let output = make_output("## main\nMM file.txt\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.message, "1 modified");
    }

    #[test]
    fn test_staged_add() {
        let formatter = StatusFormatter;
        let output = make_output("## main\nA  file.txt\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.message, "1 added");
    }

    #[test]
    fn test_am_counts_as_added() {
        let formatter = StatusFormatter;
        let output = make_output("## main\nAM file.txt\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.message, "1 added");
    }

    #[test]
    fn test_all_types() {
        let formatter = StatusFormatter;
        let output = make_output(
            "## main\nM  a.txt\nA  b.txt\nD  c.txt\nR  d.txt -> e.txt\n?? f.txt\n",
            "",
            true,
        );
        let result = formatter.format(&output);
        assert_eq!(
            result.message,
            "1 modified, 1 added, 1 deleted, 1 renamed, 1 untracked"
        );
    }

    #[test]
    fn test_clean_ahead() {
        let formatter = StatusFormatter;
        let output = make_output("## main...origin/main [ahead 2]\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.message, "clean, 2 ahead");
    }

    #[test]
    fn test_clean_behind() {
        let formatter = StatusFormatter;
        let output = make_output("## main...origin/main [behind 3]\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.message, "clean, 3 behind");
    }

    #[test]
    fn test_clean_diverged() {
        let formatter = StatusFormatter;
        let output =
            make_output("## main...origin/main [ahead 2, behind 3]\n", "", true);
        let result = formatter.format(&output);
        assert_eq!(result.message, "clean, 2 ahead, 3 behind");
    }

    #[test]
    fn test_modified_and_ahead() {
        let formatter = StatusFormatter;
        let output = make_output(
            "## main...origin/main [ahead 1]\n M file.txt\n",
            "",
            true,
        );
        let result = formatter.format(&output);
        assert_eq!(result.message, "1 modified, 1 ahead");
    }

    #[test]
    fn test_error_returns_stderr() {
        let formatter = StatusFormatter;
        let output = make_output("", "fatal: not a git repository", false);
        let result = formatter.format(&output);
        assert_eq!(result.message, "fatal: not a git repository");
        assert!(result.branch.is_empty());
    }
}
