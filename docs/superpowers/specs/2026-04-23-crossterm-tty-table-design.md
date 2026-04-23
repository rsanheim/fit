# Crossterm TTY Table Design

## Summary

Add a dedicated printer boundary to the Rust implementation so interactive terminal output can use a live crossterm-driven table without changing the current non-TTY output behavior.

This design keeps execution concerns in the runner, rendering concerns in printers, and leaves room for future output modes such as JSON without reshaping the execution pipeline again.

## Current State

The Rust runner already:

- discovers and sorts repositories in alpha order
- runs git commands in parallel
- prints non-TTY output in deterministic append-only order
- captures focused runner timing trace data for scan, per-repo timing, and summary metrics

The current problem is interactive UX. Ordered append-only output creates head-of-line blocking, so a slow early repo delays visible feedback for later repos.

## Goals

- Keep non-TTY output exactly as it behaves today in this iteration.
- Add a TTY-only live presentation using `crossterm`.
- Preserve the alpha-ordered mental model for interactive use.
- Keep the first version plain text only.
- Keep the design simple enough to live with and refine from real use.
- Separate execution from rendering so future output modes can be added cleanly.

## Non-Goals

- Adding JSON output in this iteration.
- Changing the meaning or wording of the current formatter output.
- Adding color, alternate screen handling, or live resize support in the first pass.
- Redesigning `pull`, `fetch`, or `status` formatter semantics.

## Chosen Direction

Use a dedicated printer boundary.

- `runner.rs` remains responsible for execution, worker limits, result collection, and trace emission.
- A new printer module owns presentation.
- Non-TTY output continues through a plain printer with the current behavior.
- TTY output uses a crossterm table printer that maintains an in-memory alpha-ordered row model and redraws only the visible window plus footer.

This is intentionally not a full TUI architecture. It is a narrow live-table renderer layered on top of the existing runner.

## User-Facing TTY Behavior

### Table Shape

The TTY presentation is a minimal live table with:

- no color
- no persistent header row
- no brackets or decorative wrappers
- one stable repo lane
- one stable output lane

Conceptually the table is still:

- `REPO`
- `OUTPUT`

but the first version does not need to visibly render column headers.

### Initial Row State

Each repo begins with:

- repo name in alpha order
- output cell set to `running`

When a repo finishes, the output cell is replaced with the current formatter output text. Errors remain inline in the output cell exactly as they do today.

### Viewport Policy

If the repo list is taller than the terminal:

- snapshot terminal height once at startup
- keep completed rows in place
- follow the first unfinished repo in alpha order

This keeps the alpha-ordered map intact while avoiding a viewport pinned to already-finished rows at the top.

### Footer

The footer remains visible during the run and includes:

- visible slice in the form `showing 24-47 of 98`
- completed count
- running count
- elapsed time

Example:

```text
showing 24-47 of 98 | 41 complete | 8 running | 2.1s
```

### End State

When the run completes:

- the final live table remains on screen
- the footer can remain in its final completed form
- there is no conversion to a separate plain-text summary

## Printer Boundary

Create a new printer module at `rust/src/printer.rs` with two printer implementations:

- `PlainPrinter`
- `TtyTablePrinter`

The runner selects one at startup based on whether stdout is a TTY.

### Shared Row Model

Introduce a small shared row model that is neutral with respect to rendering. It should contain only what the printer needs to render current state, such as:

- stable repo index
- repo display name
- current output text
- lifecycle state: running or finished

The row model should not contain crossterm-specific concepts.

### Printer Responsibilities

`PlainPrinter`:

- preserves current non-TTY output behavior
- emits plain text lines in deterministic ordered form
- remains the source of truth for redirected logs and pipes in this iteration

`TtyTablePrinter`:

- owns terminal size snapshot
- owns row cache for all repos
- computes current visible window
- renders visible rows and footer with crossterm cursor movement
- updates rows in place as runner events arrive

## Runner Responsibilities

The runner should stay execution-focused.

It should:

- discover and sort repos
- spawn and wait on git processes
- keep worker limit behavior unchanged
- keep trace timing behavior unchanged
- emit row updates to the selected printer

It should not:

- make TTY layout decisions
- own viewport policy details
- construct crossterm cursor commands directly

## Data Flow

1. Discover and alpha-sort repositories.
2. Build initial row state for each repo with output `running`.
3. Select printer:
   - TTY -> `TtyTablePrinter`
   - non-TTY -> `PlainPrinter`
4. Initialize printer with the full ordered row set.
5. As each repo finishes, update that repo's row state and notify the printer.
6. TTY printer redraws the visible window and footer.
7. Non-TTY printer preserves current ordered append-only behavior.
8. Trace emission remains tied to runner timing and printed order, not to crossterm internals.

## Error Handling

- Repo-level command failures remain inline in the output cell.
- If crossterm setup or rendering fails, return an error rather than silently degrading terminal state.
- The first version does not attempt to recover mid-run from terminal capability problems.
- Non-TTY mode remains free of ANSI escape sequences.

## Testing Strategy

### Unit Tests

Add focused unit tests for printer behavior, especially:

- visible window calculation
- following the first unfinished repo
- footer formatting
- row rendering without decorative wrappers

### Integration Tests

Keep and extend integration coverage for:

- non-TTY output remaining unchanged
- trace metrics remaining available
- no ANSI escapes leaking into redirected output

### Manual TTY Checks

Run manual smoke checks for:

- small repo sets
- large repo sets
- inline error rows
- final table persistence after completion

## Benchmark Validation

Validation must cover both rendering modes separately.

### Non-TTY Benchmarks

Use the existing `script/bench git` harness to compare:

- `main` vs crossterm branch
- `git-all status`
- `git-all pull`
- `~/src/work`
- `~/src/oss`

Each configuration should use warmup plus multiple measured runs.

### TTY Benchmarks

Add a pseudo-terminal mode to `script/bench` so the benchmark actually exercises the crossterm path. On macOS this should use `/usr/bin/script`.

TTY benchmarks should compare:

- `main` vs crossterm branch
- `git-all status`
- `git-all pull`
- `~/src/work`
- `~/src/oss`

Each configuration should also use warmup plus multiple measured runs.

### Benchmark Notes

- `pull` benchmarking should use real GitHub access.
- Warmup is desirable because it reduces run-to-run variability from repos changing state during the benchmark sequence.
- Trace runs should remain separate from hyperfine timing runs and be used as diagnostics, not as the timing harness itself.

## Why Not A Full TUI

A fuller TUI stack would add more control but also more code and a heavier interaction model than this tool currently needs.

The simpler live-table approach is preferred because it:

- preserves the current CLI feel
- keeps scrollback and terminal behavior straightforward
- isolates rendering without committing the tool to a broader TUI architecture

## Future Extension Points

This boundary should make later additions straightforward:

- JSON printer for machine-friendly output
- visible column headers
- richer status columns
- color and emphasis
- resize handling
- alternate viewport policies

Those are intentionally postponed until the basic crossterm table exists and has been used enough to judge the UX.
