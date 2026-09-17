use crate::hooks;
use crate::repo::{Context, LOCAL, REMOTE, REMOTE_REF};
use crate::task::{self, Entry};
use anyhow::Result;

/// Informational only: `status` never refreshes, it reports what a refresh or
/// a sync would have to do.
pub fn run(ctx: &mut Context) -> Result<()> {
    let head = ctx.main.out(&["rev-parse", "--short", LOCAL])?;
    let policy = ctx
        .get_cfg("yman.refresh")?
        .unwrap_or_else(|| "lazy".to_string());
    let hooks_state = if hooks::all_installed(ctx)? {
        "installed"
    } else {
        "not installed"
    };
    println!(".yman  {LOCAL} @ {head}   (refresh: {policy}, hooks: {hooks_state})");

    match ctx.main.rev_parse(REMOTE)? {
        None => println!("remote: {REMOTE_REF} not fetched yet"),
        Some(_) => {
            let short = ctx.main.out(&["rev-parse", "--short", REMOTE])?;
            let counts = ctx.main.out(&[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{LOCAL}...{REMOTE}"),
            ])?;
            let mut parts = counts.split_whitespace();
            let ahead: usize = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let behind: usize = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let hint = if ahead > 0 || behind > 0 {
                "   → run: yman sync"
            } else {
                ""
            };
            println!("remote: {REMOTE_REF} @ {short}   ahead {ahead}, behind {behind}{hint}");
        }
    }

    let porcelain = ctx.wt.out(&["status", "--porcelain"])?;
    let dirty = porcelain.lines().filter(|l| !l.trim().is_empty()).count();
    if dirty == 0 {
        println!("worktree: clean");
    } else {
        println!("worktree: {dirty} uncommitted change(s) (will be snapshotted by next sync)");
    }

    if ctx.merge_in_progress() {
        let unmerged = ctx.wt.out(&["diff", "--name-only", "--diff-filter=U"])?;
        let files: Vec<&str> = unmerged.lines().filter(|l| !l.trim().is_empty()).collect();
        println!(
            "merge:   in progress, {} unmerged file(s): {}   → yman sync --continue | --abort",
            files.len(),
            files.join(", ")
        );
    }

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
    let mut parts: Vec<String> = counts
        .iter()
        .map(|(s, n)| format!("{s} {n}"))
        .collect();
    if other > 0 {
        parts.push(format!("other {other}"));
    }
    let broken_note = if broken > 0 {
        format!("   ({broken} broken)")
    } else {
        String::new()
    };
    println!("tasks:   {}{broken_note}", parts.join(", "));
    Ok(())
}
