//! Picking up task commits that are already in the object store — after a
//! plain `git fetch`/`git pull`, or a hook firing one.
//!
//! Refresh never talks to the network and never rewrites local work: it only
//! fast-forwards, and only when that is trivially safe.
//!
//! Nearly every command runs this first, so the order of the checks is chosen
//! to keep the cheap ones in front: the two refs are read off disk, and each
//! question that costs a `git` process is asked only once the answers before
//! it say it still matters.

use crate::repo::{Context, LOCAL, REMOTE};
use anyhow::Result;

pub struct RefreshReport {
    pub applied: usize,
    pub skipped: Option<&'static str>,
}

impl RefreshReport {
    fn nothing() -> RefreshReport {
        RefreshReport {
            applied: 0,
            skipped: None,
        }
    }

    fn skip(why: &'static str) -> RefreshReport {
        RefreshReport {
            applied: 0,
            skipped: Some(why),
        }
    }
}

/// What the state of the two refs asks for.
enum Step {
    /// Nothing to pick up.
    Nothing,
    /// There is something, but taking it is not ours to decide.
    Skip(&'static str),
    /// `REMOTE` is strictly ahead; `LOCAL` can be fast-forwarded onto it.
    Behind,
}

/// Is the lazy policy in effect? Unset counts as lazy.
fn policy_is_lazy(ctx: &Context) -> Result<bool> {
    Ok(matches!(
        ctx.get_cfg("yman.refresh")?.as_deref(),
        None | Some("lazy")
    ))
}

fn triage(ctx: &Context) -> Result<Step> {
    // Both come off disk, so a repository with nothing to do answers here for
    // free.
    let (Some(local), Some(remote)) = (ctx.resolve_ref(LOCAL)?, ctx.resolve_ref(REMOTE)?) else {
        return Ok(Step::Nothing); // nothing fetched yet
    };
    if local == remote {
        return Ok(Step::Nothing);
    }
    if !ctx.main.is_ancestor(LOCAL, REMOTE)? {
        // Diverged or ahead: only `sync` may decide what happens next.
        return Ok(Step::Skip("local has unpushed commits; run: yman sync"));
    }
    Ok(Step::Behind)
}

/// The refresh that runs before a command, subject to `yman.refresh`.
///
/// The policy is read last on purpose: it is a `git config` call, and it only
/// changes the outcome for a repository that is genuinely behind. A repository
/// that is up to date, or one holding unpushed work, never pays for it.
pub fn lazy(ctx: &Context, quiet: bool) -> Result<RefreshReport> {
    match triage(ctx)? {
        Step::Nothing => Ok(RefreshReport::nothing()),
        Step::Skip(why) => Ok(RefreshReport::skip(why)),
        Step::Behind if policy_is_lazy(ctx)? => fast_forward(ctx, quiet),
        Step::Behind => Ok(RefreshReport::nothing()),
    }
}

/// The refresh `yman refresh` asks for, which the policy does not gate.
pub fn refresh(ctx: &Context, quiet: bool) -> Result<RefreshReport> {
    match triage(ctx)? {
        Step::Nothing => Ok(RefreshReport::nothing()),
        Step::Skip(why) => Ok(RefreshReport::skip(why)),
        Step::Behind => fast_forward(ctx, quiet),
    }
}

/// Only reached with `REMOTE` known to be strictly ahead of `LOCAL`.
fn fast_forward(ctx: &Context, quiet: bool) -> Result<RefreshReport> {
    if ctx.wt.is_dirty()? {
        if !quiet {
            eprintln!("note: .yman has uncommitted changes, refresh skipped");
        }
        return Ok(RefreshReport::skip("worktree has uncommitted changes"));
    }

    let n: usize = ctx
        .main
        .out(&["rev-list", "--count", &format!("{LOCAL}..{REMOTE}")])?
        .trim()
        .parse()
        .unwrap_or(0);
    ctx.wt.ok(&["merge", "-q", "--ff-only", REMOTE])?;
    if !quiet {
        eprintln!("refreshed: {n} new commit(s)");
    }
    Ok(RefreshReport {
        applied: n,
        skipped: None,
    })
}
