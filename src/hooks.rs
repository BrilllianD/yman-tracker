//! Optional git hooks that pick up task commits arriving with a plain
//! `git pull`, so `.yman` is current without anyone running `yman`.

use crate::repo::Context;
use anyhow::{Result, bail};
use std::path::PathBuf;

pub const MARKER: &str = "# yman-hook v1";
pub const HOOKS: [&str; 2] = ["post-merge", "post-checkout"];

const SCRIPT: &str = "#!/bin/sh\n\
                      # yman-hook v1\n\
                      command -v yman >/dev/null 2>&1 && yman refresh --quiet\n\
                      exit 0\n";

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum HookState {
    /// Written by us.
    Installed,
    /// Someone else's hook; we never touch it.
    Foreign,
    Absent,
}

impl HookState {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookState::Installed => "installed",
            HookState::Foreign => "foreign",
            HookState::Absent => "absent",
        }
    }
}

/// Where hooks live for this repo, honouring `core.hooksPath`.
pub fn hooks_dir(ctx: &Context) -> Result<(PathBuf, bool)> {
    if let Some(p) = ctx.get_cfg("core.hooksPath")?
        && !p.trim().is_empty()
    {
        let p = PathBuf::from(p.trim());
        let p = if p.is_absolute() { p } else { ctx.root.join(p) };
        return Ok((p, true));
    }
    Ok((ctx.common.join("hooks"), false))
}

pub fn state(dir: &std::path::Path, name: &str) -> HookState {
    let path = dir.join(name);
    match std::fs::read_to_string(&path) {
        Err(_) => HookState::Absent,
        Ok(text) => {
            if text.lines().any(|l| l.trim() == MARKER) {
                HookState::Installed
            } else {
                HookState::Foreign
            }
        }
    }
}

/// Write both hooks. Foreign hooks are reported and left alone; the command
/// still processes the other hook before failing.
pub fn install(ctx: &Context) -> Result<()> {
    let (dir, custom) = hooks_dir(ctx)?;
    if custom {
        eprintln!("warning: installing into core.hooksPath={}", dir.display());
    }
    std::fs::create_dir_all(&dir)?;
    let mut foreign: Vec<&str> = Vec::new();
    for name in HOOKS {
        match state(&dir, name) {
            HookState::Installed => println!("hook {name}: already installed"),
            HookState::Foreign => foreign.push(name),
            HookState::Absent => {
                let path = dir.join(name);
                std::fs::write(&path, SCRIPT)?;
                set_executable(&path)?;
                println!("hook {name}: installed");
            }
        }
    }
    if !foreign.is_empty() {
        for name in &foreign {
            eprintln!("hook {name} exists; add this line to it:\n    yman refresh --quiet");
        }
        bail!(
            "{} hook(s) not installed: {}",
            foreign.len(),
            foreign.join(", ")
        );
    }
    Ok(())
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
            HookState::Absent => println!("hook {name}: absent"),
        }
    }
    Ok(())
}

pub fn report(ctx: &Context) -> Result<()> {
    let (dir, _) = hooks_dir(ctx)?;
    for name in HOOKS {
        println!("hook {name}: {}", state(&dir, name).as_str());
    }
    Ok(())
}

/// Are both hooks ours? Used for the one-word summary in `init` and `status`.
pub fn all_installed(ctx: &Context) -> Result<bool> {
    let (dir, _) = hooks_dir(ctx)?;
    Ok(HOOKS.iter().all(|n| state(&dir, n) == HookState::Installed))
}

#[cfg(unix)]
fn set_executable(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &std::path::Path) -> Result<()> {
    Ok(())
}
