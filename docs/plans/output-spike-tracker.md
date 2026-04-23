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
| Spike 3 | `spike/bounded-worker-queue` | `0c6137c` | ordered append-only | **ruled out** | `666` | `92` | `1239` | `2802` | regressed vs baseline; worktree removed 2026-04-02; local branch retained for reference |
| Spike 2 | `spike/tty-row-updates` | `f035766` | reserved sorted TTY rows, completion-order non-TTY fallback | measured | `165` | `14` | `2` | `2525` | commit `91b213a`; `script/bench git` showed `f035766 1.02 ± 0.03x` faster; non-TTY stdout stayed plain text without ANSI escapes |
| Spike 4 | `spike-crossterm-smart-tty` | `0c6137c` | completion-order with crossterm sticky footer (TTY), plain lines (non-TTY) | **promising** | `100` | `29` | `12` | `3226` | commit `4a3f0a2`; baseline `1.01 ± 0.03x` faster (within noise); no index prefix; output variations planned |

## Plan Links

- [Spike 4 Design](./spike-crossterm-smart-tty-design.md)
- [Spike 4 Implementation Plan](./spike-crossterm-smart-tty-implementation.md)
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
script/bench git -I rust -b 0c6137c -t spike-crossterm-smart-tty -d ~/work -c status -n 8
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

## Recorded Results

### Spike 2

- Branch: `spike/tty-row-updates`
- Base ref: `f035766`
- Trace summary: `first_print_ms=165 delayed_repos=14 max_ordered_wait_ms=2 total_ms=2525`
- `script/bench git`: `f035766-f035766 ran 1.02 ± 0.03 times faster than spike/tty-row-updates-91b213a`
- TTY UX notes: From the `script` capture, visible feedback begins immediately with one sorted `running...` row per repo, and completions rewrite those fixed rows in place. That makes slow repos easier to track than Spike 1, but the initial full-screen placeholder burst is dense on a 98-repo workspace.
- Non-TTY UX notes: Redirected stdout stayed in completion order with stable IDs and no ANSI escapes. The first ten redirected lines were plain status lines such as `[002 agentic-dev                  ] clean`.

### Spike 4

- Branch: `spike-crossterm-smart-tty`
- Base ref: `0c6137c`
- Latest commit: `4a3f0a2`
- Trace summary: `first_print_ms=100 delayed_repos=29 max_ordered_wait_ms=12 total_ms=3226`
- `script/bench git`: `0c6137c-0c6137c ran 1.01 ± 0.03 times faster than spike-crossterm-smart-tty-53c5b4f`
- TTY UX notes: No placeholder burst. Completed repos scroll naturally with a sticky progress footer (`[45/98 complete | 8 running | 2.1s]`) that updates in place via crossterm. Clean scrollback history. Footer clears on completion. Numeric index prefix removed in `4a3f0a2` — output is now `[repo-name] status` for cleaner reading.
- Non-TTY UX notes: Redirected stdout is plain completion-order lines with repo names and no ANSI escapes.
- Next steps: try output format variations (different label styles, summary options)
