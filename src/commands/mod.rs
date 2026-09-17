//! Command dispatch. One module per subcommand.

pub mod add;
pub mod attach;
pub mod comment;
pub mod edit;
pub mod git;
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

/// Titles go into commit subjects verbatim except for double quotes, which
/// would fight with the quoting in the message template.
pub fn quote_title(title: &str) -> String {
    title.replace('"', "'")
}

pub fn dispatch(ctx: &mut Context, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Init(a) => init::run(ctx, a),
        Cmd::Add(a) => add::run(ctx, a),
        Cmd::Ls(a) => ls::run(ctx, a),
        Cmd::Show(a) => show::run(ctx, &a.id),
        Cmd::Edit(a) => edit::run(ctx, &a.id),
        Cmd::Set(a) => set::run(ctx, a),
        Cmd::Start(a) => set::run_start(ctx, &a.id),
        Cmd::Done(a) => set::run_done(ctx, &a.id),
        Cmd::Move(a) => set::run_move(ctx, &a.id, a.status),
        Cmd::Cancel(a) => set::run_cancel(ctx, &a.id),
        Cmd::Reopen(a) => set::run_reopen(ctx, &a.id),
        Cmd::Prio(a) => set::run_prio(ctx, &a.id, a.priority),
        Cmd::Rm(a) => rm::run(ctx, a),
        Cmd::Attach(a) => attach::run(ctx, a),
        Cmd::Detach(a) => attach::run_detach(ctx, a),
        Cmd::Comment(a) => comment::run(ctx, a),
        Cmd::Path(a) => path::run(ctx, &a.id),
        Cmd::Log(a) => log::run(ctx, a),
        Cmd::Status => status::run(ctx),
        Cmd::Refresh(a) => refresh::run(ctx, a),
        Cmd::Hooks(a) => hooks::run(ctx, a),
        Cmd::Sync(a) => sync::run(ctx, a),
        Cmd::Git(a) => git::run(ctx, a),
    }
}
