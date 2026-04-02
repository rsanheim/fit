# Output Spike Tracker

This document is the canonical place to compare the output-policy spikes. Use it to track branch origins, record trace summaries, and keep the performance and UX comparison commands in one place.

## Why A Doc, Not A PR

Use this doc as the shared tracker until one spike wins.

- A PR is a poor fit while the experiments live on separate branches with different base commits.
- `script/bench git` and `GIT_ALL_TRACE_FILE` already compare refs directly without needing one umbrella branch.
- Once the winner is clear, open one PR from the winning branch and link back to this tracker for the comparison history.

## Spike Inventory

| Spike | Branch | Base Ref | Output Policy | Status | first_print_ms | delayed_repos | max_ordered_wait_ms | total_ms | Notes |
| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| Baseline | `0c6137c` | `0c6137c` | ordered append-only | measured | `499` | `97` | `2967` | `3094` | `~/work`, `status`, `-n 8`, trace file run on 2026-04-02 |
| Spike 1 | `spike/completion-order-output` | `0c6137c` | completion-order live lines with stable repo IDs | measured | `139` | `28` | `20` | `2639` | commit `f035766`, same `~/work` trace setup as baseline |
| Spike 3 | `spike/bounded-worker-queue` | `0c6137c` | ordered append-only | planned |  |  |  |  | isolate scheduler overhead from output-policy changes |
| Spike 2 | `spike/tty-row-updates` | `f035766` | reserved sorted TTY rows, completion-order non-TTY fallback | planned |  |  |  |  | stack on validated Spike 1 non-TTY behavior |

## Plan Links

- [Spike 3 Implementation Plan](./spike-bounded-worker-queue-implementation.md)
- [Spike 2 Implementation Plan](./spike-tty-row-updates-implementation.md)
- [Original Spike Selection Notes](./output-spikes.md)
- [Ordered Output Latency Background](./ordered-output-latency.md)

## Performance Comparison Workflow

### 1. Build The Branch You Want To Trace

Run from the repo root:

```bash
script/build -t rust
```

### 2. Capture A Trace Artifact On `~/work`

Run from `~/work`:

```bash
GIT_ALL_TRACE_FILE=/tmp/git-all-current.trace /Users/rsanheim/src/rsanheim/git-all/bin/git-all-rust -n 8 status > /tmp/git-all-current.stdout
rg 'phase=summary' /tmp/git-all-current.trace
```

Record these fields in the tracker row:

- `first_print_ms`
- `delayed_repos`
- `max_ordered_wait_ms`
- `total_ms`

### 3. Compare Branches With `script/bench git`

Compare any branch to the baseline:

```bash
script/bench git -I rust -b 0c6137c -t spike/completion-order-output -d ~/work -c status -n 8
script/bench git -I rust -b 0c6137c -t spike/bounded-worker-queue -d ~/work -c status -n 8
script/bench git -I rust -b f035766 -t spike/tty-row-updates -d ~/work -c status -n 8
```

What to record:

- mean runtime difference from `hyperfine`
- variance notes if one branch is noisy
- whether the branch improved runtime without regressing the trace fields

## UX Comparison Workflow

### Non-TTY Output

Use redirected output to verify what pipes and logs will see:

```bash
cd ~/work
/Users/rsanheim/src/rsanheim/git-all/bin/git-all-rust -n 8 status > /tmp/git-all-ux.stdout
sed -n '1,10p' /tmp/git-all-ux.stdout
```

Check:

- Are lines readable in captured logs?
- Does the branch preserve stable repo identification?
- Did ANSI escape sequences leak into redirected output?

### Interactive TTY Output

Run from a real terminal so terminal-only behavior is exercised:

```bash
cd ~/work
/Users/rsanheim/src/rsanheim/git-all/bin/git-all-rust -n 8 status
```

For the TTY-row spike, also capture a session:

```bash
script -q /tmp/git-all-tty.typescript bash -lc '/Users/rsanheim/src/rsanheim/git-all/bin/git-all-rust -n 8 status'
sed -n '1,80p' /tmp/git-all-tty.typescript | cat -v
```

Score each branch with a short note for:

- Time to first visible feedback
- Readability while the run is still in progress
- Ease of spotting slow repos
- Suitability for redirected logs

## Result Template

Copy this block when recording a new spike result:

```markdown
### <Spike Name>

- Branch: `<branch>`
- Base ref: `<ref>`
- Trace summary: `first_print_ms=<...> delayed_repos=<...> max_ordered_wait_ms=<...> total_ms=<...>`
- `script/bench git`: `<paste hyperfine summary>`
- TTY UX notes: `<one or two sentences>`
- Non-TTY UX notes: `<one or two sentences>`
```
