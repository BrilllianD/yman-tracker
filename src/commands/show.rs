use crate::cli::ShowArgs;
use crate::discussion;
use crate::json;
use crate::repo::Context;
use crate::task::{self, Entry, Task, format_ts};
use anyhow::Result;

pub fn run(ctx: &mut Context, a: ShowArgs) -> Result<()> {
    let t = task::find(&ctx.ydir, &a.id)?;
    let related_by = related_by(&ctx.ydir, t.id())?;

    // Both output forms trim the discussion the same way: `-n` keeps the last
    // N entries in time order, and `total` is what the file held before
    // trimming.
    let dpath = t.dir.join(task::DISCUSSION_FILE);
    let text = std::fs::read_to_string(&dpath).unwrap_or_default();
    let mut entries = discussion::sort_by_time(discussion::parse(&text));
    let total = entries.len();
    // Only the trimmed case changes the header, so the default output
    // stays byte-identical. Raw chunks count as entries.
    let trimmed = a.comments.is_some_and(|n| n < total);
    if trimmed {
        entries.drain(..total - a.comments.unwrap_or(0));
    }

    if a.json {
        print_json(&t, &related_by, &entries, total);
    } else {
        print_text(ctx, &t, &related_by, &entries, total, trimmed);
    }
    Ok(())
}

/// The ids of every task whose `related` carries `id`: the other end of the
/// one-way relation, which an epic's steps point at. This is the one read of
/// every `m.yml` that `show` makes; `find` deliberately does not. Broken
/// folders cannot relate to anything and are skipped, as `rm` skips them.
fn related_by(ydir: &std::path::Path, id: &str) -> Result<Vec<String>> {
    let mut ids: Vec<String> = task::list(ydir)?
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Task(t) if t.id() != id && t.meta.related.iter().any(|r| r == id) => {
                Some(t.id().to_string())
            }
            _ => None,
        })
        .collect();
    ids.sort_by(|x, y| task::cmp_id(x, y));
    Ok(ids)
}

fn print_text(
    ctx: &Context,
    t: &Task,
    related_by: &[String],
    entries: &[discussion::Entry],
    total: usize,
    trimmed: bool,
) {
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
    if !related_by.is_empty() {
        println!("related by: {}", related_by.join(", "));
    }
    println!("folder:   {}", ctx.display_path(&t.dir));

    let body = t.body.trim_matches('\n');
    if !body.is_empty() {
        println!("\n{body}");
    }

    if !t.meta.attachments.is_empty() {
        println!("\nattachments:");
        for at in &t.meta.attachments {
            // The entry outlives the file if someone deletes it outside yman.
            let missing = if t.attachment_path(at).exists() {
                ""
            } else {
                " (missing)"
            };
            println!(
                "  {}   (added {} by {}){}",
                at.name,
                format_ts(&at.added),
                at.by,
                missing
            );
        }
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

/// One object. Unlike the text form, empty sections are emitted as empty
/// arrays rather than omitted: a script should not have to branch on a
/// missing key.
fn print_json(t: &Task, related_by: &[String], entries: &[discussion::Entry], total: usize) {
    let attachments = json::array(t.meta.attachments.iter().map(|at| {
        json::Object::new()
            .str("name", &at.name)
            .str("added", &format_ts(&at.added))
            .str("by", &at.by)
            .raw("missing", (!t.attachment_path(at).exists()).to_string())
            .finish()
    }));
    let discussion = json::array(entries.iter().map(|e| {
        match e {
            discussion::Entry::Comment { ts, author, text } => json::Object::new()
                .str("ts", ts)
                .str("author", author)
                .str("text", text)
                .finish(),
            discussion::Entry::Raw(raw) => json::Object::new().str("raw", raw).finish(),
        }
    }));

    println!(
        "{}",
        json::Object::new()
            .str("id", t.id())
            .raw("priority", t.priority().to_string())
            .str("status", &t.meta.status)
            .str("title", &t.title)
            .raw("tags", json::strings(&t.meta.tags))
            .opt("assignee", t.meta.assignee.as_deref())
            .raw("links", json::strings(&t.meta.links))
            .raw("related", json::strings(&t.meta.related))
            .raw("related_by", json::strings(related_by))
            .str("created", &format_ts(&t.meta.created))
            .str("updated", &format_ts(&t.meta.updated))
            .str("dir", &t.rel())
            .str("body", t.body.trim_matches('\n'))
            .raw("attachments", attachments)
            .raw("discussion", discussion)
            .raw("discussion_total", total.to_string())
            .finish()
    );
}
