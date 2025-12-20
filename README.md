# nit

`nit` is a CLI for running parallel git operations across many repositories. We implement `nit` in multiple languages (Rust, Zig, and Crystal) to compare approaches.

See [SPEC.md](SPEC.md) for the formal specification.

## Operating Modes

**Passthrough Mode**: When inside a git repository, `nit` acts as a transparent wrapper around `git`. All arguments pass through unchanged - `nit status` becomes `git status`.

**Multi-Repository Mode**: When NOT inside a git repository, `nit` discovers sub-repos at depth 1 and runs commands across all of them in parallel.

### Multi-Repository Mode Details

In multi-repository mode, `nit` provides optimized commands with condensed single-line output:

* `nit pull` - Pull all repos with single-line status per repo
* `nit fetch` - Fetch all repos with single-line status per repo
* `nit status` - Status all repos with single-line status per repo

Any other command passes through to git verbatim for each repo. Additional flags are forwarded to git.

## GLOBAL OPTIONS

The following global options should be supported:

```
NAME:
   nit - parallel git across many repositories

USAGE:
   nit [global options] [command [command options]]

COMMANDS:
   pull             Pull all repositories
   fetch            Fetch all repositories
   status           Status all repositories
   [anything else]  Pass through to git
   help, h          Shows a list of commands or help for one command

GLOBAL OPTIONS:
   -n, --max-connections N  Maximum concurrent git processes (default: 8, 0 = unlimited)
   --dry-run                Print the **exact** command for every repo without running it
   --ssh                    Force SSH URLs (git@github.com:) for all remotes
   --https                  Force HTTPS URLs (https://github.com/) for all remotes

```

### Example dry-run output
NOTE: dry-run output should be constructed in ONE PLACE, as close to actual OS level execution as possible, such that the dry-run output is as close to the actual execution as possible.  This ensures we get an accurate view of what will actually happen.  Here is a simplified, basic exmaple of what I mean (in ruby-ish pseudo code):

#### Good example
```
class Nit
  attr_reader :dry_run, :repos

  def initialize(dry_run: false)
    @dry_run = dry_run
    @repos = find_git_repos
  end

  def pull(ARGV = [])
    repos.parallel_each do |repo|
      cmd = build_git_command(repo, ARGV)
      result = cmd.run(dry_run: dry_run)
      puts result # outputs the actual one-line output from git, or the 'dry-run' output if we are in dry-run mode
    end
  end
```

#### Bad - do NOT do this!
```
class Nit
  def initialize(dry_run: false)
    @dry_run = dry_run
    @repos = find_git_repos
  end

  def pull(ARGV = [])
    if @dry_run
      puts "would run 'git pull' for #{repos.join(', ')}"
    else
      repos.parallel_each do |repo|
        cmd = build_git_command(repo, ARGV)
        result = cmd.run
        puts result # outputs just the result of the git command
      end
    end
  end
```

Ideally, a `nit pull --dry-run` should output something like this:

```
[~/src/oss] nit pull --dry-run
[nit v0.1.0] Running in **dry-run mode**, no git commands will be executed. Planned git commands below.
git -C ~/src/oss/repo1 pull
git -C ~/src/oss/repo2 pull
git -C ~/src/oss/repo3 pull

# Show the exact 'pass thru' options for every repo
[~/src/oss] nit pull --dry-run --all
[nit v0.1.0] Running in **dry-run mode**, no git commands will be executed. Planned git commands below.
git -C ~/src/oss/repo1 pull --all
git -C ~/src/oss/repo2 pull --all
git -C ~/src/oss/repo3 pull --all
```

etc...

## MVP goal

* support `nit pull` with fast parallel mode and single line output
* all other git commands should be supported via pass-through mode
* support dry-run mode with accurate output
* basic CLI parsing / help / etc

## Project structure and implementation notes

Language implementations should go into a corresponding subdir, i.e. `./nit-rust`, `./nit-zig`, etc.
All binaries are runnable via wrapper scripts in `./bin/` (e.g., `./bin/nit-rust`).

### Scripts

* `script/build` - Build implementations (optimized release builds by default)
* `script/install` - Build and install to `~/.local/bin`
* `script/test` - Run tests for implementations
* `script/bench` - Run benchmarks with hyperfine

Run any script with `--help` for detailed options.

We use `mise` for installing tools and dependencies.

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

This reduces `nit pull` time by ~3x by avoiding repeated SSH handshakes.

See [docs/benchmarks.md](docs/benchmarks.md) for detailed benchmark results.

