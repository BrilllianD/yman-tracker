//! Command dispatch. One module per subcommand.

pub mod add;
pub mod attach;
pub mod comment;
pub mod edit;
pub mod git;
pub mod guide;
pub mod hooks;
pub mod init;
pub mod log;
pub mod ls;
pub mod path;
pub mod refresh;
pub mod rm;
pub mod set;
pub mod show;
pub mod status;
pub mod sync;

use crate::cli::Cmd;
use crate::repo::Context;
use anyhow::Result;

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
        let a = a.to_string_lossy().trim().to_string();
        if !a.is_empty() {
            return Ok(a);
        }
    }
    Ok(ctx
        .get_cfg("user.name")?
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string()))
}

pub fn dispatch(ctx: &mut Context, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Init(a) => init::run(ctx, a),
        Cmd::Add(a) => add::run(ctx, a),
        Cmd::Ls(a) => ls::run(ctx, a),
        Cmd::Show(a) => show::run(ctx, a),
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
        Cmd::Status => status::run(ctx),
        Cmd::Guide => guide::run(),
        Cmd::Refresh(a) => refresh::run(ctx, a),
        Cmd::Hooks(a) => hooks::run(ctx, a),
        Cmd::Sync(a) => sync::run(ctx, a),
        Cmd::Git(a) => git::run(ctx, a),
    }
}
