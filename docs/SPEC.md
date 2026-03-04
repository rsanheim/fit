# git-all Specification

Version: 0.2.3
Status: Draft

## Abstract

This document specifies the behavior of `git-all`, a command-line interface for running parallel git operations across multiple repositories. Implementations in any language MUST conform to this specification to be considered compliant.

*Note: This project was renamed from `fit` to `git-all` in v0.2.0.*

## Conformance

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

A conforming implementation:

1. MUST implement all MUST/REQUIRED behaviors
2. SHOULD implement all SHOULD/RECOMMENDED behaviors
3. MAY implement OPTIONAL behaviors
4. MUST NOT exhibit behaviors marked MUST NOT

## 1. Operating Modes

### 1.0 git-all Meta Mode

Before anything else, the implementation MUST check for an initial subcommand of 'meta' - ie `git-all meta` or `git-all meta <subcommand>`.  If found, the implementation MUST pass all args and options to `git-all` itself and NOT passthrough to git.

`git-all meta` (with no subcommand) and `git-all meta help` MUST print the help message for `git-all` itself, including the version of git-all and the version of the underlying git implementation.

If `git-all meta` is not found, the implementation MUST continue with the next operating mode.

### 1.1 Passthrough Mode

1. Next, the implementation MUST check if the current working directory is inside a git repository.

2. The check MUST be performed using `git rev-parse --git-dir` or equivalent logic that correctly handles worktrees, bare repositories, and the `GIT_DIR` environment variable.

3. If inside a git repository, the implementation MUST exec git with all original command-line arguments unchanged.

4. In passthrough mode, git-all-specific flags (`--dry-run`, `--workers`, etc.) MUST be passed through to git verbatim. The implementation MUST NOT parse or interpret these flags.

5. The implementation MUST NOT check for sub-repositories when in passthrough mode.

6. The exec MUST replace the git-all process with git such that exit codes and signals are preserved transparently.

### 1.2 Multi-Repository Mode

1. If not inside a git repository, the implementation MUST operate in multi-repository mode.

2. In this mode, the implementation discovers git repositories under the current directory and executes commands across all of them in parallel.

## 2. Repository Discovery

### 2.1 Discovery Algorithm

1. The implementation MUST discover git repositories starting from the current working directory.

2. A directory MUST be considered a git repository if and only if it contains a `.git` subdirectory or file.

3. The default search depth MUST be 1 (immediate subdirectories only).

4. When `--scan-depth N` is specified, the implementation MUST search up to N levels deep.

5. When `--scan-depth all` is specified, the implementation MUST search recursively without limit, stopping at `.git` boundaries.

6. The implementation MUST NOT accept `--depth` as a discovery option. `--scan-depth` is the only option that controls discovery depth.

7. The implementation MUST NOT descend into discovered repositories (no nested repository discovery).

8. Repository order in output SHOULD be deterministic. Alphabetical sorting by directory name is RECOMMENDED.

### 2.2 Empty Results

1. If no repositories are discovered, the implementation MUST print a message to stdout and exit with status 9.

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

5. Output SHOULD stream progressively as results become available, using head-of-line blocking: when a result arrives, all contiguous results from the front of the queue SHOULD be printed immediately rather than waiting for all results to complete.

### 3.3 Error Handling

1. If any repository command fails, the implementation MUST continue processing remaining repositories.

2. For optimized commands, error output for a repository MAY be prefixed with an indicator such as "ERROR:" or a failure symbol, and MUST preserve the underlying stderr line after the prefix. Passthrough commands MUST NOT alter git output (see Section 7.1.2).

3. The implementation SHOULD print a summary of failures at the end when any repositories failed.

## 4. Optimized Commands

### 4.1 General Requirements

1. The following commands MUST have human-readable, optimized, single-line output:
   * `status`
   * `pull`
   * `fetch`

2. Each repository's output MUST fit on a single line.

3. The output format MUST use three pipe-delimited columns: `<repo> | <branch> | <message>`. See Section 7.1 for full formatting rules.

4. Column widths MUST be consistent across all rows. See Section 7.1 for width and truncation rules.

### 4.2 status Command

1. The implementation MUST use `git status --porcelain -b` for machine-readable output with branch tracking information.

2. The implementation SHOULD use `--no-optional-locks` to avoid index lock contention in parallel execution.

3. Output MUST conform to Section 7.2 (Status Output Format).

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

