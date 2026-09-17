use crate::cli::AddArgs;
use crate::ids;
use crate::repo::Context;
use crate::task::{self, FolderName, Meta, Task};
use anyhow::{Result, bail};

use super::edit::open_editor;
use super::quote_title;

pub fn run(ctx: &mut Context, a: AddArgs) -> Result<()> {
    let title = a.title.trim().to_string();
    if title.is_empty() {
        bail!("title must not be empty");
    }

    let status = match a.status {
        Some(s) => {
            if !ctx.config().has_status(&s) {
                bail!(
                    "unknown status \"{s}\"; allowed: {}",
                    ctx.config().statuses_joined()
                );
            }
            s
        }
        None => ctx.config().statuses.default.clone(),
    };
    let priority = a.priority.unwrap_or(ctx.config().priorities.default);

    let mut tags: Vec<String> = Vec::new();
    for t in a.tags {
        if !tags.contains(&t) {
            tags.push(t);
        }
    }

    let id = ids::new_id(ctx)?;
    let folder = FolderName {
        priority,
        id: id.clone(),
        slug: task::slugify(&title, ctx.config().slug.max_bytes),
    };
    let dir = ctx.ydir.join(folder.to_string());
    if dir.exists() {
        bail!("folder already exists: {}", folder);
    }

    std::fs::create_dir_all(&dir)?;
    let mut t = Task {
        folder,
        dir,
        title,
        body: a.message.unwrap_or_default(),
        meta: Meta::new(status, tags),
    };
    t.write_md()?;
    t.write_meta()?;

    if a.edit {
        open_editor(&t.dir.join(task::MD_FILE))?;
        let edited = task::load(&t.dir)?;
        t.title = edited.title;
        t.body = edited.body;
        // The title may have changed; the folder is not tracked yet, so this
        // is a plain rename rather than a `git mv`.
        let slug = task::slugify(&t.title, ctx.config().slug.max_bytes);
        if slug != t.folder.slug {
            let new_dir = ctx.ydir.join(format!("{}.{}.{}", t.folder.priority, t.folder.id, slug));
            std::fs::rename(&t.dir, &new_dir)?;
            t.folder.slug = slug;
            t.dir = new_dir;
        }
    }

    ctx.wt.ok(&["add", "--", &t.rel()])?;
    ctx.wt
        .commit(&format!("task({id}): add \"{}\"", quote_title(&t.title)))?;
    println!("added {id}  {}", t.rel());
    Ok(())
}
