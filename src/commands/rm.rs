use crate::cli::RmArgs;
use crate::plan;
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
        // stderr: stdout is data, and a prompt is not.
        eprint!("remove task {id} \"{}\"? [y/N] ", t.title);
        std::io::stderr().flush()?;
        let mut answer = String::new();
        std::io::stdin().lock().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y") {
            bail!("aborted");
        }
    }

    // Staged before the removal so both land in one commit: a task that is
    // gone and a reference that still names it must never be two states of
    // the repository anyone can observe.
    let (dropped, waiters, skipped) = drop_related(ctx, &id)?;
    ctx.wt.ok(&["rm", "-rq", "--", &t.rel()])?;
    ctx.wt
        .commit(&format!("task({id}): remove \"{}\"", quote_title(&t.title)))?;
    if !skipped.is_empty() {
        eprintln!(
            "warning: skipped {} unreadable task folder(s): {}",
            skipped.len(),
            skipped.join(", ")
        );
    }
    if dropped > 0 {
        eprintln!("note: dropped {dropped} reference(s) to {id}");
    }
    println!("removed {id}");
    // A removed blocker frees its waiters as a closed one does. Only an open,
    // non-epic task was blocking anyone, the rule a close applies.
    let blocked_others = !ctx.config().is_terminal(&t.meta.status) && !plan::is_epic(&t);
    let waiters: &[String] = if blocked_others { &waiters } else { &[] };
    plan::report_removed(ctx, waiters)
}

/// Drop every `related:` entry naming `id`, staged for the caller's commit.
/// Returns how many references went, the tasks that lost one, and the
/// folders that could not be read — rewriting one would mean parsing what we
/// already failed to parse.
fn drop_related(ctx: &Context, id: &str) -> Result<(usize, Vec<String>, Vec<String>)> {
    let mut dropped = 0;
    let mut waiters: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for entry in task::list(&ctx.ydir)? {
        let mut t = match entry {
            task::Entry::Task(t) => t,
            broken => {
                skipped.push(broken.rel());
                continue;
            }
        };
        // The task on its way out is about to be `git rm`'d wholesale.
        if t.id() == id {
            continue;
        }
        let before = t.meta.related.len();
        t.meta.related.retain(|r| r != id);
        if t.meta.related.len() == before {
            continue;
        }
        dropped += before - t.meta.related.len();
        waiters.push(t.id().to_string());
        t.touch();
        t.write_meta()?;
        ctx.wt.ok(&["add", "--", &t.rel()])?;
    }
    Ok((dropped, waiters, skipped))
}
