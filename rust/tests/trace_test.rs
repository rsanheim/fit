use std::process::Command;

#[cfg(unix)]
#[test]
fn trace_reports_ordered_wait_for_blocked_repos() {
    let temp = tempfile::tempdir().expect("temp dir");

    for repo in ["a", "b", "c"] {
        let repo_path = temp.path().join(repo);

        let init_status = Command::new("git")
            .args(["init", "-q"])
            .arg(&repo_path)
            .status()
            .expect("git init should run");
        assert!(init_status.success());

        let alias = r#"!name=$(basename "$PWD"); case "$name" in a) sleep 1 ;; *) : ;; esac; echo "$name done""#;
        let config_status = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["config", "alias.delay", alias])
            .status()
            .expect("git config should run");
        assert!(config_status.success());
    }

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

    let ordered_waits: Vec<u128> = stderr
        .lines()
        .filter(|line| line.contains("git-all-trace phase=repo"))
        .filter_map(parse_ordered_wait_ms)
        .collect();
    assert!(
        ordered_waits.iter().any(|wait_ms| *wait_ms >= 500),
        "expected at least one repo to wait behind ordered output: {stderr}"
    );
}

#[cfg(unix)]
fn parse_ordered_wait_ms(line: &str) -> Option<u128> {
    line.split_whitespace().find_map(|field| {
        field
            .strip_prefix("ordered_wait_ms=")
            .and_then(|value| value.parse().ok())
    })
}
