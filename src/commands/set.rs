use crate::cli::SetArgs;
use crate::repo::Context;
use crate::task;
use anyhow::{Result, bail};

use super::quote_title;

/// One applied change: how it reads in the commit subject, and how it reads
/// on the user's terminal.
struct Change {
    token: String,
    line: String,
}

pub fn run(ctx: &mut Context, a: SetArgs) -> Result<()> {
    let mut t = task::find(&ctx.ydir, &a.id)?;
    let mut changes: Vec<Change> = Vec::new();
    let mut title_changed = false;
    let old_rel = t.rel();

    if let Some(status) = a.status {
        if !ctx.config().has_status(&status) {
            bail!(
                "unknown status \"{status}\"; allowed: {}",
                ctx.config().statuses_joined()
            );
        }
        if status != t.meta.status {
            changes.push(Change {
                token: format!("status={}->{}", t.meta.status, status),
                line: format!("status {} -> {}", t.meta.status, status),
            });
            t.meta.status = status;
        }
    }

    if let Some(p) = a.priority
        && p != t.folder.priority
    {
        changes.push(Change {
            token: format!("priority={}->{}", t.folder.priority, p),
            line: format!("priority {} -> {}", t.folder.priority, p),
        });
        t.folder.priority = p;
    }

    if let Some(title) = a.title {
        let title = title.trim().to_string();
        if title.is_empty() {
            bail!("title must not be empty");
        }
        if title != t.title {
            changes.push(Change {
                token: "title".to_string(),
                line: format!("title {} -> {}", t.title, title),
            });
            t.title = title;
            title_changed = true;
        }
    }

    if a.no_assignee {
        if let Some(old) = t.meta.assignee.take() {
            changes.push(Change {
                token: format!("assignee={old}->-"),
                line: format!("assignee {old} -> -"),
            });
        }
    } else if let Some(who) = a.assignee
        && t.meta.assignee.as_deref() != Some(who.as_str())
    {
        let old = t.meta.assignee.clone().unwrap_or_else(|| "-".into());
        changes.push(Change {
            token: format!("assignee={old}->{who}"),
            line: format!("assignee {old} -> {who}"),
        });
        t.meta.assignee = Some(who);
    }

    apply_set(&mut t.meta.tags, &a.tag, &a.untag, "tags", &mut changes);
    apply_set(&mut t.meta.links, &a.link, &a.unlink, "links", &mut changes);
    apply_set(
        &mut t.meta.related,
        &a.relate,
        &a.unrelate,
        "related",
        &mut changes,
    );

    if changes.is_empty() {
        println!("no changes");
        return Ok(());
    }

    if title_changed {
        t.folder.slug = task::slugify(&t.title, ctx.config().slug.max_bytes);
    }
    let new_rel = t.folder.to_string();
    if new_rel != old_rel {
        ctx.wt.ok(&["mv", &old_rel, &new_rel])?;
        t.dir = ctx.ydir.join(&new_rel);
    }

    t.touch();
    t.write_meta()?;
    if title_changed {
        t.write_md()?;
    }
    ctx.wt.ok(&["add", "--", &new_rel])?;
    let tokens: Vec<&str> = changes.iter().map(|c| c.token.as_str()).collect();
    ctx.wt.commit(&format!(
        "task({}): set {}",
        t.id(),
        quote_title(&tokens.join(" "))
    ))?;

    for c in &changes {
        println!("{}: {}", t.id(), c.line);
    }
    Ok(())
}

/// Add/remove on a list field, order preserved, removing an absent value is
/// not an error.
fn apply_set(
    field: &mut Vec<String>,
    add: &[String],
    remove: &[String],
    name: &str,
    changes: &mut Vec<Change>,
) {
    let mut added: Vec<String> = Vec::new();
    let mut removed: Vec<String> = Vec::new();
    for v in add {
        if !field.contains(v) {
            field.push(v.clone());
            added.push(v.clone());
        }
    }
    for v in remove {
        if let Some(i) = field.iter().position(|x| x == v) {
            field.remove(i);
            removed.push(v.clone());
        }
    }
    if added.is_empty() && removed.is_empty() {
        return;
    }
    let mut parts: Vec<String> = Vec::new();
    parts.extend(added.iter().map(|v| format!("+{v}")));
    parts.extend(removed.iter().map(|v| format!("-{v}")));
    changes.push(Change {
        token: format!("{name}={}", parts.join(",")),
        line: format!("{name} {}", parts.join(" ")),
    });
}

fn one(ctx: &mut Context, id: &str, status: Option<String>, priority: Option<u8>) -> Result<()> {
    run(
        ctx,
        SetArgs {
            id: id.to_string(),
            status,
            priority,
            title: None,
            assignee: None,
            no_assignee: false,
            tag: vec![],
            untag: vec![],
            link: vec![],
            unlink: vec![],
            relate: vec![],
            unrelate: vec![],
        },
    )
}

pub fn run_start(ctx: &mut Context, id: &str) -> Result<()> {
    let status = ctx.config().start_status().to_string();
    one(ctx, id, Some(status), None)
}

pub fn run_done(ctx: &mut Context, id: &str) -> Result<()> {
    let status = ctx.config().done_status().to_string();
    one(ctx, id, Some(status), None)
}

pub fn run_prio(ctx: &mut Context, id: &str, priority: u8) -> Result<()> {
    one(ctx, id, None, Some(priority))
}
