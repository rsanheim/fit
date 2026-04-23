# Spike 4: Crossterm Smart TTY Implementation Plan

## Overview

Implement crossterm-based smart TTY handling for git-all, branched from baseline `0c6137c`. Three tasks, each producing a testable, buildable commit.

## Task 1: Add crossterm dependency and create printer module

### What

* Add `crossterm` to `Cargo.toml` dependencies
* Create `rust/src/printer.rs` with:
  * `RepoRow` struct (idx, name, id_width, name_width) with `label()` method
  * `StreamPrinter` for non-TTY output (write one line per completion)
  * `TtyPrinter` for TTY output with sticky footer
    * `print_result()` — clear footer, print completed line, reprint updated footer
    * `finish()` — clear footer line when all repos are done
* Register module in `main.rs`
* Write unit tests using `Vec<u8>` as writer:
  * `stream_printer_emits_plain_lines()`
  * `tty_printer_includes_progress_footer()`
  * `tty_printer_clears_footer_on_finish()`

### Commit message

`spike: add crossterm printer module with TTY footer`

## Task 2: Integrate printer into runner with TTY detection

### What

* In `runner.rs` `run_parallel()`:
  * Detect TTY via `crossterm::tty::IsTty` on stdout
  * If TTY: create `TtyPrinter`, use it for all output in the receive loop
  * If not TTY: create `StreamPrinter`, use it for all output
  * Extract `format_status()` as pure function from current `print_result()`
  * Preserve all trace metric collection
* Add integration-style test:
  * `non_tty_output_has_no_escape_codes()` — capture output to Vec, assert no `\x1b`

### Commit message

`spike: integrate crossterm printer into runner with TTY detection`

## Task 3: Build, manual verification, and record results

### What

* `script/build -t rust` release build
* Capture trace on `~/work`:
  ```
  GIT_ALL_TRACE_FILE=/tmp/git-all-spike4.trace \
    ~/.dx-worktrees/rsanheim/git-all/spike-crossterm-smart-tty/bin/git-all-rust -n 8 status \
    > /tmp/spike4.stdout
  ```
* Record trace metrics in `docs/plans/output-spike-tracker.md`
* Run `script/bench git` against baseline
* Verify non-TTY output (redirected) has no ANSI escapes
* Capture TTY session with `script` command for review
