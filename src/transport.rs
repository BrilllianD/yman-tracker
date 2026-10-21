//! The network half of yman, shared by `sync`, `init` and autosync.
//! `refs/yman/local` is pushed by sha, and `refs/yman/remote` is fetched
//! without touching `FETCH_HEAD`.

use crate::repo::{Context, FETCH_REFSPEC, LOCAL, REMOTE, REMOTE_REF};
use anyhow::{Result, bail};

/// A successful push carries the commit it published: the caller counts and
/// reports from that, never from a fresh read of `LOCAL`, which a concurrent
/// command may have moved since.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PushResult {
    Ok(String),
    UpToDate(String),
    Rejected,
}

/// How many times `sync` (and `init`, publishing a fresh tracker) pushes
/// into a moving origin before giving up.
pub const PUSH_ATTEMPTS: u32 = 3;

/// `git push origin refs/yman/local:refs/tasks/main`, always explicit — we
/// never set `remote.origin.push`, which would hijack the user's plain
/// `git push`.
///
/// `--porcelain` gives one machine-readable line per ref on stdout, so the
/// outcome is read from git's verdict on `refs/tasks/main` rather than guessed
/// from the human-readable stderr, which also carries whatever origin's hooks
/// printed.
///
/// `LOCAL` is resolved once and that commit is pushed by sha. Pushing the ref
/// by name and then running `update-ref REMOTE LOCAL` read `LOCAL` twice, so
/// a commit made in between was recorded as published when it was not, and
/// autosync's `REMOTE == LOCAL` check then skipped it for good.
pub fn push(ctx: &Context) -> Result<PushResult> {
    let Some(sha) = ctx.resolve_ref(LOCAL)? else {
        bail!("nothing to push: {LOCAL} does not exist");
    };
    let refspec = format!("{sha}:{REMOTE_REF}");
    // Git also moves `refs/yman/remote` here, through `remote.origin.fetch`;
    // the user's `reference-transaction` hook is not for that.
    let out = ctx.main.without_hooks().run(&[
        "push",
        "--porcelain",
        "--no-verify",
        "origin",
        &refspec,
    ])?;
    let status = ref_status(&out.stdout);
    if out.ok() {
        // Mirror what origin now has, so `status` and `refresh` agree.
        ctx.main.without_hooks().ok(&["update-ref", REMOTE, &sha])?;
        return Ok(match status {
            Some(('=', _)) => PushResult::UpToDate(sha),
            _ => PushResult::Ok(sha),
        });
    }
    if let Some(('!', summary)) = status
        && is_race(summary)
    {
        return Ok(PushResult::Rejected);
    }
    eprint!("{}", out.stderr);
    if let Some(('!', summary)) = status {
        // In porcelain mode git's reason goes to stdout; put it back where
        // the user looks, in the shape git's own report has, indented like any
        // continuation line.
        let line = match summary.split_once(" (") {
            Some((kind, reason)) => format!("{kind} {LOCAL} -> {REMOTE_REF} ({reason}"),
            None => format!("{summary} {LOCAL} -> {REMOTE_REF}"),
        };
        eprintln!("  ! {line}");
    }
    bail!("push failed")
}

/// The flag and summary of the `refs/tasks/main` line in `push --porcelain`
/// output: `<flag>\t<from>:<to>\t<summary>`, e.g.
/// `!\t<sha>:refs/tasks/main\t[rejected] (fetch first)`. `<from>` is the
/// pushed sha (older code pushed `refs/yman/local` by name).
fn ref_status(stdout: &str) -> Option<(char, &str)> {
    stdout.lines().find_map(|line| {
        let mut parts = line.splitn(3, '\t');
        let flag = parts.next()?;
        let (_, to) = parts.next()?.split_once(':')?;
        let summary = parts.next()?;
        let mut chars = flag.chars();
        let c = chars.next()?;
        (chars.next().is_none() && to == REMOTE_REF).then_some((c, summary))
    })
}

