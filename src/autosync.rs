//! Publishing task commits as they are made, when `yman.autosync = push`.
//!
//! Runs after a mutating command has already committed, so nothing here may
//! fail the command: every outcome other than a clean push is a note or a
//! warning on stderr, and stdout stays the command's own.
//!
//! It only pushes, never merges. A merge started behind the user's back could
//! stop on a conflict and leave every later mutation exiting 3; when origin has
//! moved, the user gets pointed at `yman sync` instead.

use crate::commands::sync::{PushResult, push};
use crate::repo::{Context, LOCAL, REMOTE};
use anyhow::Result;

/// Is the push policy in effect? Unset, `off` or anything else counts as off.
fn policy_is_push(ctx: &Context) -> Result<bool> {
    Ok(ctx.get_cfg("yman.autosync")?.as_deref() == Some("push"))
}

/// The push that follows a successful mutating command.
pub fn after_mutation(ctx: &Context) -> Result<()> {
    if !policy_is_push(ctx)? || ctx.origin_url()?.is_none() {
        return Ok(());
    }
    let Some(local) = ctx.resolve_ref(LOCAL)? else {
        return Ok(());
    };
    let remote = ctx.resolve_ref(REMOTE)?;
    // `set` with nothing to change, or an editor closed without saving: no
    // new commit, so no reason to touch the network.
    if remote.as_deref() == Some(local.as_str()) {
        return Ok(());
    }
    match push(ctx)? {
        PushResult::Ok(sha) => {
            // Counted against the commit actually pushed, not a re-read of
            // `LOCAL`.
            let range = match &remote {
                Some(r) => format!("{r}..{sha}"),
                None => sha,
            };
            let n: usize = ctx
                .main
                .out(&["rev-list", "--count", &range])?
                .trim()
                .parse()
                .unwrap_or(0);
            eprintln!("note: pushed {n} task commit(s)");
        }
        PushResult::UpToDate(_) => {}
        PushResult::Rejected => {
            eprintln!("warning: origin has new task commits; run: yman sync")
        }
    }
    Ok(())
}
