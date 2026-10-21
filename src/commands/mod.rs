//! Command dispatch. One module per subcommand.

pub mod add;
pub mod attach;
pub mod comment;
pub mod completions;
pub mod edit;
pub mod editor;
pub mod git;
pub mod guide;
pub mod hooks;
pub mod init;
pub mod log;
pub mod ls;
pub mod merge_driver;
pub mod path;
pub mod plan;
pub mod refresh;
pub mod rm;
pub mod set;
pub mod show;
pub mod state;
pub mod status;
pub mod sync;
pub mod table;
pub mod tags;

use crate::cli::Cmd;
use crate::repo::Context;
use anyhow::{Result, bail};
use std::io::{BufRead, IsTerminal, Write};

/// The confirmation `rm` and `tags rm` ask before removing anything: off a
/// terminal it refuses outright; on one it asks `<question>` plus the yes-or-no
/// suffix on stderr, and anything but `y`/`Y` refuses.
pub fn confirm_remove(question: &str) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        bail!("refusing to remove without -f");
    }
    // stderr: stdout is data, and a prompt is not.
    eprint!("{question} [y/N] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    if !matches!(answer.trim(), "y" | "Y") {
        bail!("aborted");
    }
    Ok(())
}

/// The stderr warning for task folders a walk over every task could not load;
/// nothing when `rels` is empty.
pub fn warn_skipped(rels: &[String]) {
    if !rels.is_empty() {
        eprintln!(
            "warning: skipped {} unreadable task folder(s): {}",
            rels.len(),
            rels.join(", ")
        );
    }
}

/// The keys every task object in `--json` output starts with, in this order:
/// `id, priority, status, title, tags, assignee`. Callers append the rest.
pub fn task_json(t: &crate::task::Task) -> crate::json::Object {
    crate::json::Object::new()
        .str("id", t.id())
        .raw("priority", t.priority().to_string())
        .str("status", &t.meta.status)
        .str("title", &t.title)
        .raw("tags", crate::json::strings(&t.meta.tags))
        .opt("assignee", t.meta.assignee.as_deref())
}

/// Body text for `add --body-file` and `set --body-file`: a path, or `-` for
/// stdin. Errors name the source, since "No such file" alone is useless in a
/// script's log.
pub fn read_text_source(path: &str) -> Result<String> {
    use std::io::Read;
    let mut buf = String::new();
    let res = if path == "-" {
        std::io::stdin().read_to_string(&mut buf).map(|_| ())
    } else {
        std::fs::File::open(path).and_then(|mut f| f.read_to_string(&mut buf).map(|_| ()))
    };
    match res {
        Ok(()) => Ok(buf),
        Err(e) => anyhow::bail!("cannot read {path}: {e}"),
    }
}

/// The rule every list field on `add` follows: first occurrence wins, order
/// kept. `set` reaches the same result through `apply_set`, and
/// `tags::normalize_all` through its own loop, because it has to fold case
/// before it can tell two values apart.
pub fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in values {
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// Refuse a value a user typed for an assignee, a link or a related id when it
/// holds an `is_unsafe_char` character. Stored values are never checked, and
/// the removing flags do not call this. The value is shown through `{:?}`, so
/// the terminal is not fed the raw character.
pub fn check_value(kind: &str, v: &str) -> Result<()> {
    if v.contains(crate::task::is_unsafe_char) {
        anyhow::bail!(
            "invalid {kind} {v:?}; {kind}s must not contain line breaks or other control characters"
        );
    }
    Ok(())
}

/// `--link` values of `add` and `set`, checked as [`check_value`] does.
pub fn check_links(values: &[String]) -> Result<()> {
    values.iter().try_for_each(|v| check_value("link", v))
}

/// `-a` on `add` and `set`: surrounding whitespace is dropped, and a value
/// that is blank or `-` after that means "unassigned", the same as leaving
/// it out. Anything else must pass [`check_value`].
pub fn assignee_arg(value: String) -> Result<Option<String>> {
    let v = value.trim();
    if crate::task::is_unassigned(v) {
        return Ok(None);
    }
    check_value("assignee", v)?;
    Ok(Some(v.to_string()))
}

/// `--relate` and `--waits-on` values: trimmed, deduplicated, and refused
/// when blank or when naming `own`, the task being changed. Neither could
/// ever match a folder, and a task waiting on itself waits on nothing.
pub fn related_args(values: Vec<String>, own: Option<&str>) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(values.len());
    for v in values {
        let v = v.trim();
        if v.is_empty() {
            anyhow::bail!("related id must not be empty");
        }
        if own == Some(v) {
            anyhow::bail!("task {v} cannot relate to itself");
        }
        check_value("related id", v)?;
        out.push(v.to_string());
    }
    Ok(dedupe(out))
}

/// Warn about every `--relate` target no folder here carries. Not a refusal:
/// a concurrent `add` on another clone can legitimately race this, and the id
/// becomes real the moment that clone's work arrives.
pub fn warn_unknown_related(ctx: &Context, wanted: &[String], already: &[String]) -> Result<()> {
    if wanted.is_empty() {
        return Ok(());
    }
    let known = crate::ids::fs_ids(&ctx.ydir)?;
    let mut warned: Vec<&String> = Vec::new();
    for id in wanted {
        if known.contains(id) || already.contains(id) || warned.contains(&id) {
            continue;
        }
        warned.push(id);
        eprintln!("warning: task {id} does not exist here; relating anyway");
    }
    Ok(())
}

/// Titles go into commit subjects verbatim except for double quotes, which
/// would fight with the quoting in the message template.
pub fn quote_title(title: &str) -> String {
    title.replace('"', "'")
}

