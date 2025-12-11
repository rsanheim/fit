const std = @import("std");
const runner = @import("../runner.zig");

/// Format fetch output to single line
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

    // Count non-empty lines in stdout and stderr (excluding "From" lines)
    var stdout_lines: usize = 0;
    var lines = std.mem.splitScalar(u8, stdout, '\n');
    while (lines.next()) |line| {
        const trimmed = std.mem.trim(u8, line, " \t\r");
        if (trimmed.len > 0) {
            stdout_lines += 1;
        }
    }

    var stderr_lines: usize = 0;
    lines = std.mem.splitScalar(u8, stderr, '\n');
    while (lines.next()) |line| {
        const trimmed = std.mem.trim(u8, line, " \t\r");
        if (trimmed.len > 0 and !std.mem.startsWith(u8, trimmed, "From")) {
            stderr_lines += 1;
        }
    }

    if (stdout_lines == 0 and stderr_lines == 0) {
        return try allocator.dupe(u8, "no new commits");
    }

    // Count branches/tags updated from stdout
    var branch_count: usize = 0;
    var tag_count: usize = 0;

    lines = std.mem.splitScalar(u8, stdout, '\n');
    while (lines.next()) |line| {
        if (std.mem.indexOf(u8, line, "->") != null or std.mem.indexOf(u8, line, "[new") != null) {
            if (std.mem.indexOf(u8, line, "[new tag]") != null) {
                tag_count += 1;
            } else {
                branch_count += 1;
            }
        }
    }

    if (branch_count > 0 or tag_count > 0) {
        // Build parts list dynamically with proper pluralization (like Rust)
        var parts: std.ArrayList([]const u8) = .empty;
        defer parts.deinit(allocator);

        var allocated_parts: std.ArrayList([]const u8) = .empty;
        defer {
            for (allocated_parts.items) |part| {
                allocator.free(part);
            }
            allocated_parts.deinit(allocator);
        }

        if (branch_count > 0) {
            const plural: []const u8 = if (branch_count == 1) "" else "es";
            const part = try std.fmt.allocPrint(allocator, "{d} branch{s}", .{ branch_count, plural });
            try parts.append(allocator, part);
            try allocated_parts.append(allocator, part);
        }
        if (tag_count > 0) {
            const plural: []const u8 = if (tag_count == 1) "" else "s";
            const part = try std.fmt.allocPrint(allocator, "{d} tag{s}", .{ tag_count, plural });
            try parts.append(allocator, part);
            try allocated_parts.append(allocator, part);
        }

        const joined = try std.mem.join(allocator, ", ", parts.items);
        defer allocator.free(joined);
        return try std.fmt.allocPrint(allocator, "{s} updated", .{joined});
    }

    return try allocator.dupe(u8, "fetched");
}

/// Build args for fetch command
pub fn buildArgs(allocator: std.mem.Allocator, extra_args: []const []const u8) ![]const []const u8 {
    var args: std.ArrayList([]const u8) = .empty;
    try args.append(allocator, "fetch");
    for (extra_args) |arg| {
        try args.append(allocator, arg);
    }
    return args.toOwnedSlice(allocator);
}

/// Run fetch command across all repos
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
