use anyhow::Result;
use std::path::PathBuf;
use std::process::Output;

use crate::runner::{run_parallel, ExecutionContext, FormattedResult, GitCommand, OutputFormatter};

struct FetchFormatter;

impl OutputFormatter for FetchFormatter {
    fn format(&self, output: &Output) -> FormattedResult {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return FormattedResult::message_only(
                stderr.lines().next().unwrap_or("unknown error").to_string(),
            );
        }

        let has_output = stdout.lines().any(|l| !l.trim().is_empty())
            || stderr.lines().any(|l| !l.trim().is_empty() && !l.starts_with("From"));

        if !has_output {
            return FormattedResult::message_only("no new commits".to_string());
        }

        let (branch_count, tag_count) = stdout
            .lines()
            .filter(|l| l.contains("->") || l.contains("[new"))
            .fold((0, 0), |(b, t), l| {
                if l.contains("[new tag]") { (b, t + 1) } else { (b + 1, t) }
            });

        if branch_count > 0 || tag_count > 0 {
            let mut parts = Vec::new();
            if branch_count > 0 {
                parts.push(format!("{} branch{}", branch_count, if branch_count == 1 { "" } else { "es" }));
            }
            if tag_count > 0 {
                parts.push(format!("{} tag{}", tag_count, if tag_count == 1 { "" } else { "s" }));
            }
            return FormattedResult::message_only(format!("{} updated", parts.join(", ")));
        }

        FormattedResult::message_only("fetched".to_string())
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
    fn test_error_returns_first_stderr_line() {
        let formatter = FetchFormatter;
        let output = make_output("", "fatal: not a git repository", false);
        let result = formatter.format(&output);
        assert_eq!(result.message, "fatal: not a git repository");
        assert!(result.branch.is_empty());
    }

    #[test]
    fn test_empty_output_returns_no_new_commits() {
        let formatter = FetchFormatter;
        let output = make_output("", "", true);
        assert_eq!(formatter.format(&output).message, "no new commits");
    }

    #[test]
    fn test_only_from_line_returns_no_new_commits() {
        let formatter = FetchFormatter;
        let output = make_output("", "From github.com:user/repo", true);
        assert_eq!(formatter.format(&output).message, "no new commits");
    }

    #[test]
    fn test_single_branch_update() {
        let formatter = FetchFormatter;
        let output = make_output("   abc123..def456  main       -> origin/main\n", "", true);
        assert_eq!(formatter.format(&output).message, "1 branch updated");
    }

    #[test]
    fn test_multiple_branch_updates() {
        let formatter = FetchFormatter;
        let stdout = "   abc123..def456  main       -> origin/main\n   111222..333444  develop    -> origin/develop\n";
        let output = make_output(stdout, "", true);
        assert_eq!(formatter.format(&output).message, "2 branches updated");
    }

    #[test]
    fn test_single_tag() {
        let formatter = FetchFormatter;
        let output = make_output(" * [new tag]         v1.0.0     -> v1.0.0\n", "", true);
        assert_eq!(formatter.format(&output).message, "1 tag updated");
    }

    #[test]
    fn test_multiple_tags() {
        let formatter = FetchFormatter;
        let stdout = " * [new tag]         v1.0.0     -> v1.0.0\n * [new tag]         v1.0.1     -> v1.0.1\n";
        let output = make_output(stdout, "", true);
        assert_eq!(formatter.format(&output).message, "2 tags updated");
    }

    #[test]
    fn test_mixed_branches_and_tags() {
        let formatter = FetchFormatter;
        let stdout = "   abc123..def456  main       -> origin/main\n * [new tag]         v1.0.0     -> v1.0.0\n";
        let output = make_output(stdout, "", true);
        assert_eq!(
            formatter.format(&output).message,
            "1 branch, 1 tag updated"
        );
    }

    #[test]
    fn test_fallback_to_fetched() {
        let formatter = FetchFormatter;
        let output = make_output("some other output\n", "", true);
        assert_eq!(formatter.format(&output).message, "fetched");
    }
}
