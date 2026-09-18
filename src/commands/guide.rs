//! `yman guide`: the short manual for scripts and agents, compiled in so it
//! is available wherever the binary is, repository or not.

use anyhow::Result;

const GUIDE: &str = include_str!("../../docs/agents.md");

pub fn run() -> Result<()> {
    print!("{GUIDE}");
    Ok(())
}
