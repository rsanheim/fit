# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`nit` is a CLI for running parallel git operations across many repositories. The project explores different implementation approaches by building the same tool in multiple languages (Rust, Ruby, etc.).

## Core Behavior

* **Basic mode**: wraps `git` and runs it for all sub-repos (depth 1) with args passed through
* **Optimized commands** (`pull`, `fetch`, `status`): run in parallel with condensed single-line output per repo
* **Pass-through**: any unrecognized command/args go directly to `git`

## Project Structure

```
./rust/          # Rust implementation
./ruby/          # Ruby implementation
./bin/           # Wrapper scripts (nit-rust, nit-ruby, etc.)
./script/       # Benchmarking and housekeeping scripts
```

All binaries should be runnable via `./bin/nit-<language>`.

## Development Tools

* `mise` for tool/dependency management
* `hyperfine` for benchmarking

## Key Implementation Details

### Dry-run mode
Dry-run output must be generated as close to actual execution as possible (same code path that builds the real command). Never construct dry-run output separately from real execution logic.

### Git execution
Use `git -C <repo-path>` for directory switching. Research and benchmark against `--git-dir` for performance.

### Parallel execution
Spawn all git processes immediately (non-blocking), then collect results. This maximizes parallelism by letting the OS handle process scheduling rather than limiting to a thread pool.

## Global CLI Options

```
--dry-run                Print exact commands without executing
```
