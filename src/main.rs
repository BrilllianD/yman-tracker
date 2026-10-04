mod autosync;
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
mod plan;
mod refresh;
mod refs;
mod repo;
mod sections;
mod tags;
mod task;
mod yml;

use clap::Parser;
use cli::{Cli, Cmd};

fn main() {
    restore_sigpipe();
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(errors::exit_code_for(&err));
        }
    }
}

/// Put SIGPIPE back to its default action, so `yman ls | head -1` dies
/// silently from the signal once `head` has gone, like git and coreutils do.
///
/// The Rust runtime sets SIGPIPE to "ignore" before `main`, which turns a
/// write to a closed pipe into an `EPIPE` error, and `println!` panics on
/// that. Children are unaffected: `std::process::Command` already resets
/// SIGPIPE to the default in every child it spawns. The one place yman itself
/// writes into a child, `Git::with_stdin`, only feeds `mktree` an empty tree;
/// a caller that streams real data there would now be killed, not handed an
/// error, if git exited before reading it all.
///
/// Declared by hand rather than through a crate: the numbers are the same on
/// Linux, macOS and the BSDs (SIGPIPE 13, SIG_DFL 0).
#[cfg(unix)]
fn restore_sigpipe() {
    const SIGPIPE: i32 = 13;
    const SIG_DFL: usize = 0;
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    // SAFETY: `signal` is async-signal-safe and called once, before any other
    // thread exists; SIG_DFL is a valid disposition for SIGPIPE.
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

#[cfg(not(unix))]
fn restore_sigpipe() {}

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
        ctx.preflight(cli.cmd.is_mutating(), cli.cmd.settles_merge())?;
    }

    if !cli.cmd.skips_lazy_refresh() {
        // A failing refresh must never take the actual command down with it.
        if let Err(err) = refresh::lazy(&ctx, true) {
            eprintln!("warning: refresh failed: {err:#}");
        }
    }

    let mutating = cli.cmd.is_mutating();
    commands::dispatch(&mut ctx, cli.cmd)?;

    if mutating {
        // The change is already committed; publishing it is a bonus, so a
        // failure here is a warning, never the command's exit code.
        if let Err(err) = autosync::after_mutation(&ctx) {
            eprintln!("warning: autosync failed: {err:#}; run: yman sync");
        }
    }
    Ok(())
}
