use crate::cli::{AttachArgs, DetachArgs};
use crate::repo::Context;
use crate::task::{self, Attachment};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

const BIG_FILE: u64 = 5 * 1024 * 1024;

pub fn run(ctx: &mut Context, a: AttachArgs) -> Result<()> {
    if a.name.is_some() && a.files.len() > 1 {
        bail!("--name only works with a single file");
    }
    let mut t = task::find(&ctx.ydir, &a.id)?;
    let by = super::actor(ctx)?;

    let fdir = t.dir.join(task::FILES_DIR);

    // Every source is checked before any is copied: a refusal on the second
    // file used to leave the first one copied under `f/` with no `m.yml` entry
    // and no commit, an untracked file the next sync would snapshot.
    let mut plan: Vec<(&Path, String, PathBuf, u64)> = Vec::new();
    for src in &a.files {
        let meta = match std::fs::metadata(src) {
            Ok(m) => m,
            Err(e) => bail!("cannot attach {}: {e}", src.display()),
        };
        if !meta.is_file() {
            bail!("cannot attach {}: not a regular file", src.display());
        }
        // Opened, not just stat'ed, so an unreadable file is refused here
        // rather than half way through the copies.
        if let Err(e) = std::fs::File::open(src) {
            bail!("cannot attach {}: {e}", src.display());
        }
        let name = match &a.name {
            Some(n) => n.clone(),
            None => match src.file_name() {
                Some(n) => n.to_string_lossy().into_owned(),
                None => bail!("cannot attach {}: no file name", src.display()),
            },
        };
        if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
            bail!("attachment name \"{name}\" must not contain a path separator");
        }
        check_portable(&name)?;
        // `--force` replaces an attachment already on the task; it cannot
        // make two of this call's files one. Case-folded, because on macOS
        // and Windows `a.png` and `A.png` are one file.
        if plan.iter().any(|(_, n, _, _)| fold(n) == fold(&name)) {
            bail!("attachment \"{name}\" given more than once");
        }
        // The entry and the file are checked alike: an entry whose file is
        // gone is still an attachment, and a stray file is still in the way.
        let taken = t
            .meta
            .attachments
            .iter()
            .any(|x| fold(&x.name) == fold(&name))
            || !same_name_files(&fdir, &name)?.is_empty();
        if taken && !a.force {
            bail!(
                "attachment \"{name}\" already exists on task {}; use --force",
                t.id()
            );
        }
        let dest = fdir.join(&name);
        plan.push((src, name, dest, meta.len()));
    }

    for (_, name, _, len) in &plan {
        if *len > BIG_FILE {
            eprintln!(
                "warning: {name} is {} MiB; git is not great at large binaries",
                len / (1024 * 1024)
            );
        }
    }
    let meta_path = t.dir.join(task::META_FILE);
    let before = std::fs::read(&meta_path)?;
    // The copies land first and `m.yml` is written only once all of them
    // have, so a copy that fails half way (disk full) is undone by `place`
    // and the task reads exactly as it did before.
    let placed = place(&fdir, &plan, |src, dest| std::fs::copy(src, dest).map(drop))?;

    let mut names: Vec<String> = Vec::new();
    for (_, name, _, _) in &plan {
        let entry = Attachment {
            path: format!("{}/{}", task::FILES_DIR, name),
            name: name.clone(),
            added: task::now(),
            by: by.clone(),
        };
        match t
            .meta
            .attachments
            .iter()
            .position(|x| fold(&x.name) == fold(name))
        {
            Some(i) => t.meta.attachments[i] = entry,
            None => t.meta.attachments.push(entry),
        }
        names.push(name.clone());
    }

    t.touch();
    if let Err(e) = t.write_meta() {
        // A short write leaves `m.yml` truncated; put back what was there.
        if let Err(w) = std::fs::write(&meta_path, &before) {
            eprintln!("warning: cannot restore {}: {w}", meta_path.display());
        }
        placed.undo();
        return Err(e);
    }
    placed.keep();
    for (_, name, dest, _) in &plan {
        println!("attached {name} -> {}", ctx.display_path(dest));
    }
    let rel = t.rel();
    ctx.wt.ok(&["add", "--", &rel])?;
    // `.yman/.gitignore` carries the editor-junk patterns `init` writes, and a
    // plain `git add <dir>` skips an ignored path without saying so: the entry
    // landed in `m.yml`, the file never reached a commit, and every other clone
    // saw a dangling attachment after `sync`. The user named this file, so the
    // ignore rule does not apply to it — stage each one by force.
    for name in &names {
        let file = format!("{rel}/{}/{name}", task::FILES_DIR);
        ctx.wt.ok(&["add", "-f", "--", &file])?;
    }
    ctx.wt
        .commit(&format!("task({}): attach {}", t.id(), names.join(", ")))?;
    Ok(())
}

