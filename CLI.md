# fit CLI Reference

## Overview

`fit` is a CLI for running git commands across multiple repositories in parallel.

It preserves the `git` passthrough model: `fit <cmd> [args]` runs `git <cmd> [args]` on multiple repos.

See also: [docs/fit-future.md](docs/fit-future.md) for planned `fit` functionality (roots-based multi-repo git).

---

## fit - Local Multi-Repo Git

```
NAME
    fit - parallel git operations across repositories in current directory

SYNOPSIS
    fit [OPTIONS] <command> [<args>...]
    fit --help | --version

DESCRIPTION
    fit discovers git repositories under the current directory and runs
    the specified git command across all of them in parallel.

    Any command not recognized by fit is passed through to git verbatim.

OPTIONS
    -d, --depth <N|all>
        Search depth for repository discovery.
        N = 1, 2, 3, ... (positive integer)
        all = unlimited recursion (stop at .git boundaries)
        Default: 1 (immediate subdirectories only)
        **Status: Not yet implemented**

    -n, --workers <N>
        Number of parallel workers.
        Default: 8 (0 = unlimited)

    --dry-run
        Print exact git commands without executing them.

    --ssh
        Force SSH URLs (git@github.com:) for all remotes.

    --https
        Force HTTPS URLs (https://github.com/) for all remotes.

    -h, --help
        Show help message.

    -V, --version
        Show version.

OPTIMIZED COMMANDS
    pull, fetch, status
        These commands run in parallel with condensed single-line output
        per repository.

PASSTHROUGH
    Any other command (checkout, commit, log, etc.) is passed directly
    to git for each repository.

EXAMPLES
    fit status
        Show single-line status for each repo in CWD (depth 1).

    fit pull -p
        Pull with prune for all repos. The -p is passed to git.

    fit --dry-run pull
        Show what git commands would run without executing.

    fit checkout main
        Checkout main branch in all repos (passthrough mode).

    fit --ssh fetch
        Fetch using SSH URLs even if remotes are configured as HTTPS.
```

---

## Output Format

```
repo-name        ✓ clean
another-repo     ↓2 ↑1 (main)
dirty-repo       M3 ?2 (feature-branch)
```

### Legend

| Symbol | Meaning |
|--------|---------|
| ✓ | Clean, up to date |
| ↓N | N commits behind remote |
| ↑N | N commits ahead of remote |
| MN | N modified files |
| ?N | N untracked files |
| (branch) | Current branch (shown if not main/master) |

---

## Dry-Run Output

```
$ fit --dry-run pull
[fit v0.2.0] Dry-run mode - commands will not execute

git -C /Users/rob/src/project-a pull
git -C /Users/rob/src/project-b pull
git -C /Users/rob/src/project-c pull
```

---

## Error Handling

* Non-zero exit if ANY repo command fails
* Continue processing remaining repos on failure (don't bail early)
* Summary at end shows which repos failed

```
$ fit pull
repo-a           ✓ Already up to date
repo-b           ✗ Could not resolve host: github.com
repo-c           ✓ Already up to date

1 of 3 repositories failed.
```

---

## Wrapper Scripts

Development wrappers in `./bin/` for testing each implementation:

```
./bin/fit-rust     → runs Rust implementation
./bin/fit-zig      → runs Zig implementation
./bin/fit-crystal  → runs Crystal implementation
```

The underlying binary is executed with arguments passed through.
