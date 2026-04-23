# Plan: Ordered Output Without Head-of-Line Blocking

## Status

Tracing implemented in Rust. Output experiments pending.

## Summary

The current Rust implementation preserves deterministic alphabetical output by buffering completed work until all earlier repos have finished. That makes output stable, but it also creates head-of-line blocking: fast repos can complete early and still remain invisible behind one slow repo.

This document describes the problem, the observable symptoms, and the main design options for keeping deterministic output without making the CLI feel stuck.

## Problem

The Rust runner currently:

1. Sorts repositories.
2. Runs git commands in parallel.
3. Stores completed results by repo index.
4. Prints results only when the next sorted repo is available.

This behavior is deterministic, but it delays visible output whenever an early repo is slow.

Example:

- `a` takes 2 seconds
- `b` takes 100 ms
- `c` takes 100 ms

If output must be appended strictly as:

1. `a`
2. `b`
3. `c`

then `b` and `c` are blocked behind `a` even though their work already finished.

## Why This Matters

For large directories such as `~/work`, the user experience degrades even when total wall-clock time is acceptable:

- output appears late
- the tool can look hung
- it is hard to tell whether git is slow or the runner is delaying printing
- slow repos dominate perceived performance

This is a UX problem first, and possibly a performance problem second.

## Constraint

There is no way to have all three of these properties at once in a plain append-only stream:

1. strict final line order
2. immediate line emission on completion
3. no placeholders or updates

If output is appended once and must remain sorted, later repos will always wait behind earlier repos.

To keep deterministic order without head-of-line blocking, the output model has to change.

## Goals

- Keep deterministic repo ordering where it adds value.
- Show useful progress as soon as work completes.
- Support both interactive terminal use and non-interactive pipe/log use.
- Make it easy to trace where time is going before changing network transport.

## Non-Goals

- Reordering repository discovery.
- Changing default git semantics.
- Switching transport from SSH to HTTPS as a first response.

## Options

### Option 1: Reserved TTY Slots With In-Place Updates

Print all repo rows in sorted order up front, then update each row in place as that repo finishes.

Example:

```text
[a] running...
[b] running...
[c] running...
```

Later:

```text
[a] already up to date
[b] 3 files changed
[c] clean
```

How it works:

- sort repos once
- assign each repo a fixed row
- printer owns terminal output
- workers send completion events over a channel
- printer moves the cursor to the assigned row and rewrites it

Pros:

- deterministic visual order
- no head-of-line blocking in the terminal
- best interactive UX

Cons:

- requires TTY-aware rendering
- needs ANSI/cursor control code or a small terminal library
- must fall back to a simpler mode for non-TTY output

### Option 2: Completion-Order Live Stream Plus Final Sorted Summary

Print live events as repos finish, then print a deterministic sorted summary at the end.

Example:

```text
[003 c] clean
[002 b] already up to date
[001 a] 3 files changed

Summary:
[001 a] 3 files changed
[002 b] already up to date
[003 c] clean
```

Pros:

- no blocking in live output
- deterministic final artifact
- works well in pipes and logs

Cons:

- the live stream itself is not sorted
- output is duplicated unless the summary is optional

### Option 3: Dual-Channel Output

Use one stream for live progress and one for deterministic final results.

Example:

- `stderr`: live completion events and progress
- `stdout`: final sorted summary only

Pros:

- preserves script-friendly deterministic `stdout`
- still gives humans live feedback

Cons:

- more complex mental model
- some users dislike split streams
- shell redirection behavior becomes more important

### Option 4: Stable Repo IDs With Out-of-Order Printing

Assign deterministic sorted IDs up front, then print results in completion order while keeping the IDs stable.

Example:

```text
[001 a] running
[002 b] running
[003 c] running
```

Live completions:

```text
[002 b] clean
[003 c] clean
[001 a] 3 files changed
```

Pros:

- deterministic identity for every repo
- easy to grep and compare runs
- simplest non-TTY live mode

Cons:

- visible order is still completion order
- not a true ordered display

### Option 5: Small Reorder Window or Timeout

