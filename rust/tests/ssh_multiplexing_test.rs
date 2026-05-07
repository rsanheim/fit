use std::process::Command;

const OVERRIDE_SUBSTRING: &str =
    r#"-c "core.sshCommand=ssh -o ControlMaster=no -o ControlPath=none""#;

fn make_repo(parent: &std::path::Path, name: &str) {
    let repo = parent.join(name);
    let status = Command::new("git")
        .args(["init", "-q"])
        .arg(&repo)
        .status()
        .expect("git init");
    assert!(status.success());
}

fn run_dry_run(temp_dir: &std::path::Path, extra_args: &[&str]) -> (bool, String, String) {
    let mut args: Vec<&str> = vec!["--dry-run"];
    args.extend(extra_args);
    let output = Command::new(env!("CARGO_BIN_EXE_git-all"))
        .args(&args)
        .current_dir(temp_dir)
        .output()
        .expect("git-all should run");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn default_disables_multiplexing_for_optimized_command() {
    let temp = tempfile::tempdir().expect("temp dir");
    make_repo(temp.path(), "a");

    let (ok, stdout, stderr) = run_dry_run(temp.path(), &["fetch"]);

    assert!(ok, "stderr: {stderr}");
    assert!(
        stdout.contains(OVERRIDE_SUBSTRING),
        "expected ControlMaster override in dry-run output, got:\n{stdout}"
    );
}

#[test]
fn default_disables_multiplexing_for_passthrough_command() {
    let temp = tempfile::tempdir().expect("temp dir");
    make_repo(temp.path(), "a");

    let (ok, stdout, stderr) = run_dry_run(temp.path(), &["ls-remote"]);

    assert!(ok, "stderr: {stderr}");
    assert!(
        stdout.contains(OVERRIDE_SUBSTRING),
        "expected ControlMaster override in passthrough dry-run output, got:\n{stdout}"
    );
}

#[test]
fn no_ssh_multiplexing_explicit_matches_default() {
    let temp = tempfile::tempdir().expect("temp dir");
    make_repo(temp.path(), "a");

    let (ok, stdout, stderr) = run_dry_run(temp.path(), &["--no-ssh-multiplexing", "fetch"]);

    assert!(ok, "stderr: {stderr}");
    assert!(
        stdout.contains(OVERRIDE_SUBSTRING),
        "--no-ssh-multiplexing should produce the default override, got:\n{stdout}"
    );
}

#[test]
fn ssh_multiplexing_flag_omits_override() {
    let temp = tempfile::tempdir().expect("temp dir");
    make_repo(temp.path(), "a");

    let (ok, stdout, stderr) = run_dry_run(temp.path(), &["--ssh-multiplexing", "fetch"]);

    assert!(ok, "stderr: {stderr}");
    assert!(
        !stdout.contains("ControlMaster=no"),
        "expected no ControlMaster override when --ssh-multiplexing set, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("core.sshCommand"),
        "expected no core.sshCommand override when --ssh-multiplexing set, got:\n{stdout}"
    );
}

#[test]
fn last_flag_wins_when_both_specified() {
    let temp = tempfile::tempdir().expect("temp dir");
    make_repo(temp.path(), "a");

    // --ssh-multiplexing then --no-ssh-multiplexing: last (no-) wins → override present
    let (ok, stdout, stderr) =
        run_dry_run(temp.path(), &["--ssh-multiplexing", "--no-ssh-multiplexing", "fetch"]);
    assert!(ok, "stderr: {stderr}");
    assert!(
        stdout.contains(OVERRIDE_SUBSTRING),
        "with --ssh-multiplexing then --no-ssh-multiplexing, expected override, got:\n{stdout}"
    );

    // --no-ssh-multiplexing then --ssh-multiplexing: last (positive) wins → override absent
    let (ok, stdout, stderr) =
        run_dry_run(temp.path(), &["--no-ssh-multiplexing", "--ssh-multiplexing", "fetch"]);
    assert!(ok, "stderr: {stderr}");
    assert!(
        !stdout.contains("ControlMaster=no"),
        "with --no-ssh-multiplexing then --ssh-multiplexing, expected no override, got:\n{stdout}"
    );
}

#[test]
fn ssh_url_rewrite_and_multiplexing_override_compose() {
    let temp = tempfile::tempdir().expect("temp dir");
    make_repo(temp.path(), "a");

    let (ok, stdout, stderr) = run_dry_run(temp.path(), &["--ssh", "fetch"]);

    assert!(ok, "stderr: {stderr}");
    assert!(
        stdout.contains("url.git@github.com:.insteadOf=https://github.com/"),
        "expected ssh url rewrite, got:\n{stdout}"
    );
    assert!(
        stdout.contains("ControlMaster=no"),
        "expected ControlMaster override alongside --ssh, got:\n{stdout}"
    );
}
