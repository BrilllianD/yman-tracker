use crate::repo::Context;
use crate::task;
use anyhow::Result;

/// Absolute path and nothing else, so `cd $(yman path 14)` works.
pub fn run(ctx: &mut Context, id: &str) -> Result<()> {
    let t = task::find(&ctx.ydir, id)?;
    println!("{}", t.dir.display());
    Ok(())
}