Hold strict ordering only briefly. If an early repo stalls past a threshold, emit a placeholder for it and continue showing later results.

Example:

```text
[a] still running...
[b] clean
[c] clean
```

Later:

```text
[a] already up to date
```

Pros:

- reduces worst-case blocking
- can preserve mostly ordered output

Cons:

- more stateful and harder to reason about
- still requires placeholders or updates
- less predictable than the other models

## Tracing Implemented

The Rust implementation now has a small-scope timing trace path intended specifically for output and scheduling experiments.

Current interfaces:

- `GIT_ALL_TRACE=1`
  Writes trace lines to `stderr`
- `GIT_ALL_TRACE_FILE=/path/to/trace.log`
  Writes trace lines to a file and implicitly enables tracing

Current per-repo fields:

- `repo`
- `idx`
- `start_ms`
- `spawn_ms`
- `exit_ms`
- `printed_ms`
- `run_ms`
- `ordered_wait_ms`
- `stdout_bytes`
- `stderr_bytes`
- `success`

Current summary fields:

- `repos`
- `first_exit_ms`
- `first_print_ms`
- `delayed_repos`
- `max_ordered_wait_ms`
- `total_ms`

There is also a scan line with:

- `command`
- `root`
- `repos`
- `workers`
- `scan_ms`

The most important signal remains `ordered_wait_ms`. If that is large, the runner is causing visible delay even when git itself is not slow.

### Representative Result

On April 2, 2026, a representative traced `status` run on `~/work` with 98 repos and `-n 8` showed:

- `scan_ms=5`
- `first_exit_ms=121`
- `first_print_ms=534`
- `delayed_repos=93`
- `max_ordered_wait_ms=1418`
- `total_ms=2690`

That confirms the main UX problem is real head-of-line blocking in the runner, not just slow git subprocesses.

### Overhead Check

A quick `hyperfine` check on the same `~/work` workload showed trace-to-file overhead within noise:

- no trace: about `2.65s`
- `GIT_ALL_TRACE_FILE=/tmp/...`: about `2.64s`

That is good enough for spike comparisons and benchmark capture.

### Notes

- Trace initialization now happens after passthrough detection, so invoking `git-all` inside a repo does not create or truncate trace files.
- Trace writes now propagate I/O errors instead of silently ignoring them.
- Per-repo timing samples are only collected when tracing is enabled.

## Transport: SSH vs HTTPS

Switching from SSH to HTTPS should not be the first change.

Reasons:

- protocol choice does not fix head-of-line blocking
- the current rewrite only applies to GitHub-style remotes
- connection/auth overhead should be measured before changing transport

If tracing shows that `git pull` time is dominated by transport setup, test these in order:

1. SSH multiplexing
2. current SSH defaults vs forced HTTPS
3. per-host configuration changes outside `git-all`

## Recommendation

Recommended default behavior:

1. Keep the current tracing path and use it for every spike.
2. Use a single printer thread with event messages from workers.
3. For TTY output, use reserved sorted rows with in-place updates.
4. For non-TTY output, use completion-order live lines with stable repo IDs and an optional final sorted summary.
5. Keep a strict append-only ordered mode only as an explicit option, not the default.

This gives deterministic structure where it matters, preserves responsive output, and avoids optimizing transport before measuring where the time actually goes.

## Implementation Sketch

Runner changes:

- replace the current print-on-contiguous-ready logic with an event channel
- send `Started`, `Finished`, and `Trace` events from workers
- centralize rendering in one printer

TTY printer:

- pre-render sorted repo rows
- update rows in place on completion

Non-TTY printer:

- print completion-order events with stable repo index
- optionally emit final sorted summary

Trace support:

- already implemented with `GIT_ALL_TRACE` and `GIT_ALL_TRACE_FILE`
- use the current text format during spike work
- only add richer export formats if the current trace becomes a bottleneck

## Related Docs

- [`docs/dev/issue-crossterm-streaming.md`](../dev/issue-crossterm-streaming.md)
- [`docs/SPEC.md`](../SPEC.md)
- [`docs/plans/output-spikes.md`](./output-spikes.md)
