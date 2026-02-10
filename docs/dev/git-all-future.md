# git-all - Planned Future Functionality

**Status: Not yet implemented**

This document describes planned functionality for `git-all-roots`, a companion CLI to `git-all` that operates on user-configured repository "roots" rather than the current working directory.

---

## git-all-roots - Roots-Based Multi-Repo Git

```
NAME
    git-all-roots - parallel git operations across registered repository roots

SYNOPSIS
    git-all-roots [OPTIONS] <command> [<args>...]
    git-all-roots roots [add|rm|list] [<path>]
    git-all-roots --help | --version

DESCRIPTION
    git-all-roots operates on repositories discovered from user-configured "roots".
    Unlike git-all, which starts from CWD, git-all-roots uses a persistent configuration
    to define where your repositories live.

    Run git-all-roots from anywhere - it always uses your configured roots.

OPTIONS
    --scan-depth <N|all>
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
    git-all-roots roots
        List all configured roots.

    git-all-roots roots add <path>
        Add a directory as a root. Path is canonicalized and stored.

    git-all-roots roots rm <path>
        Remove a root from configuration.

EXAMPLES
    git-all-roots roots add ~/src
        Register ~/src as a root directory.

    git-all-roots roots add ~/work
        Register another root.

    git-all-roots roots
        List all roots:
          ~/src
          ~/work

    git-all-roots status
        Status of all repos under all roots.

    git-all-roots pull -p
        Pull all repos from all roots.

    git-all-roots --scan-depth all fetch
        Fetch repos recursively within each root.

    git-all-roots roots rm ~/old-projects
        Remove a root.

CONFIGURATION
    Roots are stored in:
        ~/.config/git-all/roots.toml    (Linux/macOS XDG)

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
git-all (main binary)
git-all-roots -> git-all (symlink)
```

**Detection logic** (works across all implementations):

```
basename = get_basename(argv[0])  # strip path, get "git-all" or "git-all-roots"
if basename contains "roots":
    mode = ROOTS (roots-based)
else:
    mode = CWD (CWD-based)
```

This approach works for:
* Direct invocation: `./git-all`, `./git-all-roots`
* Symlinks: `git-all-roots -> git-all`
* Full paths: `/usr/local/bin/git-all`
* Wrapper scripts named appropriately
