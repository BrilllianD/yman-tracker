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
    /// Someone else's hook that already runs `yman refresh`, as the install
    /// note tells users to add; never touched, counted as installed.
    Chained,
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
            HookState::Chained => "chained",
            HookState::Unreadable(_) => "unreadable",
            HookState::Absent => "absent",
        }
    }
}

/// Where hooks live for this repo, and whether `core.hooksPath` moved them.
///
/// Unset means `COMMON/hooks`. Empty means git runs no hooks at all, so there
/// is nowhere to install: `None`. Any other value, a whitespace-only one
/// included, is a path git resolves itself — `~/` expansion and a relative
/// value meaning relative to the toplevel — through `rev-parse --git-path
/// hooks`. Its answer may be relative to the directory git ran in, which is
/// the main runner's, so it is joined onto that. The main runner, not the
/// `.yman` one: that one points `core.hooksPath` at `/dev/null`. The raw
/// value is read untrimmed, since whitespace can be part of the path.
pub fn hooks_dir(ctx: &Context) -> Result<Option<(PathBuf, bool)>> {
    let cfg = ctx.main.run(&["config", "--get", "core.hooksPath"])?;
    if !cfg.ok() {
        return Ok(Some((ctx.common.join("hooks"), false)));
    }
    if cfg
        .stdout
        .strip_suffix('\n')
        .unwrap_or(&cfg.stdout)
        .is_empty()
    {
        return Ok(None);
    }
    let out = ctx.main.ok(&["rev-parse", "--git-path", "hooks"])?;
    let raw = out.stdout.strip_suffix('\n').unwrap_or(&out.stdout);
    Ok(Some((ctx.main.dir.join(raw), true)))
}

const EMPTY_HOOKS_PATH: &str =
    "core.hooksPath is empty, which turns git hooks off; unset it or point it at a directory";

pub fn state(dir: &Path, name: &str) -> HookState {
    let path = dir.join(name);
    match std::fs::read(&path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) if text.lines().any(|l| l.trim() == MARKER) => HookState::Installed,
            Ok(text) if runs_refresh(&text) => HookState::Chained,
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

/// Does a non-comment line of `text` run `yman refresh`?
fn runs_refresh(text: &str) -> bool {
    text.lines().any(|l| {
        let l = l.trim();
        !l.starts_with('#') && l.contains("yman refresh")
    })
}

/// The first executable `name` in the directories of `PATH`.
pub fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| is_executable(p))
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Warn when hooks are installed but `yman` is not on `PATH`, so they do
/// nothing.
pub fn warn_if_yman_missing(installed: bool) {
    if installed && on_path("yman").is_none() {
        eprintln!("warning: yman is not on PATH, so the installed hooks do nothing");
    }
}

/// Write both hooks. Foreign and unreadable hooks are reported and left
/// alone; the command still processes the other hook before failing.
pub fn install(ctx: &Context) -> Result<()> {
    let Some((dir, custom)) = hooks_dir(ctx)? else {
        bail!(EMPTY_HOOKS_PATH)
    };
    if custom {
        eprintln!("warning: installing into core.hooksPath={}", dir.display());
    }
    std::fs::create_dir_all(&dir)?;
    let mut refused: Vec<&str> = Vec::new();
    let mut any_installed = false;
    for name in HOOKS {
        let path = dir.join(name);
        match state(&dir, name) {
            HookState::Installed => {
                any_installed = true;
                println!("hook {name}: already installed");
            }
            HookState::Chained => {
                any_installed = true;
                println!("hook {name}: already runs yman refresh");
            }
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
                any_installed = true;
                println!("hook {name}: installed");
            }
        }
    }
    warn_if_yman_missing(any_installed);
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

/// Delete only the hooks that are exactly the script we write.
pub fn remove(ctx: &Context) -> Result<()> {
    let Some((dir, _)) = hooks_dir(ctx)? else {
        bail!(EMPTY_HOOKS_PATH)
    };
    for name in HOOKS {
        match state(&dir, name) {
            HookState::Installed => {
                let path = dir.join(name);
                match std::fs::read(&path) {
                    Ok(bytes) if bytes == SCRIPT.as_bytes() => {
                        std::fs::remove_file(&path)?;
                        println!("hook {name}: removed");
                    }
                    Ok(_) => {
                        eprintln!(
                            "note: hook {name} carries the yman marker but was edited; \
                             delete it by hand: {}",
                            path.display()
                        );
                        println!("hook {name}: edited, left alone");
                    }
                    Err(e) => {
                        warn_unreadable(&path, &e.to_string());
                        println!("hook {name}: unreadable, left alone");
                    }
                }
            }
            HookState::Foreign | HookState::Chained => {
                println!("hook {name}: not ours, left alone")
            }
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
    let Some((dir, _)) = hooks_dir(ctx)? else {
        bail!(EMPTY_HOOKS_PATH)
    };
    let mut any_installed = false;
    for name in HOOKS {
        let st = state(&dir, name);
        if let HookState::Unreadable(why) = &st {
            warn_unreadable(&dir.join(name), why);
        }
        any_installed |= matches!(st, HookState::Installed | HookState::Chained);
        println!("hook {name}: {}", st.as_str());
    }
    warn_if_yman_missing(any_installed);
    Ok(())
}

/// Does every hook run yman, ours or chained? Used for the one-word summary in `init` and `status`.
pub fn all_installed(ctx: &Context) -> Result<bool> {
    let Some((dir, _)) = hooks_dir(ctx)? else {
        return Ok(false);
    };
    Ok(HOOKS
        .iter()
        .all(|n| matches!(state(&dir, n), HookState::Installed | HookState::Chained)))
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
