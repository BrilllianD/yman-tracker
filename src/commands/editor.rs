use crate::signal::IgnoreInterrupts;
use anyhow::{Result, bail};
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

/// `$VISUAL`, else `$EDITOR`, else `vi`. Callers that create files before
/// opening the editor (`add -e`) resolve first so a refusal leaves nothing
/// behind.
pub fn resolve_editor() -> Result<String> {
    let configured = std::env::var("VISUAL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|s| !s.trim().is_empty())
        });
    // `vi` on a pipe does not fail, it waits — in an agent's shell that is
    // a timeout followed by a retry, the single most expensive way to lose.
    // Someone who set $EDITOR explicitly asked for whatever it does.
    if configured.is_none() && !std::io::stdin().is_terminal() {
        bail!("no terminal for vi; set $EDITOR, or use -m / --body-file");
    }
    Ok(configured.unwrap_or_else(|| "vi".to_string()))
}

/// Split on whitespace so `EDITOR="code --wait"` works.
pub fn open_editor(path: &Path) -> Result<()> {
    run_editor(path, "file left as is")
}

/// `open_editor`, with `aftermath` telling the user what a failed editor run
/// did to their work: `add -e` removes the folder it created, so it says
/// "task not added" rather than "file left as is".
pub fn run_editor(path: &Path, aftermath: &str) -> Result<()> {
    let spec = resolve_editor()?;
    let mut parts = spec.split_whitespace();
    // `resolve_editor` never returns a blank spec, so there is a first word.
    let program = parts.next().unwrap_or("vi");
    let mut child = Command::new(program)
        .args(parts)
        .arg(path)
        .spawn()
        .map_err(|e| anyhow::anyhow!("cannot run editor \"{program}\": {e}"))?;
    // After the spawn, not before: an ignored signal stays ignored across
    // exec, and the editor must keep the default dispositions (git's
    // `launch_editor` does the same). A Ctrl-C then stops the editor only, and
    // the cleanup of the caller still runs when it reports the signal.
    let interrupts = IgnoreInterrupts::new();
    let status = child.wait();
    drop(interrupts);
    let status =
        status.map_err(|e| anyhow::anyhow!("cannot wait for editor \"{program}\": {e}"))?;
    match status.code() {
        Some(0) => Ok(()),
        Some(n) => bail!("editor exited with status {n}; {aftermath}"),
        None => bail!("editor was killed by a signal; {aftermath}"),
    }
}
