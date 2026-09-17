use crate::cli::CommentArgs;
use crate::discussion;
use crate::repo::Context;
use crate::task;
use anyhow::{Result, bail};
use std::io::{IsTerminal, Read};

use super::edit::open_editor;

pub fn run(ctx: &mut Context, a: CommentArgs) -> Result<()> {
    let mut t = task::find(&ctx.ydir, &a.id)?;

    let text = if let Some(m) = a.message {
        m
    } else if a.edit {
        let path = std::env::temp_dir().join(format!("yman-comment-{}.md", a.id));
        std::fs::write(&path, "")?;
        open_editor(&path)?;
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::remove_file(&path);
        text
    } else if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        bail!("empty comment")
    };

    if text.trim().is_empty() {
        bail!("empty comment");
    }

    let author = ctx
        .get_cfg("user.name")?
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    discussion::append_entry(
        &t.dir.join(task::DISCUSSION_FILE),
        task::now(),
        &author,
        text.trim(),
    )?;

    t.touch();
    t.write_meta()?;
    ctx.wt.ok(&["add", "--", &t.rel()])?;
    ctx.wt.commit(&format!("task({}): comment", t.id()))?;
    println!("commented on {}", t.id());
    Ok(())
}
