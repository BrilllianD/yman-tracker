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
    /// Friendly names of the hooks git configuration defines
    /// (`hook.<name>.command`), read once per runner on first use, and only by
    /// a runner with hooks off.
    config_hooks: std::sync::Arc<std::sync::OnceLock<Vec<String>>>,
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

/// `-c` overrides on every call `yman` makes for itself, so the user's
/// configuration cannot reshape output yman parses or prints (`log`,
/// `status --porcelain`), hide files from it, or make its own merges fail.
/// Later `-c` beats earlier `-c` and every config file.
/// `merge.conflictStyle` is deliberately absent: the user keeps diff3 in the
/// files they resolve, and `is_marker` knows its `|||||||` line. So are
/// `commit.gpgSign` and `push.gpgSign`: yman's commits are signed when the
/// user's config says so.
const PINNED_CONFIG: [&str; 15] = [
    "core.quotePath=false",
    "color.ui=never",
    "color.diff=never",
    "color.status=never",
    "color.branch=never",
    "color.advice=never",
    "color.push=never",
    "color.remote=never",
    "color.transport=never",
    "log.decorate=no",
    "log.showSignature=false",
    "status.showUntrackedFiles=normal",
    "merge.ff=true",
    "merge.verifySignatures=false",
    "merge.log=false",
];

/// The user's pathspec mode. Stripped because every call passes
/// `--noglob-pathspecs`, which git refuses to combine with the glob mode,
/// and the literal and case-insensitive modes would change what a task or
/// attachment path matches. `yman git` keeps them.
const PATHSPEC_ENV: [&str; 3] = [
    "GIT_LITERAL_PATHSPECS",
    "GIT_GLOB_PATHSPECS",
    "GIT_ICASE_PATHSPECS",
];

impl Git {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Git {
            dir: dir.into(),
            no_hooks: false,
            config_hooks: Default::default(),
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
    ///
    /// `core.hooksPath` hides hook files only; hooks defined in configuration
    /// (git 2.54+) do not look there, so each configured name is also switched
    /// off with `hook.<name>.enabled=false`. The names are read once, lazily,
    /// so a command that never uses this runner (`ls`, `show`, `tags` in the
    /// steady state) spawns nothing extra.
    pub fn without_hooks(&self) -> Self {
        Git {
            dir: self.dir.clone(),
            no_hooks: true,
            config_hooks: self.config_hooks.clone(),
        }
    }

