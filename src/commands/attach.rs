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

    let mut names: Vec<String> = Vec::new();
    for (src, name, dest, len) in plan {
        if len > BIG_FILE {
            eprintln!(
                "warning: {name} is {} MiB; git is not great at large binaries",
                len / (1024 * 1024)
            );
        }
        std::fs::create_dir_all(&fdir)?;
        // `--force A.png` over `a.png`: the old spelling goes, so a
        // case-sensitive clone does not end up holding both.
        for old in same_name_files(&fdir, &name)? {
            if old != name {
                std::fs::remove_file(fdir.join(&old))?;
            }
        }
        std::fs::copy(src, &dest)?;

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
            .position(|x| fold(&x.name) == fold(&name))
        {
            Some(i) => t.meta.attachments[i] = entry,
            None => t.meta.attachments.push(entry),
        }
        println!("attached {name} -> {}", ctx.display_path(&dest));
        names.push(name);
    }

    t.touch();
    t.write_meta()?;
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
