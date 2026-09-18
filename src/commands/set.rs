use crate::cli::SetArgs;
use crate::discussion;
use crate::repo::Context;
use crate::task;
use anyhow::{Result, bail};

use super::{actor, quote_title};

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

    // A comment is a change in its own right: `set 3 -m note` is `comment 3
    // -m note` through the one mutation path, so `done 3 -m note` is one
    // commit rather than two.
    let comment = match a.message {
        Some(m) => {
            if m.trim().is_empty() {
                bail!("empty comment");
            }
            changes.push(Change {
                token: "comment".to_string(),
                line: "commented".to_string(),
            });
            Some(m)
        }
        None => None,
    };

    if title_changed {
        t.folder.slug = task::slugify(&t.title, ctx.config().slug.max_bytes);
    }
    // Where the task belongs now. Computed before the "nothing changed" exit,
    // so a task sitting in the wrong place — a hand edit, a resolved merge, a
    // config that grew a terminal status — is put right by `yman move <id>
    // <its own status>` rather than needing a separate repair command.
    let old_parent = t.parent.clone();
    t.parent = ctx.config().archive_dir(&t.meta.status).map(str::to_string);
    let new_rel = t.rel();
    let relocating = new_rel != old_rel;

    if changes.is_empty() && !relocating {
        println!("no changes");
        return Ok(());
    }
    if changes.is_empty() {
        changes.push(Change {
            token: "folder".to_string(),
            line: format!("folder {old_rel} -> {new_rel}"),
        });
    }

    if relocating {
        // `git mv a b/a` fails outright when `b` does not exist.
        if let Some(p) = &t.parent {
            std::fs::create_dir_all(ctx.ydir.join(p))?;
        }
        ctx.wt.ok(&["mv", "--", &old_rel, &new_rel])?;
        t.dir = ctx.ydir.join(&new_rel);
        if old_parent != t.parent {
            // Git does not track directories, so the one we left behind is an
            // untracked leftover. Non-recursive: it fails harmlessly while
            // other tasks are still in there.
            if let Some(p) = &old_parent {
                let _ = std::fs::remove_dir(ctx.ydir.join(p));
            }
            // stdout is data. Someone who had cd'd into the folder needs to
            // hear that it moved, but `yman path` must stay pipeable.
            eprintln!("note: task folder is now {new_rel}");
        }
    }

    if let Some(text) = comment {
        discussion::append_entry(
            &t.dir.join(task::DISCUSSION_FILE),
            task::now(),
            &actor(ctx)?,
            text.trim(),
        )?;
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

/// One `set` per id, one commit each, in the order given. The first failure
/// stops the loop; tasks before it are already committed, which is the
/// honest outcome — each is a complete change on its own.
fn each(
    ctx: &mut Context,
    ids: &[String],
    status: Option<String>,
    priority: Option<u8>,
    message: Option<String>,
) -> Result<()> {
    for id in ids {
        run(
            ctx,
            SetArgs {
                id: id.clone(),
                status: status.clone(),
                priority,
                message: message.clone(),
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
        )?;
    }
    Ok(())
}

pub fn run_start(ctx: &mut Context, ids: &[String], message: Option<String>) -> Result<()> {
    let status = ctx.config().start_status().to_string();
    each(ctx, ids, Some(status), None, message)
}

pub fn run_done(ctx: &mut Context, ids: &[String], message: Option<String>) -> Result<()> {
    let status = ctx.config().done_status().to_string();
    each(ctx, ids, Some(status), None, message)
}

/// `move` is a positional spelling of `set --status`; an unknown status is
/// rejected by `run` with the wording every other command uses.
pub fn run_move(
    ctx: &mut Context,
    ids: &[String],
    status: String,
    message: Option<String>,
) -> Result<()> {
    each(ctx, ids, Some(status), None, message)
}

pub fn run_cancel(ctx: &mut Context, ids: &[String], message: Option<String>) -> Result<()> {
    // No fallback: guessing which of several closed statuses means "gave up"
    // is exactly the positional cleverness the roles exist to remove.
    let Some(status) = ctx.config().cancel_status().map(str::to_string) else {
        bail!("no cancel status configured; set statuses.cancel in .yman/config.toml");
    };
    each(ctx, ids, Some(status), None, message)
}

pub fn run_reopen(ctx: &mut Context, ids: &[String], message: Option<String>) -> Result<()> {
    let status = ctx.config().statuses.default.clone();
    for id in ids {
        let t = task::find(&ctx.ydir, id)?;
        if !ctx.config().is_terminal(&t.meta.status) {
            bail!(
                "task {id} is not closed (status \"{}\"); closed statuses: {}",
                t.meta.status,
                ctx.config().terminal_joined()
            );
        }
        each(
            ctx,
            std::slice::from_ref(id),
            Some(status.clone()),
            None,
            message.clone(),
        )?;
    }
    Ok(())
}

pub fn run_prio(
    ctx: &mut Context,
    ids: &[String],
    priority: u8,
    message: Option<String>,
) -> Result<()> {
    each(ctx, ids, None, Some(priority), message)
}
