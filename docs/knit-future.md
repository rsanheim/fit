# knit - Planned Future Functionality

**Status: Not yet implemented**

This document describes planned functionality for `knit`, a companion CLI to `nit` that operates on user-configured repository "roots" rather than the current working directory.

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
        Default: 8

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
nit (main binary)
knit -> nit (symlink)
```

**Detection logic** (works across all implementations):

```
basename = get_basename(argv[0])  # strip path, get "nit" or "knit"
if basename contains "knit":
    mode = KNIT (roots-based)
else:
    mode = NIT (CWD-based)
```

This approach works for:
* Direct invocation: `./nit`, `./knit`
* Symlinks: `knit -> nit`
* Full paths: `/usr/local/bin/knit`
* Wrapper scripts named appropriately
