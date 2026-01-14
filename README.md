# fit

A fast CLI for running parallel git operations across many repositories.

## Why fit?

If you work with multiple git repositories (microservices, monorepo-adjacent projects, or just many OSS checkouts), running `git pull` or `git status` across all of them is tedious. `fit` makes it fast:

```bash
# Instead of cd-ing into each repo...
$ fit pull
repo-a        Already up to date.
repo-b        Fast-forward: 3 files changed
repo-c        Already up to date.
api-service   Fast-forward: 1 file changed
```

All repos pulled in parallel with condensed single-line output.

## Installation

```bash
brew tap rsanheim/tap
brew install fit
```

**Requirements:**
* macOS (Apple Silicon or Intel)
* Git 2.25+ recommended (uses `git -C` for directory switching)

## Usage

### Multi-Repository Mode

When run from a directory containing multiple git repos (but not inside one), `fit` discovers repos at depth 1 and runs commands in parallel:

```bash
fit pull      # Pull all repos
fit fetch     # Fetch all repos
fit status    # Status all repos
```

Any other command passes through to git for each repo:

```bash
fit log --oneline -5    # Show recent commits in all repos
fit branch              # List branches in all repos
```

### Passthrough Mode

Inside a git repository, `fit` acts as a transparent wrapper. `fit status` becomes `git status`. This lets you use `fit` everywhere without thinking about which mode you're in.

### Options

```
-n, --workers N   Parallel workers (default: 8, 0 = unlimited)
--dry-run         Print commands without executing
--ssh             Force SSH URLs for remotes
--https           Force HTTPS URLs for remotes
```

## Performance Tips

For network operations (`pull`, `fetch`), SSH connection overhead adds up. Enable SSH multiplexing to reuse connections:

```
# ~/.ssh/config
Host github.com
  ControlMaster auto
  ControlPath ~/.ssh/sockets/%r@%h-%p
  ControlPersist 9m
```

Note: GitHub terminates idle SSH connections after 10 minutes, so keep `ControlPersist` under that.

```bash
mkdir -p ~/.ssh/sockets && chmod 700 ~/.ssh/sockets
```

This can reduce `fit pull` time by ~3x across many repos.

## Development

`fit` is implemented in multiple languages (Rust, Zig, Crystal) to compare approaches. The Homebrew formula installs the Rust implementation.

```bash
script/build -t rust     # Build
script/test -t rust      # Test
./bin/fit-rust status    # Run locally
```

See [docs/SPEC.md](docs/SPEC.md) for the formal specification, [docs/dev/](docs/dev/) for contributor documentation, and [CircleCI](https://app.circleci.com/pipelines/github/rsanheim/fit) for build status.

## License

MIT - see [LICENSE](LICENSE)
