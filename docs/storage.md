# Storage contract

Normative, as-built. Describes where `yman` keeps data and what shape it is in.
Companion documents: [commands.md](commands.md), [errors.md](errors.md).

## 1. Names and paths

| Term | Meaning | Source |
|---|---|---|
| `ROOT` | main repo toplevel | `git rev-parse --show-toplevel` |
| `COMMON` | main repo common git dir, absolute | `git rev-parse --path-format=absolute --git-common-dir` |
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

## 3. Repository configuration

`yman init` writes exactly these, all idempotently:

| Location | Value | Purpose |
|---|---|---|
| `COMMON/info/exclude` | `.yman/` | keeps the code worktree's `git status` clean |
| `remote.origin.fetch` (appended) | `+refs/tasks/main:refs/yman/remote` | a plain `git fetch` carries task commits |
| `yman.refresh` | `lazy` \| `manual` | see [commands.md](commands.md#refresh) |
| `yman.author` | prefix string | only meaningful for the `author` id scheme |

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
    config                      # + fetch refspec, yman.refresh, yman.author
    info/exclude                # + ".yman/"
    refs/yman/{local,remote}
    worktrees/-yman/            # git-managed; HEAD, index, MERGE_HEAD
    hooks/{post-merge,post-checkout}   # optional, `yman hooks install`
  .yman/
    .git                        # file: "gitdir: <COMMON>/worktrees/-yman"
    .gitignore                  # *.swp  *~  .#*  *.orig
    .gitattributes              # */d.md merge=union
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
- `id` never contains `.`; every scheme satisfies this (`14`, `t-7f3a`, `iv-12`).
- **The folder name is the source of truth** for priority and id. `m.yml` does
  not repeat them, and changing either is a `git mv` so history follows.
- Entries in `.yman/` that are not directories, or whose names do not match, are
  ignored entirely (`config.toml`, `.git`, dotfiles).
- A directory that matches but fails to load is a **broken task**: `ls` prints it
  with a `!` marker and the parse error; commands that target it fail.
- Two folders with the same id is an error — `duplicate task id …` — and can
  only result from a badly resolved merge.

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
- Unknown keys are dropped on rewrite — plain serde, documented and accepted.
- A `status` outside `config.statuses.list` is reported, never a hard error, so
  editing the config cannot brick existing tasks.

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

`.gitattributes` marks `*/d.md` as `merge=union`, so concurrent comments merge
without a conflict.

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

[priorities]
default = 5           # 0..=9

[slug]
max_bytes = 200       # hard cap 240
```

Validated on every load; each failure is reported as
`invalid .yman/config.toml: <why>`:

| Rule |
|---|
| `version == 1` |
| `statuses.list.len() >= 2` |
| `statuses.list` contains `statuses.default` |
| `priorities.default <= 9` |
| `slug.max_bytes` in `1..=240` |
| `ids.random_len` in `2..=16` |

Derived meanings: the **start** status is `list[1]` (`yman start`), the **done**
status is `list.last()` (`yman done`, and what `ls` hides by default). `[priorities]`
and `[slug]` may be omitted entirely.

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