4. Dry-run output SHOULD include a header indicating dry-run mode and the git-all version.

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

### 6.4 --scan-depth

**Status: Not yet implemented**

1. This option MUST accept a positive integer or the string "all".

2. The default value MUST be 1.

3. Implementations MUST NOT accept `--depth` as an alias for this option.

## 7. Output Format

### 7.1 Output Line Format

#### 7.1.1 Optimized Commands (status, pull, fetch)

Optimized commands MUST use a three-column pipe-delimited format:

```
<repo>                 | <branch>         | <message>
```

Example:

```
my-repo                | main             | clean
other-repo             | feature/login    | 1 modified, 2 untracked
third-repo             | HEAD (detached)  | 3 modified
infra-services-dock... | develop          | clean, 1 ahead
```

Column rules:

1. Each column MUST be left-aligned and padded to a consistent width across all rows.
2. The repo name column width SHOULD be computed from the actual values in the current run.
3. The branch column MUST use a fixed width of 16 characters. This enables streaming output without waiting for all results.
4. Repo and branch columns MUST have a maximum width cap to prevent overly wide output. Values exceeding the cap MUST be truncated with a trailing ellipsis (e.g. `...`).
4. When scan depth is greater than 1, the repo column MUST display paths relative to the current directory rather than just the leaf directory name.
5. When scan depth is the default (1), implementations MAY display paths instead of just the leaf name.
6. Detached HEAD state MUST be displayed as `HEAD (detached)` in the branch column.

#### 7.1.2 Passthrough Commands

Passthrough commands MUST NOT be condensed to a single line. Implementations SHOULD display git's stdout/stderr output verbatim, preserving newlines.

If a repository label is added, it MUST be on its own line before the command output and MUST NOT alter the git output itself.

### 7.2 Status Output Format

Status output for each repository MUST be a single line using human-readable word format.

#### 7.2.1 Rules

1. A clean repository with no ahead/behind MUST output: `clean`
2. File change counts MUST use the format `N <type>` where type is one of: *modified*, *added*, *deleted*, *renamed*, *untracked*
3. Multiple change types MUST be comma-separated: `1 modified, 2 untracked`
4. Ahead/behind remote counts are RECOMMENDED and MUST use format `N ahead` / `N behind`
5. When ahead/behind is shown alongside file changes, it MUST appear after file changes
6. Change types MUST appear in this order: *modified*, *added*, *deleted*, *renamed*, *untracked*, *ahead*, *behind*
7. Types with zero count MUST be omitted

#### 7.2.2 Porcelain Parsing

Each file's status is determined by the first two characters of `git status --porcelain` output:

* Position 0 (index/staged status): `M`=modified, `A`=added, `D`=deleted, `R`=renamed, `?`=untracked
* Position 1 (worktree/unstaged status): `M`=modified, `D`=deleted

When a file has both staged and unstaged changes (e.g. `MM`), it MUST be counted once using the index (position 0) status.
Worktree status (position 1) MUST only be counted when the index status is a space (no staged change).

#### 7.2.3 Expected Output Table

The following table maps `git status --porcelain -b` output to expected *branch* and *message* column values.
The repo column is determined by the repository path and is not covered here.
Implementations SHOULD use this table as a conformance test suite.

