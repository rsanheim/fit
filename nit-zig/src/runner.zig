const std = @import("std");
const repo = @import("repo.zig");
const main = @import("main.zig");

const MAX_REPO_NAME_WIDTH: usize = 24;

/// Format repo name with fixed width: truncate long names, pad short ones
pub fn formatRepoName(allocator: std.mem.Allocator, name: []const u8) ![]const u8 {
    var display_name: []const u8 = undefined;
    var allocated = false;

    if (name.len > MAX_REPO_NAME_WIDTH) {
        // Truncate: "long-name-..." (20 chars + "-...")
        const truncated = try std.fmt.allocPrint(allocator, "{s}-...", .{name[0 .. MAX_REPO_NAME_WIDTH - 4]});
        display_name = truncated;
        allocated = true;
    } else {
        display_name = name;
    }
    defer if (allocated) allocator.free(display_name);

    // Format with padding: "[name                    ]"
    return std.fmt.allocPrint(allocator, "[{s: <24}]", .{display_name});
}

/// Execution context holding configuration for running git commands
pub const ExecutionContext = struct {
    workers: usize,
    dry_run: bool,
    url_scheme: ?main.UrlScheme,

    pub fn init(workers: usize, dry_run: bool, url_scheme: ?main.UrlScheme) ExecutionContext {
        return .{
            .workers = workers,
            .dry_run = dry_run,
            .url_scheme = url_scheme,
        };
    }

    /// Get git config args for URL scheme rewriting
    pub fn urlSchemeArgs(self: *const ExecutionContext) []const []const u8 {
        return switch (self.url_scheme orelse return &.{}) {
            .ssh => &.{ "-c", "url.git@github.com:.insteadOf=https://github.com/" },
            .https => &.{ "-c", "url.https://github.com/.insteadOf=git@github.com:" },
        };
    }
};

/// A git command ready to be executed against a repository
pub const GitCommand = struct {
    repo_path: []const u8,
    args: []const []const u8,
    url_scheme_args: []const []const u8,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, repo_path: []const u8, args: []const []const u8, url_scheme_args: []const []const u8) GitCommand {
        return .{
            .repo_path = repo_path,
            .args = args,
            .url_scheme_args = url_scheme_args,
            .allocator = allocator,
        };
    }

    /// Build the command string for display (used in dry-run and errors)
    pub fn commandString(self: *const GitCommand) ![]const u8 {
        var parts: std.ArrayList(u8) = .empty;
        errdefer parts.deinit(self.allocator);

        try parts.appendSlice(self.allocator, "git");

        // Add URL scheme args (e.g., -c url.git@github.com:...)
        for (self.url_scheme_args) |arg| {
            try parts.append(self.allocator, ' ');
            try parts.appendSlice(self.allocator, arg);
        }

        try parts.appendSlice(self.allocator, " -C ");
        try parts.appendSlice(self.allocator, self.repo_path);

        for (self.args) |arg| {
            try parts.append(self.allocator, ' ');
            try parts.appendSlice(self.allocator, arg);
        }

        return parts.toOwnedSlice(self.allocator);
    }

    /// Execute the git command
    pub fn execute(self: *const GitCommand) CommandResult {
        // Build argv for child process
        var argv: std.ArrayList([]const u8) = .empty;
        defer argv.deinit(self.allocator);

        argv.append(self.allocator, "git") catch return .{ .err = .{ .repo_name = repo.repoName(self.repo_path), .message = "failed to build argv" } };
        // Add URL scheme args (e.g., -c url.git@github.com:...)
        for (self.url_scheme_args) |arg| {
            argv.append(self.allocator, arg) catch return .{ .err = .{ .repo_name = repo.repoName(self.repo_path), .message = "failed to build argv" } };
        }
        argv.append(self.allocator, "-C") catch return .{ .err = .{ .repo_name = repo.repoName(self.repo_path), .message = "failed to build argv" } };
        argv.append(self.allocator, self.repo_path) catch return .{ .err = .{ .repo_name = repo.repoName(self.repo_path), .message = "failed to build argv" } };
        for (self.args) |arg| {
            argv.append(self.allocator, arg) catch return .{ .err = .{ .repo_name = repo.repoName(self.repo_path), .message = "failed to build argv" } };
        }

        var child = std.process.Child.init(argv.items, self.allocator);
        child.stdout_behavior = .Pipe;
        child.stderr_behavior = .Pipe;

        child.spawn() catch |e| {
            return .{ .err = .{
                .repo_name = repo.repoName(self.repo_path),
                .message = @errorName(e),
            } };
        };

        // Read all output using the File's readToEndAlloc
        const stdout = child.stdout.?.readToEndAlloc(self.allocator, 1024 * 1024) catch |e| {
            return .{ .err = .{
                .repo_name = repo.repoName(self.repo_path),
                .message = @errorName(e),
            } };
        };

        const stderr = child.stderr.?.readToEndAlloc(self.allocator, 1024 * 1024) catch |e| {
            self.allocator.free(stdout);
            return .{ .err = .{
                .repo_name = repo.repoName(self.repo_path),
                .message = @errorName(e),
            } };
        };

        const term = child.wait() catch |e| {
            self.allocator.free(stdout);
            self.allocator.free(stderr);
            return .{ .err = .{
                .repo_name = repo.repoName(self.repo_path),
                .message = @errorName(e),
            } };
        };

        const success = term.Exited == 0;

        return .{ .executed = .{
            .repo_name = repo.repoName(self.repo_path),
            .stdout = stdout,
            .stderr = stderr,
            .success = success,
        } };
    }
};

