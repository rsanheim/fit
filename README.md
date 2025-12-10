# nit

`nit` is a CLI meant to explore different ways to handle parallel git tasks across many 
repositorieis. We will be implementing `nit` in many different languages to compare approaches. 

For now, `nit` should be a raw wrapper around `git` that, in its most basic form, 
simply runs git for all "sub-repos" with all args passed through.

However, for some commands, we want to do better: we want to condense output and ensure performance is as fast as possible. For example, for the following:
```
[~/src/oss] nit pull
```

This should recurse into every directory of ~/src/oss (depth of 1) that is a git repo, and run some form of `git pull` in each repo, in parallel, using up to `--workers|-n` as the number of workers to use.

This should be done as performantly as possible, which _probably_ means leveraging `git`'s "-C" flag to switch dits, or perhaps '--git-dir' ? We will want benchmarks for this.

Regarding output: `nit` should intelligently condense output from git for specific commands to ensure there is **one line** of output for each repo by default. Ideally, we can do this via `git` options to the underlying `git` subcommand.

The 'one line' commands for now are: 

- `git pull`
- `git fetch`
- `git status`

For all of these, we should research the best way to use modern git features to get some sensible, helpful single line output. If there is not a core git way to do this, we should implement it via our own code (i.e. a callback, a hook, a function, a promise).

If someone runs `nit pull` or `nit fetch` with any any additional flags, options, or args that are NOT used by `nit` itself, they should passed through to the underlying `git` command verbatim.

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
   --workers int, -n int           Number of parallel workers (default: auto-detect CPUs) (default: 0)
   --dry-run                       Print the **exact** command for every repo without running it

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

* support `nit pull` with fast parallel mode and singline line output
* all other git commands should be supported via pass-through mode
* support dry-run mode with accurate output
* basic CLI parsing / help / etc

## Project structure and implementation notes

Language implementations should go into a corresponding subdir, i.e. ./rust, ./ruby, etc.
If there are multiple implementations per language, they can just use a more speicifc name, ie `./ruby-fibers`
**ALL binairies/scripts for the end-result should be runnable via a wrapper bash script in ./bin/ - 
so `./bin/nit-ruby` should run the ruby implementation, etc.

Any scripts for benchmarking or housekeeping should into ./scripts. We can assume hyperfine is installed, and should use it!

We use `mise` for installing tools and dependencies.

