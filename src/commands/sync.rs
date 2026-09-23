use crate::cli::SyncArgs;
use crate::config::Scheme;
use crate::errors::MergePending;
use crate::ids;
use crate::repo::{Context, FETCH_REFSPEC, LOCAL, REMOTE, REMOTE_REF};
use crate::task::{self, FolderName};
use anyhow::{Result, bail};
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PushResult {
    Ok,
    UpToDate,
    Rejected,
}

/// `git push origin refs/yman/local:refs/tasks/main`, always explicit — we
/// never set `remote.origin.push`, which would hijack the user's plain
/// `git push`.
pub fn push(ctx: &Context) -> Result<PushResult> {
    let refspec = format!("{LOCAL}:{REMOTE_REF}");
    let out = ctx.main.run(&["push", "--no-verify", "origin", &refspec])?;
    if out.ok() {
        // Mirror what origin now has, so `status` and `refresh` agree.
        ctx.main.ok(&["update-ref", REMOTE, LOCAL])?;
        return Ok(if out.stderr.contains("Everything up-to-date") {
            PushResult::UpToDate
        } else {
            PushResult::Ok
        });
    }
    let err = out.stderr.to_lowercase();
    if err.contains("rejected") || err.contains("non-fast-forward") || err.contains("fetch first") {
        return Ok(PushResult::Rejected);
    }
    eprint!("{}", out.stderr);
    bail!("push failed")
}

pub fn run(ctx: &mut Context, a: SyncArgs) -> Result<()> {
    if a.abort {
        return abort(ctx);
    }
    if a.cont {
        return resume(ctx, a.no_push);
    }
    normal(ctx, a.no_push)
}

fn abort(ctx: &Context) -> Result<()> {
    if !ctx.merge_in_progress() {
        bail!("no merge in progress");
    }
    ctx.wt.ok(&["merge", "--abort"])?;
    println!("merge aborted");
    Ok(())
}

/// Finish a merge the user has resolved by hand.
fn resume(ctx: &mut Context, no_push: bool) -> Result<()> {
    if !ctx.merge_in_progress() {
        bail!("no merge in progress");
    }
    // A file counts as unresolved while it still carries markers. Staging is
    // ours to do — the message we printed told the user to edit and rerun,
    // not to run `git add`.
    let unmerged = ctx.wt.out(&["diff", "--name-only", "--diff-filter=U"])?;
    let still: Vec<&str> = unmerged
        .lines()
        .filter(|f| !f.trim().is_empty())
        .filter(|f| file_has_markers(&ctx.ydir.join(f)))
        .collect();
    if !still.is_empty() {
        return Err(MergePending::new(format!("still unmerged: {}", still.join(", "))).into());
    }
    // Staged into the merge commit itself, and before the checks, which
    // would otherwise read the left-behind folder as a duplicate id.
    rejoin_split_folders(ctx)?;
    check_resolved_tasks(ctx)?;

    ctx.wt.ok(&["add", "-A"])?;
    ctx.wt.ok(&["commit", "-q", "--no-verify", "--no-edit"])?;

    let mut totals = Totals::default();
    if !no_push {
        push_with_count(ctx, &mut totals)?;
    }
    summary(ctx, &totals, no_push)
}

/// Every task folder the merge touched must load, and must not still carry
/// conflict markers.
fn check_resolved_tasks(ctx: &Context) -> Result<()> {
    let changed = ctx.wt.out(&["diff", "--name-only", "HEAD", "MERGE_HEAD"])?;
    // A rename/rename conflict lists paths that exist in neither side's final
    // tree, so the unmerged list names folders the HEAD..MERGE_HEAD diff does
    // not.
    let unmerged = ctx.wt.out(&["diff", "--name-only", "--diff-filter=U"])?;
    let mut dirs: Vec<String> = Vec::new();
    for line in changed.lines().chain(unmerged.lines()) {
        if let Some((rel, _)) = task::task_path_of(line)
            && !dirs.contains(&rel)
        {
            dirs.push(rel);
        }
    }
    for dir in dirs {
        let path = ctx.ydir.join(&dir);
        if !path.exists() {
            // Resolved by deleting the task; nothing left to validate.
            continue;
        }
        if let Some(file) = has_conflict_markers(&path)? {
            return Err(MergePending::new(format!(
                "conflict markers or invalid task in {dir}: {file} still has merge markers"
            ))
            .into());
        }
        if let Err(e) = task::load(&ctx.ydir, &path) {
            return Err(MergePending::new(format!(
                "conflict markers or invalid task in {dir}: {e:#}"
            ))
            .into());
        }
    }
    duplicate_ids_gate(ctx)
}

