# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`git-all` is a CLI for running parallel git operations across many repositories. The project explores different implementation approaches by building the same tool in multiple languages (Rust, Zig, and Crystal).

## Core Behavior

* **Basic mode**: wraps `git` and runs it for all sub-repos (depth 1) with args passed through
* **Optimized commands** (`pull`, `fetch`, `status`): run in parallel with condensed single-line output per repo
* **Pass-through**: any unrecognized command/args go directly to `git`

## Project Structure

```
./rust/          # Rust implementation
./zig/           # Zig implementation
./crystal/       # Crystal implementation
./bin/           # Wrapper scripts (git-all-rust, git-all-zig, git-all-crystal)
./script/        # Build, test, and benchmarking scripts
```

All binaries should be runnable via `./bin/git-all-<language>`.

## Scripts

* `script/build` - Build implementations (optimized by default)
* `script/install` - Build and install to `~/.local/bin`
* `script/test` - Run tests
* `script/bench` - Run benchmarks with hyperfine

Run any script with `--help` for options.

## Development Workflow

### Quick Start
```bash
script/build -t rust     # build release
script/test -t rust      # run tests
./bin/git-all-rust status    # run locally
```

### Building
```bash
script/build -t rust         # release build (optimized, slower compile)
script/build -t rust --dev   # debug build (fast compile, slower runtime)
script/build                 # build all implementations (release)
```

### Running Locally
After building, run via wrapper scripts which point to the release binary:
```bash
./bin/git-all-rust status
./bin/git-all-zig status
./bin/git-all-crystal status
```

Or run debug builds directly for fast iteration:
```bash
./rust/target/debug/git-all status
```

### Testing
```bash
script/test -t rust          # run Rust tests
script/test                  # run all tests
cargo test                   # run directly in rust/
```

### Installing Globally
```bash
script/install -t rust       # installs release build as ~/.local/bin/git-all
```

## Development Tools

* `mise` for tool/dependency management
* `hyperfine` for benchmarking

## Key Implementation Details

### Use library features over custom code
Prefer using built-in library features (clap's help system, std library utilities, etc.) over writing custom implementations. Customize library behavior through their extension points rather than reimplementing. Less code = fewer bugs.

### Dry-run mode
Dry-run output must be generated as close to actual execution as possible (same code path that builds the real command). Never construct dry-run output separately from real execution logic.

### Git execution
Use `git -C <repo-path>` for directory switching. Research and benchmark against `--git-dir` for performance.

### Parallel execution
Spawn all git processes immediately (non-blocking), then collect results. This maximizes parallelism by letting the OS handle process scheduling rather than limiting to a thread pool.

## Global CLI Options

```
--dry-run          Print exact commands without executing
--ssh              Force SSH URLs for remotes
--https            Force HTTPS URLs for remotes
-n, --workers N    Number of parallel workers (default: 8)
```
