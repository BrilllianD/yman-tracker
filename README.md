# yman

A task tracker that lives next to the code, syncs through the project's own git
remote, and never shows up in `git branch`, your IDE's branch picker, or CI.

```
yman init
yman add "Fix login" -p 2 -t auth
yman start 1
yman comment 1 -m "reproduced on staging"
yman done 1
yman sync
```

## How it stores things

`.yman/` in the repo root is a **linked git worktree of the same repository**.
Its HEAD is the symbolic ref `refs/yman/local` — deliberately outside
`refs/heads/`, so nothing that lists branches ever sees it. Tasks are committed
into that ref; the code history is never touched.

| Thing | Where |
|---|---|
| Task history, locally | `refs/yman/local` |
| What origin last told us | `refs/yman/remote` |
| The ref on the server | `refs/tasks/main` (never a branch) |
| Working copy of the tasks | `.yman/`, listed in `.git/info/exclude` |

`yman init` adds one line to the repo config —
`remote.origin.fetch = +refs/tasks/main:refs/yman/remote` — so an ordinary
`git fetch` brings task commits along for free. Pushing is always explicit
(`git push origin refs/yman/local:refs/tasks/main`); `remote.origin.push` is
never set, so your plain `git push` keeps doing exactly what it did before.

Because `refs/tasks/main` is not a branch, hosting providers show no new branch
and start no pipeline.

One task is one folder:

```
.yman/2.14.fix-login/
  t.md      # "# Title" plus free-form body
  m.yml     # status, tags, assignee, timestamps, attachments, links, related
  d.md      # discussion, append-only, merged with merge=union
  f/        # attachments
```

The folder name `{priority}.{id}.{slug}` is the source of truth for the
priority and the id — `m.yml` does not repeat them, and changing either is a
`git mv`. Priority is `0`–`9` with `0` highest, so a plain `ls .yman/` already
lists the most urgent work first.

## Commands

```
yman init     [--id-scheme random|seq|author] [--author <pfx>] [--remote <url>]
              [--offline] [--hooks] [--refresh lazy|manual]
yman add      <title> [-p 0-9] [-s <status>] [-t <tag>]... [-m <body>] [-e]
yman ls       [-s <status>]... [-t <tag>]... [-a] [--json]
yman show     <id>
yman edit     <id>
yman set      <id> [--status S] [--priority 0-9] [--title T]
                   [--assignee A|--no-assignee] [--tag X]... [--untag X]...
                   [--link URL]... [--unlink URL]... [--relate ID]... [--unrelate ID]...
yman start    <id>              yman done <id>              yman prio <id> <0-9>
yman rm       <id> [-f]
yman attach   <id> <file>... [--name <n>] [--force]
yman detach   <id> <name>
yman comment  <id> [-m <text>] [-e]
yman path     <id>              # cd $(yman path 14)
yman log      [<id>] [-n <N>]   # follows folder renames
yman status
yman refresh  [--quiet]
yman hooks    install|remove|status
yman sync     [--continue] [--abort] [--no-push]
yman git      [--] <args>...    # raw git, inside .yman
```

Exit codes: `0` success, `1` error, `2` usage (from clap), `3` a sync merge is
waiting to be resolved.

## Staying in sync

`yman sync` is the only command that talks to the network. It snapshots any
uncommitted edits in `.yman`, fetches, merges, and pushes.

Two people working offline with the `seq` or `author` scheme will mint the same
id. On sync, the side that is *behind* renumbers its own new tasks, because the
other side's ids are already published:

```
renumbered 2 -> 3  (id taken on origin)
```

References to the old id inside other tasks' `related` lists are **not**
rewritten; sync prints a note when this can matter.

Conflicting edits to the same `m.yml` stop the sync with exit code 3. Edit the
files, remove the markers, and run `yman sync --continue` — staging is yman's
job, not yours — or `yman sync --abort` to throw the merge away. Comments never
conflict: `.gitattributes` marks `*/d.md` as `merge=union`.

### Refresh: how task commits reach your working copy

| `yman.refresh` | Effect |
|---|---|
| `lazy` (default) | Every read/write command fast-forwards `.yman` onto whatever `refs/yman/remote` already holds |
| `manual` | Only `yman refresh` and `yman sync` move `.yman` |

A refresh is always a fast-forward and never touches the network. It backs off
silently when `.yman` has uncommitted changes or local commits that are not
pushed yet; `yman refresh` and `yman status` say why.

`yman hooks install` adds `post-merge` and `post-checkout` hooks that run
`yman refresh --quiet`, so a plain `git pull` updates your tasks too. Hooks that
someone else wrote are reported, never overwritten or deleted.

## Things worth knowing

- **`git clean -fdx` deletes `.yman/`.** The history survives in
  `refs/yman/local`; run `yman init` again and everything comes back, including
  tasks you had not pushed.
- Only one `.yman` worktree per clone is supported.
- `yman` shells out to the `git` binary (2.42+), and requires a git identity
  (`user.name`, `user.email`) like any other committing tool.
- Unknown keys in `m.yml` are dropped when yman rewrites the file.
- Windows: should work with `core.longpaths`, but is not tested.

## Future work

- A field-wise merge driver for `m.yml`, so status and tag edits auto-resolve.
- Rewriting `related` ids after a renumber.
- git-lfs for attachments.
- Reading refs without spawning git on every lazy refresh.
- Several `.yman` worktrees per clone.
- Colour, a TUI, a web UI, a GitHub Issues bridge.

## Development

```
cargo test                      # unit tests plus the two-clone integration suite
cargo clippy --all-targets -- -D warnings
sh scripts/spike-symref.sh      # the git invariant this design rests on
```

The integration tests build a bare remote and two clones in a temp dir; nothing
touches the network or your real git configuration.
