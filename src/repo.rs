//! Discovery of the surrounding repository and the state of `.yman`.

use crate::config::Config;
use crate::errors::MergePending;
use crate::git::Git;
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

/// Task history head. Deliberately outside `refs/heads/`.
pub const LOCAL: &str = "refs/yman/local";
/// Remote-tracking ref for the tasks history.
pub const REMOTE: &str = "refs/yman/remote";
/// The ref name on the origin server.
pub const REMOTE_REF: &str = "refs/tasks/main";
/// The refspec `init` appends to `remote.origin.fetch`.
pub const FETCH_REFSPEC: &str = "+refs/tasks/main:refs/yman/remote";
/// Name of the `.yman` directory, and the line added to `info/exclude`.
pub const YDIR_NAME: &str = ".yman";

/// `COMMON/worktrees/yman`, the name we would get if git did not sanitize.
fn default_wt_gitdir(common: &Path) -> PathBuf {
    common.join("worktrees").join("yman")
}

/// Follow `.yman/.git` (a file reading `gitdir: <path>`) to the worktree's
/// private git dir.
fn read_gitdir_file(ydir: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(ydir.join(".git")).ok()?;
    let rest = text.trim().strip_prefix("gitdir:")?;
    let pointed = PathBuf::from(rest.trim());
    let pointed = if pointed.is_absolute() {
        pointed
    } else {
        ydir.join(pointed)
    };
    Some(pointed.canonicalize().unwrap_or(pointed))
}

