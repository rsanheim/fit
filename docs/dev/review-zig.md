# Zig Implementation Review (evergreen)

This file tracks *current* Zig TODOs/risks and a small regression watchlist.

## P0 (Spec parity gaps)

- Missing `git-all meta` mode (SPEC 1.0). Zig checks passthrough mode before any “meta” detection and has no meta dispatcher (`zig/src/main.zig`).
- Empty repo discovery exit code: SPEC requires exit status 9 when no repos found; Zig prints the message but returns success (`zig/src/main.zig`).
- Error propagation: overall exit code does not reflect per-repo failures; command failures only affect per-repo output lines, not process exit status (`zig/src/runner.zig`).
- Passthrough mode should `exec` git (process replacement) for signal/exit transparency; Zig spawns and waits instead (`zig/src/main.zig`).

## P1 (Potential correctness/robustness)

- Output size limit: `std.process.Child.run` is configured with `max_output_bytes = 1024 * 1024`; large `git` output will fail with `StdoutStreamTooLong` / `StderrStreamTooLong` and show as an error line (`zig/src/runner.zig`).

## P1 (Performance / lag-behind causes)

- Parallelism model is a fixed worker pool that runs repos sequentially within each worker; this can lag behind the “spawn everything immediately, then collect” approach (especially when many repos are I/O bound) (`zig/src/runner.zig`).
- `GitCommand.execute()` captures full stdout/stderr into memory (up to 1 MiB each). This adds allocation and copy overhead even when formatters only need the first non-empty line (`zig/src/runner.zig`, `zig/src/commands/*.zig`).
- `std.heap.GeneralPurposeAllocator` is `std.heap.DebugAllocator` in Zig 0.15.x; using it in the CLI hot path can add noticeable overhead (`zig/src/main.zig`).

## P2 (Quality / maintainability)

- Tests are thin: `parseArgs` test is a stub, and there are no integration-style tests for behavior parity (passthrough, exit codes, meta, per-repo failure) (`zig/src/main.zig`, `zig/TODO.md`).

## Regression watchlist (keep fixed)

- `-n 0` (“0 = unlimited”) must not regress to a `num_workers == 0` path (division-by-zero). Current code uses `effectiveWorkerCount()` (`zig/src/runner.zig`).
- Do not reintroduce stdout/stderr pipe deadlock by reading one pipe to EOF before the other. Current code uses `std.process.Child.run` to collect both (`zig/src/runner.zig`).
