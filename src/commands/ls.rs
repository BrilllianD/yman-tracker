use crate::cli::LsArgs;
use crate::repo::Context;
use crate::task::{self, Entry, Task, format_ts};
use anyhow::{Result, bail};
use std::io::IsTerminal;

pub fn run(ctx: &mut Context, a: LsArgs) -> Result<()> {
    for s in &a.statuses {
        if !ctx.config().has_status(s) {
            bail!(
                "unknown status \"{s}\"; allowed: {}",
                ctx.config().statuses_joined()
            );
        }
    }

    let mut tasks: Vec<Task> = Vec::new();
    let mut broken: Vec<(String, String)> = Vec::new();

    for entry in task::list(&ctx.ydir)? {
        match entry {
            Entry::Broken { dir, error } => {
                let name = dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                broken.push((name, error));
            }
            Entry::Task(t) => {
                let keep_status = if a.statuses.is_empty() {
                    a.all || !ctx.config().is_terminal(&t.meta.status)
                } else {
                    a.statuses.contains(&t.meta.status)
                };
                let keep_tags = a.tags.iter().all(|want| t.meta.tags.contains(want));
                if keep_status && keep_tags {
                    tasks.push(t);
                }
            }
        }
    }

    tasks.sort_by(|x, y| {
        x.priority()
            .cmp(&y.priority())
            .then_with(|| {
                ctx.config()
                    .status_index(&x.meta.status)
                    .cmp(&ctx.config().status_index(&y.meta.status))
            })
            .then_with(|| task::cmp_id(x.id(), y.id()))
    });

    if a.json {
        print_json(&tasks, &broken);
    } else {
        print_table(&tasks, &broken);
    }
    Ok(())
}

fn print_table(tasks: &[Task], broken: &[(String, String)]) {
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
    for row in &rows {
        println!("{}", join_row(row, &width));
    }
}

/// Columns are padded by character count; good enough for the Latin and
/// Cyrillic titles this is meant for, and never worse than raw bytes.
fn display_width(s: &str) -> usize {
    s.chars().count()
}

fn join_row(row: &[String; 7], width: &[usize; 7]) -> String {
    let mut out = String::new();
    // Trailing empty columns are dropped so short rows stay short.
    let last = row.iter().rposition(|c| !c.is_empty()).unwrap_or(0);
    for (i, cell) in row.iter().enumerate().take(last + 1) {
        if i > 0 {
            out.push_str("  ");
        }
        out.push_str(cell);
        if i < last {
            for _ in display_width(cell)..width[i] {
                out.push(' ');
            }
        }
    }
    out
}

fn count(n: usize) -> String {
    if n == 0 { String::new() } else { n.to_string() }
}

fn print_json(tasks: &[Task], broken: &[(String, String)]) {
    let mut items: Vec<String> = Vec::new();
    for t in tasks {
        items.push(format!(
            "{{\"id\":{},\"priority\":{},\"status\":{},\"title\":{},\"tags\":[{}],\
             \"assignee\":{},\"created\":{},\"updated\":{},\"attachments\":{},\
             \"comments\":{},\"dir\":{}}}",
            js(t.id()),
            t.priority(),
            js(&t.meta.status),
            js(&t.title),
            t.meta
                .tags
                .iter()
                .map(|s| js(s))
                .collect::<Vec<_>>()
                .join(","),
            match &t.meta.assignee {
                Some(a) => js(a),
                None => "null".to_string(),
            },
            js(&format_ts(&t.meta.created)),
            js(&format_ts(&t.meta.updated)),
            t.attachment_count(),
            t.comment_count(),
            js(&t.rel()),
        ));
    }
    for (name, error) in broken {
        items.push(format!("{{\"dir\":{},\"error\":{}}}", js(name), js(error)));
    }
    println!("[{}]", items.join(","));
}

/// Minimal JSON string escaping; no serde_json in the dependency list.
fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
