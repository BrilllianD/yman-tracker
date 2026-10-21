//! Discovery of the surrounding repository and the state of `.yman`.

use crate::config::{CONFIG_FILE, Config};
use crate::errors::MergePending;
use crate::git::{Git, indent_detail};
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

/// What `Context::preflight` does when a sync merge is pending and the
/// working `config.toml` does not load (it conflicted, or a hand edit broke it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeConfig {
    /// Fail with the load error, as for any command outside a merge.
    Strict,
    /// Load nothing: the command settles the merge and loads the config itself (`sync`).
    Skip,
    /// Load the config committed at HEAD and print a note (the read-only commands).
    Committed,
}

pub struct Context {
    /// Main repo toplevel.
    pub root: PathBuf,
    /// Main repo common git dir (absolute).
    pub common: PathBuf,
    /// `root/.yman`.
    pub ydir: PathBuf,
    /// Private git dir of the `.yman` worktree; `None` when `.yman` is not
    /// our worktree. Read it through [`Context::wt_gitdir`].
    pub wt_gitdir: Option<PathBuf>,
    /// Runner for ref/remote/config operations, in the main worktree.
    pub main: Git,
    /// Runner for everything touching task files, in `.yman`. Runs with the
    /// user's hooks switched off.
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
/// One `rev-parse` answers all three questions: it prints the results in the
/// order the options were given, so toplevel comes first, the common dir
/// second and the git dir third. From the main worktree that is the only
/// `git` process a read-only command needs. From a linked worktree (where the
/// git dir differs from the common dir) one `worktree list` finds the main
/// toplevel, which is what `ROOT` means, unless only the current worktree
/// holds a `.yman` of ours.
pub fn discover() -> Result<Context> {
    let here = Git::new(std::env::current_dir()?);
    // `--path-format=absolute` so worktrees and `.git`-file layouts both give
    // us a path we can join onto.
    let out = here.run(&[
        "rev-parse",
        "--path-format=absolute",
        "--show-toplevel",
        "--git-common-dir",
        "--git-dir",
    ])?;
    let mut lines = out.stdout.lines();
    let (top, common, git_dir) = match (out.ok(), lines.next(), lines.next(), lines.next()) {
        // Git before 2.31 does not know `--path-format` and echoes it back as
        // the first line, so all answers must look like absolute paths.
        (true, Some(top), Some(dir), Some(gd))
            if Path::new(top.trim()).is_absolute()
                && Path::new(dir.trim()).is_absolute()
                && Path::new(gd.trim()).is_absolute() =>
        {
            (
                PathBuf::from(top.trim()),
                PathBuf::from(dir.trim()),
                PathBuf::from(gd.trim()),
            )
        }
        // Older git, or a layout where one of the answers is unanswerable:
        // ask separately, without the flag.
        _ => discover_separately(&here)?,
    };
    let root = if same_dir(&git_dir, &common) {
        top
    } else {
        // A linked worktree, `.yman` itself included. Reading `.yman/.git`
        // costs no process, so try it first.
        match yman_parent(&top, &common) {
            Some(parent) => parent,
            None => match main_toplevel(&here)? {
                // A tracker v0.9.0 let `init` create inside this linked
                // worktree, with none in the main checkout, stays ours.
                Some(main)
                    if our_worktree_gitdir(&main.join(YDIR_NAME), &common).is_none()
                        && our_worktree_gitdir(&top.join(YDIR_NAME), &common).is_some() =>
                {
                    top
                }
                Some(main) => main,
                None => top,
            },
        }
    };
    let main = Git::new(&root);
    let ydir = root.join(YDIR_NAME);
    // git derives the worktree's private dir name from the basename and
    // sanitizes it (".yman" becomes "-yman"), so read it rather than guess.
    let wt_gitdir = our_worktree_gitdir(&ydir, &common);
    Ok(Context {
        main,
        // The user's hooks never run for yman's own commits, merges and
        // fast-forwards in `.yman`; see `Git::without_hooks`.
        wt: Git::new(&ydir).without_hooks(),
        root,
        common,
        ydir,
        wt_gitdir,
        config: None,
    })
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// `Some(private git dir)` when `ydir/.git` is a `gitdir:` file pointing under
/// `COMMON/worktrees/` at a directory holding a `gitdir` file. The one test
/// for "this `.yman` is our worktree".
fn our_worktree_gitdir(ydir: &Path, common: &Path) -> Option<PathBuf> {
    let pointed = read_gitdir_file(ydir)?;
    let worktrees = common.join("worktrees");
    let worktrees = worktrees.canonicalize().unwrap_or(worktrees);
    (pointed.starts_with(&worktrees) && pointed.join("gitdir").exists()).then_some(pointed)
}

/// From anywhere inside `.yman` — `cd $(yman path 14)` — git's toplevel is
/// `.yman` itself. When that toplevel is our own linked worktree, the project
/// is its parent. Read off disk, so it costs no process.
fn yman_parent(top: &Path, common: &Path) -> Option<PathBuf> {
    if top.file_name()? != YDIR_NAME {
        return None;
    }
    our_worktree_gitdir(top, common)?;
    top.parent().map(Path::to_path_buf)
}

/// The first record of `git worktree list --porcelain -z`, which git documents
/// as the main worktree. `None` when the main repository is bare: it has no
/// toplevel, and the caller keeps the current one.
fn main_toplevel(here: &Git) -> Result<Option<PathBuf>> {
    let text = here.out(&["worktree", "list", "--porcelain", "-z"])?;
    let mut path = None;
    for field in text.split('\0') {
        if field.is_empty() {
            break;
        }
        if field == "bare" {
            return Ok(None);
        }
        if path.is_none() {
            path = field.strip_prefix("worktree ").map(PathBuf::from);
        }
    }
    Ok(path)
}

/// Fallback for [`discover`]: separate `rev-parse` calls, without
/// `--path-format`. Returns toplevel, common dir and git dir.
fn discover_separately(here: &Git) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let root = match here.run(&["rev-parse", "--show-toplevel"])? {
        o if o.ok() => PathBuf::from(o.stdout.trim()),
        o => {
            let detail = indent_detail(&o.stderr);
            if detail.is_empty() {
                bail!("not inside a git repository");
            }
            bail!("not inside a git repository\n  {detail}");
        }
    };
    // Relative to the directory git ran in, which `-C` makes `root`.
    let ask = |flag: &str| -> Result<PathBuf> {
        let dir = PathBuf::from(Git::new(&root).out(&["rev-parse", flag])?.trim());
        Ok(if dir.is_absolute() {
            dir
        } else {
            root.join(dir)
        })
    };
    let common = ask("--git-common-dir")?;
    let git_dir = ask("--git-dir")?;
    Ok((root, common, git_dir))
}

impl Context {
    /// Private git dir of `.yman`. Only valid once `.yman` is known to be our
    /// worktree: after preflight, or in `init` after `add_worktree` or on the
    /// repair path.
    pub fn wt_gitdir(&self) -> &Path {
        self.wt_gitdir
            .as_deref()
            .expect("preflight checked .yman is our worktree")
    }

