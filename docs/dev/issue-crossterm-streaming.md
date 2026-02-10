# Issue: Add crossterm for in-place updates; restore streaming output

## Summary
The current Rust runner buffers all command outputs until *every* repo completes, which removes streaming output and can feel like a hang on long-running repos. This issue proposes adding `crossterm` to support lightweight cursor movement and per-line updates, enabling real-time output while preserving alphabetical ordering. It also outlines a simpler interim fix (ordered streaming without cursor movement) and the tradeoffs.

## Background
Recent changes in `rust/src/runner.rs` switched to a thread-per-process model and collect all results into a `Vec`, then sort and print. This avoids pipe deadlocks but has an important UX regression:

- **No streaming output**: output only appears after all repos finish.
- **Perceived hang**: a slow repo blocks visibility of all other results.

The desired behavior is **streaming + alphabetical order**. Without a terminal library, the only option is “print in order when ready,” which still blocks on the first slow repo. A minimal terminal library enables in-place updates so results can appear as soon as they’re ready while still *displaying* in sorted order.

## Current behavior (problem)
- `run_with_semaphore` and `run_unlimited` spawn threads, `join` all, collect results, then print.
- Even if repos are sorted before spawning, results are not emitted until all are complete.

## Goals
- Stream output as repos finish.
- Preserve alphabetical ordering of displayed results.
- Keep implementation lightweight and cross-platform.

## Proposed solution
Add `crossterm` and implement a minimal output renderer:

1. **Sort repos alphabetically** (same as current output ordering).
2. **Print placeholder lines** for each repo:
   - Example: `[repo-name] …` or `[repo-name] (running)`
3. As each repo completes, **move the cursor** to that repo’s line and replace it with final output.
4. Keep normal output formatting for successful/failed commands.

This allows true streaming without losing alphabetical display order.

### Why crossterm
- Cross-platform (macOS/Linux/Windows).
- Small, focused API for cursor movement/line clearing.
- Avoids full TUI stack (e.g., `ratatui`).

## Alternate/Interim solution (no terminal lib)
Implement ordered streaming without cursor updates:

- Sort repos alphabetically.
- As each repo completes, store result.
- Print results in order as soon as all prior repos are complete.

This restores some streaming but still suffers head-of-line blocking when early repos are slow.

## Implementation notes
- Introduce a small output module (e.g., `rust/src/output.rs`) with two strategies:
  - `PlainPrinter`: current stdout printing.
  - `CursorPrinter`: crossterm-based line updates.
- The runner should return results as they complete (via channels), not after all joins.
- Maintain a `Vec<Option<Result>>` indexed by repo order.
- The cursor renderer can update any finished line immediately; the plain renderer can emit in order as soon as possible.

## Dependencies
- Add `crossterm` to `rust/Cargo.toml`.
- Ensure compilation on macOS/Linux/Windows.

## Acceptance criteria
- Output begins streaming immediately after the first repo completes.
- Alphabetical display order is preserved.
- No buffering of all output before printing.
- Works on macOS/Linux; Windows builds succeed.

## Notes / Risks
- Terminal cursor control may behave differently in non-TTY environments; fallback to plain printing when stdout is not a TTY.
- Need to ensure `stderr` is also coordinated (e.g., print errors in-place or fallback to plain mode).

## References
- `rust/src/runner.rs`: current buffering and threading model.
- [`docs/SPEC.md §3.1 Git Invocation`](../SPEC.md#31-git-invocation): pipe deadlock handling requirement.
