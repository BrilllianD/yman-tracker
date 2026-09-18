# Command semantics

Normative, as-built. Companion documents: [storage.md](storage.md),
[errors.md](errors.md).

## 1. What runs before a command body

`main` does three things in order (`src/main.rs`):

1. **Discover** the repository. Failure is `not inside a git repository`.
2. **Preflight**, for every command except `init`, `hooks` and `git`:
   `.yman` must be a linked worktree of this repo; its HEAD must be the
   symbolic ref `refs/yman/local`; `config.toml` must load and validate. For
   *mutating* commands, an unresolved sync merge stops the command with exit 3.
3. **Lazy refresh**, when `yman.refresh` is unset or `lazy`, for every command
   except `init`, `sync`, `refresh`, `status`, `hooks` and `git`. A failure here
   is downgraded to a `warning:` on stderr and never takes the real command
   down.

Mutating commands, for the purposes of step 2: `add`, `edit`, `set`, `start`,
`done`, `move`, `cancel`, `reopen`, `prio`, `rm`, `attach`, `detach`, `comment`. `sync` is excluded because
it handles `MERGE_HEAD` itself.

## 2. Commit messages

Every mutation commits immediately, with `--no-verify` so the user's own hooks
(possibly under `core.hooksPath`, and knowing nothing about `.yman`) cannot
interfere. GPG signing is deliberately left to the user's configuration.

| Command | Subject |
|---|---|
| `init` | `yman: init (<scheme>)` |
| `add` | `task({id}): add "{title}"` |
| `edit` | `task({id}): edit` |
| `set`, `start`, `done`, `move`, `cancel`, `reopen`, `prio` | `task({id}): set {pairs}` |
| `rm` | `task({id}): remove "{title}"` |
| `attach` | `task({id}): attach {name}[, {name}…]` |
| `detach` | `task({id}): detach {name}` |
| `comment` | `task({id}): comment` |
| `sync` snapshot | `yman: snapshot local changes` |
| `sync` renumber | `yman: renumber 2->3, 7->8 (sync collision)` |
| `sync` merge | `yman: merge origin refs/tasks/main` |

`{title}` has `"` replaced by `'`. `{pairs}` is space-joined, e.g.
`status=todo->doing priority=5->2 title tags=+ui,-auth comment` — a changed
title contributes the bare word `title`, a move with no field change
contributes `folder`, list fields contribute `+added,-removed`, and `-m`
contributes the bare word `comment`.

`Git::commit` treats "nothing to commit" as success: re-applying an identical
change inside the same second stages nothing, and the tree already says what the
caller asked for.

## 3. Reading

### `ls`

Touches no git. Filters, then sorts by `(priority, status index, id)`, where the
status index is the position in `config.statuses.list` and ids compare
numerically when they are numbers, by numeric tail when they share a `prefix-`,
lexically otherwise.

Default filtering hides tasks in any **terminal** status — `statuses.terminal`,
or the done status when that key is unset; `-a` includes them, and naming one
with `-s` includes it too. `-t` requires **all** the tags
given. Column headers are printed only when stdout is a terminal. Trailing empty
columns are omitted, and zero counts render blank rather than `0`.

`--json` emits one object per task —
`{id, priority, status, title, tags, assignee, created, updated, attachments, comments, dir}` —
and `{dir, error}` for broken folders. `dir` is relative to `.yman` and names the
status directory for a closed task (`done/5.1.fix-login`). The escaping is hand-rolled; there is no
`serde_json` dependency.

### `show`

Prints the header block, then the body, then `attachments` and `discussion`.
Empty sections are omitted entirely, including `links` and `related`.

### `path`

Prints the absolute folder path and nothing else, so `cd $(yman path 14)` works.
The lazy refresh still runs, silently.

### `log`

`-n` defaults to 20. With an id, it collects every path the task has ever had by
parsing `--diff-filter=R -M` rename records into a graph and walking backwards
from the current one, then limits the log to those paths — so a task that
changed priority or title, or moved into a status directory, keeps its full
history.

## 4. Writing

### `add`

