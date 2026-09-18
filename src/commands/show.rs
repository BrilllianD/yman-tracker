use crate::cli::ShowArgs;
use crate::discussion;
use crate::repo::Context;
use crate::task::{self, format_ts};
use anyhow::Result;

pub fn run(ctx: &mut Context, a: ShowArgs) -> Result<()> {
    let t = task::find(&ctx.ydir, &a.id)?;

    println!("{}  {}", t.id(), t.title);
    println!(
        "priority: {}   status: {}   assignee: {}   tags: {}",
        t.priority(),
        t.meta.status,
        t.meta.assignee.as_deref().unwrap_or("-"),
        if t.meta.tags.is_empty() {
            "-".to_string()
        } else {
            t.meta.tags.join(", ")
        }
    );
    println!(
        "created: {}   updated: {}",
        format_ts(&t.meta.created),
        format_ts(&t.meta.updated)
    );
    if !t.meta.links.is_empty() {
        println!("links:    {}", t.meta.links.join(", "));
    }
    if !t.meta.related.is_empty() {
        println!("related:  {}", t.meta.related.join(", "));
    }
    println!("folder:   {}", ctx.display_path(&t.dir));

    let body = t.body.trim_matches('\n');
    if !body.is_empty() {
        println!("\n{body}");
    }

    if !t.meta.attachments.is_empty() {
        println!("\nattachments:");
        for at in &t.meta.attachments {
            println!(
                "  {}   (added {} by {})",
                at.name,
                format_ts(&at.added),
                at.by
            );
        }
    }

    let dpath = t.dir.join(task::DISCUSSION_FILE);
    if let Ok(text) = std::fs::read_to_string(&dpath) {
        let mut entries = discussion::parse(&text);
        let total = entries.len();
        // Only the trimmed case changes the header, so the default output
        // stays byte-identical. Raw chunks count as entries.
        let trimmed = a.comments.is_some_and(|n| n < total);
        if trimmed {
            entries.drain(..total - a.comments.unwrap_or(0));
        }
        if !entries.is_empty() {
            if trimmed {
                println!("\ndiscussion (last {} of {total}):", entries.len());
            } else {
                println!("\ndiscussion:");
            }
            for e in entries {
                match e {
                    discussion::Entry::Comment { ts, author, text } => {
                        println!("  {ts} — {author}");
                        for line in text.lines() {
                            println!("    {line}");
                        }
                    }
                    discussion::Entry::Raw(raw) => {
                        for line in raw.lines() {
                            println!("    {line}");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
