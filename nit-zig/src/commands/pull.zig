const std = @import("std");
const runner = @import("../runner.zig");

/// Format pull output to single line
/// Returns owned memory that the caller must free
pub fn format(allocator: std.mem.Allocator, stdout: []const u8, stderr: []const u8, success: bool) error{OutOfMemory}![]const u8 {
    if (!success) {
        var lines = std.mem.splitScalar(u8, stderr, '\n');
        return try allocator.dupe(u8, lines.next() orelse "unknown error");
    }

    // Check for "Already up to date"
    if (std.mem.indexOf(u8, stdout, "Already up to date") != null) {
        return try allocator.dupe(u8, "Already up to date");
    }

    // Try to extract summary from stdout (e.g., "3 files changed, 10 insertions(+)")
    var lines = std.mem.splitScalar(u8, stdout, '\n');
    while (lines.next()) |line| {
        if (std.mem.indexOf(u8, line, "files changed") != null) {
            return try allocator.dupe(u8, std.mem.trim(u8, line, " \t\r"));
        }
    }

    // Check for fast-forward or merge info
    lines = std.mem.splitScalar(u8, stdout, '\n');
    while (lines.next()) |line| {
        if (std.mem.indexOf(u8, line, "..") != null or std.mem.indexOf(u8, line, "Updating") != null) {
            return try allocator.dupe(u8, std.mem.trim(u8, line, " \t\r"));
        }
    }

    // Fallback: first non-empty line
    lines = std.mem.splitScalar(u8, stdout, '\n');
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

    return try allocator.dupe(u8, "completed");
}

/// Build args for pull command
pub fn buildArgs(allocator: std.mem.Allocator, extra_args: []const []const u8) ![]const []const u8 {
    var args: std.ArrayList([]const u8) = .empty;
    try args.append(allocator, "pull");
    for (extra_args) |arg| {
        try args.append(allocator, arg);
    }
    return args.toOwnedSlice(allocator);
}

/// Run pull command across all repos
pub fn run(
    allocator: std.mem.Allocator,
    ctx: *const runner.ExecutionContext,
    repos: []const []const u8,
    extra_args: []const []const u8,
) !void {
    const args = try buildArgs(allocator, extra_args);
    defer allocator.free(args);

    try runner.runParallel(allocator, ctx, repos, args, format);
}
