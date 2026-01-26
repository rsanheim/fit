# Git Performance Flags and Concurrent Operations

This document explains git's locking model, performance-related flags, and how they affect environments where multiple tools access git simultaneously (git-all, IDEs, pre-commit hooks, etc.).

## Git's Locking Model

Git uses advisory file locking via lock files:

1. Create `resource.lock` file (e.g., `index.lock`)
2. Write new content to the lock file
3. Rename lock file to target (atomic on POSIX)
4. On failure, delete lock file

### Types of Locks

**Required locks**: Operations that modify state (commit, checkout, merge) *must* take locks. These cannot be skipped.

**Optional locks**: Some read-only operations take locks as a *performance optimization*, not for correctness. For example, `git status` may update the index with fresh stat information (file sizes, mtimes) to speed up future operations.

## The `--no-optional-locks` Flag

Introduced in Git 2.15.0, this flag tells git to skip all optional locks.

### What Does `git status` Normally Do?

When you run `git status`:

1. Git reads the index (`.git/index`)
2. Stats every tracked file to detect modifications
3. **Optionally**: Updates the index with fresh stat info (this is the "stat cache refresh")
4. Reports status

Step 3 requires taking `index.lock`. This is optional because it's purely an optimization—future `git status` calls will be faster if the stat cache is current.

### The Problem with Parallel Operations

When multiple processes run `git status` simultaneously:

| Without `--no-optional-locks` | With `--no-optional-locks` |
|-------------------------------|---------------------------|
| Each process tries to update index | Processes only read the index |
| Compete for `index.lock` | No lock contention |
| One succeeds, others block or fail | All run truly in parallel |
| "Another git process seems to be running" errors | No errors |

### Usage

```bash
# Command line flag
git status --no-optional-locks

# Or as a prefix option
git --no-optional-locks status
```

## The `GIT_OPTIONAL_LOCKS=0` Environment Variable

The environment variable equivalent of `--no-optional-locks`. When set to `0`, git skips optional locks for *all* commands in that process.

```bash
GIT_OPTIONAL_LOCKS=0 git status
```

### Who Uses This?

- **VS Code**: Sets this internally for its git integration
- **IntelliJ/JetBrains**: Similar behavior
- **Build systems**: CI/CD pipelines running parallel jobs
- **Tools like git-all**: Running multiple git commands simultaneously

### Recommendation for git-all

For read-only operations like `status`, always use `--no-optional-locks`:

```rust
let mut args = vec![
    "status".to_string(),
    "--porcelain".to_string(),
    "--no-optional-locks".to_string(),
];
```

For commands executed via subprocess, consider setting the environment variable:

```rust
Command::new("git")
    .env("GIT_OPTIONAL_LOCKS", "0")
    // ...
```

## Other Performance-Related Settings

### `core.preloadindex`

Enables parallel loading of the index using multiple threads.

```bash
git config core.preloadindex true
```

- **Default**: `true` since Git 2.1 on systems with threading
- **How it works**: Divides index entries among CPU cores, each thread stats files in its portion
- **Impact**: 20-50% faster status on multi-core systems with large indexes
- **Note**: Already enabled by default; no action needed for git-all

### `core.untrackedCache`

Caches information about untracked files to avoid re-scanning directories.

```bash
git config core.untrackedCache true
```

- **How it works**: Remembers directory mtimes; only re-scans directories that changed
- **Impact**: 10-30% improvement when many untracked files
- **Trade-off**: Slightly more memory usage in index

### `core.fsmonitor`

Delegates file change detection to a filesystem monitor daemon.

```bash
# Built-in daemon (Git 2.37+)
git config core.fsmonitor true

# Or with Watchman
git config core.fsmonitor .git/hooks/fsmonitor-watchman
```

- **How it works**: Daemon watches for file changes; git queries daemon instead of stat'ing every file
- **Impact**: 50-90% improvement for large repos (10K+ files)
- **Trade-offs**:
  - Daemon uses resources
  - May not work with network mounts
  - Requires user opt-in

### `feature.manyFiles`

A meta-option (Git 2.24+) that enables multiple optimizations:

```bash
git config feature.manyFiles true
```

Enables:
- `core.untrackedCache`
- `index.version=4` (more compact index format)

## Multi-Tool Contention Scenarios

A typical developer environment might have:

