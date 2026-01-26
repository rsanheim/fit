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