Mints an id (see [storage.md §8](storage.md#8-id-schemes)), validates the status
against the config, dedupes tags, `--link`s and `--relate`d ids while keeping
their order, then writes `t.md` and `m.yml` with `created == updated`.
`-a/--assignee` is trimmed; blank means unassigned. With `-e`, the editor opens before the
first commit, so a title typed there renames the folder by plain rename — the
placeholder never enters git history.

### `edit`

Opens `$VISUAL`, else `$EDITOR`, else `vi`, split on whitespace so
`EDITOR="code --wait"` works. A non-zero editor exit aborts and leaves the file
alone. If the file no longer parses, the command fails but **does not revert the
user's text**; the next `sync` snapshots it. When nothing changed, it prints
`no changes` and commits nothing. A changed title re-slugs the folder with
`git mv`.

### `set` (and `start`, `done`, `move`, `cancel`, `reopen`, `prio`)

At least one flag is required, enforced by the argument parser. Changes are
applied in memory first, then:

1. A changed priority, slug, **or terminal boundary** renames or moves the
   folder with `git mv`. Crossing the boundary also prints
   `note: task folder is now <rel>` on stderr — stdout stays data, but someone
   who had `cd`'d into the folder needs to hear that it moved.
2. `updated` is touched; `m.yml` is rewritten; `t.md` too when the title moved.
   With `-m`, the text is appended to `d.md` exactly as `comment` would, with
   the same actor.
3. One commit, one printed line per change (`14: status todo -> doing`,
   `14: commented`).

List fields (`--tag/--untag`, `--link/--unlink`, `--relate/--unrelate`) are set
semantics with insertion order preserved: adding a value already present is not
a change, and removing one that was never there is quietly accepted. When
nothing at all changed, `set` prints `no changes` and commits nothing. `-m`
always counts as a change; an empty message is the `empty comment` error.

`start` and `done` resolve to `statuses.start` and `statuses.done`, falling back
to `list[1]` and `list.last()` when those are unset.

The folder's location is recomputed on every `set`, before the "nothing
changed" exit. Setting a task's status to the one it already has therefore
relocates a task that is in the wrong place — a hand edit, a resolved merge, or
a repository that raised its version with closed tasks already on disk — and
reports it as `folder <old> -> <new>`, with `folder` as the commit-subject
token. There is no separate repair command.

The remaining verbs are `set --status` with the status looked up for you:

| Command | Status | When it refuses |
|---|---|---|
| `move <id> <status>` | the one you name | unknown status, as for `set` |
| `cancel <id>` | `statuses.cancel` | the key is unset |
| `reopen <id>` | `statuses.default` | the task is not in a terminal status |

Every verb, and `prio`, takes `-m <text>` like `set`.

`cancel` has no fallback on purpose: picking one of several closed statuses by
position is the guesswork the named roles exist to remove.

### `rm`

On a terminal, prompts `remove task 14 "Fix login"? [y/N]`; anything but `y`/`Y`
aborts. Without a terminal and without `-f`, it refuses rather than assume
consent.

### `attach` / `detach`

Each source must be an existing regular file. `--name` applies to a single file
only, and must not contain a path separator. The `by` recorded in `m.yml` is
the actor (see `comment`). An existing attachment of the same
name needs `--force`. Files over 5 MiB produce a warning, never a refusal. An
attachment listed in `m.yml` whose file is already gone can still be detached —
the entry is simply dropped.

### `comment`

Text comes from `-m`, or from `$EDITOR` with `-e` (a temp file, initially
empty), or — when neither is given and stdin is not a terminal — from stdin.
Empty after trimming is an error. The author is `$YMAN_ACTOR` (trimmed,
non-empty), else `git config user.name`, else `unknown`. This is the display
name written into `d.md` only; the git committer is whatever git resolves.

## 5. Refresh

`refresh(quiet)` never touches the network. In order:

1. Either ref missing, or both equal → nothing to do.
2. `LOCAL` is not an ancestor of `REMOTE` → skip:
   `local has unpushed commits; run: yman sync`. Only `yman refresh` and
   `yman status` report this; the lazy path stays silent.
3. The worktree is dirty → skip, and print
   `note: .yman has uncommitted changes, refresh skipped` unless quiet.
4. Otherwise fast-forward (`merge --ff-only`) and report
   `refreshed: N new commit(s)` unless quiet.

`yman refresh` additionally prints `up to date` when there was nothing to do.

| `yman.refresh` | Effect |
|---|---|
| `lazy` (default) | every non-exempt command fast-forwards first |
| `manual` | only `yman refresh` and `yman sync` move `.yman` |

### Hooks

`yman hooks install` writes `post-merge` and `post-checkout`, each carrying the
marker line `# yman-hook v1`:

```sh
#!/bin/sh
# yman-hook v1
command -v yman >/dev/null 2>&1 && yman refresh --quiet
exit 0
```

The target directory is `core.hooksPath` when set (with a warning naming it),
otherwise `COMMON/hooks`. A hook that exists **without** the marker is never
touched: the command prints the one line to add and exits non-zero after
processing both hooks. `remove` deletes only marked files; `status` reports
`installed` / `foreign` / `absent`.

## 6. Sync

The only command that uses the network. Preflight runs with `mutating = false`;
`sync` inspects `MERGE_HEAD` itself.

### Normal path

1. A merge already in progress → exit 3.
2. A dirty worktree is committed as `yman: snapshot local changes`, reporting
   `snapshotted N local change(s)`.
3. Fetch `+refs/tasks/main:refs/yman/remote`. A stderr mentioning
   `couldn't find remote ref` means origin simply has no tasks yet; if we had a
   `REMOTE` before, it is deleted and the disappearance is reported. Any other
   failure is fatal, with git's stderr passed through.
4. No `REMOTE` → straight to the push.
5. No merge base → fatal: the histories are unrelated, and the message spells
   out the re-init recipe.
6. `base == remote` → nothing to merge. `base == local` → `merge --ff-only`.
7. Otherwise **renumber collisions** (below), then
   `merge --no-edit --no-verify`. Conflicts print the unmerged files and exit 3.
8. Push, unless `--no-push`. A rejected push means origin moved: loop back to
   step 3, at most three attempts, then fail with `origin keeps moving`.
9. Report `synced  pulled N, pushed N, renumbered N   refs/yman/local @ <sha>`.

After a successful push, `REMOTE` is set to `LOCAL`, so `status` and `refresh`
agree with what origin now holds.

### Collision renumbering

`seq` and `author` mint the same id on two machines working offline. The side
that is behind moves, because the other side's id is already published.

- Candidates are ids of tasks **created locally since the merge base** that also
  exist in `ls-tree -d -r REMOTE`. Created means the task's `t.md` appeared
  (`diff --diff-filter=A -M base LOCAL`): adding a comment or an attachment to
  an existing task adds a file under its folder but does not mint an id, and
  `-M` keeps a folder that merely moved from reading as a new one.
- The replacement comes from the same scheme, avoiding everything on disk, on
  the remote, and everything ever assigned on either ref. For `author`, the
  original prefix is kept.
- Each rename is a `git mv`, the task's `updated` is touched, and one commit
  covers the batch.
- Each move prints `renumbered {old} -> {new}  (id taken on origin)`.
- `related` references to the old id are rewritten to the new one, in the
  same commit as the moves, and `note: rewrote N reference(s) to renumbered
  ids` reports how many changed. A folder that does not load is left alone.
  The rewrite covers the renumbering clone's own tree, which is the pre-merge
  one: a reference held by another clone, or arriving in the same merge, keeps
  the old id and is left pointing at nothing.

### `--continue` and `--abort`

Both require a merge in progress.

`--continue` treats a file as unresolved while it still carries conflict
markers — staging is yman's job, since the message tells the user to edit and
rerun, not to run `git add`. It then checks that every task folder the merge
touched still loads and is marker-free, that no id has two folders, stages
everything, commits with `--no-edit`, and pushes.

The folder list comes from both `HEAD..MERGE_HEAD` and the unmerged paths,
because a rename conflict names paths that survive in neither tree.

### A task closed to two different statuses

Closing one task to `done` on one clone and `cancelled` on another renames it
two ways, and git resolves that by keeping **both** folders. Both load, and
neither holds a conflict marker, so every other check here passes — the
duplicate-id check is the only thing between that state and a pushed task no
command can touch afterwards. It reports

```
duplicate task id 1: cancelled/5.1.fix-login, done/5.1.fix-login; delete one folder, then: yman sync --continue
```

with exit 3, and `sync` names the same pair on stderr when the merge fails:

```
note: task 1 was closed to two different statuses; keep one of done/5.1.fix-login, cancelled/5.1.fix-login
```

Delete the folder you do not want, then rerun `yman sync --continue`.

`--abort` runs `git merge --abort` and reports `merge aborted`.

## 7. `git` passthrough

`yman git -- <args>` runs git inside `.yman` with inherited stdio and
propagates the exit code. It deliberately bypasses the `Git` wrapper, so the
user's own flags — including colour — behave normally.
