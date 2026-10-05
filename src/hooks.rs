//! Optional git hooks that pick up task commits arriving with a plain
//! `git pull`, so `.yman` is current without anyone running `yman`.

use crate::repo::Context;
use anyhow::{Result, bail};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

pub const MARKER: &str = "# yman-hook v1";
pub const HOOKS: [&str; 2] = ["post-merge", "post-checkout"];

const SCRIPT: &str = "#!/bin/sh\n\
                      # yman-hook v1\n\
                      command -v yman >/dev/null 2>&1 && yman refresh --quiet\n\
                      exit 0\n";

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum HookState {
    /// Written by us.
    Installed,
    /// Someone else's hook; we never touch it.
    Foreign,
    /// Something is there but we cannot tell whose it is: unreadable,
    /// not UTF-8, or a dangling symlink. Treated like `Foreign`, never
    /// replaced. Carries the reason.
    Unreadable(String),
    Absent,
}

impl HookState {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookState::Installed => "installed",
            HookState::Foreign => "foreign",
            HookState::Unreadable(_) => "unreadable",
            HookState::Absent => "absent",
        }
    }
}

/// Where hooks live for this repo, and whether `core.hooksPath` moved them.
///
/// When `core.hooksPath` is set, git itself resolves it — `~/` expansion and
/// a relative value meaning relative to the toplevel — through
/// `rev-parse --git-path hooks`. Its answer may be relative to the directory
/// git ran in, which is the main runner's, so it is joined onto that. The
/// main runner, not the `.yman` one: that one points `core.hooksPath` at
/// `/dev/null`. Unset or empty, the answer is `COMMON/hooks` without asking;
/// an empty value would make git print `./`, the toplevel.
pub fn hooks_dir(ctx: &Context) -> Result<(PathBuf, bool)> {
    match ctx.get_cfg("core.hooksPath")? {
        Some(p) if !p.is_empty() => {
            let raw = ctx.main.out(&["rev-parse", "--git-path", "hooks"])?;
            Ok((ctx.main.dir.join(raw), true))
        }
        _ => Ok((ctx.common.join("hooks"), false)),
    }
}

pub fn state(dir: &Path, name: &str) -> HookState {
    let path = dir.join(name);
    match std::fs::read(&path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) if text.lines().any(|l| l.trim() == MARKER) => HookState::Installed,
            Ok(_) => HookState::Foreign,
            Err(_) => HookState::Unreadable("not valid UTF-8".to_string()),
        },
        // A dangling symlink reads as "not found" too; writing to it would
        // create its target, so tell the two apart.
        Err(e) if e.kind() == ErrorKind::NotFound => match std::fs::symlink_metadata(&path) {
            Ok(_) => HookState::Unreadable("dangling symlink".to_string()),
            Err(_) => HookState::Absent,
        },
        Err(e) => HookState::Unreadable(e.to_string()),
    }
}

/// Write both hooks. Foreign and unreadable hooks are reported and left
/// alone; the command still processes the other hook before failing.
pub fn install(ctx: &Context) -> Result<()> {
    let (dir, custom) = hooks_dir(ctx)?;
    if custom {
        eprintln!("warning: installing into core.hooksPath={}", dir.display());
    }
    std::fs::create_dir_all(&dir)?;
    let mut refused: Vec<&str> = Vec::new();
    for name in HOOKS {
        let path = dir.join(name);
        match state(&dir, name) {
            HookState::Installed => println!("hook {name}: already installed"),
            HookState::Foreign => {
                eprintln!(
                    "note: hook {name} exists; add this line to it:\n    yman refresh --quiet"
                );
                refused.push(name);
            }
            HookState::Unreadable(why) => {
                eprintln!(
                    "warning: hook {name}: cannot read {}: {why}; not replacing it",
                    path.display()
                );
                refused.push(name);
            }
            HookState::Absent => {
                write_new(&path)?;
                set_executable(&path)?;
                println!("hook {name}: installed");
            }
        }
    }
    if !refused.is_empty() {
        bail!(
            "{} hook(s) not installed: {}",
            refused.len(),
            refused.join(", ")
        );
    }
    Ok(())
}

/// Create the hook, failing rather than following anything that appeared
/// at `path` since [`state`] looked.
fn write_new(path: &Path) -> Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(SCRIPT.as_bytes())?;
    Ok(())
}

fn warn_unreadable(path: &Path, why: &str) {
    eprintln!("warning: cannot read {}: {why}", path.display());
}

/// Delete only the hooks carrying our marker.
pub fn remove(ctx: &Context) -> Result<()> {
    let (dir, _) = hooks_dir(ctx)?;
    for name in HOOKS {
        match state(&dir, name) {
            HookState::Installed => {
                std::fs::remove_file(dir.join(name))?;
                println!("hook {name}: removed");
            }
            HookState::Foreign => println!("hook {name}: not ours, left alone"),
            HookState::Unreadable(why) => {
                warn_unreadable(&dir.join(name), &why);
                println!("hook {name}: unreadable, left alone");
            }
            HookState::Absent => println!("hook {name}: absent"),
        }
    }
    Ok(())
}

pub fn report(ctx: &Context) -> Result<()> {
    let (dir, _) = hooks_dir(ctx)?;
    for name in HOOKS {
        let st = state(&dir, name);
        if let HookState::Unreadable(why) = &st {
            warn_unreadable(&dir.join(name), why);
        }
        println!("hook {name}: {}", st.as_str());
    }
    Ok(())
}

/// Are both hooks ours? Used for the one-word summary in `init` and `status`.
pub fn all_installed(ctx: &Context) -> Result<bool> {
    let (dir, _) = hooks_dir(ctx)?;
    Ok(HOOKS.iter().all(|n| state(&dir, n) == HookState::Installed))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}