- **IDE** (VS Code/IntelliJ) polling git status every 1-5 seconds
- **git-all** running parallel git status across 90 repos
- **Git hooks** (pre-commit, husky) running on file save
- **Terminal** with manual git commands

### Common Problems

1. **Index lock contention**: `fatal: Unable to create '.git/index.lock': File exists`
2. **Stale stat cache**: One process updates stat info, another's copy becomes stale (minor correctness issue)
3. **Sporadic slowdowns**: Operations block waiting for locks

### Solutions

| Tool Type | Recommendation |
|-----------|---------------|
| Read-only tools (git-all status, IDE polling) | Always use `--no-optional-locks` |
| Write operations (commit, checkout) | Must take locks; contention acceptable |
| Hooks | Keep fast; avoid spawning many git processes |
| CI/CD | Set `GIT_OPTIONAL_LOCKS=0` globally |

## What About fetch/pull?

These network operations *do* modify refs:

- They require locks for ref updates (not optional)
- `--no-optional-locks` only affects optional locks
- Lock contention is acceptable since these are slower network operations anyway

For git-all's fetch/pull: Don't use `--no-optional-locks` (it wouldn't help and the network latency dominates).

## GitHub Concurrent Connection Limits

GitHub imposes limits on concurrent connections, though they don't publish exact numbers. This affects tools like git-all that spawn many parallel git operations.

### SSH Connections

SSH connections to GitHub can hit multiplexing limits. The error:
```
mux_client_request_session: session request failed: Session open refused by peer
```
indicates GitHub refused to open another session on the multiplexed connection.

### HTTPS Connections

GitHub states they have "no hard limits" for git operations over HTTPS, but will throttle/delay requests that could cause server overload:

> "Git operations do not consume part of your API rate limit... we don't have any hard limits for clones, though we may delay requests if they come in fast enough to potentially cause overload."

However, users report connection resets when cloning many repositories in parallel.

### Practical Limits

The [git-xargs](https://github.com/gruntwork-io/git-xargs) tool, which performs bulk git operations, defaults to **4 concurrent connections** after experiencing issues with higher parallelism.

| Protocol | Observed Safe Limit | Notes |
|----------|---------------------|-------|
| SSH | ~10 | Limited by SSH multiplexing |
| HTTPS | ~10-20 | More tolerant than SSH |

### Recommendations for git-all

1. **Default to 8-10 concurrent connections** for network operations (fetch/pull)
2. **Provide `--max-connections` flag** to override for local git servers or when using HTTPS
3. **Local operations (status) can remain unlimited** since they don't hit GitHub

### References

* [GitHub Community: Git clone limits discussion](https://github.com/orgs/commugit-ally/discussions/24841)
* [git-xargs: Rate limiting for repository cloning](https://github.com/gruntwork-io/git-xargs/issues/139)
* [git-xargs: Rate limit implementation](https://github.com/gruntwork-io/git-xargs/pull/142)

## Performance Impact Summary

| Flag/Config | Single Operation | Parallel Operations |
|-------------|------------------|---------------------|
| `--no-optional-locks` | Neutral to -5% | Eliminates contention |
| `core.preloadindex` | +20-50% | Same |
| `core.fsmonitor` | +50-90% (large repos) | Same |
| `core.untrackedCache` | +10-30% | Same |

## Recommendations for git-all

1. **Always use `--no-optional-locks` for status**: Eliminates lock contention, makes git-all a good citizen alongside IDEs

2. **Consider `GIT_OPTIONAL_LOCKS=0` env for all commands**: Simpler than per-command flags

3. **Don't force fsmonitor/untrackedCache**: These are user/repo-level choices with trade-offs

4. **`core.preloadindex` is already default**: No action needed

5. **For status, also use `--porcelain`**: Already implemented; gives machine-readable output

## References

- [Git 2.15 Release Notes](https://github.com/git/git/blob/master/Documentation/RelNotes/2.15.0.txt) - Introduced `--no-optional-locks`
- [git-status documentation](https://git-scm.com/docs/git-status)
- [git-config documentation](https://git-scm.com/docs/git-config) - `core.preloadindex`, `core.fsmonitor`, etc.
- [Scalar and Git performance](https://github.blog/2022-10-13-git-for-windows-2-38-0-released/) - Microsoft's work on git performance