/// What `place` changed under `f/`, held until the caller knows whether the
/// attach as a whole went through.
struct Placed {
    fdir: PathBuf,
    /// `f/` did not exist before this attach.
    made_fdir: bool,
    /// Files this attach wrote, in order.
    new: Vec<PathBuf>,
    /// `(aside, original)`: a file `--force` replaces, or the other spelling
    /// it drops, renamed out of the way rather than deleted.
    aside: Vec<(PathBuf, PathBuf)>,
}

impl Placed {
    /// The attach went through: the files moved aside go for good.
    fn keep(self) {
        for (aside, _) in &self.aside {
            if let Err(e) = std::fs::remove_file(aside) {
                eprintln!("warning: cannot remove {}: {e}", aside.display());
            }
        }
    }

    /// The attach failed: put `f/` back exactly as it was.
    fn undo(self) {
        for path in self.new.iter().rev() {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("warning: cannot remove {}: {e}", path.display()),
            }
        }
        for (aside, original) in self.aside.iter().rev() {
            if let Err(e) = std::fs::rename(aside, original) {
                eprintln!(
                    "warning: cannot restore {} from {}: {e}",
                    original.display(),
                    aside.display()
                );
            }
        }
        if self.made_fdir {
            // Only if empty: anything still in it is not ours to delete.
            let _ = std::fs::remove_dir(&self.fdir);
        }
    }
}

/// Copies every planned file into `f/`, all or nothing. A file in the way is
/// renamed aside first, not deleted, so a failed copy can restore it; on
/// failure everything is undone before the error is returned.
fn place<F>(fdir: &Path, plan: &[(&Path, String, PathBuf, u64)], mut copy: F) -> Result<Placed>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let mut placed = Placed {
        fdir: fdir.to_path_buf(),
        made_fdir: !fdir.exists(),
        new: Vec::new(),
        aside: Vec::new(),
    };
    match place_all(&mut placed, plan, &mut copy) {
        Ok(()) => Ok(placed),
        Err(e) => {
            placed.undo();
            Err(e)
        }
    }
}

fn place_all<F>(
    placed: &mut Placed,
    plan: &[(&Path, String, PathBuf, u64)],
    copy: &mut F,
) -> Result<()>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    std::fs::create_dir_all(&placed.fdir)?;
    for (src, name, dest, _) in plan {
        // `--force A.png` over `a.png`: the old spelling goes as well, so a
        // case-sensitive clone does not end up holding both.
        for old in same_name_files(&placed.fdir, name)? {
            let original = placed.fdir.join(&old);
            let aside = aside_name(&placed.fdir, &old);
            std::fs::rename(&original, &aside)?;
            placed.aside.push((aside, original));
        }
        // Recorded before the copy: a failed copy can leave a partial file.
        placed.new.push(dest.clone());
        copy(src, dest)?;
    }
    Ok(())
}

/// A free name in `f/` to hold `name` while the attach is in flight.
fn aside_name(fdir: &Path, name: &str) -> PathBuf {
    let mut n = 0u32;
    loop {
        let candidate = fdir.join(format!(".{name}.yman-attach.{n}"));
        if std::fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
        n += 1;
    }
}

fn fold(name: &str) -> String {
    name.to_lowercase()
}

/// Files under `f/` whose name equals `name` ignoring case.
fn same_name_files(fdir: &Path, name: &str) -> Result<Vec<String>> {
    let entries = match std::fs::read_dir(fdir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => bail!("cannot read {}: {e}", fdir.display()),
    };
    let mut out = Vec::new();
    for entry in entries {
        let n = entry?.file_name().to_string_lossy().into_owned();
        if fold(&n) == fold(name) {
            out.push(n);
        }
    }
    Ok(out)
}

/// Names a Windows checkout cannot hold: the characters NTFS reserves,
/// control characters, and a trailing dot or space, which Windows strips.
fn check_portable(name: &str) -> Result<()> {
    let why = if name.is_empty() {
        Some("it is empty".to_string())
    } else if let Some(c) = name.chars().find(|c| ":*?\"<>|".contains(*c)) {
        Some(format!("contains '{c}'"))
    } else if name.chars().any(char::is_control) {
        Some("contains a control character".to_string())
    } else if name.ends_with('.') {
        Some("ends with '.'".to_string())
    } else if name.ends_with(' ') {
        Some("ends with a space".to_string())
    } else {
        None
    };
    match why {
        Some(why) => bail!(
            "attachment name \"{}\" is not portable: {why}",
            name.escape_debug()
        ),
        None => Ok(()),
    }
}