    fn configured_hooks(&self) -> Vec<String> {
        if let Some(names) = self.config_hooks.get() {
            return names.clone();
        }
        // A plain runner, so reading the list does not ask for the list.
        let out = Git::new(&self.dir).run(&[
            "config",
            "--name-only",
            "--get-regexp",
            r"^hook\..+\.command$",
        ]);
        let names: Vec<String> = match out {
            Ok(o) if o.ok() => {
                let mut names: Vec<String> = Vec::new();
                for line in o.stdout.lines() {
                    let name = line
                        .strip_prefix("hook.")
                        .and_then(|l| l.strip_suffix(".command"));
                    if let Some(name) = name
                        && !names.iter().any(|n| n == name)
                    {
                        names.push(name.to_string());
                    }
                }
                names
            }
            // Exit 1: no such key, the usual case.
            Ok(o) if o.status == 1 => Vec::new(),
            // The real command that follows reports the actual problem.
            _ => return Vec::new(),
        };
        let _ = self.config_hooks.set(names.clone());
        names
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.dir);
        // Paths are literal (`f/[ab].png` is one file); an explicit `:(glob)`
        // still globs, which `log <id>` relies on.
        cmd.arg("--noglob-pathspecs");
        for kv in PINNED_CONFIG {
            cmd.arg("-c").arg(kv);
        }
        if self.no_hooks {
            cmd.arg("-c").arg(format!("core.hooksPath={NO_HOOKS}"));
            for name in self.configured_hooks() {
                cmd.arg("-c").arg(format!("hook.{name}.enabled=false"));
            }
        }
        cmd.args(args);
        for key in LEAKY_ENV {
            cmd.env_remove(key);
        }
        for key in PATHSPEC_ENV {
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
        // Keep the write result, and always wait: a write that fails because
        // git already exited is git's failure to report, and returning early
        // would drop the child unwaited.
        let written = child
            .stdin
            .take()
            .expect("stdin was piped")
            .write_all(data.as_bytes())
            .context("failed writing to git stdin");
        let out = child.wait_with_output().context("failed waiting for git")?;
        let out = Output {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        };
        if !out.ok() {
            return Err(git_failure(args, &out));
        }
        written?;
        Ok(out)
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

    /// `git rev-list --count <range>`; 0 when git prints something unparsable.
    pub fn count(&self, range: &str) -> Result<usize> {
        Ok(self
            .out(&["rev-list", "--count", range])?
            .trim()
            .parse()
            .unwrap_or(0))
    }

    /// Number of non-blank `git status --porcelain` lines: uncommitted
    /// changes, tracked or untracked.
    pub fn dirty_count(&self) -> Result<usize> {
        Ok(self
            .out(&["status", "--porcelain"])?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count())
    }

    /// Does the worktree have uncommitted changes (tracked or untracked)?
    pub fn is_dirty(&self) -> Result<bool> {
        Ok(self.dirty_count()? > 0)
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
        // bytes inside the same second) stages nothing, even when an untracked
        // or unstaged file elsewhere in `.yman` changes git's wording; the
        // tree already says what the caller wanted, so that is a success, not
        // a failure.
        if nothing_staged(&out) {
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

/// Git found nothing staged. It says so three ways, depending on what else
/// is in the worktree: `nothing to commit` (clean), `nothing added to
/// commit but untracked files present`, `no changes added to commit`
/// (unstaged edits). The caller's change is already in the tree.
fn nothing_staged(out: &Output) -> bool {
    const PHRASES: [&str; 3] = [
        "nothing to commit",
        "nothing added to commit",
        "no changes added to commit",
    ];
    PHRASES
        .iter()
        .any(|p| out.stdout.contains(p) || out.stderr.contains(p))
}

/// `git <subcommand> failed: <stderr>`, continuation lines indented two
/// spaces so they read as part of the message (docs/errors.md, Streams).
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
    anyhow!("git {sub} failed: {}", indent_detail(&detail))
}

/// `text` trimmed, empty lines dropped, lines joined so each continuation
/// line is indented two spaces.
pub(crate) fn indent_detail(text: &str) -> String {
    text.trim()
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indent_detail_drops_blanks_and_indents_continuations() {
        assert_eq!(
            indent_detail("fatal: a\n\nhint: b  \n"),
            "fatal: a\n  hint: b"
        );
        assert_eq!(indent_detail(""), "");
    }

    #[test]
    fn nothing_staged_covers_every_phrasing() {
        let said = |stdout: &str| Output {
            status: 1,
            stdout: stdout.into(),
            stderr: String::new(),
        };
        assert!(nothing_staged(&said(
            "nothing to commit, working tree clean"
        )));
        assert!(nothing_staged(&said(
            "nothing added to commit but untracked files present"
        )));
        assert!(nothing_staged(&said("no changes added to commit")));
        let other = Output {
            status: 1,
            stdout: String::new(),
            stderr: "fatal: unable to auto-detect email address".into(),
        };
        assert!(!nothing_staged(&other));
    }

    /// Every call carries the literal-pathspec flag and every pin, ahead of
    /// the subcommand, and drops the user's pathspec-mode variables.
    #[test]
    fn command_pins_config_and_pathspecs() {
        let cmd = Git::new("/nonexistent").command(&["status"]);
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--noglob-pathspecs".to_string()), "{args:?}");
        for kv in PINNED_CONFIG {
            assert!(
                args.windows(2).any(|w| w[0] == "-c" && w[1] == kv),
                "{kv} missing from {args:?}"
            );
        }
        assert_eq!(args.last().map(String::as_str), Some("status"));
        let envs: Vec<_> = cmd.get_envs().collect();
        for key in PATHSPEC_ENV {
            assert!(
                envs.iter().any(|(k, v)| *k == key && v.is_none()),
                "{key} not removed"
            );
        }
    }

    /// Continuation lines of git's stderr are indented, blank ones dropped.
    #[test]
    fn git_failure_indents_continuation_lines() {
        let out = Output {
            status: 128,
            stdout: String::new(),
            stderr: "fatal: Unable to create 'x/index.lock': File exists.\n\nAnother git process seems to be running.\n".into(),
        };
        assert_eq!(
            git_failure(&["merge", "-q"], &out).to_string(),
            "git merge failed: fatal: Unable to create 'x/index.lock': File exists.\n  Another git process seems to be running."
        );
    }

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

    /// Hooks defined in configuration are listed by name, once.
    #[test]
    fn configured_hooks_are_switched_off() {
        // The body reads git configuration, so it runs in a child process
        // whose HOME and global config are empty, whatever the developer has.
        if std::env::var_os("YMAN_UNIT_SANDBOXED").is_none() {
            let home = tempfile::tempdir().unwrap();
            let gitconfig = home.path().join("gitconfig");
            std::fs::write(&gitconfig, "").unwrap();
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "git::tests::configured_hooks_are_switched_off",
                    "--test-threads=1",
                ])
                .env("YMAN_UNIT_SANDBOXED", "1")
                .env("HOME", home.path())
                .env("XDG_CONFIG_HOME", home.path().join(".config"))
                .env("GIT_CONFIG_GLOBAL", &gitconfig)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(
                out.status.success() && stdout.contains("test result: ok. 1 passed"),
                "stdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            );
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let git = Git::new(dir.path());
        git.ok(&["init", "-q"]).unwrap();
        git.ok(&["config", "hook.probe.command", "true"]).unwrap();
        git.ok(&["config", "--add", "hook.probe.event", "post-commit"])
            .unwrap();
        assert_eq!(git.without_hooks().configured_hooks(), ["probe"]);

        let empty = tempfile::tempdir().unwrap();
        let git = Git::new(empty.path());
        git.ok(&["init", "-q"]).unwrap();
        assert!(git.without_hooks().configured_hooks().is_empty());
    }

    /// git exits on a usage error before it reads stdin, so the write fails
    /// with `EPIPE` (the test harness keeps SIGPIPE ignored); the error that
    /// reaches the user is git's own.
    #[test]
    fn with_stdin_reports_git_failure_over_a_failed_write() {
        let dir = tempfile::tempdir().unwrap();
        let err = Git::new(dir.path())
            .with_stdin(
                &["hash-object", "--no-such-option", "--stdin"],
                &"x".repeat(1 << 20),
            )
            .err()
            .expect("git refused the option");
        assert!(
            err.to_string().starts_with("git hash-object failed:"),
            "{err:#}"
        );
    }
}
