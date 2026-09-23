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

`guide`, `completions` and `man` run none of the three. They are
documentation compiled into the binary and are answered before the repository
is looked for, so they work from any directory.

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
| `tags rename` | `yman: tags rename {old} -> {new} ({n} tasks)` |
| `tags rm` | `yman: tags remove {tag} ({n} tasks)` |
| `sync` snapshot | `yman: snapshot local changes` |
| `sync` renumber | `yman: renumber 2->3, 7->8 (sync collision)` |
| `sync` merge | `yman: merge origin refs/tasks/main` |
| `sync` rejoin | `yman: rejoin files left under a moved folder` |

`{title}` has `"` replaced by `'`. `{pairs}` is space-joined, e.g.
`status=todo->doing priority=5->2 title tags=+ui,-auth comment` — a changed
title contributes the bare word `title`, a move with no field change
contributes `folder`, list fields contribute `+added,-removed`, a changed
body contributes the bare word `body`, and `-m` contributes the bare word
`comment`.

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
with `-s` includes it too. `-t` requires **all** the tags given, compared
lowercase on both sides — the tag yman writes is lowercase anyway, and one
written into `m.yml` by hand is still found. An invalid tag at `-t` is the same
error `add` gives.
`--assignee WHO` keeps one assignee (`-` keeps the unassigned), `-p N` one
priority, and `-q TEXT` the tasks whose title or body contains the text,
compared lowercase; all of these AND together with the status and tag
filters. `-n N` keeps the first N rows **after** sorting. None of the filters
touches git or reads anything `ls` did not already read. Broken folders are
listed regardless of filters. Column headers are printed only when stdout is
a terminal. Trailing empty
columns are omitted, and zero counts render blank rather than `0`.

`-l` prints each task's body under its row, every line indented by four spaces
and blank lines left empty, so a row is still the only line starting in column
0. A task with no body gets no extra lines. The body is the one `ls` already
read for `-q`, so `-l` costs no git either.

`--json` emits one object per task —
`{id, priority, status, title, tags, assignee, links, related, created, updated, attachments, comments, dir}` —
and `{dir, error}` for broken folders. With `-l` each task object also carries
`body`, the same trimmed text `show --json` gives; without it the key is
absent. `dir` is relative to `.yman` and names the status directory for a closed task (`done/5.1.fix-login`). `attachments` and
`comments` are counts here, not lists, and the attachment count comes from
`m.yml` alone: an entry whose file has been deleted outside yman still counts,
because `ls` stats nothing under `f/`. Use `show` to see which one is gone. The
writer is `src/json.rs`: the escaping is hand-rolled and there is no
`serde_json` dependency.

### `show`

Prints the header block, then the body, then `attachments` and `discussion`.
Empty sections are omitted entirely, including `links` and `related`.

An attachment whose file is gone — deleted outside yman, since `detach` takes
the entry with it — is still listed, with ` (missing)` after the
`(added … by …)` parenthesis. The test is a plain existence check on
`f/<name>`, one per attachment and only in `show`; it never errors and never
rewrites `m.yml`. `detach` drops such an entry as usual (§4).

`-n N` keeps only the last N discussion entries (a chunk `d.md` could not
parse counts as one) under the header `discussion (last N of M):`; `-n 0`
drops the section. When N is not smaller than the number of entries the
output is identical to the default, header included.

`--json` emits one object —
`{id, priority, status, title, tags, assignee, links, related, created, updated,
dir, body, attachments, discussion, discussion_total}`. `attachments` is
`[{name, added, by, missing}]`, where `missing` is the same existence check the
text form marks, always present as `true` or `false`; `discussion` is
`[{ts, author, text}]`, with an
unparsable chunk appearing as `{raw}`; `discussion_total` is the count before
`-n` trimmed anything. Unlike the text form nothing is omitted: an empty
section is an empty array and an absent assignee is `null`, so a script never
has to branch on a missing key. `dir` is relative to `.yman`, as in `ls --json`,
where the text form prints an absolute path.

### `status`

