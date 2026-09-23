//! `yman completions <shell>` and `yman man`: generated from the same clap
//! definition the binary parses with, so neither can drift from the CLI.

use crate::cli::{Cli, CompletionsArgs, ManArgs};
use anyhow::Result;
use clap::{Command, CommandFactory};
use std::io::Write;

pub fn run(a: CompletionsArgs) -> Result<()> {
    // clap_complete offers hidden subcommands like any other, and nobody
    // should be tab-completed into `merge-driver`. The top level carries no
    // flags of its own beyond help and version, so rebuilding it from the
    // visible subcommands loses nothing.
    let full = Cli::command();
    let mut cmd = Command::new("yman")
        .version(env!("CARGO_PKG_VERSION"))
        .disable_help_subcommand(true)
        .subcommands(full.get_subcommands().filter(|s| !s.is_hide_set()).cloned());
    // Generated into a buffer: `generate` panics on a closed pipe, and a
    // plain write turns that into an ordinary error.
    let mut buf = Vec::new();
    clap_complete::generate(a.shell, &mut cmd, "yman", &mut buf);
    std::io::stdout().write_all(&buf)?;
    Ok(())
}

/// `yman.1` on stdout, whose SUBCOMMANDS section points at `yman-<name>(1)`;
/// `--dir` writes that page and every one it points at, hidden ones skipped.
pub fn run_man(a: ManArgs) -> Result<()> {
    match a.dir {
        Some(dir) => {
            std::fs::create_dir_all(&dir)
                .map_err(|e| anyhow::anyhow!("cannot create {}: {e}", dir.display()))?;
            clap_mangen::generate_to(Cli::command(), &dir)
                .map_err(|e| anyhow::anyhow!("cannot write man pages to {}: {e}", dir.display()))?;
        }
        None => clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout())?,
    }
    Ok(())
}
