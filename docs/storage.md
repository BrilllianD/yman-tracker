# Storage contract

Normative, as-built. Describes where `yman` keeps data and what shape it is in.
Companion documents: [commands.md](commands.md), [errors.md](errors.md).

## 1. Names and paths

| Term | Meaning | Source |
|---|---|---|
| `ROOT` | main repo toplevel | `git rev-parse --path-format=absolute --show-toplevel --git-common-dir`, first line |
| `COMMON` | main repo common git dir, absolute | the same call, second line |
| `YDIR` | `ROOT/.yman` | |
| `WT_GITDIR` | private git dir of the `.yman` worktree | read from `YDIR/.git` |
| `LOCAL` | `refs/yman/local` — task history head | |
| `REMOTE` | `refs/yman/remote` — remote-tracking ref | |
| `REMOTE_REF` | `refs/tasks/main` — the ref name on origin | |

Two git runners exist, both over the same object store and refs
(`src/repo.rs`):

- **in main** — `git -C ROOT …`: refs, fetch, push, config, worktree management.
- **in wt** — `git -C YDIR …`: anything touching task files — add, mv, rm,
  commit, merge, status, diff, log.

### `WT_GITDIR` is read, never constructed

Git derives a linked worktree's private directory name from the basename and
sanitizes it, so `.yman` becomes `COMMON/worktrees/-yman` — with a leading
hyphen, not the literal name. `repo::read_gitdir_file` follows the `gitdir:`
line in `YDIR/.git` to the real path; `ydir_state` then classifies `.yman` as a
worktree only when that path lives under `COMMON/worktrees/` and contains a
`gitdir` file. Building the path from the directory name breaks every command
with `.yman is not initialized`.

## 2. The ref namespace

Task commits live on `LOCAL`, outside `refs/heads/`. `.yman` is a linked
worktree whose HEAD is the *symbolic ref* `refs/yman/local`, created as:

```sh
git worktree add --detach .yman refs/yman/local
git -C .yman symbolic-ref HEAD refs/yman/local
```

`scripts/spike-symref.sh` proves the three properties this depends on, and is
re-runnable:

1. `git commit` inside the worktree moves `refs/yman/local`, and HEAD stays
   symbolic. No `update-ref` fixup is needed.
2. `git branch -a` never mentions the ref.
3. `git merge --ff-only` works against a ref built with `commit-tree`.

The consequence for everything else: `git status`, `git log`, `git diff`, branch
pickers and CI never see task history, while `git log --all`, `git show-ref` and
`git worktree list` do.

Only one `.yman` per clone is supported. A second `worktree add` against the
same ref is rejected by git and reported as such.

### Refs are read from disk, with `git` as the fallback

Resolving a ref is a file read, and `yman` does it as one (`src/refs.rs`):
`COMMON/<ref>` first, then a scan of `COMMON/packed-refs`, chasing a `ref:`
line up to five hops. `WT_GITDIR/HEAD` is read the same way for the symbolic
HEAD check. Both files are replaced by rename, so a concurrent update is read
as either the old value or the new one, never a torn one, and a loose ref takes
precedence over the packed copy exactly as it does for git.

Anything the reader does not recognise — the `reftable` backend (git 2.45+,
detected as `COMMON/reftable/`), an unreadable file, a value that is not a full
hex object id — is not a failure: the caller falls back to `git rev-parse` or
`git symbolic-ref`, which is always authoritative. An unfamiliar repository is
therefore slower, never wrong. The write paths (`sync`, `init`, `status`) keep
asking git directly, because they read refs that git itself has just moved.

The visible consequence is the process count: `yman ls` and `yman show` in an
up-to-date repository spawn one `git`, the `rev-parse` above. A repository
holding unpushed commits spawns a second for the ancestry question, which no
file answers. `tests/cli.rs` pins both numbers.

## 3. Repository configuration

`yman init` writes exactly these, all idempotently:

