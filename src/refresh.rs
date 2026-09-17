//! Picking up task commits that are already in the object store — after a
//! plain `git fetch`/`git pull`, or a hook firing one.
//!
//! Refresh never talks to the network and never rewrites local work: it only
//! fast-forwards, and only when that is trivially safe.

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

/// Is the lazy policy in effect? Unset counts as lazy.
pub fn policy_is_lazy(ctx: &Context) -> Result<bool> {
    Ok(matches!(
        ctx.get_cfg("yman.refresh")?.as_deref(),
        None | Some("lazy")
    ))
}

pub fn refresh(ctx: &Context, quiet: bool) -> Result<RefreshReport> {
    let (Some(local), Some(remote)) = (ctx.main.rev_parse(LOCAL)?, ctx.main.rev_parse(REMOTE)?)
    else {
        return Ok(RefreshReport::nothing()); // nothing fetched yet
    };
    if local == remote {
        return Ok(RefreshReport::nothing());
    }
    if !ctx.main.is_ancestor(LOCAL, REMOTE)? {
        // Diverged or ahead: only `sync` may decide what happens next.
        return Ok(RefreshReport::skip("local has unpushed commits; run: yman sync"));
    }
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
