# Spike 4: Crossterm Smart TTY Handling

## Status

Active spike, branched from baseline `0c6137c`.

## Goal

Replace the raw ANSI escape approach (Spike 2) with crossterm-powered terminal handling that is terminal-size-aware and avoids the "placeholder burst" problem on large workspaces.

## Problem

Spike 2 reserves one row per repo and writes "running..." placeholders for all of them at startup. On a 98-repo workspace this floods the terminal with placeholder text before any useful output appears. The hand-rolled ANSI cursor moves also produce garbled scrollback history.

## Design

### TTY Mode (crossterm-powered)

* Detect whether stdout is a TTY using `crossterm::tty::IsTty`
* Get terminal dimensions via `crossterm::terminal::size()`
* Print completed repos in **completion order** as they finish (natural scrolling, like Spike 1)
* Maintain a **sticky progress footer** at the bottom of visible output:
  ```
  [45/98 complete | 8 running | 2.1s]
  ```
* Footer is rewritten in-place using crossterm cursor movement (move up 1, clear line, rewrite)
* When all repos complete, clear the footer line so scrollback is clean
* Each completed line is a permanent addition to scrollback, identical in format to Spike 1 output

### Non-TTY Mode

* Plain completion-order output with stable repo IDs
* No ANSI escapes
* Identical behavior to Spike 1's non-TTY output

### Footer Update Strategy

* Before printing a completed repo line: move cursor to footer position, clear it
* Print the completed repo line normally (scrolls up)
* Reprint the updated footer on the new last line
* This gives the effect of a growing log with a fixed status bar at the bottom

### Crossterm Usage

Specific crossterm features used:

* `crossterm::tty::IsTty` for TTY detection
* `crossterm::terminal::size()` for terminal dimensions (future: viewport-aware batching)
* `crossterm::cursor::MoveUp`, `crossterm::cursor::MoveToColumn` for footer positioning
* `crossterm::terminal::Clear(ClearType::CurrentLine)` for footer rewrites
* `crossterm::style` if color is added later (not in initial spike)

### What This Does NOT Do

* No alternate screen buffer (scrollback stays clean and useful)
* No raw mode (standard line-buffered I/O, Ctrl-C works normally)
* No per-repo row reservation (the key difference from Spike 2)
* No color (can be added later, but not part of this spike)

## Expected Trace Metrics

* `first_print_ms` should be close to Spike 1 (~139ms) since repos print on completion
* `delayed_repos` should be near zero (completion order)
* `max_ordered_wait_ms` should be near zero
* `total_ms` should be comparable to baseline

## Comparison With Other Spikes

| Property | Baseline | Spike 1 | Spike 2 | Spike 4 (this) |
|---|---|---|---|---|
| Output order | sorted | completion | sorted (TTY) / completion (non-TTY) | completion + footer (TTY) / completion (non-TTY) |
| TTY awareness | none | none | row reservation | terminal-size-aware footer |
| Placeholder burst | n/a | n/a | yes (98 rows) | no |
| Scrollback quality | clean | clean | garbled | clean |
| Progress indicator | none | none | implicit (see pending rows) | explicit footer |
| External deps | none | none | none | crossterm |

## Success Criteria

* `first_print_ms` comparable to Spike 1
* Clean scrollback history (no ANSI artifacts)
* Footer provides real-time progress without overwhelming the terminal
* Non-TTY output identical to Spike 1
* No performance regression vs baseline
