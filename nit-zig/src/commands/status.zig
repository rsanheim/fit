const std = @import("std");
const runner = @import("../runner.zig");

/// Format status output to single line (expects --porcelain output)
/// Returns owned memory that the caller must free
pub fn format(allocator: std.mem.Allocator, stdout: []const u8, stderr: []const u8, success: bool) error{OutOfMemory}![]const u8 {
    if (!success) {
        var lines = std.mem.splitScalar(u8, stderr, '\n');
        return try allocator.dupe(u8, lines.next() orelse "unknown error");
    }

    // Parse porcelain output to count file states
    var modified: usize = 0;
    var added: usize = 0;
    var deleted: usize = 0;
    var untracked: usize = 0;
    var renamed: usize = 0;

    var lines = std.mem.splitScalar(u8, stdout, '\n');
    while (lines.next()) |line| {
        if (line.len < 2) continue;

        const index_status = line[0];
        const worktree_status = line[1];

        // Untracked files
        if (index_status == '?') {
            untracked += 1;
            continue;
        }

        // Check index status (staged changes)
        switch (index_status) {
            'M' => modified += 1,
            'A' => added += 1,
            'D' => deleted += 1,
            'R' => renamed += 1,
            else => {},
        }

        // Check worktree status (unstaged changes) - only if index is clean
        if (index_status == ' ') {
            switch (worktree_status) {
                'M' => modified += 1,
                'D' => deleted += 1,
                else => {},
            }
        }
    }

    // Build human-readable summary
    if (modified == 0 and added == 0 and deleted == 0 and untracked == 0 and renamed == 0) {
        return try allocator.dupe(u8, "clean");
    }

    // Build parts list dynamically (like Rust implementation)
    var parts: std.ArrayList([]const u8) = .empty;
    defer parts.deinit(allocator);

    // Track allocations so we can free them after join
    var allocated_parts: std.ArrayList([]const u8) = .empty;
    defer {
        for (allocated_parts.items) |part| {
            allocator.free(part);
        }
        allocated_parts.deinit(allocator);
    }

    if (modified > 0) {
        const part = try std.fmt.allocPrint(allocator, "{d} modified", .{modified});
        try parts.append(allocator, part);
        try allocated_parts.append(allocator, part);
    }
    if (added > 0) {
        const part = try std.fmt.allocPrint(allocator, "{d} added", .{added});
        try parts.append(allocator, part);
        try allocated_parts.append(allocator, part);
    }
    if (deleted > 0) {
        const part = try std.fmt.allocPrint(allocator, "{d} deleted", .{deleted});
        try parts.append(allocator, part);
        try allocated_parts.append(allocator, part);
    }
    if (renamed > 0) {
        const part = try std.fmt.allocPrint(allocator, "{d} renamed", .{renamed});
        try parts.append(allocator, part);
        try allocated_parts.append(allocator, part);
    }
    if (untracked > 0) {
        const part = try std.fmt.allocPrint(allocator, "{d} untracked", .{untracked});
        try parts.append(allocator, part);
        try allocated_parts.append(allocator, part);
    }

    // Join parts with ", "
    return try std.mem.join(allocator, ", ", parts.items);
}

/// Build args for status command (uses --porcelain for machine-readable output)
pub fn buildArgs(allocator: std.mem.Allocator, extra_args: []const []const u8) ![]const []const u8 {
    var args: std.ArrayList([]const u8) = .empty;
    try args.append(allocator, "status");
    try args.append(allocator, "--porcelain");
    for (extra_args) |arg| {
        try args.append(allocator, arg);
    }
    return args.toOwnedSlice(allocator);
}

/// Run status command across all repos
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