/// Who a comment or attachment is recorded as coming from: `$YMAN_ACTOR`,
/// else `git config user.name`, else `unknown`. Only the display name in
/// `d.md` and `m.yml`; the git committer stays whoever git says it is, so an
/// agent working under a person's account is still attributable both ways.
pub fn actor(ctx: &Context) -> Result<String> {
    if let Some(a) = std::env::var_os("YMAN_ACTOR") {
        let a = crate::task::one_line(&a.to_string_lossy());
        if !a.is_empty() {
            return Ok(a);
        }
    }
    Ok(ctx
        .get_cfg("user.name")?
        .map(|s| crate::task::one_line(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string()))
}

/// Runs one command. When a command that writes to `.yman` fails, whatever it left
/// staged is taken back, and, if `.yman` had no uncommitted changes before the command
/// started, the files it wrote are put back to the last commit.
///
/// The hard reset and the clean only ever run when `.yman` was clean before this command
/// started (read here, after the per-clone lock is held and the lazy refresh has run), so
/// the only changes they discard are the ones this failed command wrote. (On a filesystem
/// that cannot lock, the command runs unlocked and the reset still runs.) Commits the
/// command already made (a multi-id verb, `add --sections`) are kept. `git clean` is run
/// without `-x`, so ignored files are never touched. `edit` is excluded from the hard
/// reset because its file holds the user's own text: it is only unstaged.
pub fn dispatch(ctx: &mut Context, cmd: Cmd) -> Result<()> {
    if !cmd.is_mutating() {
        return run_cmd(ctx, cmd);
    }
    let keep_edits = matches!(cmd, Cmd::Edit(_));
    let clean_before = match ctx.wt.is_dirty() {
        Ok(dirty) => !dirty,
        // Cannot tell: assume the tree held something of the user's.
        Err(_) => false,
    };
    let result = run_cmd(ctx, cmd);
    if result.is_err() {
        undo_uncommitted(ctx, clean_before && !keep_edits);
    }
    result
}

/// Takes back what a failed command left in `.yman`: with `restore_tree`, everything
/// uncommitted (a no-op when the tree is already clean); without it, only the index.
/// Never fails: a problem is a warning.
fn undo_uncommitted(ctx: &Context, restore_tree: bool) {
    let undone = if restore_tree {
        match ctx.wt.is_dirty() {
            Ok(false) => Ok(()),
            Ok(true) => ctx
                .wt
                .ok(&["reset", "-q", "--hard", "HEAD"])
                .and_then(|_| ctx.wt.ok(&["clean", "-fdq"]))
                .map(|_| ()),
            Err(e) => Err(e),
        }
    } else {
        ctx.wt.ok(&["reset", "-q"]).map(|_| ())
    };
    if let Err(e) = undone {
        eprintln!("warning: could not undo the failed change: {e:#}; run: yman status");
    }
}

fn run_cmd(ctx: &mut Context, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Init(a) => init::run(ctx, a),
        Cmd::Add(a) => add::run(ctx, a),
        Cmd::Ls(a) => ls::run(ctx, a),
        Cmd::Show(a) => show::run(ctx, a),
        Cmd::Plan(a) => plan::run(ctx, a),
        Cmd::Edit(a) => edit::run(ctx, &a.id),
        Cmd::Set(a) => set::run(ctx, a),
        Cmd::Start(a) => set::run_start(ctx, &a.ids, a.message),
        Cmd::Done(a) => set::run_done(ctx, &a.ids, a.message),
        Cmd::Move(a) => set::run_move(ctx, &a.ids, a.status, a.message),
        Cmd::Cancel(a) => set::run_cancel(ctx, &a.ids, a.message),
        Cmd::Reopen(a) => set::run_reopen(ctx, &a.ids, a.message),
        Cmd::Prio(a) => set::run_prio(ctx, &a.ids, a.priority, a.message),
        Cmd::Rm(a) => rm::run(ctx, a),
        Cmd::Attach(a) => attach::run(ctx, a),
        Cmd::Detach(a) => attach::run_detach(ctx, a),
        Cmd::Comment(a) => comment::run(ctx, a),
        Cmd::Path(a) => path::run(ctx, &a.id),
        Cmd::Log(a) => log::run(ctx, a),
        Cmd::Status(a) => status::run(ctx, a),
        Cmd::Tags(a) => tags::run(ctx, a),
        Cmd::State(a) => state::run(ctx, a),
        // `main` answers documentation commands and `merge-driver` before
        // discovery; they never reach dispatch.
        Cmd::Guide | Cmd::Completions(_) | Cmd::Man(_) | Cmd::MergeDriver(_) => {
            unreachable!("main answers documentation commands and merge-driver before discovery")
        }
        Cmd::Refresh(a) => refresh::run(ctx, a),
        Cmd::Hooks(a) => hooks::run(ctx, a),
        Cmd::Sync(a) => sync::run(ctx, a),
        Cmd::Git(a) => git::run(ctx, a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_value_names_the_kind_and_escapes_the_value() {
        for kind in ["assignee", "link", "related id"] {
            let err = check_value(kind, "a\u{202e}b").unwrap_err().to_string();
            assert_eq!(
                err,
                format!(
                    "invalid {kind} \"a\\u{{202e}}b\"; {kind}s must not contain line breaks or \
                     other control characters"
                )
            );
        }
        assert!(check_value("link", "https://x/y?z=1").is_ok());
    }

    #[test]
    fn values_reach_their_checks() {
        assert!(assignee_arg("a\nb".into()).is_err());
        assert_eq!(assignee_arg(" - ".into()).unwrap(), None);
        assert!(check_links(&["ok".into(), "x\u{200f}".into()]).is_err());
        assert!(related_args(vec!["2\u{202e}".into()], None).is_err());
    }
}
