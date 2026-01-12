# fit - Planned Future Functionality

**Status: Not yet implemented**

This document describes planned functionality for `fit`, a companion CLI to `fit` that operates on user-configured repository "roots" rather than the current working directory.

---

## fit - Roots-Based Multi-Repo Git

```
NAME
    fit - parallel git operations across registered repository roots

SYNOPSIS
    fit [OPTIONS] <command> [<args>...]
    fit roots [add|rm|list] [<path>]
    fit --help | --version

DESCRIPTION
    fit operates on repositories discovered from user-configured "roots".
    Unlike fit, which starts from CWD, fit uses a persistent configuration
    to define where your repositories live.

    Run fit from anywhere - it always uses your configured roots.

OPTIONS
    -d, --depth <N|all>
        Search depth within each root.
        Default: 1

    -n, --workers <N>
        Number of parallel workers.
        Default: 8

    --dry-run
        Print exact git commands without executing them.

    -h, --help
        Show help message.

    -V, --version
        Show version.

ROOT MANAGEMENT
    fit roots
        List all configured roots.

    fit roots add <path>
        Add a directory as a root. Path is canonicalized and stored.

    fit roots rm <path>
        Remove a root from configuration.

EXAMPLES
    fit roots add ~/src
        Register ~/src as a root directory.

    fit roots add ~/work
        Register another root.

    fit roots
        List all roots:
          ~/src
          ~/work

    fit status
        Status of all repos under all roots.

    fit pull -p
        Pull all repos from all roots.

    fit -d all fetch
        Fetch repos recursively within each root.

    fit roots rm ~/old-projects
        Remove a root.

CONFIGURATION
    Roots are stored in:
        ~/.config/fit/roots.toml    (Linux/macOS XDG)

    Format:
        [[roots]]
        path = "/Users/rob/src"

        [[roots]]
        path = "/Users/rob/work"
```

---

## Output Format (Grouped by Root)

```
~/src
  project-a      ✓ clean
  project-b      ↓2 ↑1 (main)
~/work
  client-app     M3 ?2 (feature-branch)
```

---

## Implementation Notes

### Binary Structure

Single binary with symlink detection. The binary inspects `argv[0]` to determine
which mode to run in:

```
fit (main binary)
fit -> fit (symlink)
```

**Detection logic** (works across all implementations):

```
basename = get_basename(argv[0])  # strip path, get "fit" or "fit"
if basename contains "fit":
    mode = FIT (roots-based)
else:
    mode = FIT (CWD-based)
```

This approach works for:
* Direct invocation: `./fit`, `./fit`
* Symlinks: `fit -> fit`
* Full paths: `/usr/local/bin/fit`
* Wrapper scripts named appropriately
