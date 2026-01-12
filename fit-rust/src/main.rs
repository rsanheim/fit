use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

mod commands;
mod repo;
mod runner;

use commands::{fetch, passthrough, pull, status};
use repo::{find_git_repos, is_inside_git_repo};
use runner::{ExecutionContext, UrlScheme};

#[derive(Parser)]
#[command(name = "fit", version, about = "parallel git across many repositories")]
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
    /// Pass through to git (any other command)
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Exec git with all original args, replacing the fit process.
/// This is used when fit is invoked from inside a git repository.
#[cfg(unix)]
fn passthrough_to_git() -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let err = Command::new("git").args(&args).exec();
    // exec() only returns on error
    eprintln!("fit: failed to exec git: {}", err);
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
    // Passthrough mode: if we're inside a git repo, just exec git directly
    if is_inside_git_repo() {
        passthrough_to_git();
    }

    let cli = Cli::parse();

    let repos = find_git_repos()?;
    if repos.is_empty() {
        println!("No git repositories found in current directory");
        return Ok(());
    }

    let url_scheme = if cli.ssh {
        Some(UrlScheme::Ssh)
    } else if cli.https {
        Some(UrlScheme::Https)
    } else {
        None
    };

    let ctx = ExecutionContext::new(cli.dry_run, url_scheme, cli.workers);

    if cli.dry_run {
        println!(
            "[fit v{}] Running in **dry-run mode**, no git commands will be executed. Planned git commands below.",
            env!("CARGO_PKG_VERSION")
        );
    }

    match cli.command {
        Some(Commands::Pull { args }) => pull::run(&ctx, &repos, &args),
        Some(Commands::Fetch { args }) => fetch::run(&ctx, &repos, &args),
        Some(Commands::Status { args }) => status::run(&ctx, &repos, &args),
        Some(Commands::External(args)) => passthrough::run(&ctx, &repos, &args),
        None => {
            // No command given - show help
            println!("No command specified. Use --help for usage information.");
            Ok(())
        }
    }
}
