const std = @import("std");
const fs = std.fs;

/// Check if the current working directory is inside a git repository.
/// Uses `git rev-parse --git-dir` which correctly handles worktrees,
/// bare repos, and the GIT_DIR environment variable.
pub fn isInsideGitRepo(allocator: std.mem.Allocator) bool {
    const argv = [_][]const u8{ "git", "rev-parse", "--git-dir" };
    var child = std.process.Child.init(&argv, allocator);
    child.stdout_behavior = .Ignore;
    child.stderr_behavior = .Ignore;

    child.spawn() catch return false;
    const term = child.wait() catch return false;
    return term.Exited == 0;
}

/// Find all git repositories in the current directory (depth 1).
/// Returns a sorted list of absolute paths to directories containing a .git folder.
/// Caller owns the returned slice and paths.
pub fn findGitRepos(allocator: std.mem.Allocator) ![][]const u8 {
    const cwd = try std.fs.cwd().realpathAlloc(allocator, ".");
    defer allocator.free(cwd);

    var repos: std.ArrayList([]const u8) = .empty;
    errdefer {
        for (repos.items) |path| {
            allocator.free(path);
        }
        repos.deinit(allocator);
    }

    var dir = try std.fs.cwd().openDir(".", .{ .iterate = true });
    defer dir.close();

    var iter = dir.iterate();
    while (try iter.next()) |entry| {
        if (entry.kind != .directory) continue;

        // Check if .git exists in this directory
        var subdir = dir.openDir(entry.name, .{}) catch continue;
        defer subdir.close();

        subdir.access(".git", .{}) catch continue;

        // Found a git repo - construct full path
        const full_path = try std.fs.path.join(allocator, &.{ cwd, entry.name });
        try repos.append(allocator, full_path);
    }

    // Sort the repos by path
    const items = try repos.toOwnedSlice(allocator);
    std.mem.sort([]const u8, items, {}, struct {
        fn lessThan(_: void, a: []const u8, b: []const u8) bool {
            return std.mem.order(u8, a, b) == .lt;
        }
    }.lessThan);

    return items;
}

/// Extract just the repository name from a path
pub fn repoName(path: []const u8) []const u8 {
    return std.fs.path.basename(path);
}

test "repoName extracts directory name" {
    const name = repoName("/home/user/src/my-repo");
    try std.testing.expectEqualStrings("my-repo", name);
}

test "repoName handles simple name" {
    const name = repoName("my-repo");
    try std.testing.expectEqualStrings("my-repo", name);
}
