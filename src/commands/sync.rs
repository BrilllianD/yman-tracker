use crate::cli::SyncArgs;
use crate::config::{Config, Scheme};
use crate::errors::MergePending;
use crate::ids;
use crate::repo::{Context, LOCAL, REMOTE, REMOTE_REF};
use crate::task::{self, FolderName};
use crate::transport::{self, PUSH_ATTEMPTS, PushResult, push};
use anyhow::{Context as _, Result, bail};
use std::collections::{HashMap, HashSet};

pub fn run(ctx: &mut Context, a: SyncArgs) -> Result<()> {
    if a.abort {
        return abort(ctx);
    }
    // Checked up front: git's own "'origin' does not appear to be a git
    // repository" would only surface as "fetch failed".
    if ctx.origin_url()?.is_none() {
        bail!("no \"origin\" remote; tasks are local only. Connect one: yman init --remote <url>");
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
    forget_marked(ctx);
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
    // not to run `git add`. The exception is a conflict git never marked up
    // (binary, modify/delete, a failed merge driver): with no markers to
    // remove, the file on disk is just ours, and taking it would drop theirs
    // without a word. That one stays unmerged until the user stages a side.
    let marked = read_marked(ctx);
    let mut still: Vec<String> = Vec::new();
    let mut unmarked: Vec<String> = Vec::new();
    for (f, stages) in unmerged_paths(ctx)? {
        if unresolved(ctx, &f)? {
            still.push(f);
        } else if needs_a_side(stages) && !marked.contains(&f) {
            unmarked.push(f.clone());
            still.push(f);
        }
    }
    if !still.is_empty() {
        let mut msg = format!("still unmerged: {}", still.join(", "));
        if !unmarked.is_empty() {
            msg.push_str(&format!(
                "; no markers to remove in {}, stage the version to keep: \
                 yman git checkout --ours|--theirs -- <file> && yman git add <file>",
                unmarked.join(", ")
            ));
        }
        return Err(MergePending::new(msg).into());
    }
    // `preflight` let us through with whatever config.toml the merge left;
    // a hand-resolution that does not load must not be committed and pushed.
    ctx.config = Some(Config::load(&ctx.ydir).map_err(|e| MergePending::new(format!("{e:#}")))?);
    // Staged into the merge commit itself, and before the checks, which
    // would otherwise read the left-behind folder as a duplicate id, or a
    // removed task's remnant as an unloadable task.
    rejoin_split_folders(ctx)?;
    if let Some(base) = ctx.wt.merge_base("HEAD", "MERGE_HEAD")? {
        drop_deleted_remnants(ctx, &base, "HEAD", "MERGE_HEAD")?;
    }
    check_resolved_tasks(ctx)?;

    // What the merge brings in, counted before the commit makes it ours.
    let mut totals = Totals {
        pulled: ctx
            .wt
            .out(&["rev-list", "--count", "HEAD..MERGE_HEAD"])?
            .trim()
            .parse()
            .unwrap_or(0),
        ..Totals::default()
    };
    ctx.wt.ok(&["add", "-A"])?;
    ctx.wt.commit_merge()?;
    forget_marked(ctx);

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

/// In the `.yman` worktree's private git dir: the unmerged files that carried
/// conflict markers when the merge stopped. A file on this list whose markers
/// are gone was resolved by hand; an unmerged file that is not on it never had
/// markers to remove.
const MARKED_FILE: &str = "YMAN_MARKED";

fn read_marked(ctx: &Context) -> HashSet<String> {
    std::fs::read_to_string(ctx.wt_gitdir.join(MARKED_FILE))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn forget_marked(ctx: &Context) {
    let _ = std::fs::remove_file(ctx.wt_gitdir.join(MARKED_FILE));
}

/// Unmerged paths, in index order, with which of stages 1 (base), 2 (ours)
/// and 3 (theirs) each one holds.
fn unmerged_paths(ctx: &Context) -> Result<Vec<(String, [bool; 3])>> {
    let out = ctx.wt.out(&["ls-files", "-u", "-z"])?;
    let mut paths: Vec<(String, [bool; 3])> = Vec::new();
    // `<mode> <hash> <stage>\t<path>`, NUL-terminated.
    for entry in out.split('\0').filter(|e| !e.is_empty()) {
        let Some((head, path)) = entry.split_once('\t') else {
            continue;
        };
        let stage = match head.rsplit(' ').next() {
            Some("1") => 0,
            Some("2") => 1,
            Some("3") => 2,
            _ => continue,
        };
        match paths.last_mut() {
            Some((p, stages)) if p == path => stages[stage] = true,
            _ => {
                let mut stages = [false; 3];
                stages[stage] = true;
                paths.push((path.to_string(), stages));
            }
        }
    }
    Ok(paths)
}

/// Both sides changed the content (stages 2 and 3), or one side changed what
/// the other deleted (base plus exactly one side). Either way the file on
/// disk is one side's version and the other is lost unless someone picks.
/// A rename conflict's paths hold one side and no base, and are settled by
/// the folder checks instead.
fn needs_a_side([base, ours, theirs]: [bool; 3]) -> bool {
    (ours && theirs) || (base && ours != theirs)
}

fn file_has_markers(path: &std::path::Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false; // binary attachment, or gone because the user deleted it
    };
    text.lines().any(is_marker)
}

/// Markers left in an unmerged file, or a separator its sides never had.
/// The three stage reads happen only when the file has a bare `=======`.
fn unresolved(ctx: &Context, f: &str) -> Result<bool> {
    let Ok(text) = std::fs::read_to_string(ctx.ydir.join(f)) else {
        return Ok(false); // binary, or deleted by the user
    };
    if text.lines().any(is_marker) {
        return Ok(true);
    }
    if separator_lines(&text) == 0 {
        return Ok(false);
    }
    // Bare separators in stages 1 (base), 2 (ours) and 3 (theirs); a stage a
    // side deleted counts as none.
    let mut stages = [0; 3];
    for (n, count) in stages.iter_mut().enumerate() {
        let out = ctx
            .wt
            .run(&["cat-file", "blob", &format!(":{}:{f}", n + 1)])?;
        if out.ok() {
            *count = separator_lines(&out.stdout);
        }
    }
    Ok(leftover_separator(&text, stages))
}

fn separator_lines(text: &str) -> usize {
    text.lines().filter(|l| *l == "=======").count()
}

/// An unmerged file whose markers were removed by hand still has a
/// forgotten separator when it holds more bare `=======` lines than a merge
/// of the sides' own could: those of ours plus theirs, minus the ones both
/// inherited from the base (counted twice otherwise), and never fewer than
/// either side alone. Every other one is content.
fn leftover_separator(text: &str, [base, ours, theirs]: [usize; 3]) -> bool {
    let allowed = (ours + theirs).saturating_sub(base).max(ours).max(theirs);
    separator_lines(text) > allowed
}

/// A line only a conflict writes. A bare `=======` is not one: a setext
/// heading or a pasted separator is ordinary content; see
/// `leftover_separator`. `|||||||` opens the base section
/// `merge.conflictStyle=diff3` and `zdiff3` write.
fn is_marker(l: &str) -> bool {
    l.starts_with("<<<<<<< ")
        || l.starts_with(">>>>>>> ")
        || l == "|||||||"
        || l.starts_with("||||||| ")
}

fn has_conflict_markers(dir: &std::path::Path) -> Result<Option<String>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        // `file_type` does not follow symlinks, and a symlink is skipped:
        // git stores its target, not a file to scan, and `is_dir` would
        // follow a committed `f/loop -> ..` forever.
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
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
        transport::fetch(ctx, "fetch failed")?;

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
                    // Pinned before the rejoin, whose commit moves HEAD off
                    // the merge: `HEAD^2` would then name nothing.
                    let Some(merge) = ctx.wt.rev_parse("HEAD")? else {
                        bail!("merge left no HEAD in .yman");
                    };
                    if rejoin_split_folders(ctx)? > 0 {
                        ctx.wt
                            .commit("yman: rejoin files left under a moved folder")?;
                    }
                    let (ours, theirs) = (format!("{merge}^1"), format!("{merge}^2"));
                    if drop_deleted_remnants(ctx, &base, &ours, &theirs)? > 0 {
                        ctx.wt
                            .commit("yman: drop files left under a removed task")?;
                    }
                }
            }
        }

        if no_push {
            break;
        }
        let before_push = ctx.main.rev_parse(REMOTE)?;
        match push(ctx)? {
            PushResult::Ok(after) | PushResult::UpToDate(after) => {
                totals.pushed += match before_push {
                    Some(before) => count(ctx, &format!("{before}..{after}"))?,
                    None => count(ctx, &after)?,
                };
                break;
            }
            PushResult::Rejected => {
                if attempt >= PUSH_ATTEMPTS {
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
        // A merge driver that could not run says why only here (`sh: ...:
        // not found`), and nothing else would tell the user.
        eprint!("{}", out.stderr);
        // The merge is stopped; whatever fails now must still exit 3, or the
        // user is left mid-merge with no hint how to leave it.
        let n = report_stopped_merge(ctx).map_err(|e| {
            MergePending::new(format!(
                "merge stopped with conflicts, but yman could not list them: {e:#}; resolve then: yman sync --continue  (or: yman sync --abort)"
            ))
        })?;
        return Err(MergePending::new(format!(
            "conflicts in {n} file(s); edit them, remove markers, then: yman sync --continue  (or: yman sync --abort)"
        ))
        .into());
    }
    eprint!("{}", out.stderr);
    bail!("merge failed")
}

/// List the unmerged files, remember which ones git marked up, and name split
/// closes. Returns the number of unmerged files.
fn report_stopped_merge(ctx: &Context) -> Result<usize> {
    let files = unmerged_paths(ctx)?;
    eprintln!("note: unmerged files:");
    let mut marked = String::new();
    for (f, stages) in &files {
        if file_has_markers(&ctx.ydir.join(f)) {
            marked.push_str(f);
            marked.push('\n');
            eprintln!("  {f}");
        } else if needs_a_side(*stages) {
            eprintln!(
                "  {f}  (no conflict markers: binary, modify/delete, or the merge driver failed)"
            );
        } else {
            eprintln!("  {f}");
        }
    }
    let path = ctx.wt_gitdir.join(MARKED_FILE);
    std::fs::write(&path, marked).with_context(|| format!("cannot write {}", path.display()))?;
    report_split_closes(ctx)?;
    Ok(files.len())
}

/// Name the two folders when a task was closed to a different status on each
/// side. The unmerged list alone is baffling here: it includes the task's old
/// path, which no longer exists on disk, and says nothing about the two folders
/// that now do.
///
/// Only folders that carry the task (`m.yml`) under two different closed
/// statuses count. A stopped merge can also leave a file the other side added
/// under a folder this side retitled or closed; that remnant has no `m.yml`,
/// `rejoin_split_folders` moves it once the merge goes through, and calling it
/// a split close would point at a fix that does not apply.
fn report_split_closes(ctx: &Context) -> Result<()> {
    let cfg = ctx.config();
    let mut seen: HashMap<String, (String, String)> = HashMap::new();
    for (parent, name, folder) in task::task_dirs(&ctx.ydir)? {
        let Some(status) = parent.filter(|s| cfg.is_terminal(s)) else {
            continue;
        };
        let rel = format!("{status}/{name}");
        if !ctx.ydir.join(&rel).join(task::META_FILE).exists() {
            continue;
        }
        match seen.get(&folder.id) {
            Some((first_status, first)) if *first_status != status => eprintln!(
                "note: task {} was closed to two different statuses; keep one of {first}, {rel}",
                folder.id
            ),
            Some(_) => {}
            None => {
                seen.insert(folder.id, (status, rel));
            }
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

/// A task removed on one side while the other added a file to it — a first
/// comment's `d.md`, an attachment — merges cleanly: git sees a delete and an
/// unrelated add. What is left is a folder with neither `t.md` nor `m.yml`,
/// which no command can load. The removal wins, as it does inside `m.yml`
/// ("removed on one side goes"), and ids are never reused, so the files go
/// too, each with a note. Only folders whose id the base had and one side
/// dropped qualify; a folder broken by hand is left alone. Staged, not
/// committed. Costs no git process unless such a folder exists.
fn drop_deleted_remnants(ctx: &Context, base: &str, ours: &str, theirs: &str) -> Result<usize> {
    let mut remnants: Vec<(String, String)> = Vec::new();
    for (parent, name, folder) in task::task_dirs(&ctx.ydir)? {
        let rel = match parent {
            Some(p) => format!("{p}/{name}"),
            None => name,
        };
        let dir = ctx.ydir.join(&rel);
        if !dir.join(task::MD_FILE).exists() && !dir.join(task::META_FILE).exists() {
            remnants.push((rel, folder.id));
        }
    }
    if remnants.is_empty() {
        return Ok(0);
    }
    let base_ids = tree_ids(ctx, base)?;
    let our_ids = tree_ids(ctx, ours)?;
    let their_ids = tree_ids(ctx, theirs)?;
    let mut dropped = 0;
    for (rel, id) in remnants {
        if !base_ids.contains(&id) || (our_ids.contains(&id) && their_ids.contains(&id)) {
            continue;
        }
        let files = ctx.wt.out(&["ls-files", "--", &rel])?;
        let n = files.lines().filter(|l| !l.trim().is_empty()).count();
        // `yman rm` leaves ignored and untracked files behind (`*.swp`, a note
        // dropped in by hand), so the folder can outlive the task with nothing
        // tracked in it. `git rm` would fail on that pathspec, and there is
        // nothing for it to do, nor anything the other side lost to report.
        if n > 0 {
            ctx.wt.ok(&["rm", "-r", "-q", "--", &rel])?;
        }
        // `git rm` leaves the emptied directories; anything untracked stays.
        let dir = ctx.ydir.join(&rel);
        for d in [dir.join(task::FILES_DIR), dir] {
            let _ = std::fs::remove_dir(d);
        }
        if n == 0 {
            continue;
        }
        eprintln!(
            "note: dropped {rel}: task {id} was removed on one side; {n} file(s) added on the other are gone"
        );
        dropped += 1;
    }
    Ok(dropped)
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

    // Plan first: every lookup that can fail (a task that does not load, an
    // exhausted id space) fails here, before the first `git mv`.
    let scheme = ctx.config().ids.scheme;
    let mut plan: Vec<(task::Task, String)> = Vec::new();
    for old in &colliding {
        let t = task::find(&ctx.ydir, old)?;
        let prefix = match scheme {
            Scheme::Author => ids::prefix_of(old).map(|p| p.to_string()),
            _ => None,
        };
        let new = ids::next_free(scheme, ctx.config(), prefix.as_deref(), &taken)?;
        taken.insert(new.clone());
        plan.push((t, new));
    }

    // Then apply. `snapshot_dirty` committed every user file before this, so
    // HEAD is a complete restore point: a failure part-way (a write refused,
    // a `git mv` that fails) resets to it instead of leaving half a renumber
    // for the next snapshot to commit.
    let moves = match apply_renumber(ctx, plan) {
        Ok(moves) => moves,
        Err(e) => {
            let reset = ctx.wt.run(&["reset", "-q", "--hard", "HEAD"])?;
            // `git mv` leaves the new folder behind once the index forgets
            // it; without `-x`, so ignored files survive.
            let clean = ctx.wt.run(&["clean", "-q", "-f", "-d"])?;
            if !reset.ok() || !clean.ok() {
                return Err(e.context(
                    "renumber aborted; restoring .yman failed too, see: yman git status",
                ));
            }
            return Err(e.context("renumber aborted; .yman restored"));
        }
    };
    for (old, new) in &moves {
        println!("renumbered {old} -> {new}  (id taken on origin)");
    }
    Ok(moves.len())
}

/// The writing half of `renumber_collisions`: move each folder, rewrite its
/// `m.yml` and every `related` that named it, and commit the batch.
fn apply_renumber(ctx: &Context, plan: Vec<(task::Task, String)>) -> Result<Vec<(String, String)>> {
    let mut moves: Vec<(String, String)> = Vec::new();
    for (mut t, new) in plan {
        let old = t.id().to_string();
        let old_rel = t.rel();
        t.folder.id = new.clone();
        // `rel`, not the bare folder: a closed task stays in its status
        // directory.
        let new_rel = t.rel();
        ctx.wt.ok(&["mv", "--", &old_rel, &new_rel])?;
        t.dir = ctx.ydir.join(&new_rel);
        t.touch();
        t.write_meta()
            .map_err(|e| e.context(format!("cannot write {new_rel}/{}", task::META_FILE)))?;
        ctx.wt.ok(&["add", "--", &new_rel])?;
        moves.push((old, new));
    }

    let rewritten = rewrite_related(ctx, &moves)?;
    let pairs: Vec<String> = moves.iter().map(|(o, n)| format!("{o}->{n}")).collect();
    ctx.wt.commit(&format!(
        "yman: renumber {} (sync collision)",
        pairs.join(", ")
    ))?;
    if rewritten > 0 {
        eprintln!("note: rewrote {rewritten} reference(s) to renumbered ids");
    }
    Ok(moves)
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
        PushResult::Ok(after) | PushResult::UpToDate(after) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_separator_is_not_a_marker() {
        assert!(!is_marker("======="));
        assert!(is_marker("<<<<<<< HEAD"));
        assert!(leftover_separator("a\n=======\nb\n", [0, 0, 0]));
        assert!(!leftover_separator("Summary\n=======\n", [0, 1, 0]));
        assert!(leftover_separator("x\n=======\ny\n=======\n", [0, 1, 0]));
        // A heading both sides inherited is one separator, not two.
        let shared = "Summary\n=======\nx\n=======\ny\n";
        assert!(leftover_separator(shared, [1, 1, 1]));
        assert!(!leftover_separator("Summary\n=======\nx\n", [1, 1, 1]));
        // A side's own new heading is allowed on top of the shared one.
        assert!(!leftover_separator(shared, [1, 2, 1]));
    }

    #[test]
    fn is_marker_knows_the_diff3_base_line() {
        assert!(is_marker("||||||| 66a9ddb"));
        assert!(is_marker("|||||||"));
        assert!(!is_marker("|| x"));
    }
}
