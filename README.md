# yman

A task tracker that lives next to the code, travels through the project's own
git remote, and never shows up in `git branch`, your IDE's branch picker, or CI.

No server, no account, no extra remote. Tasks are plain Markdown and YAML in
your repository — but in a namespace ordinary git use never walks into.

```console
$ yman init
$ yman add "Fix login" -p 2 -t auth
added 1  2.1.fix-login
$ yman start 1
1: status todo -> doing
$ yman comment 1 -m "reproduced on staging"
$ yman sync
synced  pulled 0, pushed 3, renumbered 0   refs/yman/local @ 3f2a1c9
```

Your teammate, in their own clone:

```console
$ yman init
$ yman ls
2  1  doing  Fix login  auth    1
```

---

## Contents

- [Install](#install)
- [Why it stays out of the way](#why-it-stays-out-of-the-way)
- [How tasks are stored](#how-tasks-are-stored)
- [Command reference](#command-reference)
- [Configuration](#configuration)
- [Sharing work: sync, refresh, hooks](#sharing-work-sync-refresh-hooks)
- [When two people collide](#when-two-people-collide)
- [Troubleshooting](#troubleshooting)
- [Limits and future work](#limits-and-future-work)
- [Development](#development)
- [License](#license)

---

## Install

Requires a Rust toolchain and a `git` binary, 2.42 or newer.

```sh
cargo install --path .
```

The binary is called `yman`. It must be on your `PATH` for the optional git
hooks to find it.

Then, in any repository with an `origin` you can push to:

```sh
yman init
```

That is the only setup step. It creates `.yman/`, adds one fetch refspec to the
repository config, and excludes `.yman/` from the code worktree. Everyone else
on the project runs the same command and gets the existing tasks.

---

## Why it stays out of the way

A task tracker inside a repository usually means a branch, and a branch means a
CI run, an entry in every branch picker, and a merge conflict waiting to happen.

`yman` stores the task history under **`refs/yman/local`** — a ref outside
`refs/heads/`. Git is perfectly happy to keep commits there; almost nothing goes
looking for them:

| Command | Sees the task history? |
|---|---|
| `git branch -a` | no |
| `git status`, `git log`, `git diff` | no |
| your IDE's branch list | no |
| your CI provider | no — `refs/tasks/main` is not a branch |
| `git log --all`, `git show-ref`, `git worktree list` | yes |

`.yman/` is a **linked git worktree of the same repository** whose HEAD is that
symbolic ref, so your code worktree keeps a clean `git status` no matter how
many tasks you write.

| Thing | Where it lives |
|---|---|
| Task history, locally | `refs/yman/local` |
| What origin last told us | `refs/yman/remote` |
| The ref on the server | `refs/tasks/main` — never a branch |
| Working copy of the tasks | `.yman/`, listed in `.git/info/exclude` |

`yman init` appends exactly one line to the repository config:

```
remote.origin.fetch = +refs/tasks/main:refs/yman/remote
```

so an ordinary `git fetch` or `git pull` brings task commits along at no cost.
Pushing is always explicit — `git push origin refs/yman/local:refs/tasks/main`.
`remote.origin.push` is never set, so your plain `git push` keeps doing exactly
what it did before.

---

## How tasks are stored

One task is one folder:

```
.yman/
  config.toml              # committed; shared by everyone on the project
  .gitignore
  .gitattributes           # **/d.md merge=union
  2.14.fix-login/
    t.md                   # "# Title" plus a free-form Markdown body
    m.yml                  # status, tags, assignee, timestamps, attachments…
    d.md                   # discussion, append-only  (only once commented)
    f/                     # attachments               (only once attached)
      screenshot.png
  done/                    # version 2 only: closed tasks, out of the way
    5.9.old-thing/
```

### The folder name is the data

`{priority}.{id}.{slug}` is the source of truth for the priority and the id —
`m.yml` does not repeat them. Changing either is a `git mv`, and `yman log <id>`
follows the rename, so history is never lost to a retitling.

Priority runs `0`–`9` with `0` highest, which means a plain `ls .yman/` already
sorts the most urgent work to the top. The slug is the title lowercased with
every non-alphanumeric run collapsed to `-`; Unicode survives intact
(`1.1.первая-задача`).

### `t.md`

```markdown
# Fix login

Anything you like down here. Headings, code fences, checklists — it is
just Markdown, and it is written back verbatim.
```

The first `# ` line is the title. A file without one is a broken task: `yman ls`
prints it with a `!` marker and the parse error instead of failing, and commands
that target it say so.

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

Timestamps are RFC 3339, UTC, second precision. Keys yman does not know are
dropped when it rewrites the file.

### `d.md`

Append-only, one entry per comment:

```markdown
## 2026-09-16T10:10:00Z — Ivan

reproduced on staging
```

`.gitattributes` marks `**/d.md` as `merge=union`, so two people commenting on
the same task at the same time merge cleanly instead of conflicting — as long
as neither of them also retitled it, which renames the folder the comment was
written against.

---

## Command reference

### Creating and reading

```sh
yman add <title> [-p 0-9] [-s <status>] [-t <tag>]... [-m <body> | --body-file <path> | -e]
         [-a <who>] [--link <url>]... [--relate <id>]...
```
Creates a task and commits it. `-t`, `--link` and `--relate` repeat.
`--body-file -` reads the body from stdin. `-e` opens `$EDITOR` on the new
`t.md` first — whatever title you type there wins, and the folder is named after
it.

```sh
yman ls [-s <status>]... [-t <tag>]... [-a] [--assignee <who>] [-p 0-9]
        [-q <text>] [-n <N>] [--json]
yman tags [--json]
yman tags rename <old> <new>
yman tags rm <tag> [-f]
```
Lists tasks sorted by priority, then status order, then id. Tasks in a closed
status are hidden unless you pass `-a` or name that status with `-s`. `-t`
requires *all* the tags given and matches case-insensitively. `--assignee -` means unassigned, `-q` is a
case-insensitive search over title and body, `-n` caps the rows after sorting.
Column headers appear only when stdout is a terminal, so `yman ls | grep` stays
predictable. `yman tags` is the inventory of the tags in use, one row per tag
with the number of tasks carrying it — closed tasks included, since the
question is what exists rather than what is open. `yman tags rename` and
`yman tags rm` edit that vocabulary across every task carrying the tag, in one
commit; `rm` confirms first, like `yman rm`. `--json` prints one object per
task — including its `links` and `related` ids — plus `{dir, error}` for
anything broken.

```
P  ID  STATUS  TITLE       TAGS      F  C
2  1   doing   Fix login   auth,bug  1  3
5  2   todo    Write docs
```

`F` is the attachment count, `C` the comment count; both blank at zero.

```sh
yman show <id> [-n N] [--json]   # everything about one task; -n keeps the last N comments
yman path <id>     # just the absolute path:  cd $(yman path 14)
yman log [<id>] [-n N]
```
`yman log <id>` follows the task across every rename it has been through.

### Changing things

```sh
yman set <id> [--status S] [--priority 0-9] [--title T]
              [--assignee A | --no-assignee]
              [--tag X]... [--untag X]...
              [--link URL]... [--unlink URL]...
              [--relate ID]... [--unrelate ID]...
              [--body <text> | --body-file <path>] [-m <comment>]
yman start <id>...          # = set --status <start status>
yman done  <id>...          # = set --status <done status>
yman move <id>... <status>  # = set --status <status>
yman cancel <id>...         # = set --status <cancel status>   (version 2)
yman reopen <id>...         # a closed task back to the default status
yman prio  <id>... <0-9>    # = set --priority
                            # every one of these also takes -m <comment>
yman edit  <id>          # $VISUAL, else $EDITOR, else vi (only on a terminal)
yman rm    <id> [-f]
```

List fields keep their insertion order. Adding a value that is already there is
not a change and commits nothing; removing one that was never there is quietly
accepted. A `set` that changes nothing prints `no changes` and leaves the
history alone. `-m` appends a comment in the same commit, so
`yman done 14 -m "fixed in 3f2a"` closes and explains in one step. `--body`
rewrites the description without an editor; `--body ""` clears it. The verbs
take several ids — `yman done 14 15 16` — and commit each task on its own,
stopping at the first error.

`yman rm` asks for confirmation on a terminal and refuses outright without `-f`
when there is no terminal to ask at.

### Attachments and discussion

```sh
yman attach <id> <file>... [--name <n>] [--force]
yman detach <id> <name>
yman comment <id> [-m <text>] [-e]
```

`--name` renames a single file on the way in. `--force` replaces an attachment
that already exists. Files over 5 MiB get a warning, never a refusal. With
neither `-m` nor `-e`, `yman comment` reads the comment from stdin — so
`git log -1 | yman comment 14` works.

### Plumbing

```sh
yman status [--json]     # where everything stands; never changes anything
yman refresh [--quiet]   # fast-forward onto already-fetched task commits
yman sync [--continue] [--abort] [--no-push]
yman hooks install|remove|status
yman git [--] <args>...  # raw git, run inside .yman
yman guide               # the short manual for scripts and agents; works anywhere
```

```console
$ yman status
.yman  refs/yman/local @ 3f2a1c9   (refresh: lazy, hooks: installed)
remote: refs/tasks/main @ 9b8c7d6   ahead 2, behind 1   → run: yman sync
worktree: clean
tasks:   todo 4, doing 1, done 7
```

`yman status --json` reports the same facts — refs, ahead/behind, the merge
state and the per-status counts — as one object, for a script that would
otherwise parse those four lines.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | success |
| `1` | error — the message says what happened |
| `2` | usage error, from the argument parser |
| `3` | a sync merge is waiting to be resolved |
| `4` | no task has that id |

Errors go to stderr prefixed `error: `, warnings `warning: `, notes `note: `.
Everything a script would want to read goes to stdout.

### Scripts and agents

`yman guide` prints [docs/agents.md](docs/agents.md), a one-page manual for a
reader that pays per token: which calls to make, what `ls` columns mean, the
exit codes, and how to recover from a stuck sync. It starts with a block to
paste into a project's `CLAUDE.md`. The short version: filter with `ls -n`,
read with `show -n`, close with `done <id> -m`, never open an editor, and set
`YMAN_ACTOR` so the work is attributed to the agent.

---

## Configuration

`.yman/config.toml` is committed, so a project agrees on these once:

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

`statuses.list` is yours to change — the order is meaningful. `yman start` moves
a task to the second entry, `yman done` to the last, and `yman ls` hides the
last one by default. A status that is no longer in the list never breaks a
task; it is reported, not rejected.

Those last three are *derived from position*, which means inserting a status
quietly changes what `yman start` does. At `version = 2` you can name them
instead, and say which statuses count as closed:

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

`ls` then hides every terminal status, not just the last one. All four keys are
optional; leaving them out reproduces the positional behaviour exactly.

Version 2 also **files closed tasks one directory down**, at
`.yman/done/5.1.fix-login/`, so the top level only ever holds open work.
`yman done` moves the folder there in the same commit as the status change, and
setting the task back to an open status moves it out again and takes the
emptied directory with it. `m.yml` stays the source of truth: a task in the
wrong directory is still listed correctly, and the next `set` puts it where it
belongs.

Raising the version is a hand edit. Existing closed tasks are not moved for you — `yman done <id>` on each
one does it, and reports `folder <old> -> <new>`.

Two people closing the *same* task to *different* statuses is the one case that
gets harder: git keeps both folders, and `yman sync` names the pair and asks you
to delete one. `yman sync --continue` refuses until you do.

### Choosing an id scheme

Pick this at `yman init`; it is stored in `config.toml` and cannot be changed
per-clone afterwards.

| Scheme | Ids look like | Best when |
|---|---|---|
| `seq` (default) | `1`, `2`, `14` | Small team, short ids worth typing |
| `author` | `iv-1`, `an-3` | Several people adding tasks offline a lot |
| `random` | `t-7f3a` | Ids must never collide, at any cost |

`seq` and `author` can mint the same id on two machines at once; `yman sync`
sorts that out (see below). `random` cannot collide in practice, at the price of
ids nobody remembers.

The `author` prefix comes from `git config yman.author`, else `$YMAN_AUTHOR`,
else the initials of your `user.name`.

### Repository-local settings

| Key | Values | Set by |
|---|---|---|
| `yman.refresh` | `lazy` (default), `manual` | `yman init --refresh` |
| `yman.author` | letters, digits, `_` or `-` | `yman init --author` |

Two environment variables, easy to confuse:

| Variable | Used for |
|---|---|
| `YMAN_AUTHOR` | fallback id prefix for the `author` scheme, after `yman.author` |
| `YMAN_ACTOR` | display name on comments and attachments, before `user.name` |

`YMAN_ACTOR` is meant for an agent or a script working under a person's git
account. The git committer is never changed by it.

---

## Sharing work: sync, refresh, hooks

**`yman sync` is the only command that talks to the network.** It snapshots any
uncommitted edits in `.yman`, fetches, merges, and pushes:

```console
$ yman sync
snapshotted 1 local change(s)
synced  pulled 3, pushed 2, renumbered 0   refs/yman/local @ 3f2a1c9
```

Getting *other* people's tasks does not need a sync at all, because they arrive
with any ordinary fetch:

| `yman.refresh` | What happens |
|---|---|
| `lazy` (default) | Every read or write command fast-forwards `.yman` onto whatever `refs/yman/remote` already holds |
| `manual` | Only `yman refresh` and `yman sync` move `.yman` |

A refresh is always a fast-forward, never touches the network, and backs off
silently when `.yman` has uncommitted changes or local commits you have not
pushed. `yman refresh` and `yman status` explain why when it does.

`yman hooks install` adds `post-merge` and `post-checkout` hooks that run
`yman refresh --quiet`, so a plain `git pull` updates your tasks as a side
effect. Hooks somebody else wrote are reported and left alone, never overwritten
or deleted — yman tells you the one line to add instead. `core.hooksPath` is
honoured.

---

## When two people collide

### Same id, two machines

With `seq` or `author`, two people working offline will both mint id `2`. On
sync, whoever is *behind* renumbers their own new task, because the other side's
id is already published:

```console
$ yman sync
renumbered 2 -> 3  (id taken on origin)
note: rewrote 1 reference(s) to renumbered ids
synced  pulled 1, pushed 3, renumbered 1   refs/yman/local @ 1f7d06f
```

References to the old id in other tasks' `related` lists follow the move, in the
same commit as the rename.

### Same field, two edits

Two people changing the status of the same task conflict like any other git
conflict. Sync stops with exit code 3 and tells you which files:

```console
$ yman sync
  5.1.fix-login/m.yml
error: conflicts in 1 file(s); edit them, remove markers, then: yman sync --continue  (or: yman sync --abort)
```

Edit the files and remove the markers — staging is yman's job, not yours — then:

```sh
yman sync --continue     # or: yman sync --abort
```

`--continue` refuses while any conflict marker is still in place, and checks
that every task the merge touched still parses before it commits. Until the
merge is settled, commands that would change a task exit 3 rather than build on
a half-merged state. `yman status` shows the same information at any time.

Comments never conflict: `merge=union` keeps both sides.

The exception is closing one task to two *different* statuses at version 2.
Each side moves the folder somewhere else, git keeps both, and `yman sync` says
so:

```console
$ yman sync
note: task 1 was closed to two different statuses; keep one of done/5.1.fix-login, cancelled/5.1.fix-login
error: conflicts in 3 file(s); edit them, remove markers, then: yman sync --continue

$ rm -rf .yman/cancelled/5.1.fix-login
$ yman sync --continue
```

`--continue` refuses while both folders are there, so the tracker cannot end up
with two folders for one id.

---

## Troubleshooting

**`git clean -fdx` deleted `.yman/`.** Nothing is lost — the history lives in
`refs/yman/local`, which is not in the worktree. Run `yman init` again and
everything comes back, including tasks you had never pushed.

**`.yman is not initialized; run: yman init`.** Either this clone has never run
`init`, or `.yman/` was removed. Same fix.

**`refs/yman/local is already checked out in another worktree`.** One `.yman`
per clone is supported. Remove the other worktree, or use a separate clone.

**`git identity missing`.** yman commits like any other tool; set `user.name`
and `user.email`.

**`yman path` prints a different path than it used to.** At version 2 a closed
task lives in `.yman/<status>/`. Shell bookmarks to the old location break;
`yman path <id>` always knows where it is now.

**A task shows as broken.** Its `t.md` lost its `# Title`, or its `m.yml` will
not parse — usually a hand-edit or a merge resolved carelessly. `yman ls` shows
the error; fix the file and the task comes back. Nothing else is affected.

**Tasks are not appearing after someone else pushed.** You have not fetched
(`yman refresh` never uses the network on its own), or `yman.refresh` is
`manual`, or your `.yman` has uncommitted changes. `yman status` says which.

---

## Limits and future work

- `m.yml` conflicts are resolved by hand; a field-wise merge driver would settle
  status and tag edits automatically.
- Attachments go straight into git; there is no git-lfs integration.
- **`add` slows as the project ages.** An id is never reused, which means
  reading every id the history ever assigned before minting one. Measured with
  `scripts/synthetic-project.sh` on a Ryzen 7 4700U, git 2.55.0, release build:
  32 ms on an empty tracker, 157 ms at 1000 tasks over 1081 commits — a linear
  18 ms + 0.13 ms per existing task.
- Every ref read spawns a `git` process, including on the lazy refresh path.
  On that same repository `yman ls` costs 6 processes and 32 ms, `yman show`
  6 and 23 ms, `yman add` 11.
- `yman log <id>` rebuilds the rename graph of the whole history on every call,
  not just the task's: 420 ms against 3802 rename records.
- One `.yman` worktree per clone.
- Windows should work with `core.longpaths`, but is not tested.
- No colour, no TUI, no web UI, no GitHub Issues bridge.

---

## Development

```sh
cargo test                                  # unit tests + the two-clone suite
cargo clippy --all-targets -- -D warnings
sh scripts/spike-symref.sh                  # the git invariant this rests on
sh scripts/synthetic-project.sh             # a 1000-task repository, driven (~3 min)
```

The integration suite builds a bare remote and two clones in a temp directory
and drives both through init, add, sync, id collisions, conflicts and hooks.
Nothing touches the network or your real git configuration.

`scripts/synthetic-project.sh` covers what a suite of short-lived fixtures
cannot: it builds four clones and a thousand tasks over a couple of thousand
commits, churns them, closes them, crosses them in and out of the archive
directories, and then reports what `add`, `ls` and `log` cost on the result.
The repository is regenerated rather than committed — it lands in
`target/synthetic`, is kept so you can `cd` in and poke at it, and is replaced
by the next `--force` run. Run it before a release; it is deliberately not part
of CI.

Layout:

| Path | What it holds |
|---|---|
| `src/git.rs` | the `git` runner; every invocation goes through it |
| `src/repo.rs` | repository discovery, `.yman` state, preflight checks |
| `src/task.rs` | folder names, slugs, `t.md`/`m.yml`, load and save |
| `src/yml.rs` | the `m.yml` reader and writer, hand-written |
| `src/ids.rs` | id generation and what counts as taken |
| `src/refresh.rs` | the fast-forward policy |
| `src/commands/` | one module per subcommand |
| `scripts/spike-symref.sh` | proves committing in the worktree moves `refs/yman/local` |
| `scripts/synthetic-project.sh` | builds a large repository and measures what commands cost on it |
| `docs/` | the normative specification |

`docs/` holds the normative, as-built specification — [storage.md](docs/storage.md)
for the ref and on-disk contract, [commands.md](docs/commands.md) for per-command
semantics, [errors.md](docs/errors.md) for exit codes and the pinned messages.
They describe what the code does, so a disagreement between the two is a bug in
one of them.

---

## License

MIT. See [LICENSE](LICENSE).
