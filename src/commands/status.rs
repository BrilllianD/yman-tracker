use crate::cli::StatusArgs;
use crate::hooks;
use crate::json;
use crate::repo::{Context, LOCAL, REMOTE, REMOTE_REF};
use crate::task::{self, Entry};
use anyhow::Result;

/// Everything both output forms report, gathered once. The text arm is pinned
/// by `tests/cli.rs`, so it renders from here rather than printing as it goes.
struct Report {
    head: String,
    policy: String,
    hooks_installed: bool,
    /// `None` until the remote ref has been fetched at least once.
    remote: Option<Remote>,
    dirty: usize,
    /// `Some(unmerged files)` while a sync merge is unresolved.
    merge: Option<Vec<String>>,
    counts: Vec<(String, usize)>,
    other: usize,
    broken: usize,
}

struct Remote {
    head: String,
    ahead: usize,
    behind: usize,
}

/// Informational only: `status` never refreshes, it reports what a refresh or
/// a sync would have to do.
pub fn run(ctx: &mut Context, a: StatusArgs) -> Result<()> {
    let report = collect(ctx)?;
    if a.json {
        print_json(&report);
    } else {
        print_text(&report);
    }
    Ok(())
}

fn collect(ctx: &mut Context) -> Result<Report> {
    let head = ctx.main.out(&["rev-parse", "--short", LOCAL])?;
    let policy = ctx
        .get_cfg("yman.refresh")?
        .unwrap_or_else(|| "lazy".to_string());
    let hooks_installed = hooks::all_installed(ctx)?;

    let remote = match ctx.main.rev_parse(REMOTE)? {
        None => None,
        Some(_) => {
            let head = ctx.main.out(&["rev-parse", "--short", REMOTE])?;
            let counts = ctx.main.out(&[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{LOCAL}...{REMOTE}"),
            ])?;
            let mut parts = counts.split_whitespace();
            let ahead: usize = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let behind: usize = parts.next().unwrap_or("0").parse().unwrap_or(0);
            Some(Remote {
                head,
                ahead,
                behind,
            })
        }
    };

    let porcelain = ctx.wt.out(&["status", "--porcelain"])?;
    let dirty = porcelain.lines().filter(|l| !l.trim().is_empty()).count();

    let merge = if ctx.merge_in_progress() {
        let unmerged = ctx.wt.out(&["diff", "--name-only", "--diff-filter=U"])?;
        Some(
            unmerged
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string)
                .collect(),
        )
    } else {
        None
    };

    let mut counts: Vec<(String, usize)> = ctx
        .config()
        .statuses
        .list
        .iter()
        .map(|s| (s.clone(), 0usize))
        .collect();
    let mut other = 0usize;
    let mut broken = 0usize;
    for entry in task::list(&ctx.ydir)? {
        match entry {
            Entry::Broken { .. } => broken += 1,
            Entry::Task(t) => match counts.iter_mut().find(|(s, _)| *s == t.meta.status) {
                Some((_, n)) => *n += 1,
                None => other += 1,
            },
        }
    }

    Ok(Report {
        head,
        policy,
        hooks_installed,
        remote,
        dirty,
        merge,
        counts,
        other,
        broken,
    })
}

fn print_text(r: &Report) {
    let hooks_state = if r.hooks_installed {
        "installed"
    } else {
        "not installed"
    };
    println!(
        ".yman  {LOCAL} @ {}   (refresh: {}, hooks: {hooks_state})",
        r.head, r.policy
    );

    match &r.remote {
        None => println!("remote: {REMOTE_REF} not fetched yet"),
        Some(rm) => {
            let hint = if rm.ahead > 0 || rm.behind > 0 {
                "   → run: yman sync"
            } else {
                ""
            };
            println!(
                "remote: {REMOTE_REF} @ {}   ahead {}, behind {}{hint}",
                rm.head, rm.ahead, rm.behind
            );
        }
    }

    if r.dirty == 0 {
        println!("worktree: clean");
    } else {
        println!(
            "worktree: {} uncommitted change(s) (will be snapshotted by next sync)",
            r.dirty
        );
    }

    if let Some(files) = &r.merge {
        println!(
            "merge:   in progress, {} unmerged file(s): {}   → yman sync --continue | --abort",
            files.len(),
            files.join(", ")
        );
    }

    let mut parts: Vec<String> = r.counts.iter().map(|(s, n)| format!("{s} {n}")).collect();
    if r.other > 0 {
        parts.push(format!("other {}", r.other));
    }
    let broken_note = if r.broken > 0 {
        format!("   ({} broken)", r.broken)
    } else {
        String::new()
    };
    println!("tasks:   {}{broken_note}", parts.join(", "));
}

fn print_json(r: &Report) {
    let local = json::Object::new()
        .str("ref", LOCAL)
        .str("head", &r.head)
        .finish();
    let remote = match &r.remote {
        None => json::Object::new()
            .str("ref", REMOTE_REF)
            .raw("fetched", "false")
            .opt("head", None)
            .raw("ahead", "0")
            .raw("behind", "0")
            .finish(),
        Some(rm) => json::Object::new()
            .str("ref", REMOTE_REF)
            .raw("fetched", "true")
            .str("head", &rm.head)
            .raw("ahead", rm.ahead.to_string())
            .raw("behind", rm.behind.to_string())
            .finish(),
    };
    let worktree = json::Object::new()
        .raw("dirty", r.dirty.to_string())
        .finish();
    let merge = json::Object::new()
        .raw("in_progress", r.merge.is_some().to_string())
        .raw("unmerged", json::strings(r.merge.as_deref().unwrap_or(&[])))
        .finish();
    // The per-status counts live in their own object: a configured status
    // could be named `other` or `broken` and would otherwise collide.
    let mut by_status = json::Object::new();
    for (status, n) in &r.counts {
        by_status = by_status.raw(status, n.to_string());
    }
    let tasks = json::Object::new()
        .raw("by_status", by_status.finish())
        .raw("other", r.other.to_string())
        .raw("broken", r.broken.to_string())
        .finish();

    println!(
        "{}",
        json::Object::new()
            .raw("local", local)
            .raw("remote", remote)
            .str("refresh", &r.policy)
            .raw("hooks", r.hooks_installed.to_string())
            .raw("worktree", worktree)
            .raw("merge", merge)
            .raw("tasks", tasks)
            .finish()
    );
}
