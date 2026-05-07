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
   --scan-depth <N|all>  Repository scan depth (default: 1)
   --dry-run         Print the exact command for every repo without running it
   --ssh             Force SSH URLs (git@github.com:) for all remotes
   --https           Force HTTPS URLs (https://github.com/) for all remotes
   --ssh-multiplexing  Enable SSH connection multiplexing (disabled by default)
```

## Example: dry-run mode

```bash
[~/src/oss] git-all pull --dry-run
[git-all v0.7.2-rc.1] Running in **dry-run mode**, no git commands will be executed. Planned git commands below.
git -c "core.sshCommand=ssh -o ControlMaster=no -o ControlPath=none" -C ~/src/oss/repo1 pull
git -c "core.sshCommand=ssh -o ControlMaster=no -o ControlPath=none" -C ~/src/oss/repo2 pull
git -c "core.sshCommand=ssh -o ControlMaster=no -o ControlPath=none" -C ~/src/oss/repo3 pull
```

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
