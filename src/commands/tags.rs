//! `yman tags`: what tags exist, and how many tasks carry each.
//!
//! An inventory, not a task list, so it counts every task on disk including
//! the closed ones `ls` hides by default. Like `ls` it reads the filesystem
//! and never asks git.

use crate::cli::TagsArgs;
use crate::json;
use crate::repo::Context;
use crate::tags;
use anyhow::Result;
use std::io::IsTerminal;

pub fn run(ctx: &mut Context, a: TagsArgs) -> Result<()> {
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

    if a.json {
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
