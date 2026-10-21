use crate::cli::LsArgs;
use crate::json;
use crate::plan;
use crate::repo::Context;
use crate::tags;
use crate::task::{self, Entry, Task, format_ts};
use anyhow::Result;
use std::io::IsTerminal;

use super::table::{cmp_tasks, display_width, join_row, print_body};

pub fn run(ctx: &mut Context, a: LsArgs) -> Result<()> {
    for s in &a.statuses {
        ctx.config().check_status(s)?;
    }

    // Folded on both sides, as `-q` already is: a tag yman wrote is lowercase
    // anyway, and one written into `m.yml` by hand should still be findable.
    let want_tags = tags::normalize_all(&a.tags)?;
    let needle = a.grep.as_deref().map(str::to_lowercase);
    let mut tasks: Vec<Task> = Vec::new();
    let mut broken: Vec<(String, String)> = Vec::new();

    for entry in task::list(&ctx.ydir)? {
        let rel = entry.rel();
        match entry {
            Entry::Broken { error, .. } => broken.push((rel, error)),
            Entry::Task(t) => {
                let keep_status = if a.statuses.is_empty() {
                    a.all || !ctx.config().is_terminal(&t.meta.status)
                } else {
                    a.statuses.contains(&t.meta.status)
                };
                let keep_tags = want_tags
                    .iter()
                    .all(|want| t.meta.tags.iter().any(|have| tags::fold(have) == *want));
                let keep_assignee = match a.assignee.as_deref() {
                    None => true,
                    Some("-") => t.meta.assignee.is_none(),
                    Some(who) => t.meta.assignee.as_deref() == Some(who),
                };
                let keep_priority = a.priority.is_none_or(|p| p == t.priority());
                // Exact match, like `--relate` writes it: an unknown id is an
                // empty listing, not a warning, so reads stay quiet. A task
                // never relates to itself.
                let keep_related = a
                    .related
                    .as_deref()
                    .is_none_or(|id| plan::relates_to(&t, id));
                // Plain substring, lowercased on both sides: no regex crate,
                // and an agent's query is a word or two, not a pattern.
                let keep_text = needle.as_deref().is_none_or(|n| {
                    t.title.to_lowercase().contains(n) || t.body.to_lowercase().contains(n)
                });
                if keep_status
                    && keep_tags
                    && keep_assignee
                    && keep_priority
                    && keep_related
                    && keep_text
                {
                    tasks.push(t);
                }
            }
        }
    }

    tasks.sort_by(|x, y| cmp_tasks(ctx.config(), x, y));
    if let Some(n) = a.limit {
        tasks.truncate(n);
    }

    if a.json {
        print_json(&tasks, &broken, a.long);
    } else {
        print_table(&tasks, &broken, a.long);
    }
    Ok(())
}

fn print_table(tasks: &[Task], broken: &[(String, String)], long: bool) {
    let mut rows: Vec<[String; 7]> = Vec::new();
    for t in tasks {
        rows.push([
            t.priority().to_string(),
            t.id().to_string(),
            t.meta.status.clone(),
            t.title.clone(),
            t.meta.tags.join(","),
            count(t.attachment_count()),
            count(t.comment_count()),
        ]);
    }
    for (name, error) in broken {
        rows.push([
            "!".into(),
            name.clone(),
            "broken".into(),
            error.clone(),
            String::new(),
            String::new(),
            String::new(),
        ]);
    }
    if rows.is_empty() {
        return;
    }

    let header = ["P", "ID", "STATUS", "TITLE", "TAGS", "F", "C"];
    let show_header = std::io::stdout().is_terminal();
    let mut width = [0usize; 7];
    if show_header {
        for (i, h) in header.iter().enumerate() {
            width[i] = h.len();
        }
    }
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            width[i] = width[i].max(display_width(cell));
        }
    }
    if show_header {
        println!("{}", join_row(&header.map(String::from), &width));
    }
    for (i, row) in rows.iter().enumerate() {
        println!("{}", join_row(row, &width));
        if long && let Some(t) = tasks.get(i) {
            print_body(&t.body);
        }
    }
}

fn count(n: usize) -> String {
    if n == 0 { String::new() } else { n.to_string() }
}

fn print_json(tasks: &[Task], broken: &[(String, String)], long: bool) {
    let mut items: Vec<String> = Vec::new();
    for t in tasks {
        let mut obj = super::task_json(t)
            .raw("links", json::strings(&t.meta.links))
            .raw("related", json::strings(&t.meta.related))
            .str("created", &format_ts(&t.meta.created))
            .str("updated", &format_ts(&t.meta.updated))
            .raw("attachments", t.attachment_count().to_string())
            .raw("comments", t.comment_count().to_string())
            .str("dir", &t.rel());
        if long {
            obj = obj.str("body", t.body.trim_matches('\n'));
        }
        items.push(obj.finish());
    }
    for (name, error) in broken {
        items.push(
            json::Object::new()
                .str("dir", name)
                .str("error", error)
                .finish(),
        );
    }
    println!("{}", json::array(items));
}
