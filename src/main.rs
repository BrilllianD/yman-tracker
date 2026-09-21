mod cli;
mod commands;
mod config;
mod discussion;
mod errors;
mod git;
mod hooks;
mod ids;
mod json;
mod refresh;
mod refs;
mod repo;
mod tags;
mod task;
mod yml;

use clap::Parser;
use cli::{Cli, Cmd};

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(errors::exit_code_for(&err));
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    // Documentation, not a command on a repository: it must work from a
    // fresh shell in any directory, so it is answered before discovery.
    if matches!(cli.cmd, Cmd::Guide) {
        return commands::guide::run();
    }

    let mut ctx = repo::discover()?;

    if !cli.cmd.skips_preflight() {
        ctx.preflight(cli.cmd.is_mutating())?;
    }

    if !cli.cmd.skips_lazy_refresh() {
        // A failing refresh must never take the actual command down with it.
        if let Err(err) = refresh::lazy(&ctx, true) {
            eprintln!("warning: refresh failed: {err:#}");
        }
    }

    commands::dispatch(&mut ctx, cli.cmd)
}
