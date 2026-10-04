use crate::cli::AddArgs;
use crate::ids;
use crate::plan::{self, BLOCKED};
use crate::repo::Context;
use crate::sections;
use crate::tags;
use crate::task::{self, FolderName, Meta, Task};
use anyhow::{Result, anyhow, bail};

use super::edit::run_editor;
use super::quote_title;

/// What every task created by one `add` shares: all of it but the title
/// and the body.
struct Spec {
    status: String,
    priority: u8,
    tags: Vec<String>,
    assignee: Option<String>,
    links: Vec<String>,
    related: Vec<String>,
    edit: bool,
}

pub fn run(ctx: &mut Context, a: AddArgs) -> Result<()> {
    let title = a.title.as_deref().map(task::clean_title).transpose()?;
    // Before the editor, before stdin, before an id is minted: a bad tag must
    // not cost the user a burned id or a half-written folder.
    let tags = tags::normalize_all(&a.tags)?;
    if !a.waits_on.is_empty() {
        plan::require_blocked(ctx.config())?;
    }
    // Before an id is minted, so the warning cannot read as being about the
    // task we are creating.
    let related = super::dedupe([a.related, a.waits_on.clone()].concat());
    super::warn_unknown_related(ctx, &related, &[])?;
    if a.edit {
        // Refuse before the folder exists; see `edit::resolve_editor`.
        super::edit::resolve_editor()?;
    }
    // `--sections` is read and parsed whole before the first id is minted:
    // a bad file costs nothing.
    let items: Vec<(String, String)> = match (title, a.sections) {
        (Some(title), _) => {
            let body = match a.body_file {
                Some(src) => super::read_text_source(&src)?,
                None => a.message.unwrap_or_default(),
            };
            vec![(title, body)]
        }
        (None, Some(src)) => {
            // Every heading is checked before the first task is created, so
            // a bad one in the middle leaves none of the others behind.
            let mut items = sections::parse(&super::read_text_source(&src)?, &src)?;
            for (title, _) in &mut items {
                *title = task::clean_title(title)?;
            }
            items
        }
        (None, None) => unreachable!("clap requires a title or --sections"),
    };

    let status = match a.status {
        // `--waits-on` is `-s blocked` plus `--relate`; clap keeps it apart
        // from `-s`.
        None if !a.waits_on.is_empty() => BLOCKED.to_string(),
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

    let assignee = a
        .assignee
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let spec = Spec {
        status,
        priority,
        tags,
        assignee,
        links: super::dedupe(a.links),
        related,
        edit: a.edit,
    };
    // One commit per task, as the multi-id verbs do; the first failure stops
    // the run with the tasks before it already committed.
    for (title, body) in items {
        create(ctx, &spec, title, body)?;
    }
    Ok(())
}

fn create(ctx: &mut Context, spec: &Spec, title: String, body: String) -> Result<()> {
    let id = ids::new_id(ctx)?;
    let folder = FolderName {
        priority: spec.priority,
        id: id.clone(),
        slug: task::slugify(&title, ctx.config().slug.max_bytes),
    };
    // A task added straight into a closed status is born in its archive
    // directory rather than being created and immediately moved.
    let parent = ctx.config().archive_dir(&spec.status).map(str::to_string);
    let dir = match &parent {
        Some(p) => ctx.ydir.join(p).join(folder.to_string()),
        None => ctx.ydir.join(folder.to_string()),
    };
    if dir.exists() {
        bail!("folder already exists: {}", folder);
    }

    std::fs::create_dir_all(&dir)?;
    let mut meta = Meta::new(spec.status.clone(), spec.tags.clone());
    meta.assignee = spec.assignee.clone();
    meta.links = spec.links.clone();
    meta.related = spec.related.clone();
    let mut t = Task {
        folder,
        parent,
        dir,
        title,
        body,
        meta,
    };
    t.write_md()?;
    t.write_meta()?;

    if spec.edit {
        let edited = run_editor(&t.dir.join(task::MD_FILE), "task not added").and_then(|()| {
            task::load(&ctx.ydir, &t.dir)
                .and_then(|edited| task::clean_title(&edited.title).map(|_| edited))
                .map_err(|e| anyhow!("t.md invalid after edit: {e:#}; task not added"))
        });
        let edited = match edited {
            Ok(edited) => edited,
            Err(e) => {
                // Nothing is tracked or committed yet, so the folder is all
                // there is to undo. Left behind, `ls` would list it, the
                // lazy refresh would skip a dirty tree, and the next `sync`
                // would snapshot and publish the task the user abandoned.
                if let Err(rm) = std::fs::remove_dir_all(&t.dir) {
                    eprintln!("warning: cannot remove {}: {rm}", t.rel());
                }
                return Err(e);
            }
        };
        t.title = edited.title;
        t.body = edited.body;
        // The title may have changed; the folder is not tracked yet, so this
        // is a plain rename rather than a `git mv`.
        let slug = task::slugify(&t.title, ctx.config().slug.max_bytes);
        if slug != t.folder.slug {
            let name = format!("{}.{}.{}", t.folder.priority, t.folder.id, slug);
            let new_dir = match &t.parent {
                Some(p) => ctx.ydir.join(p).join(&name),
                None => ctx.ydir.join(&name),
            };
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