/// Does a rejected push's summary mean origin's ref moved under us — the race
/// a refetch and merge settles — rather than origin refusing the push?
///
/// Four reasons say exactly that and nothing else:
/// - `[rejected] (fetch first)`: origin's ref points at a commit we do not
///   have; someone pushed after our fetch.
/// - `[rejected] (non-fast-forward)`: the same, when we happen to hold that
///   commit already.
/// - `[remote rejected] (incorrect old value provided)`: the ref moved on the
///   server between its advertisement and its update, so receive-pack's own
///   compare-and-swap failed. A narrower window, but the same race.
/// - `[remote rejected] (reference already exists)`: the same compare-and-swap
///   when we push to create the ref (`init`, or after it vanished) and
///   someone created it first.
///
/// Everything else is a refusal, and retrying it would only repeat it and end
/// in a misleading `origin keeps moving`: `pre-receive hook declined`,
/// protected or denied refs, and the ambiguous `failed to update ref` and
/// `failed to lock`, which older servers report for a moved ref but also for
/// a full disk, a permission problem or a stale lock file.
fn is_race(summary: &str) -> bool {
    const RACE_REASONS: [&str; 4] = [
        "fetch first",
        "non-fast-forward",
        "incorrect old value provided",
        "reference already exists",
    ];
    summary
        .rsplit_once(" (")
        .and_then(|(_, reason)| reason.strip_suffix(')'))
        .is_some_and(|reason| RACE_REASONS.contains(&reason))
}

/// Fetch `refs/tasks/main` into `REMOTE`. Origin having no tasks ref is not
/// an error; if we still hold a `REMOTE` from an earlier fetch, it is stale
/// and goes, with a warning. Any other failure prints git's stderr and fails
/// with `failure_msg`.
pub fn fetch(ctx: &Context, failure_msg: &str) -> Result<()> {
    // `--no-write-fetch-head`: `FETCH_HEAD` in the main repository is the
    // user's, left by their own last fetch.
    // The user's hooks, a `reference-transaction` one included, are not for
    // yman's own refs.
    let out = ctx.main.without_hooks().run(&[
        "fetch",
        "--no-write-fetch-head",
        "origin",
        FETCH_REFSPEC,
    ])?;
    if out.ok() {
        return Ok(());
    }
    if out.stderr.contains("couldn't find remote ref") {
        // A failed fetch moves no ref, so asking now is the same as asking before.
        if ctx.resolve_ref(REMOTE)?.is_some() {
            ctx.main.without_hooks().ok(&["update-ref", "-d", REMOTE])?;
            eprintln!("warning: {REMOTE_REF} disappeared from origin; will recreate it");
        }
        return Ok(());
    }
    eprint!("{}", out.stderr);
    bail!("{failure_msg}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_status_finds_the_tasks_ref() {
        let out = "To /tmp/r.git\n=\trefs/yman/local:refs/tasks/main\t[up to date]\nDone\n";
        assert_eq!(ref_status(out), Some(('=', "[up to date]")));
        let out = "To /tmp/r.git\n \trefs/yman/local:refs/tasks/main\tabc..def\nDone\n";
        assert_eq!(ref_status(out), Some((' ', "abc..def")));
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let out = format!("To /tmp/r.git\n!\t{sha}:refs/tasks/main\t[rejected] (fetch first)\n");
        assert_eq!(ref_status(&out), Some(('!', "[rejected] (fetch first)")));
        assert_eq!(ref_status("To /tmp/r.git\nDone\n"), None);
        assert_eq!(
            ref_status("!\tHEAD:refs/heads/main\t[rejected] (fetch first)\n"),
            None
        );
    }

    #[test]
    fn only_a_moved_ref_counts_as_a_race() {
        assert!(is_race("[rejected] (fetch first)"));
        assert!(is_race("[rejected] (non-fast-forward)"));
        assert!(is_race("[remote rejected] (incorrect old value provided)"));
        assert!(is_race("[remote rejected] (reference already exists)"));
        assert!(!is_race("[remote rejected] (pre-receive hook declined)"));
        assert!(!is_race("[remote rejected] (failed to update ref)"));
        assert!(!is_race("[remote rejected] (failed to lock)"));
        assert!(!is_race("[remote rejected] (deny updating a hidden ref)"));
        assert!(!is_race("[rejected] (stale info)"));
        assert!(!is_race(
            "[remote failure] (remote failed to report status)"
        ));
    }
}
