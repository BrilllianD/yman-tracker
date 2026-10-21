use crate::cli::LogArgs;
use crate::ids;
use crate::repo::{Context, LOCAL};
use crate::task;
use anyhow::Result;

pub fn run(ctx: &mut Context, a: LogArgs) -> Result<()> {
    let out = match &a.id {
        None => ctx
            .wt
            .out(&["log", "--oneline", "-n", &a.number.to_string(), LOCAL])?,
        Some(id) => {
            let t = task::find(&ctx.ydir, id)?;
            let mut lines: Vec<String> = Vec::new();
            task_log(ctx, &t.folder.id, LOCAL, a.number, &mut lines)?;
            lines.join("\n")
        }
    };
    if !out.is_empty() {
        println!("{out}");
    }
    Ok(())
}

/// Append up to `n` lines of the log of task `id`, as reached from `tip`.
///
/// The id is in the folder name through every rename a task goes through —
/// a retitle, a priority change, a close — so a pathspec on the id alone
/// finds all of them without git's rename detection, and `-n` stops the walk
/// as soon as enough commits are found. Only a sync collision renumber
/// changes the id, and it says so in its subject.
fn task_log(ctx: &Context, id: &str, tip: &str, n: usize, lines: &mut Vec<String>) -> Result<()> {
    if lines.len() >= n {
        return Ok(());
    }
    let limit = (n - lines.len()).to_string();
    let mut args: Vec<String> = vec![
        "log".into(),
        "--format=%h%x09%H%x09%s".into(),
        "-n".into(),
        limit,
        tip.into(),
    ];
    // A renumber moved another task *off* this id. That task was not in the
    // merge base, so everything it did under this id, the renumber included,
    // is reachable from the renumber, and nothing of the task that kept the
    // id is.
    for commit in renumbered_away(ctx, id, tip)? {
        args.push(format!("^{commit}"));
    }
    // The wrapper passes `--noglob-pathspecs`, so these two are the only
    // globbing pathspecs yman sends, and `GIT_LITERAL_PATHSPECS` from the
    // user's environment is stripped before it can turn them off.
    args.push("--".into());
    let pat = glob_escape(id);
    args.push(format!(":(glob)[0-9].{pat}.*/**"));
    args.push(format!(":(glob)*/[0-9].{pat}.*/**"));

    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = ctx.wt.out(&refs)?;
    let mut earlier: Option<(String, String)> = None;
    for line in out.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(short), Some(full), Some(subject)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        lines.push(format!("{short} {subject}"));
        // The task arrived here under another id; its history before that
        // is the renumber's parent's, under the old one.
        if let Some((old, _)) = ids::renumber_pairs(subject)
            .into_iter()
            .find(|(_, new)| new == id)
        {
            earlier = Some((old, format!("{full}^")));
        }
    }
    match earlier {
        Some((old, parent)) => task_log(ctx, &old, &parent, n, lines),
        None => Ok(()),
    }
}

/// Every sync renumber reachable from `tip` that moved a task off `id`.
/// Matching subjects is a walk with no diffs, which is cheap next to the
/// rename detection it replaces.
fn renumbered_away(ctx: &Context, id: &str, tip: &str) -> Result<Vec<String>> {
    let out = ctx.wt.out(&[
        "log",
        "--format=%H%x09%s",
        &format!("--grep=^{}", ids::RENUMBER_PREFIX),
        tip,
    ])?;
    Ok(out
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(_, subject)| {
            ids::renumber_pairs(subject)
                .iter()
                .any(|(old, _)| old == id)
        })
        .map(|(commit, _)| commit.to_string())
        .collect())
}

/// An id is `[^./\\]+`, which leaves room for glob metacharacters in a
/// hand-made folder name.
fn glob_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '*' | '?' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_glob_metacharacters() {
        assert_eq!(glob_escape("iv-2"), "iv-2");
        assert_eq!(glob_escape("a*b?[c]"), "a\\*b\\?\\[c\\]");
    }
}
