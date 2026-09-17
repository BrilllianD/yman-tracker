mod cli;
mod commands;
mod config;
mod discussion;
mod errors;
mod git;
mod hooks;
mod ids;
mod refresh;
mod repo;
mod task;
mod yml;

use clap::Parser;
use cli::Cli;

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
    let mut ctx = repo::discover()?;

    if !cli.cmd.skips_preflight() {
        ctx.preflight(cli.cmd.is_mutating())?;
    }

    if !cli.cmd.skips_lazy_refresh() && refresh::policy_is_lazy(&ctx)? {
        // A failing refresh must never take the actual command down with it.
        if let Err(err) = refresh::refresh(&ctx, true) {
            eprintln!("warning: refresh failed: {err:#}");
        }
    }

    commands::dispatch(&mut ctx, cli.cmd)
}
