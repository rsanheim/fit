# Disable SSH ControlMaster (Rust) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a paired `--ssh-multiplexing` / `--no-ssh-multiplexing` global toggle to the Rust `git-all` impl that defaults to disabled and injects `-c core.sshCommand="ssh -o ControlMaster=no -o ControlPath=none"` into every git subprocess, so that parallel runs do not saturate OpenSSH's `MaxSessions` cap or hit cold-start races on the master socket.

**Architecture:** Two clap fields on `Cli` form a `--foo`/`--no-foo` pair via mutual `overrides_with`. Both default to `false`; whichever is specified last wins. The resolved value (`cli.ssh_multiplexing`) flows into a new `ssh_multiplexing` field on `ExecutionContext`. `GitCommand::spawn` and `GitCommand::command_string_with_scheme` learn about it via a small `GitInvocationOptions` Copy struct that bundles `url_scheme` + `ssh_multiplexing`, replacing the bare `Option<UrlScheme>` parameter at the two call sites in `runner.rs`. When `ssh_multiplexing` is `false` (the default), the override is emitted as `-c core.sshCommand=...` (consistent with the existing `--ssh`/`--https` mechanism) on every git invocation regardless of subcommand or remote URL scheme — this matches Section 6.5 of `docs/SPEC.md`.

**Tech Stack:** Rust 2024, clap 4.5 (derive), anyhow, std::process::Command. No new dependencies.

---

## File Structure

| File | Role |
|---|---|
| `rust/src/main.rs` | Add the `--ssh-multiplexing` / `--no-ssh-multiplexing` paired flags to `Cli`; pass resolved value into `ExecutionContext::new` |
| `rust/src/runner.rs` | New `GitInvocationOptions` struct; field + accessor on `ExecutionContext`; thread the struct through `GitCommand::spawn` and `command_string_with_scheme`; rename the latter to `command_string` |
| `rust/Cargo.toml` | Bump version to `0.7.2-rc.1` |
| `rust/tests/ssh_multiplexing_test.rs` | **New** — integration tests for default-disabled, `--ssh-multiplexing` opt-in, `--no-ssh-multiplexing` explicit, and last-wins ordering, via dry-run output |
| `docs/index.md` | Replace the "Performance: SSH Multiplexing" section with text reflecting the new default and the `--ssh-multiplexing` opt-in |

---

## Task 1: Add SPEC-shaped integration tests (failing)

**Files:**
- Create: `rust/tests/ssh_multiplexing_test.rs`

The tests use the `git-all` binary in dry-run mode against a temp directory containing one empty git repo. Dry-run is the cleanest end-to-end probe: per `SPEC.md` §6.1 + §6.5, dry-run output is built from the same code path as real execution, so asserting the override appears in dry-run output is equivalent to asserting it would be passed to real `git`.

- [ ] **Step 1: Write the failing tests**

```rust
// rust/tests/ssh_multiplexing_test.rs
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd rust && cargo test --test ssh_multiplexing_test`
Expected: all six tests FAIL — `--ssh-multiplexing` and `--no-ssh-multiplexing` are not yet known clap args, and the dry-run output does not yet contain the override.

- [ ] **Step 3: Commit**

```bash
git add rust/tests/ssh_multiplexing_test.rs
git commit -m "test: add failing tests for --[no-]ssh-multiplexing"
```

---

## Task 2: Introduce `GitInvocationOptions` and thread through `GitCommand`

This is a pure refactor — no behavior change yet. It widens `spawn` and the dry-run command-string method to take a struct so Task 3 can add a new field without churning call sites.

**Files:**
- Modify: `rust/src/runner.rs`

- [ ] **Step 1: Add the struct and switch `GitCommand` methods to use it**

Replace the existing `pub fn spawn(&self, url_scheme: Option<UrlScheme>)` and `pub fn command_string_with_scheme(&self, url_scheme: Option<UrlScheme>)` definitions in `rust/src/runner.rs` with the following. Note the rename of `command_string_with_scheme` → `command_string` (the new name reflects that the struct carries more than scheme).

