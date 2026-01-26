const std = @import("std");
const main = @import("main.zig");

/// Handle `git-all meta` subcommands.
/// Meta mode provides git-all-specific commands (help, etc.) rather than git passthrough.
pub fn run(allocator: std.mem.Allocator, args: []const []const u8) !void {
    if (args.len == 0) {
        try printHelp(allocator);
        return;
    }

    const subcommand = args[0];
    if (std.mem.eql(u8, subcommand, "help")) {
        try printHelp(allocator);
        return;
    }

    // Unknown subcommand
    const stderr = std.fs.File.stderr();
    var buf: [256]u8 = undefined;
    const msg = std.fmt.bufPrint(&buf, "Unknown meta subcommand: {s}\nAvailable: help\n", .{subcommand}) catch return;
    stderr.writeAll(msg) catch {};
    std.process.exit(1);
}

fn printHelp(allocator: std.mem.Allocator) !void {
    const stdout = std.fs.File.stdout();
    const git_version = try getGitVersion(allocator);
    defer allocator.free(git_version);

    // Print version header
    var header_buf: [128]u8 = undefined;
    const header = std.fmt.bufPrint(&header_buf, "git-all v{s} (git {s})\n\n", .{ main.VERSION, git_version }) catch return;
    stdout.writeAll(header) catch {};

    // Print help text
    stdout.writeAll(help_text) catch {};
}

fn getGitVersion(allocator: std.mem.Allocator) ![]const u8 {
    var child = std.process.Child.init(&.{"git", "--version"}, allocator);
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Ignore;

    child.spawn() catch {
        return try allocator.dupe(u8, "unknown");
    };

    const stdout = child.stdout orelse {
        _ = child.wait() catch {};
        return try allocator.dupe(u8, "unknown");
    };

    var buf: [128]u8 = undefined;
    const bytes_read = stdout.read(&buf) catch {
        _ = child.wait() catch {};
        return try allocator.dupe(u8, "unknown");
    };

    _ = child.wait() catch {};

    // Parse "git version X.Y.Z" -> "X.Y.Z"
    const output = buf[0..bytes_read];
    const trimmed = std.mem.trim(u8, output, " \t\n\r");

    if (std.mem.startsWith(u8, trimmed, "git version ")) {
        const version = trimmed["git version ".len..];
        return try allocator.dupe(u8, version);
    }

    return try allocator.dupe(u8, trimmed);
}

const help_text =
    \\USAGE:
    \\    git-all [OPTIONS] <COMMAND> [ARGS...]
    \\
    \\OPTIONS:
    \\    -n, --workers <NUM>   Number of parallel workers (default: 8)
    \\    --dry-run             Print exact commands without executing
    \\    --ssh                 Force SSH URLs (git@github.com:) for all remotes
    \\    --https               Force HTTPS URLs (https://github.com/) for all remotes
    \\    -h, --help            Print help information
    \\    -V, --version         Print version
    \\
    \\COMMANDS:
    \\    pull      Git pull with condensed output
    \\    fetch     Git fetch with condensed output
    \\    status    Git status with condensed output
    \\    meta      git-all internal commands (help, version info)
    \\    <any>     Pass-through to git verbatim
    \\
    \\EXAMPLES:
    \\    git-all pull                  Pull all repos
    \\    git-all status                Status of all repos
    \\    git-all --dry-run pull        Show commands without executing
    \\    git-all -n 4 fetch            Fetch with 4 workers
    \\    git-all checkout main         Switch all repos to main
    \\    git-all meta                  Show this help with version info
    \\
;

test "getGitVersion returns version string" {
    const allocator = std.testing.allocator;
    const version = try getGitVersion(allocator);
    defer allocator.free(version);

    // Should get some version string (not empty, not "unknown" if git is installed)
    try std.testing.expect(version.len > 0);
}
