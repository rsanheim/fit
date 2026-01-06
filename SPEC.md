# nit Specification

Version: 0.1.0
Status: Draft

## Abstract

This document specifies the behavior of `nit`, a command-line interface for running parallel git operations across multiple repositories. Implementations in any language MUST conform to this specification to be considered compliant.

## Conformance

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

A conforming implementation:

1. MUST implement all MUST/REQUIRED behaviors
2. SHOULD implement all SHOULD/RECOMMENDED behaviors
3. MAY implement OPTIONAL behaviors
4. MUST NOT exhibit behaviors marked MUST NOT

## 1. Operating Modes

### 1.1 Passthrough Mode

1. Before any other operation, the implementation MUST check if the current working directory is inside a git repository.

2. The check MUST be performed using `git rev-parse --git-dir` or equivalent logic that correctly handles worktrees, bare repositories, and the `GIT_DIR` environment variable.

3. If inside a git repository, the implementation MUST exec git with all original command-line arguments unchanged.

4. In passthrough mode, nit-specific flags (`--dry-run`, `--workers`, etc.) MUST be passed through to git verbatim. The implementation MUST NOT parse or interpret these flags.

5. The implementation MUST NOT check for sub-repositories when in passthrough mode.

6. The exec MUST replace the nit process with git such that exit codes and signals are preserved transparently.

### 1.2 Multi-Repository Mode

1. If not inside a git repository, the implementation MUST operate in multi-repository mode.

2. In this mode, the implementation discovers git repositories under the current directory and executes commands across all of them in parallel.

## 2. Repository Discovery

### 2.1 Discovery Algorithm

1. The implementation MUST discover git repositories starting from the current working directory.

2. A directory MUST be considered a git repository if and only if it contains a `.git` subdirectory or file.

3. The default search depth MUST be 1 (immediate subdirectories only).

4. When `--depth N` is specified, the implementation MUST search up to N levels deep.

5. When `--depth all` is specified, the implementation MUST search recursively without limit, stopping at `.git` boundaries.

6. The implementation MUST NOT descend into discovered repositories (no nested repository discovery).

7. Repository order in output SHOULD be deterministic. Alphabetical sorting by directory name is RECOMMENDED.

### 2.2 Empty Results

1. If no repositories are discovered, the implementation MUST print a message to stdout and exit with status 0.

2. The message SHOULD be: "No git repositories found in current directory"

## 3. Command Execution

### 3.1 Git Invocation

1. All git commands MUST be executed using `git -C <repo-path>` for directory switching.

2. The implementation MUST set `GIT_TERMINAL_PROMPT=0` in the subprocess environment to prevent interactive prompts.

3. The implementation MUST capture both stdout and stderr from git subprocesses.

