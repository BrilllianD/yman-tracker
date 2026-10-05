//! Thin wrapper around the `git` binary. No libgit2, no gix.
//!
//! Phase-0 spike result (`scripts/spike-symref.sh`, git 2.55.0): committing
//! inside a linked worktree whose HEAD is the symbolic ref `refs/yman/local`
//! moves that ref, HEAD stays symbolic, `git branch -a` never mentions it, and
//! `git merge --ff-only` against a ref built with `commit-tree` works. So the
//! *primary* path of the spec applies: plain `git commit`, and preflight
//! asserts `symbolic-ref HEAD == refs/yman/local`. No `update-ref` dance.

use anyhow::{Context as _, Result, anyhow, bail};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Result of one git invocation.
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(&self) -> bool {
        self.status == 0
    }
}

/// A git runner bound to one working directory.
#[derive(Clone, Debug)]
pub struct Git {
    pub dir: PathBuf,
    /// Point `core.hooksPath` at [`NO_HOOKS`] for every call.
    no_hooks: bool,
}

/// The `core.hooksPath` that holds no hooks. Git looks a hook up as
/// `<hooksPath>/<name>`; below a device file there is nothing to find, and git
/// treats that as "no hook" without a warning (on Windows the path names no
/// existing directory, with the same result). Unlike an empty directory under
/// the worktree's private git dir, it needs no setup and is usable before
/// `.yman` exists — `worktree add` fires `post-checkout` while creating it.
pub const NO_HOOKS: &str = "/dev/null";

/// Env vars that would leak repository state into our invocations when `yman`
/// is run from inside a git hook, alias, or filter: everything
/// `git rev-parse --local-env-vars` lists (a unit test keeps the two in step),
/// plus `GIT_NAMESPACE`, which silently rewrites every ref name.
///
/// `GIT_CONFIG_KEY_<n>` / `GIT_CONFIG_VALUE_<n>` cannot be listed, but git
/// reads none of them without `GIT_CONFIG_COUNT`, so stripping that one is
/// enough. `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM` and
/// `GIT_CEILING_DIRECTORIES` stay: they say which config and which repository
/// the user means, not where a hook happens to run. The list is static, not
/// asked of git at run time, because the spawn count is part of the contract.
pub const LEAKY_ENV: [&str; 16] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
];

