const std = @import("std");
const runner = @import("../runner.zig");

/// Format passthrough output - just show first non-empty line or "ok"
/// Returns owned memory that the caller must free
pub fn format(allocator: std.mem.Allocator, stdout: []const u8, stderr: []const u8, success: bool) error{OutOfMemory}![]const u8 {
    if (!success) {
        var lines = std.mem.splitScalar(u8, stderr, '\n');
        while (lines.next()) |line| {
            const trimmed = std.mem.trim(u8, line, " \t\r");
            if (trimmed.len > 0) {
                return try allocator.dupe(u8, trimmed);
            }
        }
        return try allocator.dupe(u8, "unknown error");
    }

    // First non-empty line from stdout
    var lines = std.mem.splitScalar(u8, stdout, '\n');
    while (lines.next()) |line| {
        const trimmed = std.mem.trim(u8, line, " \t\r");
        if (trimmed.len > 0) {
            return try allocator.dupe(u8, trimmed);
        }
    }

    // Try stderr
    lines = std.mem.splitScalar(u8, stderr, '\n');
    while (lines.next()) |line| {
        const trimmed = std.mem.trim(u8, line, " \t\r");
        if (trimmed.len > 0) {
            return try allocator.dupe(u8, trimmed);
        }
    }

    return try allocator.dupe(u8, "ok");
}

/// Run passthrough command across all repos
pub fn run(
    allocator: std.mem.Allocator,
    ctx: *const runner.ExecutionContext,
    repos: []const []const u8,
    args: []const []const u8,
) !void {
    if (args.len == 0) {
        return error.NoCommandSpecified;
    }

    try runner.runParallel(allocator, ctx, repos, args, format);
}
