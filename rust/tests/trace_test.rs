use std::fs;
use std::process::Command;

#[cfg(unix)]
#[test]
fn completion_order_output_uses_stable_repo_ids() {
    let temp = tempfile::tempdir().expect("temp dir");

    create_delay_repos(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_git-all"))
        .args(["-n", "3", "delay"])
        .current_dir(temp.path())
        .output()
        .expect("git-all should run");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    assert_eq!(
        lines.len(),
        3,
        "expected one output line per repo: {stdout}"
    );
    assert!(
        lines[0].starts_with("[002 "),
        "expected repo b to print first with stable id 002: {stdout}"
    );
    assert!(
        lines[1].starts_with("[003 "),
        "expected repo c to print second with stable id 003: {stdout}"
    );
    assert!(
        lines[2].starts_with("[001 "),
        "expected repo a to print last with stable id 001: {stdout}"
    );
}

#[cfg(unix)]
#[test]
fn trace_reports_low_ordered_wait_for_completion_order_output() {
    let temp = tempfile::tempdir().expect("temp dir");

    create_delay_repos(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_git-all"))
        .args(["-n", "3", "delay"])
        .current_dir(temp.path())
        .env("GIT_ALL_TRACE", "1")
        .output()
        .expect("git-all should run");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        stdout.lines().count(),
        3,
        "expected one output line per repo: {stdout}"
    );
    assert!(
        stderr.contains("git-all-trace phase=scan"),
        "expected scan trace output: {stderr}"
    );
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.contains("git-all-trace phase=repo"))
            .count(),
        3,
        "expected one repo trace line per repo: {stderr}"
    );
    assert!(
        stderr.contains("git-all-trace phase=summary"),
        "expected summary trace output: {stderr}"
    );

    let summary = stderr
        .lines()
        .find(|line| line.contains("git-all-trace phase=summary"))
        .expect("expected summary trace line");

    let first_print_ms =
        parse_trace_u128_field(summary, "first_print_ms").expect("first_print_ms in summary");
    let max_ordered_wait_ms = parse_trace_u128_field(summary, "max_ordered_wait_ms")
        .expect("max_ordered_wait_ms in summary");

    assert!(
        first_print_ms < 500,
        "expected completion-order output to print before slow repo a finishes: {stderr}"
    );
    assert!(
        max_ordered_wait_ms < 200,
        "expected ordered wait to stay low with completion-order output: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn trace_file_captures_trace_output() {
    let temp = tempfile::tempdir().expect("temp dir");
    let trace_file = temp.path().join("trace.log");
    create_delay_repos(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_git-all"))
        .args(["-n", "3", "delay"])
        .current_dir(temp.path())
        .env("GIT_ALL_TRACE_FILE", &trace_file)
        .output()
        .expect("git-all should run");

    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("git-all-trace"),
        "trace output should go to the file sink: {stderr}"
    );

    let trace = fs::read_to_string(&trace_file).expect("trace file should be readable");
    assert!(
        trace.contains("git-all-trace phase=scan"),
        "expected scan trace output in file: {trace}"
    );
    assert!(
        trace.contains("git-all-trace phase=summary"),
        "expected summary trace output in file: {trace}"
    );
}

#[cfg(unix)]
fn create_delay_repos(root: &std::path::Path) {
    for repo in ["a", "b", "c"] {
        let repo_path = root.join(repo);

        let init_status = Command::new("git")
            .args(["init", "-q"])
            .arg(&repo_path)
            .status()
            .expect("git init should run");
        assert!(init_status.success());

        let alias = r#"!name=$(basename "$PWD"); case "$name" in a) sleep 1 ;; c) sleep 0.2 ;; *) : ;; esac; echo "$name done""#;
        let config_status = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["config", "alias.delay", alias])
            .status()
            .expect("git config should run");
        assert!(config_status.success());
    }
}

#[cfg(unix)]
fn parse_trace_u128_field(line: &str, field_name: &str) -> Option<u128> {
    line.split_whitespace().find_map(|field| {
        field
            .strip_prefix(&format!("{field_name}="))
            .and_then(|value| value.parse().ok())
    })
}
