# git-all

A fast CLI for running parallel git operations across many repositories.

## Why git-all?

If you work with multiple git repositories (microservices, monorepo-adjacent projects, or just many OSS checkouts), running `git pull` or `git status` across all of them is tedious. `git-all` makes it fast and scannable:

```bash
$ git-all pull
api-service   Fast-forward: 1 file changed
repo-a        Already up to date.
repo-b        Fast-forward: 3 files changed
repo-c        Already up to date.
```

* *Parallel execution* - all repos update simultaneously
* *Single-line output* - one line per repo for easy scanning
* *Deterministic alpha ordering* - quickly find the repo you care about

## Installation

```bash
brew tap rsanheim/tap
brew install git-all
```

**Requirements:**
* macOS only for now (Apple Silicon or Intel) - cross-platform builds coming
* Git 2.25+ recommended (uses `git -C` for directory switching)

## Usage

### Multi-Repository Mode

When run from a directory containing multiple git repos (but not inside one), `git-all` discovers repos at depth 1 and runs commands in parallel:

```bash
git-all pull      # Pull all repos
git-all fetch     # Fetch all repos
git-all status    # Status all repos
```

Any other command passes through to git for each repo:

```bash
git-all log --oneline -5    # Show recent commits in all repos
git-all branch              # List branches in all repos
```

### Passthrough Mode

Inside a git repository, `git-all` acts as a transparent wrapper. `git-all status` becomes `git status`. This lets you use `git-all` everywhere without thinking about which mode you're in.

### Options

```
-n, --workers N   Parallel workers (default: 8, 0 = unlimited)
--scan-depth <N|all>  Repository scan depth (default: 1)
--dry-run         Print commands without executing
--https           Force HTTPS URLs for remotes
--ssh             Force SSH URLs for remotes
```

### Meta Commands

`git-all meta help` shows version info and `git-all`'s own help (`git-all help` passes through to git's help)

```bash
$ git-all meta help
git-all v0.6.0 (git 2.52.0)
...
```

## Performance Tips

For network operations (`pull`, `fetch`), SSH connection overhead adds up if you use SSH git remotes. Enable SSH multiplexing to reuse connections...for example, with GitHub:

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

This can reduce `git-all pull` time by ~3x across many repos.

## Similar tools

There are a lot of similar tools out there, and most of them are more powerful and 'set it and  forget it' than git-all. They also tend to require more configuration and setup. Use what works for you!

* [ghorg](https://github.com/gabrie30/ghorg) - clone or backup entire user/org repos into one dir
* [ghorg](https://github.com/gabrie30/ghorg) - remote repo managemnt made easy
* [git-xargs](https://github.com/gruntwork-io/git-xargs) - run git commands across many repos

## Development

`git-all` is implemented in multiple languages (Rust, Zig, Crystal) to compare approaches, benchmarks, and to generally see how LLMs do working from a SPEC.md driven approach. The _blessed_ implementation is the Rust implementation, so homebrew installs that. If you want to play with other implementations, clone this repo and build them yourself:

```bash
script/build -t rust         # Build
script/test -t rust          # Test
./bin/git-all-rust status    # Run locally
```

See [docs/SPEC.md](docs/SPEC.md) for the formal specification, [docs/dev/](docs/dev/) for contributor documentation, and [CircleCI](https://app.circleci.com/pipelines/github/rsanheim/git-all) for build status.

**scary AI warning** I built this with much help from AI agents. Its been fun to see what LLMs can do with something like a formal(ish) specifications across languages I don't know (Rust, Zig) and languages I do know (Ruby, Go, Crystal).

FWIW, I use `git-all` everyday, so far it works well for me.

## License

MIT - see [LICENSE](LICENSE)
