use crate::repo::Context;
use crate::task::{self, Task};
use anyhow::{Result, bail};
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

/// `$VISUAL`, else `$EDITOR`, else `vi`. Callers that create files before
/// opening the editor (`add -e`) resolve first so a refusal leaves nothing
/// behind.
pub fn resolve_editor() -> Result<String> {
    let configured = std::env::var("VISUAL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|s| !s.trim().is_empty())
        });
    // `vi` on a pipe does not fail, it waits — in an agent's shell that is
    // a timeout followed by a retry, the single most expensive way to lose.
    // Someone who set $EDITOR explicitly asked for whatever it does.
    if configured.is_none() && !std::io::stdin().is_terminal() {
        bail!("no terminal for vi; set $EDITOR, or use -m / --body-file");
    }
    Ok(configured.unwrap_or_else(|| "vi".to_string()))
}

/// Split on whitespace so `EDITOR="code --wait"` works.
pub fn open_editor(path: &Path) -> Result<()> {
    let spec = resolve_editor()?;
    let mut parts = spec.split_whitespace();
    let Some(program) = parts.next() else {
        bail!("no editor configured; set $EDITOR");
    };
    let status = Command::new(program)
        .args(parts)
        .arg(path)
        .status()
        .map_err(|e| anyhow::anyhow!("cannot run editor \"{program}\": {e}"))?;
    match status.code() {
        Some(0) => Ok(()),
        Some(n) => bail!("editor exited with status {n}; file left as is"),
        None => bail!("editor was killed by a signal; file left as is"),
    }
}

pub fn run(ctx: &mut Context, id: &str) -> Result<()> {
    let before = task::find(&ctx.ydir, id)?;
    open_editor(&before.dir.join(task::MD_FILE))?;

    let mut t = task::load(&ctx.ydir, &before.dir).map_err(|e| {
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
    let new = t.folder.to_string();
    ctx.wt.ok(&["mv", &old, &new])?;
    t.dir = ctx.ydir.join(&new);
    Ok(())
}
