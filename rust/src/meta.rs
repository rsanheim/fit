use clap::CommandFactory;
use std::process::Command;

use crate::Cli;

pub fn run(args: &[String]) {
    match args.first().map(|s| s.as_str()) {
        None | Some("help") => print_help(),
        Some(unknown) => {
            eprintln!("Unknown meta subcommand: {}", unknown);
            eprintln!("Available: help");
            std::process::exit(1);
        }
    }
}

fn print_help() {
    let git_version = get_git_version();
    println!(
        "git-all v{} (git {})",
        env!("CARGO_PKG_VERSION"),
        git_version
    );
    println!();

    let mut cmd = Cli::command();
    cmd.print_help().expect("failed to print help");
    println!();
}

fn get_git_version() -> String {
    Command::new("git")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().replace("git version ", ""))
        .unwrap_or_else(|| "unknown".to_string())
}