Reports and never changes anything: the local ref and its short head, the
remote ref with `ahead`/`behind` when it has been fetched, the refresh policy,
whether the hooks are installed, the number of uncommitted changes under
`.yman`, an unresolved merge with its unmerged files, and the task counts per
configured status.

`--json` emits one object —
`{local: {ref, head}, remote: {ref, fetched, head, ahead, behind}, refresh,
hooks, worktree: {dirty}, merge: {in_progress, unmerged}, tasks: {by_status,
other, broken}}`. Before the first fetch `fetched` is `false`, `head` is `null`
and both counters are `0`. The per-status counts are nested under `by_status`
so a status named `other` or `broken` cannot collide with the two totals beside
it.

### `tags`

An inventory of the tags in use: one row per tag with the number of tasks
carrying it, sorted by name. Touches no git, and counts **every** task on disk
— closed ones included, unlike `ls`, because the question is what tags exist,
not what is open. Tags are folded to lowercase before counting, so a hand-written
`UI` lands in the same row as `ui`, and a task that spells one tag two ways
counts once. Column headers follow the `ls` rule: terminal only.

    TAG   N
    auth  1
    ui    3

`--json` emits `[{tag, tasks}]` in the same order. Folders that would not load
are skipped with `warning: skipped <n> unreadable task folder(s): <rels>` on
stderr; `ls` is the command that lists them properly.

