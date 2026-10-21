use crate::repo::Context;
use crate::task::{self, Task};
use anyhow::Result;

use super::editor::open_editor;

pub fn run(ctx: &mut Context, id: &str) -> Result<()> {
    let before = task::find(&ctx.ydir, id)?;
    open_editor(&before.dir().join(task::MD_FILE))?;

    // A refused title takes the same path as an unparsable file: the text
    // stays on disk for the user to fix, nothing is committed.
    let loaded =
        task::load(&ctx.ydir, &before.dir()).and_then(|t| task::clean_title(&t.title).map(|_| t));
    let mut t = loaded.map_err(|e| {
        anyhow::anyhow!("t.md invalid after edit: {e:#}; fix the file then run: yman edit {id}")
    })?;

    let rel_md = format!("{}/{}", t.rel(), task::MD_FILE);
    if ctx.wt.run(&["diff", "--quiet", "--", &rel_md])?.ok() {
        println!("no changes");
        return Ok(());
    }

    if t.title != before.title {
        rename_for_title(ctx, &mut t)?;
    }
    t.touch();
    t.write_meta()?;
    ctx.wt.ok(&["add", "--", &t.rel()])?;
    ctx.wt.commit(&format!("task({id}): edit"))?;
    println!("edited {id}  {}", t.rel());
    Ok(())
}

/// Re-slug a task whose title changed and move the folder with `git mv`.
pub fn rename_for_title(ctx: &Context, t: &mut Task) -> Result<()> {
    let max = ctx.config().slug.max_bytes;
    let slug = task::slugify(&t.title, max);
    if slug == t.folder.slug {
        return Ok(());
    }
    let old = t.rel();
    t.folder.slug = slug;
    t.relocate(Some(&ctx.wt), &old)?;
    Ok(())
}
