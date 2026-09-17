use crate::cli::{AttachArgs, DetachArgs};
use crate::repo::Context;
use crate::task::{self, Attachment};
use anyhow::{Result, bail};

const BIG_FILE: u64 = 5 * 1024 * 1024;

pub fn run(ctx: &mut Context, a: AttachArgs) -> Result<()> {
    if a.name.is_some() && a.files.len() > 1 {
        bail!("--name only works with a single file");
    }
    let mut t = task::find(&ctx.ydir, &a.id)?;
    let by = ctx
        .get_cfg("user.name")?
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let fdir = t.dir.join(task::FILES_DIR);
    let mut names: Vec<String> = Vec::new();

    for src in &a.files {
        let meta = match std::fs::metadata(src) {
            Ok(m) => m,
            Err(e) => bail!("cannot attach {}: {e}", src.display()),
        };
        if !meta.is_file() {
            bail!("cannot attach {}: not a regular file", src.display());
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
        let dest = fdir.join(&name);
        if dest.exists() && !a.force {
            bail!("attachment \"{name}\" already exists on task {}; use --force", t.id());
        }
        if meta.len() > BIG_FILE {
            eprintln!(
                "warning: {name} is {} MiB; git is not great at large binaries",
                meta.len() / (1024 * 1024)
            );
        }
        std::fs::create_dir_all(&fdir)?;
        std::fs::copy(src, &dest)?;

        let entry = Attachment {
            path: format!("{}/{}", task::FILES_DIR, name),
            name: name.clone(),
            added: task::now(),
            by: by.clone(),
        };
        match t.meta.attachments.iter().position(|x| x.name == name) {
            Some(i) => t.meta.attachments[i] = entry,
            None => t.meta.attachments.push(entry),
        }
        println!(
            "attached {name} -> {}",
            ctx.display_path(&dest)
        );
        names.push(name);
    }

    t.touch();
    t.write_meta()?;
    ctx.wt.ok(&["add", "--", &t.rel()])?;
    ctx.wt
        .commit(&format!("task({}): attach {}", t.id(), names.join(", ")))?;
    Ok(())
}

pub fn run_detach(ctx: &mut Context, a: DetachArgs) -> Result<()> {
    let mut t = task::find(&ctx.ydir, &a.id)?;
    let Some(i) = t.meta.attachments.iter().position(|x| x.name == a.name) else {
        bail!("no attachment \"{}\" on task {}", a.name, t.id());
    };
    let rel = format!("{}/{}/{}", t.rel(), task::FILES_DIR, a.name);
    // A listed attachment whose file is already gone just loses its entry.
    if t.dir.join(task::FILES_DIR).join(&a.name).exists() {
        ctx.wt.ok(&["rm", "-q", "--", &rel])?;
    }
    t.meta.attachments.remove(i);
    t.touch();
    t.write_meta()?;
    ctx.wt.ok(&["add", "--", &t.rel()])?;
    ctx.wt
        .commit(&format!("task({}): detach {}", t.id(), a.name))?;
    println!("detached {}", a.name);
    Ok(())
}
