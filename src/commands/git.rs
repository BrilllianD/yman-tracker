use crate::cli::GitArgs;
use crate::repo::Context;
use anyhow::{Result, anyhow};
use std::process::Command;

/// Raw git passthrough inside `.yman`, exit code and all.
pub fn run(ctx: &mut Context, a: GitArgs) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(&ctx.ydir)
        .args(&a.args)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow!("git not found in PATH")
            } else {
                anyhow::Error::new(e).context("failed to run git")
            }
        })?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}
