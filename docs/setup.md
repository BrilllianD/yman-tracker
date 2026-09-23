# Setup

Normative, as-built. Getting from "no `.yman`" to a tracker that reads, writes
and syncs. Companion documents: [commands.md](commands.md) for per-command
semantics, [storage.md](storage.md) for the ref and on-disk contract,
[errors.md](errors.md) for the pinned messages.

## 1. Prerequisites

`yman` runs `git` as a subprocess; without it, every command fails with
`git not found in PATH` except `guide`, `completions` and `man`, which are
answered before any git process runs. The current directory must be inside the project's git
repository — any subdirectory will do — or the command fails with
`not inside a git repository`. There is no `-C <dir>` flag and no global
configuration file outside the repository.

Until `.yman` exists, every command except `init`, `hooks`, `git`, `guide`,
`completions` and `man` fails with `.yman is not initialized; run: yman init` (`commands.md` §1;
`merge-driver` is exempt too, but git calls that, not people).

## 2. `yman init`

```sh
yman init                     # uses "origin"; with none, the tracker stays local
yman init --remote <url>      # no origin yet: create one pointing there
yman init --offline           # no network at all
```

| Flag | Effect |
|---|---|
| `--id-scheme <random\|seq\|author>` | how ids are minted; default `seq`. Stored in `config.toml`, so it is only read when the history is being created — see below |
| `--author <PFX>` | prefix for the `author` scheme; stored as git config `yman.author` |
| `--remote <URL>` | create `origin` from this URL when the repo has none |
| `--offline` | do not fetch and do not push |
| `--hooks` | also run `hooks install` |
| `--refresh <lazy\|manual>` | the refresh policy, stored as git config `yman.refresh`; default `lazy` |

Remote resolution happens first. With both an `origin` and `--remote`, the
existing `origin` wins. `--remote` is ignored, with a warning when its URL
differs from `origin`'s and silently when it is the same. With neither,
`init` sets up a **local-only** tracker and says so on stderr:
`note: no "origin" remote; tasks stay local until you run: yman init --remote <url>`.
It behaves as if `--offline` were given, and adds no fetch refspec, since
there is no `remote.origin.fetch` to add it to. Every command except `sync`
works as usual (`sync --abort` still runs, since it needs no remote); `status` reports `remote: none (no "origin"; tasks are local
only)`.

To connect a local-only tracker later, run `yman init --remote <url>`. It takes
the repair path below, creates `origin` and the refspec, and the next
`yman sync` publishes the local history. That works when the remote has no
`refs/tasks/main` yet. If another clone already published a tracker there, the
two histories are unrelated and `sync` refuses with the re-init message in
[errors.md](errors.md). Adding `origin` by hand with `git remote add` works as
well, though a plain `git fetch` will not carry task commits until `yman init`
has added the refspec.

`init` is idempotent. Run against an existing `.yman` worktree it takes the
repair path — re-adds the exclude entry and the fetch refspec, re-registers the
merge driver, applies `--refresh`, `--author` and `--hooks` if given — and
reports `already initialized .yman`. A `.yman` that is a plain directory or a
standalone repository is never touched; the command says so and stops.

## 3. What `init` changes

In the **main repo**, all local and none of it committed:

- `.yman/` added to `COMMON/info/exclude`;
- the fetch refspec `+refs/tasks/main:refs/yman/remote` appended to
  `remote.origin.fetch`, when there is an `origin`;
- `yman.refresh`, and `yman.author` when `--author` was given;
- `merge.ymanmeta.name` and `merge.ymanmeta.driver`, pointing at the running
  binary by absolute path. This pairs with the `**/m.yml merge=ymanmeta` line
  in `.gitattributes`, which travels in the history while the config does not —
  which is why every clone must run `init` even when the history already
  exists.

In **`.yman`** — only when the history is being created — `config.toml`,
`.gitignore` and `.gitattributes`, committed as `yman: init (<scheme>)` on top
of an empty root commit `yman: init`, and pushed unless the run is `--offline`
or the tracker is local-only.

## 4. There is no clone command

`.yman` is a linked worktree of the project repo, not a second repository, so
cloning the project does not bring the tasks with it. `init` covers both cases:

- `refs/yman/local` already exists → keep it;
- otherwise `refs/tasks/main` was fetched from `origin` → **adopt it**
  (`update-ref refs/yman/local refs/yman/remote`). This is the clone path;
- otherwise create an empty root commit and write a fresh tracker.

Because the second case reuses a history that already has an id scheme,
`--id-scheme` is ignored there, with
`warning: id scheme is "<s>" (from config.toml); --id-scheme ignored`. A remote
whose `refs/tasks/main` is not a yman history fails with
`refs/tasks/main on origin is not a yman history (missing or invalid
config.toml)`.

One `.yman` per clone. When git refuses the checkout, a second attempt fails
with `refs/yman/local is already checked out in another worktree of this repo`.
From a linked worktree of the project git may not refuse it (the worktree is
added with `--detach`); see [storage.md §2](storage.md#2-the-ref-namespace).

## 5. Staying up to date

`yman sync` is the command that uses the network day to day: fetch, merge,
push. The only other one is `init`, which fetches and pushes unless given
`--offline`.
`--no-push` stops before publishing; `--continue` and `--abort` finish or drop a
conflicted merge (`commands.md` §6, and exit code 3 in `errors.md`).

`yman refresh` never uses the network — it fast-forwards `.yman` onto a
`refs/yman/remote` that some earlier fetch already brought in. The policy
decides who calls it (`commands.md` §5):

| `yman.refresh` | Effect |
|---|---|
| `lazy` (default) | every non-exempt command fast-forwards first; a failure is a `warning:`, never fatal |
| `manual` | only `yman refresh` and `yman sync` move `.yman` |

`yman hooks install` (or `init --hooks`) writes `post-merge` and `post-checkout`
hooks that run `yman refresh --quiet`, so an ordinary `git pull` of the project
also picks up the tasks it fetched. `hooks status` reports
`installed` / `foreign` / `absent`, and a hook yman did not write is never
overwritten.

## 6. Identity

| Variable | Effect |
|---|---|
| `YMAN_ACTOR` | the name recorded as a comment's author and an attachment's `by`. Empty or unset falls back to `git config user.name`, then `unknown`. It does not change the git committer |
| `YMAN_AUTHOR` | prefix for the `author` id scheme when `yman.author` is unset |
| `VISUAL`, `EDITOR` | the editor `edit`, `add -e` and `comment -e` open. Scripts and agents should use `-m` or `--body-file` instead and never set these |

## 7. Checking the result

```sh
yman status      # ref heads, ahead/behind, refresh policy, hooks, task counts
yman ls          # the tasks themselves
```

`status` changes nothing and refreshes nothing, so it is the safe first call on
a tracker in an unknown state — including one with an unresolved merge, which it
names file by file.
