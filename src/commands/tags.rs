//! `yman tags`: what tags exist, how many tasks carry each, and the two edits
//! that only make sense across every task at once.
//!
//! The listing is an inventory, not a task list, so it counts every task on
//! disk including the closed ones `ls` hides by default. Like `ls` it reads
//! the filesystem and never asks git.
//!
//! `rename` and `rm` are the reason the listing exists: a tag is a shared
//! vocabulary, and fixing a typo in it one `set --untag X --tag Y` at a time
//! is both tedious and a dozen commits. They are one commit for the lot —
//! renaming a tag is a single decision, and a half-applied rename would leave
//! the vocabulary in a state nobody chose.

use crate::cli::{TagRenameArgs, TagRmArgs, TagsAction, TagsArgs};
use crate::json;
use crate::repo::Context;
use crate::tags;
use crate::task::{self, Entry, Task};
use anyhow::{Result, bail};
use std::io::{BufRead, IsTerminal, Write};

pub fn run(ctx: &mut Context, a: TagsArgs) -> Result<()> {
    match a.action {
        None => list(ctx, a.json),
        Some(TagsAction::Rename(r)) => rename(ctx, r),
        Some(TagsAction::Rm(r)) => remove(ctx, r),
    }
}

fn list(ctx: &mut Context, as_json: bool) -> Result<()> {
    let inv = tags::counts(&ctx.ydir)?;
    if !inv.broken.is_empty() {
        // stdout is data: the count belongs on stderr, as in `ls`, where the
        // broken folders themselves are listed.
        eprintln!(
            "warning: skipped {} unreadable task folder(s): {}",
            inv.broken.len(),
            inv.broken.join(", ")
        );
    }

    if as_json {
        let items = inv.counts.iter().map(|(tag, n)| {
            json::Object::new()
                .str("tag", tag)
                .raw("tasks", n.to_string())
                .finish()
        });
        println!("{}", json::array(items));
        return Ok(());
    }
    print_table(&inv.counts);
    Ok(())
}

/// Two columns, padded to the widest tag. The header appears only on a
/// terminal, so `yman tags | while read ...` stays predictable — the same rule
/// `ls` follows.
fn print_table(counts: &[(String, usize)]) {
    if counts.is_empty() {
        return;
    }
    let mut width = counts
        .iter()
        .map(|(t, _)| t.chars().count())
        .max()
        .unwrap_or(0);
    if std::io::stdout().is_terminal() {
        width = width.max("TAG".len());
        println!("{:<width$}  N", "TAG", width = width);
    }
    for (tag, n) in counts {
        // Padded by character count, as `ls` pads its columns.
        let pad = width.saturating_sub(tag.chars().count());
        println!("{tag}{}  {n}", " ".repeat(pad));
    }
}

/// One task the walk is going to rewrite: the loaded task, and the line the
/// command prints for it.
struct Hit {
    task: Task,
    line: String,
}

/// Every task carrying `tag`, with `edit` applied to its tag list. `edit`
/// returns the `+added -removed` half of the printed line, or `None` when the
/// task turns out not to need touching after all.
fn walk(
    ctx: &Context,
    tag: &str,
    edit: impl Fn(&mut Vec<String>) -> Option<String>,
) -> Result<Vec<Hit>> {
    let mut hits: Vec<Hit> = Vec::new();
    let mut broken: Vec<String> = Vec::new();
    for entry in task::list(&ctx.ydir)? {
        let rel = entry.rel();
        match entry {
            // A folder that does not load cannot be rewritten either. Say so
            // rather than reporting a count that quietly excludes it.
            Entry::Broken { .. } => broken.push(rel),
            Entry::Task(mut t) => {
                if !t.meta.tags.iter().any(|have| tags::fold(have) == tag) {
                    continue;
                }
                if let Some(parts) = edit(&mut t.meta.tags) {
                    let line = format!("{}: tags {parts}", t.id());
                    hits.push(Hit { task: t, line });
                }
            }
        }
    }
    if !broken.is_empty() {
        eprintln!(
            "warning: skipped {} unreadable task folder(s): {}",
            broken.len(),
            broken.join(", ")
        );
    }
    Ok(hits)
}

/// Write, stage and commit the walk's hits as one change, then print a line
/// per task. Zero hits is `no changes` and no commit, as in `set`.
fn apply(ctx: &mut Context, hits: Vec<Hit>, subject: impl Fn(usize) -> String) -> Result<()> {
    if hits.is_empty() {
        println!("no changes");
        return Ok(());
    }
    for hit in &hits {
        let mut t = hit.task.clone();
        t.touch();
        t.write_meta()?;
        // No `git mv`: a tag never names the folder, so nothing relocates.
        ctx.wt.ok(&["add", "--", &t.rel()])?;
    }
    ctx.wt.commit(&subject(hits.len()))?;
    for hit in &hits {
        println!("{}", hit.line);
    }
    Ok(())
}

/// `7 tasks`, but `1 task`.
fn tasks(n: usize) -> String {
    if n == 1 {
        "1 task".to_string()
    } else {
        format!("{n} tasks")
    }
}

fn rename(ctx: &mut Context, a: TagRenameArgs) -> Result<()> {
    let old = tags::normalize(&a.old)?;
    let new = tags::normalize(&a.new)?;
    if old == new {
        println!("no changes");
        return Ok(());
    }
    let hits = walk(ctx, &old, |list| {
        let i = list.iter().position(|have| tags::fold(have) == old)?;
        if list.iter().any(|have| tags::fold(have) == new) {
            // The task already carries the destination: the rename is a
            // removal here, not a duplicate entry.
            list.remove(i);
            return Some(format!("-{old}"));
        }
        list[i] = new.clone();
        Some(format!("+{new} -{old}"))
    })?;
    apply(ctx, hits, |n| {
        format!("yman: tags rename {old} -> {new} ({})", tasks(n))
    })
}

fn remove(ctx: &mut Context, a: TagRmArgs) -> Result<()> {
    let tag = tags::normalize(&a.tag)?;
    let hits = walk(ctx, &tag, |list| {
        list.retain(|have| tags::fold(have) != tag);
        Some(format!("-{tag}"))
    })?;
    if hits.is_empty() {
        println!("no changes");
        return Ok(());
    }
    // Asked only once the count is known: confirming "remove ui" without
    // knowing it touches forty tasks is not consent. Same wording and same
    // refusal as `rm`.
    if !a.force {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to remove without -f");
        }
        print!("remove tag \"{tag}\" from {}? [y/N] ", tasks(hits.len()));
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().lock().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y") {
            bail!("aborted");
        }
    }
    apply(ctx, hits, |n| {
        format!("yman: tags remove {tag} ({})", tasks(n))
    })
}
