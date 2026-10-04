use crate::cli::CommentArgs;
use crate::discussion;
use crate::repo::Context;
use crate::task;
use anyhow::{Result, bail};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::edit::{resolve_editor, run_editor};

/// The file `comment -e` hands the editor, removed when dropped so that
/// success, a failed editor and an empty comment all clean up alike.
struct TempFile(PathBuf);

impl TempFile {
    /// A new, empty file in the temp dir under a name no other run picks.
    /// `create_new` refuses any existing path, a symlink included, so a link
    /// planted in a shared `/tmp` is never followed and nothing is truncated.
    fn create(id: &str) -> Result<TempFile> {
        let dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let mut last = None;
        for attempt in 0..8u32 {
            let name = format!(
                "yman-comment-{id}-{}-{nanos}-{attempt}.md",
                std::process::id()
            );
            let path = dir.join(name);
            match open_new(&path) {
                Ok(_) => return Ok(TempFile(path)),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => last = Some(e),
                Err(e) => bail!("cannot create {}: {e}", path.display()),
            }
        }
        let e = last.expect("the loop ran");
        bail!("cannot create a temp file in {}: {e}", dir.display())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn open_new(path: &Path) -> std::io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}

pub fn run(ctx: &mut Context, a: CommentArgs) -> Result<()> {
    let mut t = task::find(&ctx.ydir, &a.id)?;

    let text = if let Some(m) = a.message {
        m
    } else if a.edit {
        // Resolve first: a refused editor must not leave a file behind.
        resolve_editor()?;
        let tmp = TempFile::create(t.id())?;
        run_editor(tmp.path(), "comment not added")?;
        std::fs::read_to_string(tmp.path()).unwrap_or_default()
    } else if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        bail!("empty comment")
    };

    if text.trim().is_empty() {
        bail!("empty comment");
    }

    let author = super::actor(ctx)?;
    discussion::append_entry(
        &t.dir.join(task::DISCUSSION_FILE),
        task::now(),
        &author,
        text.trim(),
    )?;

    t.touch();
    t.write_meta()?;
    ctx.wt.ok(&["add", "--", &t.rel()])?;
    ctx.wt.commit(&format!("task({}): comment", t.id()))?;
    println!("commented on {}", t.id());
    Ok(())
}
