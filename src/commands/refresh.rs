use crate::cli::RefreshArgs;
use crate::refresh;
use crate::repo::Context;
use anyhow::Result;

pub fn run(ctx: &mut Context, a: RefreshArgs) -> Result<()> {
    let report = refresh::refresh(ctx, a.quiet)?;
    if let Some(why) = report.skipped {
        if !a.quiet {
            eprintln!("note: {why}");
        }
    } else if report.applied == 0 && !a.quiet {
        println!("up to date");
    }
    Ok(())
}
