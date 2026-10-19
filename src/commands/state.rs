//! `yman state`: one free-form markdown file, `.yman/state.md`, where an agent
//! keeps a short summary of the project's situation for the next session.
//!
//! It is a plain file at the top of the worktree, committed and synced like
//! everything else. Each `state set` replaces it whole; `yman log` has the
//! history, and two clones writing at once meet in an ordinary git merge.

use crate::cli::{StateAction, StateArgs, StateSetArgs};
use crate::repo::Context;
use anyhow::{Result, bail};

pub const STATE_FILE: &str = "state.md";

pub fn run(ctx: &mut Context, a: StateArgs) -> Result<()> {
    match a.action {
        None => show(ctx),
        Some(StateAction::Set(s)) => set(ctx, s),
    }
}

fn show(ctx: &Context) -> Result<()> {
    match std::fs::read_to_string(ctx.ydir.join(STATE_FILE)) {
        Ok(text) => {
            print!("{text}");
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("note: no project state yet; write one with yman state set -m <text>");
            Ok(())
        }
        Err(e) => bail!("cannot read {STATE_FILE}: {e}"),
    }
}

fn set(ctx: &mut Context, a: StateSetArgs) -> Result<()> {
    let text = match (a.message, a.body_file) {
        (Some(m), _) => m,
        (None, Some(path)) => super::read_text_source(&path)?,
        (None, None) => bail!("empty state"),
    };
    let text = text.trim();
    if text.is_empty() {
        bail!("empty state");
    }
    std::fs::write(ctx.ydir.join(STATE_FILE), format!("{text}\n"))?;
    ctx.wt.ok(&["add", "--", STATE_FILE])?;
    ctx.wt.commit("yman: state")?;
    println!("state updated");
    Ok(())
}