```rust
/// Cross-cutting options that apply to every git invocation in a run.
#[derive(Clone, Copy)]
pub struct GitInvocationOptions {
    pub url_scheme: Option<UrlScheme>,
}

impl GitCommand {
    pub fn new(repo_path: PathBuf, args: Vec<String>) -> Self {
        Self { repo_path, args }
    }

    /// Spawn the git command without waiting for completion.
    /// Returns immediately with a Child process handle.
    pub fn spawn(&self, opts: GitInvocationOptions) -> std::io::Result<std::process::Child> {
        let mut cmd = Command::new("git");

        // Inject URL scheme override if specified (must come before other args)
        if let Some(scheme) = opts.url_scheme {
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
    pub fn command_string(&self, opts: GitInvocationOptions) -> String {
        let scheme_args = match opts.url_scheme {
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
```

- [ ] **Step 2: Update `ExecutionContext` to expose options**

Add the accessor below `pub fn url_scheme(&self) -> Option<UrlScheme>`:

```rust
pub fn git_invocation_options(&self) -> GitInvocationOptions {
    GitInvocationOptions {
        url_scheme: self.url_scheme,
    }
}
```

- [ ] **Step 3: Update the two call sites in `run_parallel`**

In `run_parallel` (`rust/src/runner.rs`), replace the two uses of `url_scheme` against `GitCommand`:

Before:
```rust
let url_scheme = ctx.url_scheme();
// ...
println!("{}", cmd.command_string_with_scheme(url_scheme));
// ...
let spawn_result = cmd.spawn(url_scheme);
```

After:
```rust
let opts = ctx.git_invocation_options();
// ...
println!("{}", cmd.command_string(opts));
// ...
let spawn_result = cmd.spawn(opts);
```

The `opts` binding is `Copy`, so it can be moved into the per-thread closure without `clone()`.

- [ ] **Step 4: Run tests to verify the refactor compiles and existing tests pass**

Run: `cd rust && cargo test`
Expected: existing tests PASS (no behavior change). New `ssh_multiplexing_test` tests still FAIL (no flag yet).

- [ ] **Step 5: Commit**

```bash
git add rust/src/runner.rs
git commit -m "refactor: bundle git invocation options into a struct"
```

---

## Task 3: Add `ssh_multiplexing` field, CLI flag pair, and override emission

**Files:**
- Modify: `rust/src/main.rs`
- Modify: `rust/src/runner.rs`

- [ ] **Step 1: Add the field to `GitInvocationOptions` and emit the override**

In `rust/src/runner.rs`, extend the struct:

```rust
#[derive(Clone, Copy)]
pub struct GitInvocationOptions {
    pub url_scheme: Option<UrlScheme>,
    pub ssh_multiplexing: bool,
}
```

In `GitCommand::spawn`, add this block immediately after the `url_scheme` block (and before the `cmd.arg("-C")` line). The override is emitted when multiplexing is **off** (the default):

```rust
if !opts.ssh_multiplexing {
    cmd.arg("-c")
        .arg("core.sshCommand=ssh -o ControlMaster=no -o ControlPath=none");
}
```

In `GitCommand::command_string`, change the `format!` to include the override. Replace the entire body:

```rust
pub fn command_string(&self, opts: GitInvocationOptions) -> String {
    let scheme_args = match opts.url_scheme {
        Some(UrlScheme::Ssh) => "-c \"url.git@github.com:.insteadOf=https://github.com/\" ",
        Some(UrlScheme::Https) => "-c \"url.https://github.com/.insteadOf=git@github.com:\" ",
        None => "",
    };
    let ssh_args = if opts.ssh_multiplexing {
        ""
    } else {
        "-c \"core.sshCommand=ssh -o ControlMaster=no -o ControlPath=none\" "
    };
    format!(
        "git {}{}-C {} {}",
        scheme_args,
        ssh_args,
        self.repo_path.display(),
        self.args.join(" ")
    )
}
```