/// Result of executing a git command
pub const CommandResult = union(enum) {
    dry_run: []const u8,
    executed: struct {
        repo_name: []const u8,
        stdout: []const u8,
        stderr: []const u8,
        success: bool,
    },
    err: struct {
        repo_name: []const u8,
        message: []const u8,
    },
};

/// Output formatter function type
/// Formatters accept an allocator and return owned memory that the caller must free
pub const OutputFormatter = *const fn (allocator: std.mem.Allocator, stdout: []const u8, stderr: []const u8, success: bool) error{OutOfMemory}![]const u8;

/// Thread context for parallel execution
const ThreadContext = struct {
    repos: []const []const u8,
    start_idx: usize,
    end_idx: usize,
    extra_args: []const []const u8,
    formatter: OutputFormatter,
    ctx: *const ExecutionContext,
    stdout_mutex: *std.Thread.Mutex,
    allocator: std.mem.Allocator,
};

fn workerThread(thread_ctx: ThreadContext) void {
    const allocator = thread_ctx.allocator;
    const url_scheme_args = thread_ctx.ctx.urlSchemeArgs();

    for (thread_ctx.start_idx..thread_ctx.end_idx) |i| {
        const repo_path = thread_ctx.repos[i];
        const cmd = GitCommand.init(allocator, repo_path, thread_ctx.extra_args, url_scheme_args);

        var output_line: []const u8 = undefined;
        var needs_free = true;

        if (thread_ctx.ctx.dry_run) {
            if (cmd.commandString()) |cmd_str| {
                output_line = cmd_str;
            } else |_| {
                output_line = "error building command";
                needs_free = false;
            }
        } else {
            const result = cmd.execute();

            switch (result) {
                .dry_run => |cmd_str| {
                    output_line = cmd_str;
                },
                .executed => |exec| {
                    // Get formatted output (or fallback)
                    var formatted: []const u8 = undefined;
                    var formatted_allocated = false;
                    if (thread_ctx.formatter(allocator, exec.stdout, exec.stderr, exec.success)) |f| {
                        formatted = f;
                        formatted_allocated = true;
                    } else |_| {
                        formatted = "format error";
                    }

                    const repo_name = repo.repoName(repo_path);
                    var name_formatted: []const u8 = undefined;
                    var name_allocated = false;
                    if (formatRepoName(allocator, repo_name)) |n| {
                        name_formatted = n;
                        name_allocated = true;
                    } else |_| {
                        name_formatted = "[???]";
                    }

                    // Build final output line
                    if (std.fmt.allocPrint(allocator, "{s} {s}", .{ name_formatted, formatted })) |line| {
                        output_line = line;
                    } else |_| {
                        output_line = "format error";
                        needs_free = false;
                    }

                    // Free intermediate allocations
                    if (formatted_allocated) allocator.free(formatted);
                    if (name_allocated) allocator.free(name_formatted);

                    // Free stdout/stderr from command execution
                    allocator.free(exec.stdout);
                    allocator.free(exec.stderr);
                },
                .err => |e| {
                    const repo_name = repo.repoName(repo_path);
                    var name_formatted: []const u8 = undefined;
                    var name_allocated = false;
                    if (formatRepoName(allocator, repo_name)) |n| {
                        name_formatted = n;
                        name_allocated = true;
                    } else |_| {
                        name_formatted = "[???]";
                    }

                    if (std.fmt.allocPrint(allocator, "{s} ERROR: {s}", .{ name_formatted, e.message })) |line| {
                        output_line = line;
                    } else |_| {
                        output_line = "format error";
                        needs_free = false;
                    }

                    if (name_allocated) allocator.free(name_formatted);
                },
            }
        }

        // Lock stdout and print atomically
        thread_ctx.stdout_mutex.lock();
        const stdout_file = std.fs.File.stdout();
        stdout_file.writeAll(output_line) catch {};
        stdout_file.writeAll("\n") catch {};
        thread_ctx.stdout_mutex.unlock();

        // Free this iteration's allocation
        if (needs_free) {
            allocator.free(output_line);
        }
    }
}