| Location | Value | Purpose |
|---|---|---|
| `COMMON/info/exclude` | `.yman/` | keeps the code worktree's `git status` clean |
| `remote.origin.fetch` (appended) | `+refs/tasks/main:refs/yman/remote` | a plain `git fetch` carries task commits |
| `yman.refresh` | `lazy` \| `manual` | see [commands.md](commands.md#refresh) |
| `yman.author` | prefix string | only meaningful for the `author` id scheme |
| `merge.ymanmeta.name` | `yman m.yml field-wise merge` | shown by git when the driver runs |
| `merge.ymanmeta.driver` | `"<yman>" merge-driver %O %A %B` | see [Merging `m.yml`](#merging-myml) |
| `.yman/.gitattributes` | `**/d.md merge=union`, `**/m.yml merge=ymanmeta` | committed, so every clone inherits it |

`remote.origin.push` is **never** set: that would hijack the user's plain
`git push`. Publishing is always the explicit refspec
`refs/yman/local:refs/tasks/main`.

When the repository has no `origin` and `--remote <url>` is given, `init`
creates the remote — `fetch` and `push` address `origin` by name, so reporting
the URL without creating it would lead nowhere.

## 4. Directory layout

```
ROOT/
  .git/
    config                      # + fetch refspec, yman.refresh, yman.author,
                                #   merge.ymanmeta.*
    info/exclude                # + ".yman/"
    refs/yman/{local,remote}
    worktrees/-yman/            # git-managed; HEAD, index, MERGE_HEAD
    hooks/{post-merge,post-checkout}   # optional, `yman hooks install`
  .yman/
    .git                        # file: "gitdir: <COMMON>/worktrees/-yman"
    .gitignore                  # *.swp  *~  .#*  *.orig
    .gitattributes              # **/d.md merge=union, **/m.yml merge=ymanmeta
    config.toml
    2.14.fix-login/
      t.md
      m.yml
      d.md                      # only once commented
      f/                        # only once something is attached
        screenshot.png
```

## 5. Task folder names

Grammar, anchored: `^([0-9])\.([^./\\]+)\.(.+)$` → `priority: u8`, `id: String`,
`slug: String`. Parsed by hand in `task::FolderName::parse`; there is no `regex`
dependency.

- Priority `0`–`9`, `0` highest, so a plain `ls .yman/` sorts urgent work first.
  Only the priority sorts numerically: the listing is lexical, so within one
  priority `5.10.x` comes before `5.2.x`. `yman ls` orders ids properly
  (commands.md §3); zero-padding them here would change the id contract.
- `id` never contains `.`. `FolderName::parse` splits at the *first* dot after
  the priority and only rejects `/` and `\` outright, so a dotted id would be
  silently mis-read rather than refused — `5.v.i-1.slug` parses as id `v` with
  slug `i-1.slug`. `seq` and `random` cannot produce one, and the `author`
  prefix is validated before an id is minted (§8).
- **The folder name is the source of truth** for priority and id. `m.yml` does
  not repeat them, and changing either is a `git mv` so history follows.
- Entries in `.yman/` that are not directories, or whose names do not match, are
  ignored entirely (`config.toml`, `.git`, dotfiles) — except that a directory
  whose name is a legal status name is descended into, because closed tasks live
  one level down (below).
- A directory that matches but fails to load is a **broken task**: `ls` prints it
  with a `!` marker and the parse error; commands that target it fail.
- Two folders with the same id is an error — `duplicate task id …` — and can
  only result from a badly resolved merge. The message names paths relative to
  `.yman`, not folder names, because the two copies can differ only in which
  status directory they sit in.

### Closed tasks live one level down

A task in a terminal status (§7) is stored at
`.yman/{status}/{priority}.{id}.{slug}/` instead of at the top level. Only
`version = 2` does this; a version 1 repository keeps every task flat.

- **`m.yml` is the source of truth, not the path.** A task physically under
  `done/` whose `m.yml` says `todo` is listed as `todo`, and the next `set`
  moves it where it belongs. This is what keeps a config change from bricking
  a repository, the same promise §6 makes about unknown status values.
- The second level is found by *grammar* — any directory whose name is a legal
  status name is descended into — not by consulting `statuses.terminal`. A task
  archived under a status later removed from the list must still be found.
- Nothing is nested deeper than one level.

### `slugify(title, max_bytes)`

1. Unicode lowercase.
2. Alphanumerics (`char::is_alphanumeric`) survive; every other character
   becomes a separator.
3. Separator runs collapse to one `-`; leading and trailing `-` are trimmed.
4. Truncate to `max_bytes` **at a char boundary**, then trim a trailing `-`
   again.
5. An empty result becomes `task`.

Non-ASCII survives: `"Первая задача"` → `первая-задача`.

## 6. File formats

### `t.md`

```markdown
# Fix login

Free-form body. May be empty.
```

The title is the first `^#\s+…` line, scanning from the top and skipping blank
lines. Anything non-blank before it is a parse error
(`t.md must start with "# Title"`). The body is everything after the title line
with one leading blank line stripped, and is written back verbatim — `#`, `---`
and code fences inside it are preserved.

### `m.yml`

```yaml
status: doing
tags:
- auth
- bug
assignee: Ivan
created: 2026-09-16T10:00:00Z
updated: 2026-09-16T10:12:30Z
attachments:
- path: f/screenshot.png
  name: screenshot.png
  added: 2026-09-16T10:05:00Z
  by: Ivan
links:
- https://example.com/issues/12
related:
- '5'
```

- Every field but `status`, `created` and `updated` defaults to empty.
- Timestamps are RFC 3339, UTC, **second precision** (`task::now()` zeroes the
  nanoseconds), serialized with a `Z` suffix.
- `attachments[].path` is always `f/<name>`.
- `tags` entries written by yman are trimmed, lowercased and free of
  whitespace, commas and control characters (see
  [commands.md §4](commands.md#add)). The reader enforces none of that: a
  hand-written `Tags: [UI]` loads as it stands, and `ls -t` and `yman tags`
  fold case so it still matches and still counts.
- Unknown top-level keys survive a rewrite. The reader keeps the lines a key
  owns verbatim and the writer replays them after `related`, so a field a newer
  yman writes — or one added by hand — is not destroyed by an older binary.
  They are preserved, not interpreted: nothing reads them and none of them
  appear in `--json` output. Comments, and unknown keys *inside* an
  `attachments` item, are still dropped.
- `m.yml` is read and written by hand in `src/yml.rs`; there is no YAML
  crate. The writer emits exactly the shape above: block sequences at column
  zero, `[]` for an empty list, `null` for an absent `assignee`, and a scalar
  quoted only when the plain form would read back as a number, a boolean or
  null. A value containing newlines becomes a literal block (`|-`, `|`,
  `|+`). The preserved unknown blocks follow, which is why a hand-edited file
  that interleaved one comes back with it moved to the end.
- The reader is deliberately wider than the writer, because people edit this
  file and resolve merge conflicts in it: comments, flow sequences
  (`tags: [a, b]`), single- and double-quoted scalars, folded blocks (`>-`)
  and sequences indented under their key are all accepted. A key repeated at
  the top level or inside an `attachments` item keeps its **last** occurrence,
  which is the half of a conflict a person usually means to keep.
- A `status` outside `config.statuses.list` is reported, never a hard error, so
  editing the config cannot brick existing tasks. An *empty* `status` — bare,
  `null`, or whitespace — is a different thing: it reads as a missing field, so
  the task is reported broken rather than loaded with no status at all.

### Merging `m.yml`

`.gitattributes` marks `**/m.yml` as `merge=ymanmeta`, and `yman init` points
that name at `yman merge-driver %O %A %B` in the clone's own config. The
attribute travels in the history; the `merge.ymanmeta.*` config does not, so
every clone registers it for itself. A clone that never did falls back to git's
text merge, which is what `m.yml` got before the driver existed — an
unregistered driver cannot corrupt a file.

Why it exists: the writer emits keys in one fixed order, so a status change on
one clone and a tag change on another land on *adjacent lines* and the text
merge conflicts. `updated` makes it worse — it changes on every edit on both
sides, so it is a permanently-differing line wedged between `created` and
`attachments`, dragging its neighbours into conflicts they do not deserve.

The driver merges field by field:

| Field | Rule |
|---|---|
| `updated` | the later of the two; never conflicts |
| `created` | the earlier of the two; never conflicts |
| `status`, `assignee` | the side that moved wins; both moved differently is a conflict |
| `tags`, `links`, `related`, `attachments`, unknown blocks | merged entry by entry (below) |

A list is merged one entry at a time, so a tag added on each clone keeps both.
Each entry is the same three-way decision as a scalar, over "is it there, and
with what value": added on one side stays, removed on one side goes, and only an
entry *changed* differently on both sides conflicts. Entries are identified by
themselves for `tags`, `links` and `related`, by `path` for `attachments`, and
by the top-level key for an unknown block — so two clones attaching different
files under one name, or writing the same unknown key with different text, do
conflict.

The result's **order** is the one thing the driver cannot take from either side.
Git hands `%A` and `%B` over the other way round on the other clone, so anything
derived from "ours first" would give the two clones two orderings of one set,
and they would conflict on that at the next sync. So: entries that were already
in the merge base keep the base's order, and everything new is appended sorted
by key. A merge can therefore reorder a task's tags relative to the order the
user typed them in — only on a merge, and only among newly added entries. A
duplicate entry in a hand-edited list collapses to its first occurrence.

The driver is all-or-nothing. One conflicting field abandons the whole
field-wise merge and hands the three files to `git merge-file`, so what the user
resolves by hand is byte for byte what they would have seen without the driver:
`<<<<<<<` markers in `m.yml`, `yman sync` exiting 3, `yman sync --continue`
after. The two sides are labelled `ours` and `theirs` rather than `HEAD` and
the ref name: the driver is handed three temporary files and never learns what
is being merged into what. The same happens when any of the three versions does not parse — a
half-resolved file, or the empty `%O` git passes when the two sides share no
ancestor.

### `d.md`

Append-only. One entry is exactly:

```markdown
## 2026-09-16T10:10:00Z — Ivan

comment text, possibly several lines

```

i.e. `## {rfc3339} — {author}`, blank line, text, blank line. The separator is
an em dash. The parser splits on lines starting with `## ` and keeps
unparsable chunks as raw text, so a hand-edited or union-merged file never makes
`show` fail.

`.gitattributes` marks `**/d.md` as `merge=union`, so concurrent comments merge
without a conflict. The pattern is `**`, not `*`, because a `*` does not cross a
`/` and a closed task's discussion is one level deeper.

`merge=union` settles concurrent comments only while the folder keeps its name.
A retitle is a `git mv`, so a comment written on another clone against the old
name arrives as modify/delete on a path the union driver is never asked about,
and the merge stops with exit 3.

## 7. `config.toml`

Committed inside `.yman/`, so a project agrees on it once.

```toml
version = 1

[ids]
scheme = "seq"        # "random" | "seq" | "author"
random_len = 4        # hex chars, random scheme only

[statuses]
list = ["todo", "doing", "done"]
default = "todo"
# version 2 only, all optional:
# start = "doing"           # yman start;  default list[1]
# done = "done"             # yman done;   default list.last()
# cancel = "cancelled"      # yman cancel; no default
# terminal = ["done"]       # closed statuses, hidden by ls

[priorities]
default = 5           # 0..=9

[slug]
max_bytes = 200       # hard cap 240
```

Validated on every load; each failure is reported as
`invalid .yman/config.toml: <why>`:

| Rule |
|---|
| `version` is `1` or `2` |
| `statuses.list.len() >= 2` |
| `statuses.list` contains `statuses.default` |
| `statuses.start`, `.done`, `.cancel`, `.terminal` need `version = 2` |
| `statuses.start`, `.done`, `.cancel` are in `statuses.list` |
| every `statuses.terminal` entry is in `statuses.list` |
| every `statuses.terminal` entry is a usable directory name |
| `statuses.terminal` has no repeat, and none differing only in ASCII case |
| `statuses.terminal` contains the done status, and the cancel status when set |
| `statuses.done` is named whenever `statuses.terminal` lists more than one status |
| `statuses.terminal` excludes `statuses.default` and the start status (`version = 2` only) |
| at least one status stays open (`version = 2` only) |
| `priorities.default <= 9` |
| `slug.max_bytes` in `1..=240` |
| `ids.random_len` in `2..=16` |

`[priorities]` and `[slug]` may be omitted entirely.

### Status roles

Three roles name the statuses the verbs move to, and one list says which
statuses count as closed:

| Key | Meaning | When unset |
|---|---|---|
| `statuses.start` | `yman start` | `list[1]` |
| `statuses.done` | `yman done` | `list.last()` |
| `statuses.cancel` | `yman cancel` | the command refuses |
| `statuses.terminal` | closed: hidden by `ls` | `[done status]` |

Naming them is what stops a reordered `list` from silently changing what
`yman start` means. For the same reason `statuses.done` is required as soon as
`statuses.terminal` names more than one status: with one closed status the
`done ∈ terminal` rule pins it down, but with two, `list.last()` decides what
`yman done` means and can get it wrong without failing. Leaving them unset reproduces the positional behaviour
exactly, so an untouched file keeps working.

A terminal status doubles as a directory name (see §5), so it is restricted to
letters, digits, `_` and `-`, at most 64 bytes, and must not start with `-`.
That grammar also keeps a status from colliding with `config.toml`, `.git` or a
task folder, since all of those contain a `.`. Two terminal statuses differing
only in ASCII case are rejected because they are one directory on macOS and
Windows.

Three rules — `terminal` excluding `default` and the start status, and keeping
one status open — apply only at `version = 2`. A version 1 list of
`["todo", "done"]` derives `start == done == the terminal status` and works
fine; enforcing the rules against it would reject a config that ships today.

### Opting in to version 2

`yman init` writes `version = 1`, so a new repository stays readable by older
binaries. Raising the version is a hand edit, and it is what turns archiving on:

```toml
version = 2

[statuses]
list = ["todo", "doing", "blocked", "done", "cancelled"]
default = "todo"
start = "doing"
done = "done"
cancel = "cancelled"
terminal = ["done", "cancelled"]
```

From that point an older `yman` refuses the repository outright with
`unsupported version 2 (this yman understands 1)` — which is the intended
failure, because it also would not find the archived tasks.

## 8. Id schemes

Chosen at `init`, stored in `config.toml`, identical for everyone on the
project.

| Scheme | Shape | Next id |
|---|---|---|
| `seq` | `1`, `2`, `14` | highest all-digit id taken, plus one |
| `author` | `iv-1`, `an-3` | highest `{prefix}-N` taken, plus one |
| `random` | `t-7f3a` | `t-` plus `random_len` hex chars, redrawn until free |

"Taken" means the union of the ids on disk **and every id that ever had a file
added under it** across `LOCAL` and `REMOTE`
(`git log --diff-filter=A --name-only`). Deleting a task therefore never frees
its id — a later `add` cannot reuse it and confuse the history.

The `author` prefix resolves in order: `git config yman.author`, `$YMAN_AUTHOR`,
then the initials of `user.name` (first letter of each word, lowercased, ASCII
letters only). With none of those available, `add` fails rather than guess.

The prefix must match `[A-Za-z0-9_-]+`, and whichever source supplies it is
checked every time it is read. A setting that is empty or all whitespace counts
as unset and falls through to the next source; anything else that fails the
grammar fails `add`, naming the value and the source it came from. The reason
is §5: the prefix lands in the folder name, where a `.` is read as the id
separator. `yman init --author` stores the value without checking it, so a bad
prefix surfaces at the first `add`. Ids minted before the rule are left alone —
a collision renumber re-derives the prefix from the id it finds, so an old task
stays renumberable.