- [ ] **Step 2: Add the field and constructor argument to `ExecutionContext`**

In `rust/src/runner.rs`:

```rust
pub struct ExecutionContext {
    dry_run: bool,
    url_scheme: Option<UrlScheme>,
    ssh_multiplexing: bool,
    max_connections: usize,
    display_root: PathBuf,
    trace: TraceSink,
}

impl ExecutionContext {
    pub fn new(
        dry_run: bool,
        url_scheme: Option<UrlScheme>,
        ssh_multiplexing: bool,
        max_connections: usize,
        display_root: PathBuf,
        trace: TraceSink,
    ) -> Self {
        Self {
            dry_run,
            url_scheme,
            ssh_multiplexing,
            max_connections,
            display_root,
            trace,
        }
    }

    // ... existing accessors ...

    pub fn git_invocation_options(&self) -> GitInvocationOptions {
        GitInvocationOptions {
            url_scheme: self.url_scheme,
            ssh_multiplexing: self.ssh_multiplexing,
        }
    }
}
```

- [ ] **Step 3: Add the paired CLI flags in `main.rs`**

The `--foo` / `--no-foo` idiom in clap derive uses two fields with mutual `overrides_with`. Both default to `false`; whichever is specified last on the command line wins. The negative field is hidden from `--help` since it's the default — surfacing it would clutter help output.

In `rust/src/main.rs`, add to the `Cli` struct (place after the `https` flag, before `workers`):

```rust
/// Enable SSH connection multiplexing (ControlMaster) for git operations.
/// Disabled by default to avoid OpenSSH MaxSessions saturation and cold-start
/// races when many git subprocesses run in parallel.
#[arg(long, overrides_with = "_no_ssh_multiplexing")]
ssh_multiplexing: bool,

/// Negative form of --ssh-multiplexing (the default). Hidden from help.
#[arg(
    long = "no-ssh-multiplexing",
    overrides_with = "ssh_multiplexing",
    hide = true
)]
_no_ssh_multiplexing: bool,
```

Then update the `ExecutionContext::new` call site (currently around `let mut ctx = ExecutionContext::new(...)`):

```rust
let mut ctx = ExecutionContext::new(
    cli.dry_run,
    url_scheme,
    cli.ssh_multiplexing,
    cli.workers,
    cwd,
    trace,
);
```

The `_no_ssh_multiplexing` field is read indirectly — clap uses `overrides_with` to flip `cli.ssh_multiplexing` back to `false` when `--no-ssh-multiplexing` is the last occurrence. We never read `_no_ssh_multiplexing` ourselves; the underscore-prefix tells the Rust compiler this is intentional and silences the dead-code lint.

- [ ] **Step 4: Run tests to verify they all pass**

Run: `cd rust && cargo test`
Expected: all tests PASS, including the six new `ssh_multiplexing_test` tests and all existing trace/meta/runner tests.

- [ ] **Step 5: Run a quick manual smoke check**

Run from a non-repo directory containing some git repos:
```bash
cd rust && cargo build --release
../bin/git-all-rust --dry-run fetch | head -3
```
Expected: each printed line includes `-c "core.sshCommand=ssh -o ControlMaster=no -o ControlPath=none"`.

```bash
../bin/git-all-rust --dry-run --ssh-multiplexing fetch | head -3
```
Expected: lines do NOT include `core.sshCommand`.

```bash
../bin/git-all-rust --help | grep -i multiplex
```
Expected: shows `--ssh-multiplexing` only (the `--no-ssh-multiplexing` form is hidden).

- [ ] **Step 6: Commit**

```bash
git add rust/src/main.rs rust/src/runner.rs
git commit -m "feat: disable SSH multiplexing by default for parallel git runs"
```

---

## Task 4: Update user-facing docs and bump version

**Files:**
- Modify: `docs/index.md`
- Modify: `rust/Cargo.toml`