/// Run commands in parallel across all repos
pub fn runParallel(
    allocator: std.mem.Allocator,
    ctx: *const ExecutionContext,
    repos: []const []const u8,
    extra_args: []const []const u8,
    formatter: OutputFormatter,
) !void {
    if (repos.len == 0) return;

    var stdout_mutex = std.Thread.Mutex{};

    // Calculate work distribution
    const num_workers = @min(ctx.workers, repos.len);
    const repos_per_worker = repos.len / num_workers;
    const extra_repos = repos.len % num_workers;

    var threads: std.ArrayList(std.Thread) = .empty;
    defer threads.deinit(allocator);

    var start_idx: usize = 0;
    for (0..num_workers) |worker_id| {
        // Distribute extra repos among first workers
        const worker_repos = repos_per_worker + @as(usize, if (worker_id < extra_repos) 1 else 0);
        const end_idx = start_idx + worker_repos;

        const thread_ctx = ThreadContext{
            .repos = repos,
            .start_idx = start_idx,
            .end_idx = end_idx,
            .extra_args = extra_args,
            .formatter = formatter,
            .ctx = ctx,
            .stdout_mutex = &stdout_mutex,
            .allocator = allocator,
        };

        const thread = try std.Thread.spawn(.{}, workerThread, .{thread_ctx});
        try threads.append(allocator, thread);

        start_idx = end_idx;
    }

    // Wait for all threads to complete
    for (threads.items) |thread| {
        thread.join();
    }
}

test "formatRepoName short name" {
    const allocator = std.testing.allocator;
    const result = try formatRepoName(allocator, "my-repo");
    defer allocator.free(result);
    // 26 chars total: [ + 24 chars padded + ]
    try std.testing.expectEqualStrings("[my-repo                 ]", result);
    try std.testing.expect(result.len == 26);
}

test "formatRepoName truncated" {
    const allocator = std.testing.allocator;
    const result = try formatRepoName(allocator, "this-is-a-very-long-repository-name");
    defer allocator.free(result);
    // Truncated to 20 chars + "-..." = 24 chars, wrapped in brackets
    try std.testing.expectEqualStrings("[this-is-a-very-long--...]", result);
    try std.testing.expect(result.len == 26);
}
