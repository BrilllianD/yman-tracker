mod cli;
mod commands;
mod config;
mod discussion;
mod errors;
mod git;
mod hooks;
mod ids;
mod json;
mod merge;
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
    // Documentation, not commands on a repository: they must work from a
    // fresh shell in any directory, so they are answered before discovery.
    match cli.cmd {
        Cmd::Guide => return commands::guide::run(),
        Cmd::Completions(a) => return commands::completions::run(a),
        Cmd::Man(a) => return commands::completions::run_man(a),
        _ => {}
    }

    // Git calls this one with three temp files, from inside a merge it is
    // itself driving: there is no repository state to discover, and preflight
    // would refuse on the `MERGE_HEAD` that is always present here.
    if let Cmd::MergeDriver(a) = cli.cmd {
        return match commands::merge_driver::run(a)? {
            true => Ok(()),
            // A conflict the driver could not settle. Git wants a non-zero
            // exit and has already been handed the marked-up file.
            false => std::process::exit(1),
        };
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
