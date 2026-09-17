use crate::cli::RmArgs;
use crate::repo::Context;
use crate::task;
use anyhow::{Result, bail};
use std::io::{BufRead, IsTerminal, Write};

use super::quote_title;

pub fn run(ctx: &mut Context, a: RmArgs) -> Result<()> {
    let t = task::find(&ctx.ydir, &a.id)?;
    let id = t.id().to_string();

    if !a.force {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to remove without -f");
        }
        print!("remove task {id} \"{}\"? [y/N] ", t.title);
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().lock().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y") {
            bail!("aborted");
        }
    }

    ctx.wt.ok(&["rm", "-rq", "--", &t.rel()])?;
    ctx.wt
        .commit(&format!("task({id}): remove \"{}\"", quote_title(&t.title)))?;
    println!("removed {id}");
    Ok(())
}