- [ ] **Step 1: Replace the SSH Multiplexing section in `docs/index.md`**

The existing section at the bottom of `docs/index.md` recommends enabling SSH multiplexing for ~3x speedup. With the new default, that advice is now contradicted at parallel scale, so the section needs to flip.

Replace the entire `## Performance: SSH Multiplexing` section (everything from that header to the end of the file) with:

```markdown
## SSH Connection Multiplexing

By default, `git-all` disables SSH `ControlMaster` for every git subprocess it spawns. Specifically, every git invocation runs as if you had passed:

`git -c "core.sshCommand=ssh -o ControlMaster=no -o ControlPath=none" ...`

### Why disabled by default

When `git-all` fans out N parallel git processes against a single host (typically `github.com`), SSH multiplexing produces two failure modes:

* **MaxSessions ceiling.** All channels multiplex over a single SSH connection. OpenSSH's default `MaxSessions` is 10, and GitHub enforces a similar server-side cap. Firing 50 parallel git fetches through one master gives you ~10 truly concurrent + 40 queued, not 50 in parallel.
* **Cold-start race.** When no master socket exists and N processes fan out simultaneously, they race to create it. Most lose the race and either fall back to their own connection or block briefly waiting for the master to come up.

Disabling multiplexing forces each subprocess to open its own connection, which scales linearly with `--workers`.

### Opting back in

If you have a small number of repos and your workflow benefits from multiplexing, you can re-enable it for a run:

```bash
git-all --ssh-multiplexing pull
```

This makes `git-all` inherit your `~/.ssh/config` unchanged.
```

- [ ] **Step 2: Update the GLOBAL OPTIONS block in the same file**

In the existing usage block in `docs/index.md`, add the new flag. Replace:

```
   --ssh             Force SSH URLs (git@github.com:) for all remotes
   --https           Force HTTPS URLs (https://github.com/) for all remotes
```

with:

```
   --ssh             Force SSH URLs (git@github.com:) for all remotes
   --https           Force HTTPS URLs (https://github.com/) for all remotes
   --ssh-multiplexing  Enable SSH connection multiplexing (disabled by default)
```

- [ ] **Step 3: Bump the Rust crate version**

Edit `rust/Cargo.toml`:

```toml
version = "0.7.2-rc.1"
```

- [ ] **Step 4: Run tests one more time to confirm nothing regressed**

Run: `cd rust && cargo test`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add docs/index.md rust/Cargo.toml
git commit -m "docs: document --[no-]ssh-multiplexing and bump to 0.7.2-rc.1"
```

---

## Task 5: Commit the SPEC.md update

`docs/SPEC.md` was already updated with Section 6.5 and the v0.2.3 changelog entry as part of preparing this plan. Commit it as a separate logical change so the cross-impl spec bump is its own reviewable unit.

- [ ] **Step 1: Verify the SPEC changes are present and correct**

Run: `git diff docs/SPEC.md`
Expected: a new Section 6.5 specifying the `--ssh-multiplexing` / `--no-ssh-multiplexing` pair, an updated Appendix A grammar block listing both flag forms, a v0.2.3 entry in Appendix C, and a version bump on line 3.

- [ ] **Step 2: Commit**

```bash
git add docs/SPEC.md
git commit -m "spec: add 6.5 --[no-]ssh-multiplexing, bump to v0.2.3"
```

---

## Out of scope (explicitly)

* **Zig and Crystal implementations.** This plan only updates the Rust impl. Section 6.5 of `SPEC.md` is now normative for both, but porting is tracked separately.
* **Inspecting remotes to apply the override conditionally.** Section 6.5 forbids this — even if a repo only uses HTTPS, the cost of carrying the unused `core.sshCommand` config is effectively zero, and the lookup cost to determine the scheme is not.
* **Auto-tuning based on repo count.** A future optimization could keep ControlMaster on for very small `--workers` values, but that is a separate decision and not part of this change.