    /// Config of the task history. Only valid after `preflight`, and not for
    /// a command `preflight` let through a merge with an unloadable config.
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
        let existing = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => bail!("cannot read {}: {e}", path.display()),
        };
        if existing
            .split(|&b| b == b'\n')
            .any(|l| l.trim_ascii() == line.as_bytes() || l.trim_ascii() == YDIR_NAME.as_bytes())
        {
            return Ok(false);
        }
        std::fs::create_dir_all(path.parent().expect("info/exclude has a parent"))?;
        let mut add = Vec::new();
        if !existing.is_empty() && !existing.ends_with(b"\n") {
            add.push(b'\n');
        }
        add.extend_from_slice(line.as_bytes());
        add.push(b'\n');
        // Append only: the bytes already there are never rewritten.
        let write = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, &add));
        if let Err(e) = write {
            bail!("cannot write {}: {e}", path.display());
        }
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
        if our_worktree_gitdir(&self.ydir, &self.common).is_some() {
            YdirState::Worktree
        } else {
            YdirState::PlainDir
        }
    }

    /// Re-resolve the worktree's private git dir, after creating `.yman`.
    pub fn relearn_wt_gitdir(&mut self) {
        self.wt_gitdir = our_worktree_gitdir(&self.ydir, &self.common);
    }

    pub fn merge_in_progress(&self) -> bool {
        self.wt_gitdir().join("MERGE_HEAD").exists()
    }

    /// HEAD of the `.yman` worktree must be the symbolic ref `refs/yman/local`
    /// (see the spike note at the top of `git.rs`).
    pub fn check_worktree_head(&self) -> Result<()> {
        // HEAD is a file in the worktree's private git dir; only a layout
        // `refs` does not recognise costs a process here.
        let head = match crate::refs::head_symref(self.wt_gitdir()) {
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

    /// Checks every command runs first, except the ones
    /// `Cmd::skips_preflight` lists (`init`, `hooks`, `git`, `guide`,
    /// `completions`, `man`, `merge-driver`).
    ///
    /// The merge check comes before the config load: a sync merge that
    /// conflicted in `config.toml` leaves it unparseable, and that must still
    /// read as exit 3 for a command that writes. For the rest, `merge_config`
    /// says what happens while a merge is pending and the config does not
    /// load: `sync` gets past it without a config (settling the merge is its
    /// job), the read-only commands use the config committed at `HEAD`, and
    /// every other command fails with the load error.
    pub fn preflight(&mut self, mutating: bool, merge_config: MergeConfig) -> Result<()> {
        if self.ydir_state() != YdirState::Worktree {
            bail!("{YDIR_NAME} is not initialized; run: yman init");
        }
        self.check_worktree_head()?;
        let merging = self.merge_in_progress();
        if mutating && merging {
            return Err(MergePending::new(
                "sync merge in progress; resolve conflicts then run: yman sync --continue  (or: yman sync --abort)",
            )
            .into());
        }
        match Config::load(&self.ydir) {
            Ok(cfg) => self.config = Some(cfg),
            Err(_) if merging && merge_config == MergeConfig::Skip => {}
            Err(_) if merging && merge_config == MergeConfig::Committed => {
                self.config = Some(self.head_config()?);
                eprintln!("note: sync merge in progress; using config.toml as committed at HEAD");
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// `config.toml` as committed at `HEAD`. During a sync merge `HEAD` is
    /// this clone's side, committed before the merge started, so it is the
    /// config the readers used a moment earlier.
    fn head_config(&self) -> Result<Config> {
        Config::parse(
            &self
                .wt
                .out(&["cat-file", "blob", &format!("HEAD:{CONFIG_FILE}")])?,
        )
    }

    /// Load `config.toml` again after `.yman` moved. A no-op when preflight
    /// loaded none (a merge it let through).
    pub fn reload_config(&mut self) -> Result<()> {
        if self.config.is_some() {
            self.config = Some(Config::load(&self.ydir)?);
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
