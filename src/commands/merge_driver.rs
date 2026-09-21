//! `yman merge-driver <base> <ours> <theirs>` — the git merge driver for
//! `m.yml`, registered by `yman init` as `merge.ymanmeta.driver` and selected
//! by the `**/m.yml merge=ymanmeta` line in `.yman/.gitattributes`.
//!
//! Git runs this with three temporary files and reads the result back out of
//! `<ours>`. Exit 0 means merged, non-zero means conflict.
//!
//! It never builds a [`Context`](crate::repo::Context): a merge driver runs
//! with `MERGE_HEAD` present by definition, and preflight refuses that. `main`
//! dispatches it before discovery for the same reason `guide` is dispatched
//! there.

use crate::cli::MergeDriverArgs;
use crate::git::Git;
use crate::{merge, yml};
use anyhow::{Context as _, Result};

pub fn run(a: MergeDriverArgs) -> Result<bool> {
    let base = read(&a.base)?;
    let ours = read(&a.ours)?;
    let theirs = read(&a.theirs)?;

    // A hand-edited or half-resolved `m.yml` does not parse; so does the empty
    // file git passes for `%O` when the two sides share no ancestor. Neither is
    // an error — it is just not a case this driver can improve on.
    let parsed = (yml::parse(&base), yml::parse(&ours), yml::parse(&theirs));
    if let (Ok(base), Ok(ours), Ok(theirs)) = parsed
        && let Some(merged) = merge::three_way(&base, &ours, &theirs)
    {
        std::fs::write(&a.ours, yml::render(&merged))
            .with_context(|| format!("cannot write {}", a.ours))?;
        return Ok(true);
    }

    text_merge(&a)
}

fn read(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("cannot read {path}"))
}

/// What git would have done without the driver: labelled conflict markers in
/// `<ours>`. Keeping this as the fallback is what makes the field-wise merge
/// safe to turn on — a conflict it cannot settle looks exactly as it always
/// did, and `yman sync` reports it the same way.
fn text_merge(a: &MergeDriverArgs) -> Result<bool> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let out = Git::new(cwd).run(&[
        "merge-file",
        "-L",
        "ours",
        "-L",
        "base",
        "-L",
        "theirs",
        &a.ours,
        &a.base,
        &a.theirs,
    ])?;
    // `merge-file` exits with the number of conflicts, and with a negative
    // value — 255 by the time the shell has it — on a real failure.
    if !(0..=127).contains(&out.status) {
        eprint!("{}", out.stderr);
        anyhow::bail!("git merge-file failed");
    }
    Ok(out.status == 0)
}