| Porcelain input | Branch | Message | Description |
|---|---|---|---|
| *(empty)* | *(unknown)* | `clean` | No changes, no branch info |
| `## main` | `main` | `clean` | Clean, no tracking branch |
| `## main...origin/main` | `main` | `clean` | Clean, up to date with remote |
| `## HEAD (no branch)` | `HEAD (detached)` | `clean` | Detached HEAD, clean |
| `## main`\n` M file.txt` | `main` | `1 modified` | One unstaged modification |
| `## main`\n` M a.txt`\n` M b.txt`\n` M c.txt` | `main` | `3 modified` | Multiple unstaged modifications |
| `## main`\n`M  file.txt` | `main` | `1 modified` | One staged modification |
| `## main`\n`MM file.txt` | `main` | `1 modified` | Staged + unstaged mod on same file counts once |
| `## main`\n`A  file.txt` | `main` | `1 added` | One staged new file |
| `## main`\n`AM file.txt` | `main` | `1 added` | Staged add, subsequent worktree mod counts as added |
| `## main`\n`D  file.txt` | `main` | `1 deleted` | One staged deletion |
| `## main`\n` D file.txt` | `main` | `1 deleted` | One unstaged deletion |
| `## main`\n`R  old.txt -> new.txt` | `main` | `1 renamed` | One rename |
| `## main`\n`?? file.txt` | `main` | `1 untracked` | One untracked file |
| `## main`\n`?? a.txt`\n`?? b.txt` | `main` | `2 untracked` | Multiple untracked files |
| `## main`\n` M mod.txt`\n`?? new.txt` | `main` | `1 modified, 1 untracked` | Mixed: modified + untracked |
| `## main`\n` M mod.txt`\n`A  add.txt`\n`?? new.txt` | `main` | `1 modified, 1 added, 1 untracked` | Mixed: multiple types |
| `## main`\n`M  a.txt`\n`A  b.txt`\n`D  c.txt`\n`R  d.txt -> e.txt`\n`?? f.txt` | `main` | `1 modified, 1 added, 1 deleted, 1 renamed, 1 untracked` | All types present |
| `## main...origin/main [ahead 2]` | `main` | `clean, 2 ahead` | Clean but ahead of remote |
| `## main...origin/main [behind 3]` | `main` | `clean, 3 behind` | Clean but behind remote |
| `## main...origin/main [ahead 2, behind 3]` | `main` | `clean, 2 ahead, 3 behind` | Clean, diverged from remote |
| `## main...origin/main [ahead 1]`\n` M file.txt` | `main` | `1 modified, 1 ahead` | Modified and ahead |
| `## feat...origin/feat [ahead 2, behind 1]`\n` M a.txt`\n`?? b.txt` | `feat` | `1 modified, 1 untracked, 2 ahead, 1 behind` | Mixed changes + diverged |

#### 7.2.4 Error Handling

For optimized command output, the message column MUST contain the first non-empty line of stderr. Implementations MAY prefix this line with "ERROR:" or a failure symbol; when prefixed, the stderr line MUST be preserved verbatim after the prefix.

| Condition | Expected output |
|---|---|
| Git command fails (non-zero exit) | First non-empty line of stderr (optionally prefixed) |
| Git command fails with empty stderr | `unknown error` (optionally prefixed) |

## 8. Exit Codes

1. Exit code 0 MUST indicate the git-all command itself succeeded.

2. The implementation MAY exit non-zero if any individual repository operation failed.

3. Exit code 9 SHOULD be used for git-all-level failures (invalid arguments, etc.).

## Appendix A: Grammar

```
git-all [OPTIONS] <COMMAND> [ARGS...]

OPTIONS:
    --dry-run
    --ssh
    --https
    -n, --workers <N>
    --scan-depth <N|all>
    -h, --help
    -V, --version

COMMAND:
    meta | status | pull | fetch | <git-command>

ARGS:
    Passed through to git
```

## Appendix B: References

* [RFC 2119 - Key words for use in RFCs to Indicate Requirement Levels](https://www.rfc-editor.org/rfc/rfc2119)
* [Git Documentation](https://git-scm.com/docs)

## Appendix C: Changelog

### v0.2.3 (2026-02-11)

* Changed branch column to fixed width of 16 characters (Section 7.1.1)
* Added streaming output via head-of-line blocking as RECOMMENDED behavior (Section 3.2)

### v0.2.2 (2026-02-10)

* Changed optimized command output to three-column pipe-delimited format: `repo | branch | message` (Section 7.1.1)
* Clarified passthrough command output is not condensed and preserves git output (Section 7.1.2)
* Clarified optimized error prefixing is optional and must preserve stderr content (Sections 3.3, 7.2.4)
* Replaced Section 7.2 status symbols with human-readable word format and executable test matrix
* Updated Section 4.2 to require `--porcelain -b` for branch tracking info
* Added ahead/behind remote as RECOMMENDED output

### v0.2.1 (2026-02-10)

* Renamed discovery depth option to `--scan-depth` to avoid conflicts with git's `--depth`

### v0.2.0 (2026-01-26)

* **Renamed project from `fit` to `git-all`**
* Updated all command references and examples
* Renamed `--fit-depth` option to `--depth`

### v0.1.1 (2026-01-12)

* Added `fit meta` Mode (Section 1.0)
* Added update exit codes (Section 8)

### v0.1.0 (Initial Draft)

* Initial specification based on Rust and Zig implementations
* Added Passthrough Mode (Section 1.1)
