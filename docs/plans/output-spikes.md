# Plan: Output Strategy Spikes

## Status

Draft.

## Summary

Tracing is now implemented and cheap enough to use during experiments. The next step is not more instrumentation; it is a small number of focused spikes that test different ways to remove or reduce head-of-line blocking without making the codebase hard to compare.

This document identifies the three highest-value spikes to try next and recommends how to branch them.

## What The Trace Already Tells Us

Representative `status` runs on `~/work` with 98 repos and `-n 8` show:

- the first repo can finish hundreds of milliseconds before the first line is printed
- most repos are delayed by ordered printing
- the worst ordered wait can exceed one second
- trace-to-file overhead is within benchmark noise

That makes the next experiments clear:

- change output behavior first
- keep the current trace format so comparisons stay consistent
- isolate internal concurrency changes from output-policy changes

## Spike Selection Criteria

The next spikes should:

1. attack head-of-line blocking directly
2. keep the code small enough that the result is easy to reason about
3. produce benchmarkable outcomes using the current trace
4. improve UX for real terminal use, not just synthetic benchmark results

## Recommended Spikes

### Spike 1: Completion-Order Live Output With Stable Repo IDs

#### Goal

Remove head-of-line blocking in the simplest non-TTY-friendly way.

#### Behavior

- repos are still discovered and sorted deterministically
- each repo gets a stable numeric index
- final lines print in completion order
- optional final sorted summary may be enabled for reviewability

Example:

```text
[002 agentic-dev] clean
[003 amion-api] clean
[001 activities] clean
```

#### Why This Is High Value

- smallest change that directly fixes delayed output
- works in terminals, pipes, and captured logs
- easy to compare against the current runner using `ordered_wait_ms`
- likely the fastest way to validate whether head-of-line blocking is the main UX problem

#### What To Measure

- `first_print_ms`
- `delayed_repos`
- `max_ordered_wait_ms`
- total wall time

#### Expected Result

- `delayed_repos` should collapse toward zero
- `first_print_ms` should move much closer to `first_exit_ms`
- total runtime should stay roughly unchanged

### Spike 2: Reserved TTY Rows With In-Place Updates

#### Goal

Test the best interactive UX while preserving sorted display order.

#### Behavior

- only enabled when stdout is a TTY
- print one placeholder row per repo in sorted order
- update each row in place when its repo finishes
- fallback to a simpler non-TTY strategy when output is redirected

Example:

```text
[activities] running...
[agentic-dev] running...
[amion-api] running...
```

Later:

```text
[activities] clean
[agentic-dev] clean
[amion-api] clean
```

#### Why This Is High Value

- best balance of sorted display and responsiveness
- directly tests whether preserving visual order is worth the extra rendering complexity
- likely the most user-friendly end state for interactive runs

#### What To Measure

- same trace metrics as Spike 1
- render correctness under TTY vs redirected output
- qualitative complexity cost in the printer code

#### Expected Result

- perceived UX should be best
- trace metrics should show the same low HOL delay as Spike 1
- implementation complexity will be materially higher than Spike 1

### Spike 3: Bounded Worker Queue With Current Trace And Printer API

#### Goal

Reduce scheduler overhead and simplify internals without changing git semantics.

#### Behavior

- replace thread-per-repo plus semaphore with a fixed worker queue
- keep current command formatting
- keep the trace format unchanged
- keep printer behavior constant within the spike so internal performance can be measured cleanly

#### Why This Is High Value

- isolates internal orchestration cost from output-policy changes
- makes later output strategies easier to implement on top of a cleaner execution model
- directly targets the current thread-per-repo design, which is heavier than needed

#### What To Measure

- wall time across worker counts
- system CPU time
- any change in `start_ms` to `spawn_ms` spread
- code complexity relative to the current runner

#### Expected Result

- modest performance improvement or reduced variance
- cleaner base architecture for later output work
- less direct UX benefit than Spikes 1 and 2

## Priority Order

Recommended order:

1. Spike 1: completion-order live output
2. Spike 3: bounded worker queue
3. Spike 2: reserved TTY rows

Reasoning:

- Spike 1 is the cheapest way to validate the main UX hypothesis.
- Spike 3 gives a cleaner execution model and may help performance before doing terminal-specific work.
- Spike 2 is likely the best final UX, but it is the highest implementation-complexity spike and should be informed by the results of the simpler experiments first.

## Branch Strategy

Separate branches do make sense for the first round of spikes.

Recommended approach:

- branch each spike from the same clean baseline commit
- keep the current trace format unchanged across spikes
- use `script/bench git` and trace files to compare branches

Why separate branches are the right default:

- the spikes touch the same core files, especially `rust/src/runner.rs`
- isolated branches make causality clearer
- trace comparisons are easier to interpret when each branch changes one main idea
- reverting a losing spike is trivial

Why comparison is not actually much harder:

- the repo already has `script/bench git` for branch/ref comparisons
- `GIT_ALL_TRACE_FILE` gives a per-run artifact you can diff or summarize later
- the harder part is attributing performance changes correctly, and isolated branches help with that

## Branch Recommendations

Suggested spike branches:

- `spike/completion-order-output`
- `spike/bounded-worker-queue`
- `spike/tty-row-updates`

One caveat:

- if Spike 2 ends up needing the same event/printer plumbing introduced by Spike 1, it is reasonable to fork `spike/tty-row-updates` from the validated Spike 1 branch instead of the original baseline

That is the one place where a stacked branch can be justified, because the diff stays smaller and the comparison remains meaningful:

- baseline vs Spike 1
- Spike 1 vs Spike 2

## Success Criteria

A spike is worth carrying forward if it improves at least one of these materially without regressing the others badly:

- lower `first_print_ms`
- lower `delayed_repos`
- lower `max_ordered_wait_ms`
- better interactive readability
- equal or better total runtime
- simpler long-term implementation path

## Recommended Next Step

Start with Spike 1 on its own branch and collect:

- `script/bench git` comparison against the current runner
- one trace file from `~/work`
- one `pull` run on a realistic remote-heavy directory

That should tell us quickly whether output policy alone solves most of the UX problem.
