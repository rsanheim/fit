# Benchmark Results

Benchmarks run on macOS with ~90 git repositories, using `hyperfine` with minimum 3-5 runs per configuration.

## Environment

* macOS (Darwin 24.6.0)
* 20 CPU cores
* SSH multiplexing enabled (ControlMaster)
* Rust implementation (`rust`)

## git status

Local filesystem operation - bottleneck is process spawning and filesystem I/O.

| Workers | Time | Notes |
|---------|------|-------|
| 1 | 2.18s | |
| 2 | 1.27s | |
| **4** | **0.85s** | Optimal |
| 8 | 0.98s | Contention starts |
| 12 | 1.17s | |
| 16 | 1.80s | |
| 20 (auto) | 1.62s | |

**Finding**: 4 workers is optimal for local operations. More parallelism causes filesystem/process contention.

## git pull

Network-bound operation - bottleneck is SSH connection and remote server.

| Workers | Time | Notes |
|---------|------|-------|
| 1 | 26.5s | |
| 2 | 15.1s | |
| 4 | 8.4s | |
| **8** | **5.5s** | Optimal |

**Finding**: 8 workers is optimal for network operations. More parallelism hides network latency.

## git fetch

Similar to pull - network-bound.

| Workers | Time | Notes |
|---------|------|-------|
| 1 | 25.0s | |
| 4 | 8.2s | |
| **8** | **5.3s** | Optimal |

## SSH Multiplexing Impact

Without SSH multiplexing, `git pull` with 4 workers took ~25s. With multiplexing enabled, it dropped to ~8s - a **3x improvement**.

See README.md for SSH multiplexing configuration.

## Recommendation

Default worker count: **8**

This provides the best balance:
* Near-optimal for network operations (pull, fetch)
* Acceptable for local operations (status is ~1s vs 0.85s optimal)

## TTY Benchmarking

The crossterm renderer is only exercised when stdout is a TTY. Use `script/bench git --tty`
to wrap benchmarked commands in a pseudo-terminal:

```bash
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c status -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/work -c pull -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/src/oss -c status -n 8 --tty
script/bench git -I rust -b main -t crossterm-smart-tty -d ~/src/oss -c pull -n 8 --tty
```

Run the same matrix without `--tty` to verify non-TTY behavior remains stable.

## Crossterm TTY Table Validation

Validation run on April 23, 2026 with `script/bench git -I rust -b main -t HEAD -n 8`.
Benchmarks used `-w 1 -m 3` unless noted otherwise.

### Non-TTY

- `~/work status`: `main` 3.157s vs `current` 3.169s, effectively neutral.
- `~/work pull`: rerun with `-w 2 -m 5` due to an earlier noisy sample; `main` 7.272s vs `current` 7.478s, current about 3% slower.
- `~/src/oss status`: `main` 4.392s vs `current` 4.334s, current about 1% faster.
- `~/src/oss pull`: `main` 2.851s vs `current` 3.034s, current about 6% slower.

### TTY

- `~/work status`: `main` 3.459s vs `current` 3.453s, effectively neutral.
- `~/work pull`: `main` 7.366s vs `current` 7.650s, current about 4% slower.
- `~/src/oss status`: `main` 4.688s vs `current` 4.694s, effectively neutral.
- `~/src/oss pull`: `main` 3.446s vs `current` 3.190s, current about 7% faster, though the baseline side had high variance.

### Visual Checks

- `~/work status`: `tmux` live/final capture reviewed. Live pane showed `running` rows with no header row, and the final table remained on screen.
- `~/src/oss status`: `tmux` live/final capture reviewed. Live pane showed `running` rows with no header row, and the final table remained on screen.
- `~/work pull`: `tmux` live/final capture reviewed. Final table kept inline pull errors in the output column and remained on screen.
- `~/src/oss pull`: `tmux` live/final capture reviewed. Final table kept inline tracking-info errors in the output column and remained on screen.

### Trace Spot Check

- `status ~/work`: `repos=100 first_exit_ms=158 first_print_ms=515 delayed_repos=94 max_ordered_wait_ms=1534 total_ms=3447`
