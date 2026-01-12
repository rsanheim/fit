# Zig Implementation TODO

## ✅ Completed

* **Memory leak in workerThread** - Fixed proper cleanup of allocations
* **Static string formatters** - Changed to allocator-passing style returning owned memory
* **Dynamic count formatting** - status/fetch now show actual counts ("21 untracked", "3 branches updated")

## Remaining (Medium Priority)

* Process termination check - use proper pattern matching for signal terminations
* Missing ERROR prefix in pull.zig error output
* Incomplete test suite - parseArgs test is a stub
* Exit code differentiation - currently always exits 0

## Low Priority

* Thread pool vs manual threading (Rust uses Rayon work-stealing)
* CLI argument string duplication

## Notes

Built for Zig 0.15.2. Key API patterns:
* ArrayList is "unmanaged" - pass allocator to each method
* I/O via `std.fs.File.stdout()`
* Formatters use `error{OutOfMemory}![]const u8` return type
