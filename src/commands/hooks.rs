use crate::cli::{HooksAction, HooksArgs};
use crate::hooks;
use crate::repo::Context;
use anyhow::Result;

pub fn run(ctx: &mut Context, a: HooksArgs) -> Result<()> {
    match a.action {
        HooksAction::Install => hooks::install(ctx),
        HooksAction::Remove => hooks::remove(ctx),
        HooksAction::Status => hooks::report(ctx),
    }
}
