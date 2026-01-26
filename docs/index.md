# git-all

`git-all` is a CLI for running parallel git operations across many repositories.

## Operating Modes

**Passthrough Mode**: When inside a git repository, `git-all` acts as a transparent wrapper around `git`. All arguments pass through unchanged - `git-all status` becomes `git status`.

**Multi-Repository Mode**: When NOT inside a git repository, `git-all` discovers sub-repos at depth 1 and runs commands across all of them in parallel.

### Optimized Commands

In multi-repository mode, `git-all` provides optimized commands with condensed single-line output:

* `git-all pull` - Pull all repos with single-line status per repo
* `git-all fetch` - Fetch all repos with single-line status per repo
* `git-all status` - Status all repos with single-line status per repo

Any other command passes through to git verbatim for each repo.

## Installation

Build from source using the implementation of your choice:

```bash
# Rust implementation
script/build -t rust
script/install -t rust

# Zig implementation
script/build -t zig
script/install -t zig

# Crystal implementation
script/build -t crystal
script/install -t crystal
```

## Usage

```
git-all [global options] [command [command options]]

COMMANDS:
   pull             Pull all repositories
   fetch            Fetch all repositories
   status           Status all repositories
   [anything else]  Pass through to git

GLOBAL OPTIONS:
   -n, --workers N   Number of parallel workers (default: 8, 0 = unlimited)
   --dry-run         Print the exact command for every repo without running it
   --ssh             Force SSH URLs (git@github.com:) for all remotes
   --https           Force HTTPS URLs (https://github.com/) for all remotes
```

## Example: dry-run mode

```bash
[~/src/oss] git-all pull --dry-run
[git-all v0.6.0] Running in **dry-run mode**, no git commands will be executed.
git -C ~/src/oss/repo1 pull
git -C ~/src/oss/repo2 pull
git -C ~/src/oss/repo3 pull
```

## Performance: SSH Multiplexing

For network operations (`pull`, `fetch`), SSH connection overhead is significant. Enable SSH multiplexing to reuse connections:

Add to `~/.ssh/config`:

```
Host github.com
  ControlMaster auto
  ControlPath ~/.ssh/sockets/%r@%h-%p
  ControlPersist 8h

Host *
  ControlMaster auto
  ControlPath ~/.ssh/sockets/%r@%h-%p
  ControlPersist 20m
```

Create the sockets directory:

```bash
mkdir -p ~/.ssh/sockets && chmod 700 ~/.ssh/sockets
```

This reduces `git-all pull` time by ~3x by avoiding repeated SSH handshakes.
