const std = @import("std");
const repo = @import("repo.zig");
const runner = @import("runner.zig");
const pull = @import("commands/pull.zig");
const fetch = @import("commands/fetch.zig");
const status = @import("commands/status.zig");
const passthrough = @import("commands/passthrough.zig");

const VERSION = "0.3.0";
const DEFAULT_WORKERS: usize = 8;

const Command = enum {
    pull,
    fetch,
    status,
    help,
    version,
    external,
};

pub const UrlScheme = enum {
    ssh,
    https,
};

const ParsedArgs = struct {
    workers: usize,
    dry_run: bool,
    url_scheme: ?UrlScheme,
    command: Command,
    extra_args: []const []const u8,
};

fn printHelp() void {
    const help =
        \\fit - parallel git across many repositories
        \\
        \\USAGE:
        \\    fit [OPTIONS] <COMMAND> [ARGS...]
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
        \\    <any>     Pass-through to git verbatim
        \\
        \\EXAMPLES:
        \\    fit pull                      Pull all repos
        \\    fit status                    Status of all repos
        \\    fit --dry-run pull            Show commands without executing
        \\    fit -n 4 fetch                Fetch with 4 workers
        \\    fit checkout main             Switch all repos to main
        \\
    ;
    const stdout = std.fs.File.stdout();
    stdout.writeAll(help) catch {};
}

fn printVersion() void {
    const stdout = std.fs.File.stdout();
    var buf: [64]u8 = undefined;
    const msg = std.fmt.bufPrint(&buf, "fit {s}\n", .{VERSION}) catch return;
    stdout.writeAll(msg) catch {};
}

/// Exec git with all original args, replacing the fit process.
/// This is used when fit is invoked from inside a git repository.
fn passthroughToGit(allocator: std.mem.Allocator) noreturn {
    // Collect original args, replacing argv[0] with "git"
    var args_iter = std.process.argsWithAllocator(allocator) catch std.process.exit(1);
    defer args_iter.deinit();

    var argv: std.ArrayList([]const u8) = .empty;
    defer argv.deinit(allocator);

    // Skip program name (fit) and prepend "git"
    _ = args_iter.next();
    argv.append(allocator, "git") catch std.process.exit(1);

    // Add remaining args
    while (args_iter.next()) |arg| {
        argv.append(allocator, arg) catch std.process.exit(1);
    }

    // Spawn git and wait, then exit with its code
    var child = std.process.Child.init(argv.items, allocator);

    child.spawn() catch |err| {
        std.debug.print("fit: failed to exec git: {s}\n", .{@errorName(err)});
        std.process.exit(1);
    };

    const term = child.wait() catch |err| {
        std.debug.print("fit: failed to wait for git: {s}\n", .{@errorName(err)});
        std.process.exit(1);
    };

    switch (term) {
        .Exited => |code| std.process.exit(code),
        else => std.process.exit(1),
    }
}

fn parseArgs(allocator: std.mem.Allocator) !ParsedArgs {
    var args_iter = try std.process.argsWithAllocator(allocator);
    defer args_iter.deinit();

    // Skip program name
    _ = args_iter.next();

    var workers: usize = DEFAULT_WORKERS;
    var dry_run: bool = false;
    var url_scheme: ?UrlScheme = null;
    var command: ?Command = null;
    var extra_args: std.ArrayList([]const u8) = .empty;
    errdefer extra_args.deinit(allocator);

    var found_command = false;

    while (args_iter.next()) |arg| {
        if (found_command) {
            // After command, everything is extra args
            const owned = try allocator.dupe(u8, arg);
            try extra_args.append(allocator, owned);
            continue;
        }

        if (std.mem.eql(u8, arg, "-h") or std.mem.eql(u8, arg, "--help")) {
            return .{
                .workers = workers,
                .dry_run = dry_run,
                .url_scheme = url_scheme,
                .command = .help,
                .extra_args = try extra_args.toOwnedSlice(allocator),
            };
        }

        if (std.mem.eql(u8, arg, "-V") or std.mem.eql(u8, arg, "--version")) {
            return .{
                .workers = workers,
                .dry_run = dry_run,
                .url_scheme = url_scheme,
                .command = .version,
                .extra_args = try extra_args.toOwnedSlice(allocator),
            };
        }

        if (std.mem.eql(u8, arg, "--dry-run")) {
            dry_run = true;
            continue;
        }

        if (std.mem.eql(u8, arg, "--ssh")) {
            url_scheme = .ssh;
            continue;
        }

        if (std.mem.eql(u8, arg, "--https")) {
            url_scheme = .https;
            continue;
        }

        if (std.mem.eql(u8, arg, "-n") or std.mem.eql(u8, arg, "--workers")) {
            if (args_iter.next()) |num_str| {
                workers = std.fmt.parseInt(usize, num_str, 10) catch DEFAULT_WORKERS;
            }
            continue;
        }

        // Check if it's a -n<NUM> combined form
        if (std.mem.startsWith(u8, arg, "-n")) {
            const num_str = arg[2..];
            if (num_str.len > 0) {
                workers = std.fmt.parseInt(usize, num_str, 10) catch DEFAULT_WORKERS;
            }
            continue;
        }

        // This must be the command
        found_command = true;

        if (std.mem.eql(u8, arg, "pull")) {
            command = .pull;
        } else if (std.mem.eql(u8, arg, "fetch")) {
            command = .fetch;
        } else if (std.mem.eql(u8, arg, "status")) {
            command = .status;
        } else {
            // External command - add it as first extra arg
            command = .external;
            const owned = try allocator.dupe(u8, arg);
            try extra_args.append(allocator, owned);
        }
    }

    return .{
        .workers = workers,
        .dry_run = dry_run,
        .url_scheme = url_scheme,
        .command = command orelse .help,
        .extra_args = try extra_args.toOwnedSlice(allocator),
    };
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Passthrough mode: if we're inside a git repo, just exec git directly
    if (repo.isInsideGitRepo(allocator)) {
        passthroughToGit(allocator);
    }

    const args = try parseArgs(allocator);
    defer allocator.free(args.extra_args);

    // Handle help and version early
    if (args.command == .help) {
        printHelp();
        return;
    }

    if (args.command == .version) {
        printVersion();
        return;
    }

    // Find git repos
    const repos = try repo.findGitRepos(allocator);
    defer {
        for (repos) |r| {
            allocator.free(r);
        }
        allocator.free(repos);
    }

    if (repos.len == 0) {
        std.fs.File.stdout().writeAll("No git repositories found in current directory\n") catch {};
        return;
    }

    const ctx = runner.ExecutionContext.init(args.workers, args.dry_run, args.url_scheme);

    // Print dry-run header if applicable
    if (args.dry_run) {
        const stdout = std.fs.File.stdout();
        var buf: [256]u8 = undefined;
        const msg = std.fmt.bufPrint(&buf, "[fit v{s}] Running in **dry-run mode**, no git commands will be executed. Planned git commands below.\n", .{VERSION}) catch return;
        stdout.writeAll(msg) catch {};
    }

    // Dispatch to command handler
    switch (args.command) {
        .pull => try pull.run(allocator, &ctx, repos, args.extra_args),
        .fetch => try fetch.run(allocator, &ctx, repos, args.extra_args),
        .status => try status.run(allocator, &ctx, repos, args.extra_args),
        .external => try passthrough.run(allocator, &ctx, repos, args.extra_args),
        .help, .version => unreachable,
    }
}

test "parseArgs with pull" {
    // Basic test structure - would need proper test setup
}

test {
    // Run all module tests
    _ = @import("repo.zig");
    _ = @import("runner.zig");
}