4. The implementation MUST handle stdout and stderr pipes to prevent buffer deadlock. When capturing output, pipes MUST be drained concurrently with process execution. This can be achieved through non-blocking I/O, dedicated reader threads, or stdlib facilities that handle this internally (e.g., Rust's `wait_with_output()`).

### 3.2 Parallelism

1. Commands MUST be executed in parallel across repositories.

2. The default maximum concurrent processes SHOULD be 8.

3. When `--workers 0` or `--max-connections 0` is specified, the implementation MUST spawn all processes immediately with no limit.

4. Output MUST be printed in a deterministic order (repository discovery order), regardless of process completion order.

### 3.3 Error Handling

1. If any repository command fails, the implementation MUST continue processing remaining repositories.

2. Error output for a repository SHOULD be prefixed with an indicator such as "ERROR:" or a failure symbol.

3. The implementation SHOULD print a summary of failures at the end when any repositories failed.

## 4. Optimized Commands

### 4.1 General Requirements

1. The following commands MUST have optimized, single-line output:
   * `status`
   * `pull`
   * `fetch`

2. Each repository's output MUST fit on a single line.

3. The output format MUST be: `<repo-name> <status-message>`

4. Repository names exceeding the display width SHOULD be truncated with an ellipsis pattern.

### 4.2 status Command

1. The implementation MUST use `git status --porcelain` or equivalent for machine-readable output.

2. The implementation SHOULD use `--no-optional-locks` to avoid index lock contention in parallel execution.

3. A repository with no changes MUST indicate a clean state.

4. The implementation SHOULD show counts of modified and untracked files.

### 4.3 pull Command

1. Successful pulls with no changes SHOULD output "Already up to date" or equivalent.

2. Successful pulls with changes SHOULD show a summary of what changed.

### 4.4 fetch Command

1. Successful fetches with no new data MAY output an empty status or indicate up-to-date state.

## 5. Passthrough Commands

### 5.1 Behavior

1. Any command not recognized as an optimized command MUST be passed through to git verbatim.

2. All arguments after the command name MUST be forwarded to git without modification.

3. Passthrough commands MUST still execute in parallel across repositories.

4. Output from passthrough commands SHOULD be displayed without single-line condensation.

## 6. Global Options

### 6.1 --dry-run

1. When `--dry-run` is specified, the implementation MUST NOT execute any git commands.

2. The implementation MUST print the exact command that would be executed for each repository.

3. The dry-run output MUST be generated from the same code path that builds actual commands.

4. Dry-run output SHOULD include a header indicating dry-run mode and the nit version.

#### 6.1.1 Dry-Run Implementation Constraint

The dry-run output MUST be constructed as close to actual execution as possible. Implementations MUST NOT construct dry-run strings separately from execution logic.

Conformant pattern (pseudocode):
```
cmd = build_git_command(repo, args)
if dry_run:
    print(cmd.to_string())
else:
    cmd.execute()
```

Non-conformant pattern:
```
if dry_run:
    print("would run git " + args)  // Separate construction - WRONG
else:
    build_and_execute(repo, args)
```

### 6.2 --ssh / --https

1. When `--ssh` is specified, the implementation MUST rewrite HTTPS URLs to SSH format using git config: `-c "url.git@github.com:.insteadOf=https://github.com/"`

2. When `--https` is specified, the implementation MUST rewrite SSH URLs to HTTPS format using git config: `-c "url.https://github.com/.insteadOf=git@github.com:"`

3. These flags MUST be mutually exclusive.

### 6.3 --workers / -n

1. This option MUST accept a non-negative integer.

2. Value 0 MUST mean "unlimited" (spawn all processes immediately).

3. The default value SHOULD be 8.

### 6.4 --depth / -d

**Status: Not yet implemented**

1. This option MUST accept a positive integer or the string "all".

2. The default value MUST be 1.

## 7. Output Format

### 7.1 Repository Name Display

1. Repository names MUST be displayed clearly, typically left-aligned.

2. The implementation SHOULD use a consistent width for repository names.

3. Names exceeding the width SHOULD be truncated with a trailing ellipsis.

### 7.2 Status Symbols

The following symbols are RECOMMENDED for status output:

| Symbol | Meaning |
|--------|---------|
| ✓ | Clean, up to date |
| ↓N | N commits behind remote |
| ↑N | N commits ahead of remote |
| MN | N modified files |
| ?N | N untracked files |

## 8. Exit Codes

1. Exit code 0 MUST indicate the nit command itself succeeded.

2. The implementation MAY exit non-zero if any individual repository operation failed.

3. Exit code 1 SHOULD be used for nit-level failures (invalid arguments, etc.).

## Appendix A: Grammar

```
nit [OPTIONS] <COMMAND> [ARGS...]

OPTIONS:
    --dry-run
    --ssh
    --https
    -n, --workers <N>
    -d, --depth <N|all>
    -h, --help
    -V, --version

COMMAND:
    status | pull | fetch | <git-command>

ARGS:
    Passed through to git
```

## Appendix B: References

* [RFC 2119 - Key words for use in RFCs to Indicate Requirement Levels](https://www.rfc-editor.org/rfc/rfc2119)
* [Git Documentation](https://git-scm.com/docs)
* [CLI.md](./CLI.md) - User-facing CLI reference

## Appendix C: Changelog

### v0.1.0 (Initial Draft)

* Initial specification based on Rust and Zig implementations
* Added Passthrough Mode (Section 1.1)
