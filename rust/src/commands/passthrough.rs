use anyhow::Result;
use std::path::PathBuf;

use crate::runner::{run_passthrough, ExecutionContext, GitCommand};

pub fn run(ctx: &ExecutionContext, repos: &[PathBuf], args: &[String]) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!("No git command specified");
    }

    run_passthrough(ctx, repos, |repo| GitCommand::new(repo.clone(), args.to_vec()))
}