pub struct Context {
    /// Main repo toplevel.
    pub root: PathBuf,
    /// Main repo common git dir (absolute).
    pub common: PathBuf,
    /// `root/.yman`.
    pub ydir: PathBuf,
    /// Private git dir of the `.yman` worktree.
    pub wt_gitdir: PathBuf,
    /// Runner for ref/remote/config operations, in the main worktree.
    pub main: Git,
    /// Runner for everything touching task files, in `.yman`.
    pub wt: Git,
    /// Loaded by `preflight`.
    pub config: Option<Config>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum YdirState {
    /// `.yman` does not exist.
    Absent,
    /// `.yman` is a linked worktree of this repo.
    Worktree,
    /// `.yman` has its own `.git` directory.
    StandaloneRepo,
    /// `.yman` exists but is neither of the above.
    PlainDir,
}

/// Find the repository containing the current directory.
///
/// One `rev-parse` answers both questions: it prints the results in the order
/// the options were given, so toplevel comes first and the common dir second.
/// This is the only `git` process a read-only command needs.
pub fn discover() -> Result<Context> {
    let here = Git::new(std::env::current_dir()?);
    // `--path-format=absolute` so worktrees and `.git`-file layouts both give
    // us a path we can join onto.
    let out = here.run(&[
        "rev-parse",
        "--path-format=absolute",
        "--show-toplevel",
        "--git-common-dir",
    ])?;
    let mut lines = out.stdout.lines();
    let (root, common) = match (out.ok(), lines.next(), lines.next()) {
        (true, Some(top), Some(dir)) => (PathBuf::from(top.trim()), PathBuf::from(dir.trim())),
        // Older git without `--path-format`, or a layout where one of the two
        // is unanswerable: ask the way we always did.
        _ => discover_separately(&here)?,
    };
    let main = Git::new(&root);
    let ydir = root.join(YDIR_NAME);
    // git derives the worktree's private dir name from the basename and
    // sanitizes it (".yman" becomes "-yman"), so read it rather than guess.
    let wt_gitdir = read_gitdir_file(&ydir).unwrap_or_else(|| default_wt_gitdir(&common));
    Ok(Context {
        main,
        wt: Git::new(&ydir),
        root,
        common,
        ydir,
        wt_gitdir,
        config: None,
    })
}

/// Fallback for [`discover`]: the two separate `rev-parse` calls.
fn discover_separately(here: &Git) -> Result<(PathBuf, PathBuf)> {
    let root = match here.run(&["rev-parse", "--show-toplevel"])? {
        o if o.ok() => PathBuf::from(o.stdout.trim()),
        _ => bail!("not inside a git repository"),
    };
    let main = Git::new(&root);
    let common = PathBuf::from(
        main.out(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?
            .trim(),
    );
    Ok((root, common))
}

impl Context {
    /// Config of the task history. Only valid after `preflight`.
    pub fn config(&self) -> &Config {
        self.config
            .as_ref()
            .expect("preflight() loads the config before any command body runs")
    }

    pub fn origin_url(&self) -> Result<Option<String>> {
        self.get_cfg("remote.origin.url")
    }

    pub fn get_cfg(&self, key: &str) -> Result<Option<String>> {
        let out = self.main.run(&["config", "--get", key])?;
        if out.ok() {
            Ok(Some(out.stdout.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    pub fn set_cfg(&self, key: &str, val: &str) -> Result<()> {
        self.main.ok(&["config", key, val])?;
        Ok(())
    }

    /// Append `.yman/` to the repo's `info/exclude` unless it is already
    /// ignored. Returns whether the line was added.
    pub fn exclude_add(&self) -> Result<bool> {
        let line = format!("{YDIR_NAME}/");
        let path = self.common.join("info").join("exclude");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if existing
            .lines()
            .any(|l| l.trim() == line || l.trim() == YDIR_NAME)
        {
            return Ok(false);
        }
        std::fs::create_dir_all(path.parent().expect("info/exclude has a parent"))?;
        let mut text = existing;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&line);
        text.push('\n');
        std::fs::write(&path, text)?;
        Ok(true)
    }

    pub fn fetch_refspec_present(&self) -> Result<bool> {
        let out = self
            .main
            .run(&["config", "--get-all", "remote.origin.fetch"])?;
        Ok(out.ok() && out.stdout.lines().any(|l| l.trim() == FETCH_REFSPEC))
    }

    pub fn fetch_refspec_add(&self) -> Result<()> {
        self.main
            .ok(&["config", "--add", "remote.origin.fetch", FETCH_REFSPEC])?;
        Ok(())
    }

    pub fn ydir_state(&self) -> YdirState {
        if !self.ydir.exists() {
            return YdirState::Absent;
        }
        if self.ydir.join(".git").is_dir() {
            return YdirState::StandaloneRepo;
        }
        let Some(pointed) = read_gitdir_file(&self.ydir) else {
            return YdirState::PlainDir;
        };
        // A linked worktree of *this* repo keeps its private dir under the
        // common dir's `worktrees/`.
        let worktrees = self
            .common
            .join("worktrees")
            .canonicalize()
            .unwrap_or_else(|_| self.common.join("worktrees"));
        if pointed.starts_with(&worktrees) && pointed.join("gitdir").exists() {
            YdirState::Worktree
        } else {
            YdirState::PlainDir
        }
    }

    /// Re-resolve the worktree's private git dir, after creating `.yman`.
    pub fn relearn_wt_gitdir(&mut self) {
        if let Some(p) = read_gitdir_file(&self.ydir) {
            self.wt_gitdir = p;
        }
    }

    pub fn merge_in_progress(&self) -> bool {
        self.wt_gitdir.join("MERGE_HEAD").exists()
    }

    /// HEAD of the `.yman` worktree must be the symbolic ref `refs/yman/local`
    /// (see the spike note at the top of `git.rs`).
    pub fn check_worktree_head(&self) -> Result<()> {
        // HEAD is a file in the worktree's private git dir; only a layout
        // `refs` does not recognise costs a process here.
        let head = match crate::refs::head_symref(&self.wt_gitdir) {
            Ok(v) => v,
            Err(_) => {
                let out = self.wt.run(&["symbolic-ref", "-q", "HEAD"])?;
                out.ok().then(|| out.stdout.trim().to_string())
            }
        };
        if head.as_deref() == Some(LOCAL) {
            return Ok(());
        }
        bail!(".yman worktree is not on {LOCAL}; run: yman init");
    }

    /// Resolve one of our refs to a full hash, off disk where the layout
    /// allows it and through `git` where it does not.
    pub fn resolve_ref(&self, name: &str) -> Result<Option<String>> {
        match crate::refs::resolve(&self.common, name) {
            Ok(v) => Ok(v),
            Err(_) => self.main.rev_parse(name),
        }
    }

    /// Checks every command (except `init`, `hooks`, `git`) runs first.
    pub fn preflight(&mut self, mutating: bool) -> Result<()> {
        if self.ydir_state() != YdirState::Worktree {
            bail!("{YDIR_NAME} is not initialized; run: yman init");
        }
        self.check_worktree_head()?;
        let cfg = Config::load(&self.ydir)?;
        self.config = Some(cfg);
        if mutating && self.merge_in_progress() {
            return Err(MergePending::new(
                "sync merge in progress; resolve conflicts then run: yman sync --continue  (or: yman sync --abort)",
            )
            .into());
        }
        Ok(())
    }

    /// Path of a task folder relative to the repo root, for display.
    pub fn display_path(&self, dir: &Path) -> String {
        match dir.strip_prefix(&self.root) {
            Ok(rel) => rel.to_string_lossy().into_owned(),
            Err(_) => dir.to_string_lossy().into_owned(),
        }
    }
}
