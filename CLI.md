# nit & knit CLI Reference

## Overview

Two complementary CLIs for running git commands across multiple repositories:

* **nit**: Operates on repos found from the current working directory
* **knit**: Operates on repos from user-configured "roots"

Both preserve the `git` passthrough model: `nit <cmd> [args]` runs `git <cmd> [args]` on multiple repos.

---

## nit - Local Multi-Repo Git

```
NAME
    nit - parallel git operations across repositories in current directory

SYNOPSIS
    nit [OPTIONS] <command> [<args>...]
    nit --help | --version

DESCRIPTION
    nit discovers git repositories under the current directory and runs
    the specified git command across all of them in parallel.

    Any command not recognized by nit is passed through to git verbatim.

OPTIONS
    -d, --depth <N|all>
        Search depth for repository discovery.
        N = 1, 2, 3, ... (positive integer)
        all = unlimited recursion (stop at .git boundaries)
        Default: 1 (immediate subdirectories only)

    -n, --workers <N>
        Number of parallel workers.
        Default: auto-detect CPU count

    --dry-run
        Print exact git commands without executing them.

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
    nit status
        Show single-line status for each repo in CWD (depth 1).

    nit pull -p
        Pull with prune for all repos. The -p is passed to git.

    nit -d 3 fetch
        Fetch repos up to 3 levels deep.

    nit -d all status
        Status of ALL repos recursively from CWD.

    nit --dry-run pull
        Show what git commands would run without executing.

    nit checkout main
        Checkout main branch in all repos (passthrough mode).
```

---

## knit - Roots-Based Multi-Repo Git

```
NAME
    knit - parallel git operations across registered repository roots

SYNOPSIS
    knit [OPTIONS] <command> [<args>...]
    knit roots [add|rm|list] [<path>]
    knit --help | --version

DESCRIPTION
    knit operates on repositories discovered from user-configured "roots".
    Unlike nit, which starts from CWD, knit uses a persistent configuration
    to define where your repositories live.

    Run knit from anywhere - it always uses your configured roots.

OPTIONS
    -d, --depth <N|all>
        Search depth within each root.
        Default: 1

    -n, --workers <N>
        Number of parallel workers.
        Default: auto-detect CPU count

    --dry-run
        Print exact git commands without executing them.

    -h, --help
        Show help message.

    -V, --version
        Show version.

ROOT MANAGEMENT
    knit roots
        List all configured roots.

    knit roots add <path>
        Add a directory as a root. Path is canonicalized and stored.

    knit roots rm <path>
        Remove a root from configuration.

EXAMPLES
    knit roots add ~/src
        Register ~/src as a root directory.

    knit roots add ~/work
        Register another root.

    knit roots
        List all roots:
          ~/src
          ~/work

    knit status
        Status of all repos under all roots.

    knit pull -p
        Pull all repos from all roots.

    knit -d all fetch
        Fetch repos recursively within each root.

    knit roots rm ~/old-projects
        Remove a root.

CONFIGURATION
    Roots are stored in:
        ~/.config/nit/roots.toml    (Linux/macOS XDG)

    Format:
        [[roots]]
        path = "/Users/rob/src"

        [[roots]]
        path = "/Users/rob/work"
```

---

## Output Format

### nit output (flat, from CWD)

```
repo-name        ✓ clean
another-repo     ↓2 ↑1 (main)
dirty-repo       M3 ?2 (feature-branch)
```

### knit output (grouped by root)

```
~/src
  project-a      ✓ clean
  project-b      ↓2 ↑1 (main)
~/work
  client-app     M3 ?2 (feature-branch)
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
$ nit --dry-run pull
[nit v0.2.0] Dry-run mode - commands will not execute

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
$ nit pull
repo-a           ✓ Already up to date
repo-b           ✗ Could not resolve host: github.com
repo-c           ✓ Already up to date

1 of 3 repositories failed.
```

---

## Configuration

### File Locations

```
~/.config/nit/
└── roots              # knit root configuration (plain text)

~/.cache/nit/
└── ...                # Cached repo discovery (future)
```

### roots File Format

Plain text, one path per line. Lines starting with `#` are comments.

```
# ~/.config/nit/roots
~/src
~/work
/absolute/path/also/works
```

Paths are canonicalized when added via `knit roots add`.

---

## Implementation Notes

### Binary Structure

Single binary with symlink detection:

```
nit (main binary)
knit -> nit (symlink)
```

Binary detects invocation name (`argv[0]`) to determine mode.

### Wrapper Scripts

```
./bin/nit-rust    → runs Rust implementation
./bin/knit-rust   → symlink to nit-rust
./bin/nit-zig     → runs Zig implementation
./bin/knit-zig    → symlink to nit-zig
```