pub fn run_detach(ctx: &mut Context, a: DetachArgs) -> Result<()> {
    let mut t = task::find(&ctx.ydir, &a.id)?;
    // Names compare ignoring case, as in `attach`. An exact match still wins,
    // so a task that holds both spellings from before `attach` case-folded
    // can lose either one.
    let atts = &t.meta.attachments;
    let found = atts
        .iter()
        .position(|x| x.name == a.name)
        .or_else(|| atts.iter().position(|x| fold(&x.name) == fold(&a.name)));
    let Some(i) = found else {
        bail!("no attachment \"{}\" on task {}", a.name, t.id());
    };
    // Everything below uses the stored spelling, not the typed one: that is
    // the file git tracks.
    let name = t.meta.attachments[i].name.clone();
    let rel = format!("{}/{}/{}", t.rel(), task::FILES_DIR, name);
    // A listed attachment whose file is already gone just loses its entry.
    if t.attachment_path(&t.meta.attachments[i]).exists() {
        ctx.wt.ok(&["rm", "-q", "--", &rel])?;
    }
    t.meta.attachments.remove(i);
    t.touch();
    t.write_meta()?;
    ctx.wt.ok(&["add", "--", &t.rel()])?;
    ctx.wt
        .commit(&format!("task({}): detach {}", t.id(), name))?;
    println!("detached {name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Every file under `dir`, name to bytes.
    fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .map(|e| {
                    let e = e.unwrap();
                    let name = e.file_name().to_string_lossy().into_owned();
                    (name, std::fs::read(e.path()).unwrap())
                })
                .collect(),
            Err(_) => BTreeMap::new(),
        }
    }

    /// A copy that fails on the `n`th call (zero-based), after the ones
    /// before it landed, as a full disk would.
    fn failing_at(n: usize) -> impl FnMut(&Path, &Path) -> std::io::Result<()> {
        let mut calls = 0;
        move |src, dest| {
            calls += 1;
            if calls > n {
                std::fs::write(dest, b"partial")?;
                return Err(std::io::Error::other("no space left on device"));
            }
            std::fs::copy(src, dest).map(drop)
        }
    }

    #[test]
    fn a_failed_copy_restores_overwritten_and_dropped_files() {
        let tmp = tempfile::tempdir().unwrap();
        let fdir = tmp.path().join("f");
        std::fs::create_dir(&fdir).unwrap();
        std::fs::write(fdir.join("a.png"), b"old a").unwrap();
        std::fs::write(fdir.join("keep.txt"), b"untouched").unwrap();
        std::fs::write(fdir.join("b.txt"), b"old b").unwrap();
        let before = snapshot(&fdir);

        let src = tmp.path().join("src");
        std::fs::create_dir(&src).unwrap();
        for name in ["A.png", "new.txt", "b.txt"] {
            std::fs::write(src.join(name), format!("new {name}")).unwrap();
        }
        let (s1, s2, s3) = (src.join("A.png"), src.join("new.txt"), src.join("b.txt"));
        // `A.png` replaces `a.png` under another spelling, `new.txt` is new,
        // and the copy of `b.txt` over the old one is the one that fails.
        let plan: Vec<(&Path, String, PathBuf, u64)> = vec![
            (&s1, "A.png".into(), fdir.join("A.png"), 0),
            (&s2, "new.txt".into(), fdir.join("new.txt"), 0),
            (&s3, "b.txt".into(), fdir.join("b.txt"), 0),
        ];
        let err = place(&fdir, &plan, failing_at(2)).err().unwrap();
        assert_eq!(err.to_string(), "no space left on device");
        assert_eq!(snapshot(&fdir), before);
    }

    #[test]
    fn a_failed_copy_removes_the_files_dir_it_made() {
        let tmp = tempfile::tempdir().unwrap();
        let fdir = tmp.path().join("f");
        let src = tmp.path().join("x.txt");
        std::fs::write(&src, b"x").unwrap();
        let plan: Vec<(&Path, String, PathBuf, u64)> = vec![
            (&src, "x.txt".into(), fdir.join("x.txt"), 0),
            (&src, "y.txt".into(), fdir.join("y.txt"), 0),
        ];
        assert!(place(&fdir, &plan, failing_at(1)).is_err());
        assert!(!fdir.exists());
    }

    #[test]
    fn a_kept_attach_drops_what_it_moved_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let fdir = tmp.path().join("f");
        std::fs::create_dir(&fdir).unwrap();
        std::fs::write(fdir.join("a.png"), b"old").unwrap();
        let src = tmp.path().join("A.png");
        std::fs::write(&src, b"new").unwrap();
        let plan: Vec<(&Path, String, PathBuf, u64)> =
            vec![(&src, "A.png".into(), fdir.join("A.png"), 0)];
        place(&fdir, &plan, |s, d| std::fs::copy(s, d).map(drop))
            .unwrap()
            .keep();
        let after = snapshot(&fdir);
        assert_eq!(after.len(), 1, "{:?}", after.keys());
        assert_eq!(after["A.png"], b"new");
    }
}