/// Closing one task to two different statuses on two clones is a rename/rename
/// conflict, and git resolves it by keeping *both* folders. Both load, neither
/// carries a conflict marker, so every other check here passes and the merge
/// would be committed and pushed with two folders for one id — after which no
/// command can touch that task again. This is the only thing that catches it.
fn duplicate_ids_gate(ctx: &Context) -> Result<()> {
    let mut seen: HashMap<String, String> = HashMap::new();
    for entry in task::list(&ctx.ydir)? {
        let rel = entry.rel();
        let Some(f) = FolderName::parse(&entry.dir_name()) else {
            continue;
        };
        if let Some(first) = seen.insert(f.id.clone(), rel.clone()) {
            return Err(MergePending::new(format!(
                "duplicate task id {}: {first}, {rel}; delete one folder, then: \
                 yman sync --continue",
                f.id
            ))
            .into());
        }
    }
    Ok(())
}

fn file_has_markers(path: &std::path::Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false; // binary attachment, or gone because the user deleted it
    };
    text.lines().any(is_marker)
}

fn is_marker(l: &str) -> bool {
    l.starts_with("<<<<<<< ") || l == "=======" || l.starts_with(">>>>>>> ")
}

fn has_conflict_markers(dir: &std::path::Path) -> Result<Option<String>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = has_conflict_markers(&path)? {
                return Ok(Some(found));
            }
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue; // binary attachment
        };
        if text.lines().any(is_marker) {
            return Ok(Some(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

#[derive(Default)]
struct Totals {
    pulled: usize,
    pushed: usize,
    renumbered: usize,
}

fn normal(ctx: &mut Context, no_push: bool) -> Result<()> {
    if ctx.merge_in_progress() {
        return Err(MergePending::new(
            "merge in progress; resolve then: yman sync --continue  (or --abort)",
        )
        .into());
    }

    snapshot_dirty(ctx)?;

    let mut totals = Totals::default();
    let mut attempt = 0;
    loop {
        attempt += 1;
        let had_remote = ctx.main.rev_parse(REMOTE)?.is_some();
        fetch(ctx, had_remote)?;

        if let Some(remote) = ctx.main.rev_parse(REMOTE)? {
            let local = ctx.main.rev_parse(LOCAL)?.unwrap_or_default();
            let Some(base) = ctx.main.merge_base(LOCAL, REMOTE)? else {
                bail!(
                    "task history unrelated to origin {REMOTE_REF}; re-init from remote:  rm -rf .yman && git update-ref -d {LOCAL} && yman init"
                );
            };
            if base != remote {
                totals.pulled += count(ctx, &format!("{base}..{remote}"))?;
                if base == local {
                    ctx.wt.ok(&["merge", "-q", "--ff-only", REMOTE])?;
                } else {
                    totals.renumbered += renumber_collisions(ctx, &base)?;
                    merge_remote(ctx)?;
                    if rejoin_split_folders(ctx)? > 0 {
                        ctx.wt
                            .commit("yman: rejoin files left under a moved folder")?;
                    }
                }
            }
        }

        if no_push {
            break;
        }
        let before_push = ctx.main.rev_parse(REMOTE)?;
        match push(ctx)? {
            PushResult::Ok | PushResult::UpToDate => {
                let after = ctx.main.rev_parse(LOCAL)?.unwrap_or_default();
                totals.pushed += match before_push {
                    Some(before) => count(ctx, &format!("{before}..{after}"))?,
                    None => count(ctx, &after)?,
                };
                break;
            }
            PushResult::Rejected => {
                if attempt >= 3 {
                    bail!("origin keeps moving; retry yman sync");
                }
            }
        }
    }

    summary(ctx, &totals, no_push)
}

/// Uncommitted edits (hand-edited files, a half-finished `yman edit`) are
/// committed as they are rather than silently merged over.
fn snapshot_dirty(ctx: &Context) -> Result<()> {
    let porcelain = ctx.wt.out(&["status", "--porcelain"])?;
    let n = porcelain.lines().filter(|l| !l.trim().is_empty()).count();
    if n == 0 {
        return Ok(());
    }
    ctx.wt.ok(&["add", "-A"])?;
    ctx.wt.commit("yman: snapshot local changes")?;
    println!("snapshotted {n} local change(s)");
    Ok(())
}

fn fetch(ctx: &Context, had_remote: bool) -> Result<()> {
    let out = ctx.main.run(&["fetch", "origin", FETCH_REFSPEC])?;
    if out.ok() {
        return Ok(());
    }
    if out.stderr.contains("couldn't find remote ref") {
        if had_remote {
            ctx.main.ok(&["update-ref", "-d", REMOTE])?;
            eprintln!("warning: {REMOTE_REF} disappeared from origin; will recreate it");
        }
        return Ok(());
    }
    eprint!("{}", out.stderr);
    bail!("fetch failed")
}

fn merge_remote(ctx: &Context) -> Result<()> {
    // A retitle or a close moves a task's whole folder. A file the other side
    // added under the old folder — a first comment's `d.md`, an attachment —
    // belongs in the new one, and git's default for that is to stop and ask.
    let out = ctx.wt.run(&[
        "-c",
        "merge.directoryRenames=true",
        "merge",
        "--no-edit",
        "--no-verify",
        "-m",
        &format!("yman: merge origin {REMOTE_REF}"),
        REMOTE,
    ])?;
    if out.ok() {
        return Ok(());
    }
    if ctx.merge_in_progress() {
        let unmerged = ctx.wt.out(&["diff", "--name-only", "--diff-filter=U"])?;
        let files: Vec<&str> = unmerged.lines().filter(|l| !l.trim().is_empty()).collect();
        for f in &files {
            eprintln!("  {f}");
        }
        report_split_closes(ctx)?;
        return Err(MergePending::new(format!(
            "conflicts in {} file(s); edit them, remove markers, then: yman sync --continue  (or: yman sync --abort)",
            files.len()
        ))
        .into());
    }
    eprint!("{}", out.stderr);
    bail!("merge failed")
}

/// Name the two folders when a task was closed to a different status on each
/// side. The unmerged list alone is baffling here: it includes the task's old
/// path, which no longer exists on disk, and says nothing about the two folders
/// that now do.
fn report_split_closes(ctx: &Context) -> Result<()> {
    let mut seen: HashMap<String, String> = HashMap::new();
    for entry in task::list(&ctx.ydir)? {
        let rel = entry.rel();
        let Some(f) = FolderName::parse(&entry.dir_name()) else {
            continue;
        };
        if let Some(first) = seen.insert(f.id.clone(), rel.clone()) {
            eprintln!(
                "note: task {} was closed to two different statuses; keep one of {first}, {rel}",
                f.id
            );
        }
    }
    Ok(())
}

/// Git carries a file the other side added under a folder this side moved —
/// a retitle, a close — only when something was also added directly in that
/// folder. An attachment on its own is `f/<name>`, one level down, and stays
/// behind under the old name, in a folder with no `m.yml`: the task is split
/// in two and the merge reports success. Move whatever was left behind into
/// the folder that carries the task. Staged, not committed.
fn rejoin_split_folders(ctx: &Context) -> Result<usize> {
    let mut by_id: HashMap<String, Vec<String>> = HashMap::new();
    for (parent, name, folder) in task::task_dirs(&ctx.ydir)? {
        let rel = match parent {
            Some(p) => format!("{p}/{name}"),
            None => name,
        };
        by_id.entry(folder.id).or_default().push(rel);
    }
    let mut moved = 0;
    for rels in by_id.values().filter(|r| r.len() > 1) {
        let (live, left): (Vec<&String>, Vec<&String>) = rels
            .iter()
            .partition(|rel| ctx.ydir.join(rel).join(task::META_FILE).exists());
        // Two folders that both carry a task are a real conflict, and
        // `duplicate_ids_gate` is what speaks up about it.
        let [live] = live[..] else {
            continue;
        };
        for old in left {
            let files = ctx.wt.out(&["ls-files", "--", old])?;
            let mut n = 0;
            for file in files.lines().filter(|l| !l.trim().is_empty()) {
                let Some(sub) = file.strip_prefix(old.as_str()) else {
                    continue;
                };
                let dest = format!("{live}{sub}");
                if ctx.ydir.join(&dest).exists() {
                    eprintln!("warning: {file} not moved; {dest} already exists");
                    continue;
                }
                if let Some(dir) = ctx.ydir.join(&dest).parent() {
                    std::fs::create_dir_all(dir)?;
                }
                ctx.wt.ok(&["mv", "--", file, &dest])?;
                n += 1;
            }
            // `git mv` leaves the emptied directories behind.
            for dir in [ctx.ydir.join(old).join(task::FILES_DIR), ctx.ydir.join(old)] {
                let _ = std::fs::remove_dir(dir);
            }
            if n > 0 {
                eprintln!("note: moved {n} file(s) left under {old} into {live}");
            }
            moved += n;
        }
    }
    Ok(moved)
}

/// Every task id in `rev`'s tree, read from the folder names alone.
fn tree_ids(ctx: &Context, rev: &str) -> Result<HashSet<String>> {
    // `-r` as well as `-d`: a closed task is a tree one level down, and
    // without it only the status directory itself would be listed.
    let tree = ctx.wt.out(&["ls-tree", "-d", "-r", "--name-only", rev])?;
    let mut ids: HashSet<String> = HashSet::new();
    for name in tree.lines() {
        let name = name.trim();
        // `ls-tree` yields the folder itself, with no file under it, so ask
        // about a path one segment longer than what `task_path_of` needs.
        if let Some((rel, f)) = task::task_path_of(&format!("{name}/{}", task::META_FILE))
            && rel == name
        {
            ids.insert(f.id);
        }
    }
    Ok(ids)
}

/// Two people offline with the same id scheme will mint the same id. Ours
/// moves, because theirs is already published.
fn renumber_collisions(ctx: &mut Context, base: &str) -> Result<usize> {
    // A candidate is a task *created* here since the base: an id that the
    // local side has and the base did not. Reading that off the trees is the
    // whole point. The earlier version asked `git diff --diff-filter=A -M`
    // which `t.md` files had appeared, and that answer depends on git's
    // rename scoring: a retitle is a `git mv` plus a rewritten first line of a
    // short file, which scores well under the default 50% threshold, so the
    // pair read as a delete plus an add and the task looked new. Its own id
    // was then "taken on origin" — by itself — and sync renumbered it. The id
    // is in the folder name at both ends, so comparing id sets settles it
    // without consulting the rename detector at all.
    let base_ids = tree_ids(ctx, base)?;
    let remote_ids = tree_ids(ctx, REMOTE)?;

    let mut colliding: Vec<String> = tree_ids(ctx, LOCAL)?
        .into_iter()
        .filter(|id| !base_ids.contains(id) && remote_ids.contains(id))
        .collect();
    if colliding.is_empty() {
        return Ok(0);
    }
    colliding.sort_by(|a, b| task::cmp_id(a, b));

    let mut taken: HashSet<String> = ids::fs_ids(&ctx.ydir)?;
    taken.extend(remote_ids.iter().cloned());
    taken.extend(ids::ever_assigned(ctx, &[LOCAL, REMOTE])?);

    let scheme = ctx.config().ids.scheme;
    let mut pairs: Vec<String> = Vec::new();
    let mut moves: Vec<(String, String)> = Vec::new();
    for old in &colliding {
        let mut t = task::find(&ctx.ydir, old)?;
        let prefix = match scheme {
            Scheme::Author => ids::prefix_of(old).map(|p| p.to_string()),
            _ => None,
        };
        let new = ids::next_free(scheme, ctx.config(), prefix.as_deref(), &taken);
        taken.insert(new.clone());

        let old_rel = t.rel();
        t.folder.id = new.clone();
        let new_rel = t.folder.to_string();
        ctx.wt.ok(&["mv", "--", &old_rel, &new_rel])?;
        t.dir = ctx.ydir.join(&new_rel);
        t.touch();
        t.write_meta()?;
        ctx.wt.ok(&["add", "--", &new_rel])?;
        println!("renumbered {old} -> {new}  (id taken on origin)");
        pairs.push(format!("{old}->{new}"));
        moves.push((old.clone(), new));
    }

    let rewritten = rewrite_related(ctx, &moves)?;
    ctx.wt.commit(&format!(
        "yman: renumber {} (sync collision)",
        pairs.join(", ")
    ))?;
    if rewritten > 0 {
        eprintln!("note: rewrote {rewritten} reference(s) to renumbered ids");
    }
    Ok(colliding.len())
}

/// Point every `related:` entry that named a renumbered id at its new one.
/// Staged but not committed: the caller commits it together with the `git mv`
/// batch, so a failure in between cannot leave half a renumber on the ref.
fn rewrite_related(ctx: &Context, moves: &[(String, String)]) -> Result<usize> {
    let mut rewritten = 0;
    for entry in task::list(&ctx.ydir)? {
        // A folder that does not load is left alone; rewriting it would mean
        // parsing what we already failed to parse.
        let task::Entry::Task(mut t) = entry else {
            continue;
        };
        let mut related: Vec<String> = Vec::with_capacity(t.meta.related.len());
        let mut hits = 0;
        for id in &t.meta.related {
            let mapped = match moves.iter().find(|(old, _)| old == id) {
                Some((_, new)) => {
                    hits += 1;
                    new.clone()
                }
                None => id.clone(),
            };
            // A task related to both ids of a collision pair keeps one entry.
            if !related.contains(&mapped) {
                related.push(mapped);
            }
        }
        if hits == 0 {
            continue;
        }
        t.meta.related = related;
        t.touch();
        t.write_meta()?;
        let rel = t.rel();
        ctx.wt.ok(&["add", "--", &rel])?;
        rewritten += hits;
    }
    Ok(rewritten)
}

fn push_with_count(ctx: &Context, totals: &mut Totals) -> Result<()> {
    let before = ctx.main.rev_parse(REMOTE)?;
    match push(ctx)? {
        PushResult::Rejected => {
            bail!("origin moved while finishing the merge; run: yman sync")
        }
        _ => {
            let after = ctx.main.rev_parse(LOCAL)?.unwrap_or_default();
            totals.pushed = match before {
                Some(before) => count(ctx, &format!("{before}..{after}"))?,
                None => count(ctx, &after)?,
            };
            Ok(())
        }
    }
}

fn count(ctx: &Context, range: &str) -> Result<usize> {
    Ok(ctx
        .main
        .out(&["rev-list", "--count", range])?
        .trim()
        .parse()
        .unwrap_or(0))
}

fn summary(ctx: &Context, totals: &Totals, no_push: bool) -> Result<()> {
    let head = ctx
        .main
        .out(&["rev-parse", "--short", LOCAL])
        .unwrap_or_default();
    let pushed = if no_push { 0 } else { totals.pushed };
    println!(
        "synced  pulled {}, pushed {}, renumbered {}   {LOCAL} @ {head}",
        totals.pulled, pushed, totals.renumbered
    );
    Ok(())
}