`tags rename` and `tags rm` edit the vocabulary itself; they are under
[§4](#4-writing).

### `path`

Prints the absolute folder path and nothing else, so `cd $(yman path 14)` works.
The lazy refresh still runs, silently.

### `log`

`-n` defaults to 20. With an id, the log is limited to the pathspecs
`:(glob)[0-9].<id>.*/**` and `:(glob)*/[0-9].<id>.*/**`. The id is in the folder
name through a priority change, a retitle and a move into or out of a status
directory, so those keep the task's full history without consulting git's
rename detection, and `-n` stops the walk once enough commits are found.

A sync renumber is the one move that changes the id, and `log` reads it from
the commit subject (`yman: renumber 2->3, …`):

- For a task that *arrived* at its id by a renumber, the log continues from
  the renumber's parent under the old id, and `-n` counts across both.
- For a task that *kept* an id another task was renumbered away from, every
  such renumber commit is excluded with `^<commit>`, together with its
  ancestry: the moved task was not in the merge base, so all it did under the
  id is behind the renumber, and none of the kept task's history is. An
  excluded revision makes git walk the whole range before `-n` applies.

### `guide`

Prints [agents.md](agents.md) — the short manual for scripts and agents — to
stdout, byte for byte, via `include_str!`. Needs no repository. Every other
`--help` ends by pointing at it.

### `completions`

`yman completions <bash|zsh|fish|elvish|powershell>` prints a completion
script for that shell to stdout, generated by `clap_complete` from the same
`src/cli.rs` definition the binary parses with, so it cannot drift from the
CLI. The hidden `merge-driver` is left out. An unknown shell is a usage error,
exit 2. Needs no repository.

### `man`

`yman man` prints the `yman(1)` page in roff to stdout, generated by
`clap_mangen` from the same definition; its SUBCOMMANDS section refers to
`yman-<name>(1)`. `yman man --dir <DIR>` creates `DIR` if needed and writes
`yman.1` plus one page per visible subcommand, nested ones included
(`yman-tags-rename.1`), and prints nothing. Needs no repository.

## 4. Writing

### `add`

Mints an id (see [storage.md §8](storage.md#8-id-schemes)), validates the status
against the config, normalizes the tags, dedupes them together with the
`--link`s and `--relate`d ids while keeping their order, then writes `t.md` and
`m.yml` with `created == updated`. A tag is trimmed, lowercased, and refused
when it is empty or carries whitespace, a comma or a control character — the
comma separates the `ls` TAGS column and the flow sequence `m.yml` accepts, and
whitespace makes `-t` unusable without quoting. Tags are checked before the
editor opens, before `--body-file` reads stdin and before an id is minted, so a
typo costs neither an id nor a half-written folder. `-t UI -t ui` is one tag.
`-a/--assignee` is trimmed; blank means unassigned. `--relate` to an id no
folder here carries warns exactly as `set --relate` does, before the id is
minted. The body comes from `-m`,
or from `--body-file PATH` (`-` reads stdin); the two exclude each other and
`-e`. With `-e`, the editor opens before the
first commit, so a title typed there renames the folder by plain rename — the
placeholder never enters git history.

### `edit`

Opens `$VISUAL`, else `$EDITOR`, else `vi`, split on whitespace so
`EDITOR="code --wait"` works. The `vi` fallback is taken only when stdin is a
terminal; otherwise the command fails at once with
`no terminal for vi; set $EDITOR, or use -m / --body-file` rather than leaving
a `vi` waiting on a pipe. The same rule covers `add -e` and `comment -e`. A
non-zero editor exit aborts and leaves the file alone. If the file no longer parses, the command fails but **does not revert the
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
2. `updated` is touched; `m.yml` is rewritten; `t.md` too when the title or
   the body moved. With `-m`, the text is appended to `d.md` exactly as
   `comment` would, with the same actor.
3. One commit, one printed line per change (`14: status todo -> doing`,
   `14: commented`).

List fields (`--tag/--untag`, `--link/--unlink`, `--relate/--unrelate`) are set
semantics with insertion order preserved: adding a value already present is not
a change, and removing one that was never there is quietly accepted. Both
`--tag` and `--untag` normalize their values the way `add` does, and both refuse
an invalid one — nothing yman wrote can look like that, so such a value is a
typo rather than something waiting to be removed. When
nothing at all changed, `set` prints `no changes` and commits nothing. `-m`
always counts as a change; an empty message is the `empty comment` error.

`--relate` to an id no folder here carries prints
`warning: task <id> does not exist here; relating anyway` on stderr and commits
regardless. It is not a refusal because a concurrent `add` on another clone can
legitimately race it, and the id becomes real the moment that clone's work
arrives. Re-adding an id the task already relates to is not a change and warns
about nothing.

`related` is **one-way**: relating 1 to 2 says nothing about 2. yman maintains
the field in exactly three places — here, the renumber rewrite in §6, and `rm`
below — and nowhere else. In particular, an id written into the prose of `t.md`
or `d.md` is text; it is never checked and never rewritten.

`--body TEXT` and `--body-file PATH` (`-` for stdin) replace the body of
`t.md`; they exclude each other. The comparison ignores leading and trailing
newlines, as `t.md` is rendered that way, so re-applying the same text is
`no changes`. `--body ""` clears the body. The line is `<id>: body updated`
and the token `body`; a body change alone never renames the folder. A source
that cannot be read fails with `cannot read <path>: <why>` before anything is
written.

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
| `move <id>... <status>` | the one you name | unknown status, as for `set` |
| `cancel <id>...` | `statuses.cancel` | the key is unset |
| `reopen <id>...` | `statuses.default` | the task is not in a terminal status |

Every verb, and `prio <id>... <0-9>`, takes several ids and `-m <text>` like
`set`. Ids are processed in the order given, one commit each, printing the
same lines `set` would. The first failure stops the run with that error; the
tasks before it are already committed. `set`, `edit`, `show`, `path` and `rm`
take exactly one id.

`cancel` has no fallback on purpose: picking one of several closed statuses by
position is the guesswork the named roles exist to remove.

### `tags rename` / `tags rm`

The two edits that only make sense across every task at once. Both normalize
their arguments the way `add` does, match stored tags folded, and touch only
`m.yml` — a tag never names the folder, so nothing is renamed or moved.

`tags rename <old> <new>` replaces `old` in place, keeping its position in the
list. A task already carrying `new` loses `old` rather than gaining a duplicate,
so its line reads `<id>: tags -<old>` where the others read
`<id>: tags +<new> -<old>`. `old` and `new` that normalize to the same value is
`no changes`.

`tags rm <tag>` drops it. On a terminal it prompts
`remove tag "<tag>" from <n> task(s)? [y/N] ` — asked only once the count is
known, since confirming a removal without knowing it touches forty tasks is not
consent. Anything but `y`/`Y` aborts. Without a terminal and without `-f` it
refuses with `refusing to remove without -f`, the same wording as `rm`.

Both are **one** commit for every task they touch: renaming a tag is a single
decision, and a half-applied rename would leave the vocabulary in a state
nobody chose. No match is `no changes` and no commit, as in `set`. A folder
that does not load cannot be rewritten either, so it is reported on stderr with
the same `warning: skipped …` the listing prints rather than silently left out
of the count.

### `rm`

On a terminal, prompts `remove task 14 "Fix login"? [y/N]`; anything but `y`/`Y`
aborts. Without a terminal and without `-f`, it refuses rather than assume
consent.

Every other task's `related` loses the removed id in the **same commit** as the
removal, so a task that is gone and a reference still naming it are never two
states of the repository anyone can observe. The count goes to stderr as
`note: dropped N reference(s) to <id>`, and is omitted when there was nothing to
drop. A folder that does not load cannot be rewritten, so it is reported with
the same `warning: skipped N unreadable task folder(s): …` the listing prints —
a reference inside one survives the removal.

### `attach` / `detach`

Each source must be an existing regular file. `--name` applies to a single file
only, and must not contain a path separator. The `by` recorded in `m.yml` is
the actor (see `comment`). An existing attachment of the same
name needs `--force`. Files over 5 MiB produce a warning, never a refusal. An
attachment listed in `m.yml` whose file is already gone can still be detached —
the entry is simply dropped. Each copied file is staged with `git add -f`, so a
name matching `.yman/.gitignore` (`*.swp`, `*~`, `.#*`, `*.orig`) is committed
like any other: naming a file to attach overrides the ignore rule.

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

On the lazy path the policy is consulted between steps 2 and 3, not before
step 1: it can only change the outcome for a repository that is actually
behind, and reading it costs a `git` process. A repository that is up to date
or holds unpushed commits reaches the same answer without it.

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
   `merge --no-edit --no-verify` with `merge.directoryRenames=true`. `m.yml`
   goes through the field-wise merge driver
   ([storage.md](storage.md#merging-myml)), so edits to different fields of one
   task settle on their own; anything it cannot settle falls back to the text
   merge. Conflicts print the unmerged files and exit 3. A clean merge is
   followed by the **rejoin** below.
8. Push, unless `--no-push`. A rejected push means origin moved: loop back to
   step 3, at most three attempts, then fail with `origin keeps moving`.
9. Report `synced  pulled N, pushed N, renumbered N   refs/yman/local @ <sha>`.

After a successful push, `REMOTE` is set to `LOCAL`, so `status` and `refresh`
agree with what origin now holds.

### Collision renumbering

`seq` and `author` mint the same id on two machines working offline. The side
that is behind moves, because the other side's id is already published.

- Candidates are ids of tasks **created locally since the merge base** that also
  exist on the remote. All three sets are read from folder names in
  `ls-tree -d -r` — of the base, of `LOCAL` and of `REMOTE` — and an id created
  locally is one `LOCAL` has that the base does not. Nothing here consults
  git's rename detection: adding a comment or an attachment to an existing task
  does not mint an id, and neither does a retitle or a close, both of which move
  the folder while the id stays in its name.
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
  one, and that is every reference that can mean the moved task: a candidate
  is absent from the merge base, so it never reached origin and no other
  clone has seen it. A reference on the remote side, including one arriving
  in the same merge, names the remote's task, which keeps its id.

### Rejoining a moved folder

A retitle or a close moves a task's folder, and a file the other clone added
under the old name belongs in the new one. `merge.directoryRenames=true` has
git move it — but only when something was also added directly in that folder,
so a first comment's `d.md` follows and an attachment on its own, `f/<name>`,
does not. The merge then succeeds with the task split across two folders.

So after the merge, any folder that shares its id with exactly one folder
carrying an `m.yml`, and has none itself, is emptied into that folder with
`git mv`, printing `note: moved N file(s) left under <old> into <rel>`. A file
whose destination already exists stays put, with
`warning: <path> not moved; <dest> already exists`. After a clean merge the
moves are committed as `yman: rejoin files left under a moved folder`; under
`--continue` they go into the merge commit, ahead of its checks.

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
