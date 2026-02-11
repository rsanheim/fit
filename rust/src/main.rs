use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

mod commands;
mod meta;
mod repo;
mod runner;

use commands::{fetch, passthrough, pull, status};
use repo::{find_git_repos_in, is_inside_git_repo, parse_scan_depth, ScanDepth};
use runner::{ExecutionContext, UrlScheme};

#[derive(Parser)]
#[command(name = "git-all", version, about = "parallel git across many repositories")]
struct Cli {
    /// Print exact commands without executing
    #[arg(long)]
    dry_run: bool,

    /// Force SSH URLs (git@github.com:) for all remotes
    #[arg(long, conflicts_with = "https")]
    ssh: bool,

    /// Force HTTPS URLs (https://github.com/) for all remotes
    #[arg(long, conflicts_with = "ssh")]
    https: bool,

    /// Number of parallel workers (default: 8, 0 = unlimited)
    #[arg(short = 'n', long, default_value = "8")]
    workers: usize,

    /// How deep to scan for repositories (positive integer or "all")
    #[arg(long, default_value = "1", value_parser = parse_scan_depth, value_name = "DEPTH|all")]
    scan_depth: ScanDepth,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Pull all repositories
    Pull {
        /// Additional arguments to pass to git pull
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Fetch all repositories
    Fetch {
        /// Additional arguments to pass to git fetch
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Status of all repositories
    Status {
        /// Additional arguments to pass to git status
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// git-all internal commands (help, version info)
    Meta {
        /// Subcommand (help is the only option)
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Pass through to git (any other command)
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Exec git with all original args, replacing the git-all process.
/// This is used when git-all is invoked from inside a git repository.
#[cfg(unix)]
fn passthrough_to_git() -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let err = Command::new("git").args(&args).exec();
    // exec() only returns on error
    eprintln!("git-all: failed to exec git: {}", err);
    std::process::exit(1);
}

#[cfg(not(unix))]
fn passthrough_to_git() -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let status = Command::new("git")
        .args(&args)
        .status()
        .expect("failed to execute git");
    std::process::exit(status.code().unwrap_or(1));
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let is_meta = args.first().map(|s| s == "meta").unwrap_or(false);

    if !is_meta && is_inside_git_repo() {
        passthrough_to_git();
    }

    let cli = Cli::parse();

    if let Some(Commands::Meta { args }) = &cli.command {
        meta::run(args);
        return Ok(());
    }

    let cwd = std::env::current_dir()?;
    let repos = find_git_repos_in(&cwd, cli.scan_depth)?;
    if repos.is_empty() {
        println!("No git repositories found in current directory");
        std::process::exit(9);
    }

    let url_scheme = if cli.ssh {
        Some(UrlScheme::Ssh)
    } else if cli.https {
        Some(UrlScheme::Https)
    } else {
        None
    };

    let ctx = ExecutionContext::new(cli.dry_run, url_scheme, cli.workers, cwd);

    if cli.dry_run {
        println!(
            "[git-all v{}] Running in **dry-run mode**, no git commands will be executed. Planned git commands below.",
            env!("CARGO_PKG_VERSION")
        );
    }

    match cli.command {
        Some(Commands::Pull { args }) => pull::run(&ctx, &repos, &args),
        Some(Commands::Fetch { args }) => fetch::run(&ctx, &repos, &args),
        Some(Commands::Status { args }) => status::run(&ctx, &repos, &args),
        Some(Commands::External(args)) => {
            let resolved =
                resolve_git_alias(&args[0]).and_then(|v| optimized_command_for(&v));
            match resolved {
                Some("status") => status::run(&ctx, &repos, &args[1..]),
                Some("pull") => pull::run(&ctx, &repos, &args[1..]),
                Some("fetch") => fetch::run(&ctx, &repos, &args[1..]),
                _ => passthrough::run(&ctx, &repos, &args),
            }
        }
        Some(Commands::Meta { .. }) => unreachable!(), // handled above
        None => {
            // No command given - show help
            println!("No command specified. Use --help for usage information.");
            Ok(())
        }
    }
}

/// Ask git for the alias value of `name`. Returns `None` if the alias
/// doesn't exist, git isn't installed, or any other error occurs.
fn resolve_git_alias(name: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", &format!("alias.{name}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// If `alias_value` resolves to one of our optimized commands, return
/// the canonical command name. Only the first word is considered; shell
/// aliases (starting with `!`) are never matched.
fn optimized_command_for(alias_value: &str) -> Option<&'static str> {
    let first_word = alias_value.split_whitespace().next()?;
    match first_word {
        "status" => Some("status"),
        "pull" => Some("pull"),
        "fetch" => Some("fetch"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimized_command_for_resolves_aliases() {
        let cases = [
            ("status", Some("status")),
            ("status -sb", Some("status")),
            ("pull --rebase", Some("pull")),
            ("fetch --all", Some("fetch")),
            ("!git log --oneline", None),
            ("log --oneline", None),
            ("", None),
        ];
        for (input, expected) in cases {
            assert_eq!(optimized_command_for(input), expected, "input: {input:?}");
        }
    }
}