impl Git {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Git {
            dir: dir.into(),
            no_hooks: false,
        }
    }

    /// The same runner with the user's hooks switched off.
    ///
    /// `--no-verify` only skips the pre-hooks; `post-commit`, `post-merge`,
    /// `post-checkout` and `post-rewrite` still fire, and in `.yman` they run
    /// with the task files as their working directory. A hook that writes
    /// files (husky, lefthook, LFS) would leave `.yman` dirty — refresh then
    /// skips forever and the next snapshot commits the junk. The `.yman`
    /// runner is built this way; the main one is not, so `yman hooks` and the
    /// user's own hooks keep working for the user's own git commands.
    pub fn without_hooks(&self) -> Self {
        Git {
            dir: self.dir.clone(),
            no_hooks: true,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.dir);
        // Non-ASCII paths come back unescaped; no color anywhere.
        cmd.args(["-c", "core.quotePath=false", "-c", "color.ui=never"]);
        if self.no_hooks {
            cmd.arg("-c").arg(format!("core.hooksPath={NO_HOOKS}"));
        }
        cmd.args(args);
        for key in LEAKY_ENV {
            cmd.env_remove(key);
        }
        // Control flow matches git's English messages ("couldn't find remote
        // ref", "nothing to commit"), so a localized git must not translate
        // them. Under the C locale gettext ignores `LANGUAGE` as well.
        cmd.env("LC_ALL", "C");
        cmd
    }

    fn spawn_err(err: std::io::Error) -> anyhow::Error {
        if err.kind() == std::io::ErrorKind::NotFound {
            anyhow!("git not found in PATH")
        } else {
            anyhow::Error::new(err).context("failed to run git")
        }
    }

    /// Run and capture both streams. A non-zero exit is *not* an error here;
    /// the caller inspects `status`.
    pub fn run(&self, args: &[&str]) -> Result<Output> {
        let out = self.command(args).output().map_err(Self::spawn_err)?;
        Ok(Output {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    /// Run and capture; a non-zero exit becomes an error carrying the stderr.
    pub fn ok(&self, args: &[&str]) -> Result<Output> {
        let out = self.run(args)?;
        if out.ok() {
            Ok(out)
        } else {
            Err(git_failure(args, &out))
        }
    }

    /// [`Git::ok`] returning the trimmed stdout.
    pub fn out(&self, args: &[&str]) -> Result<String> {
        Ok(self.ok(args)?.stdout.trim_end().to_string())
    }

    /// Run with `stdin` fed from `data` (`mktree`, `update-ref --stdin`).
    pub fn with_stdin(&self, args: &[&str], data: &str) -> Result<Output> {
        let mut child = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(Self::spawn_err)?;
        child
            .stdin
            .take()
            .expect("stdin was piped")
            .write_all(data.as_bytes())
            .context("failed writing to git stdin")?;
        let out = child.wait_with_output().context("failed waiting for git")?;
        let out = Output {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        };
        if out.ok() {
            Ok(out)
        } else {
            Err(git_failure(args, &out))
        }
    }

    /// Resolve a ref to a full hash; `None` when it does not exist.
    pub fn rev_parse(&self, r: &str) -> Result<Option<String>> {
        let out = self.run(&["rev-parse", "-q", "--verify", r])?;
        if out.ok() {
            Ok(Some(out.stdout.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    /// Is `a` an ancestor of `b`?
    pub fn is_ancestor(&self, a: &str, b: &str) -> Result<bool> {
        Ok(self.run(&["merge-base", "--is-ancestor", a, b])?.ok())
    }

    pub fn merge_base(&self, a: &str, b: &str) -> Result<Option<String>> {
        let out = self.run(&["merge-base", a, b])?;
        if out.ok() {
            Ok(Some(out.stdout.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    /// Does the worktree have uncommitted changes (tracked or untracked)?
    pub fn is_dirty(&self) -> Result<bool> {
        Ok(!self.out(&["status", "--porcelain"])?.trim().is_empty())
    }

    /// Commit whatever is staged. `--no-verify` so main-repo hooks (which may
    /// live under `core.hooksPath` and know nothing about `.yman`) cannot
    /// interfere; on the `.yman` runner [`Git::without_hooks`] keeps the
    /// post-hooks out too. GPG signing is deliberately left to the user's
    /// config.
    pub fn commit(&self, msg: &str) -> Result<()> {
        self.commit_with(&["commit", "-q", "--no-verify", "-m", msg])
    }

    /// Conclude a merge with git's own message, with the same failure
    /// handling (identity hint included) as `commit`.
    pub fn commit_merge(&self) -> Result<()> {
        self.commit_with(&["commit", "-q", "--no-verify", "--no-edit"])
    }

    fn commit_with(&self, args: &[&str]) -> Result<()> {
        let out = self.run(args)?;
        if out.ok() {
            return Ok(());
        }
        // Re-applying an identical change (`attach --force` with the same
        // bytes inside the same second) stages nothing; the tree already says
        // what the caller wanted, so that is a success, not a failure.
        if out.stdout.contains("nothing to commit") || out.stderr.contains("nothing to commit") {
            return Ok(());
        }
        if out.stderr.contains("Please tell me who you are") {
            bail!(
                "git identity missing; run: git config --global user.name \"…\" && git config --global user.email \"…\""
            );
        }
        Err(git_failure(&["commit"], &out))
    }
}

/// `git <subcommand> failed: <trimmed stderr>`
fn git_failure(args: &[&str], out: &Output) -> anyhow::Error {
    let sub = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .copied()
        .unwrap_or("git");
    let mut detail = out.stderr.trim().to_string();
    if detail.is_empty() {
        detail = out.stdout.trim().to_string();
    }
    if detail.is_empty() {
        detail = format!("exit status {}", out.status);
    }
    anyhow!("git {sub} failed: {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variable git itself calls repository-local is stripped.
    #[test]
    fn leaky_env_covers_local_env_vars() {
        let out = Command::new("git")
            .args(["rev-parse", "--local-env-vars"])
            .output()
            .expect("git runs");
        let listed = String::from_utf8(out.stdout).unwrap();
        assert!(!listed.trim().is_empty());
        for var in listed.lines() {
            assert!(LEAKY_ENV.contains(&var), "LEAKY_ENV misses {var}");
        }
    }
}
