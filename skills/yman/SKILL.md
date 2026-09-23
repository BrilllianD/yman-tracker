---
name: yman
description: Drive the `yman` git-native task tracker from a non-interactive shell - list, claim, update, close, comment on and relate tasks without ever opening an editor. Use whenever the repository has a `.yman/` directory or `yman` is on PATH, whenever the user mentions a task, ticket, backlog or issue in such a repository, and whenever a prompt contains a `yman` command. Covers the agent-safe subset, the exit codes, and how to recover from an unresolved sync merge (exit 3).
---

# yman for agents

Tasks live in `.yman/`, a git worktree on a ref outside `refs/heads/`. Every
mutation is its own commit, so there is nothing to save, stage or flush. All
commands run from anywhere inside the project's git repository.

## Hot path

```sh
yman ls -n 10 --assignee -                        # pick: unassigned, best priority first
yman show <id> -n 3                               # read: header, body, last 3 comments
yman set <id> --status doing -a <me> -m "on it"   # claim: status, assignee, note, one commit
yman done <id> -m "what changed"                  # close and explain in one call
yman add "Title" -m "body" -a <me> --relate <id>  # follow-up, linked to its parent
```

## When asked for the task list

1. Run `yman ls -l`: open tasks with their bodies, already sorted by priority,
   status order and id. On a large tracker cap it with `-n`.
2. Show the rows as a markdown table, `P | ID | Status | Title | Tags`, in the
   order `ls` printed them. A `! <dir> broken <error>` row goes under the
   table as-is, not dropped.
3. Work out what can be taken next. Prefer the best priority; within it, a
   `todo` task that is unassigned (or assigned to you) over one somebody else
   already has in progress. Skip a task whose body or relations say it waits on
   another open task.
4. Read only the top one to three candidates with `yman show <id> -n 0`, for
   their "Where", "Done when" and relations. Do not `show` every task.
5. Finish with a short recommendation: which id to take, why, and roughly how
   big it is.

## Reading

`ls` prints one row per task and touches no git:

```
P  ID  STATUS  TITLE  TAGS  F  C
```

`P` is priority, 0 (highest) to 9. `F` is the attachment count, `C` the comment
count; a zero renders blank, not `0`, and trailing empty columns are dropped.
The header line appears only when stdout is a terminal, so a piped `ls` is
rows only. An empty result prints nothing at all. A folder yman cannot parse is
always listed, as `! <dir> broken <error>`.

Filters AND together; `-s` and `-t` are repeatable, and `-t` and `-q` compare
lowercase:

| Flag | Meaning |
| --- | --- |
| `-s, --status <STATUS>` | only these statuses; naming a closed status includes it |
| `-t, --tag <TAG>` | tasks carrying **all** the given tags |
| `--assignee <WHO>` | `-` means unassigned |
| `-p, --priority <N>` | 0 to 9 |
| `-q, --grep <TEXT>` | case-insensitive substring of title or body, not a regex |
| `-a, --all` | include closed tasks, hidden by default |
| `-n, --limit <N>` | first N rows, applied after sorting |
| `-l, --long` | each task's body under its row, indented four spaces; `body` in `--json` |

Sorting is by priority, then status order, then id — so `-n` keeps the most
important rows, not arbitrary ones.

Other readers: `show <id>` (one id; `-n <N>` keeps the last N discussion
entries, `-n 0` drops the discussion entirely; an attachment whose file was
deleted outside yman is listed marked `(missing)`, `"missing":true` in
`--json`), `path <id>` (prints the absolute
folder path and nothing else, so `cd "$(yman path 14)"` works), `log [<id>]`
(`-n`, default 20; with an id it follows the task across renames), `tags` (every
tag with its task count, closed tasks included) and `status`.

`--json` exists on `ls`, `show`, `status` and `tags` only. There is no global
`--json`, no `-C <dir>`, no `--verbose`. JSON omits nothing: empty lists are
`[]`, an absent assignee is `null`.

## Writing

`add "<title>"` takes `-p`, `-s`, `-t` (repeatable), `-m <text>` for the body,
`--body-file <path|->`, `-a <who>`, `--link <url>` and `--relate <id>` (both
repeatable). It prints `added <id>  <dir>`.

`set <id>` changes any field and needs at least one change flag, or the parser
exits 2: `--status`, `--priority`, `--title` (re-slugs the folder), `-a/--assignee`,
`--no-assignee`, `--tag`/`--untag`, `--link`/`--unlink`, `--relate`/`--unrelate`,
`--body <text>` (empty string clears it) or `--body-file <path|->`, and
`-m <text>` to comment in the same commit. List fields are sets: adding a tag
that is already there, or removing one that is not, is silently a no-op. When
nothing actually changed it prints `no changes` and commits nothing — that is
success, not failure.

`start`, `done`, `cancel`, `reopen`, `move <id>... <status>` and
`prio <id>... <0-9>` all take **several ids** and all take `-m <text>`. Each id
is its own commit, processed in order.

`comment <id> -m "<text>"` appends to the discussion. With no `-m` and stdin not
a terminal it reads the comment from stdin, so `yman comment 14 < note.md` is
safe. `attach <id> <file>...` (`--name <name>` for a single file, `--force` to
overwrite; over 5 MiB warns but never refuses) and `detach <id> <name>` handle
files. `tags rename <old> <new>` and `tags rm <tag> -f` rewrite every task
carrying a tag in one commit.

Prefer one verb carrying `-m` over a verb followed by a separate `comment`: half
the calls, half the commits.

## Never do this

| Do not | Why | Instead |
| --- | --- | --- |
| `yman edit <id>` | always opens `$VISUAL`/`$EDITOR`/`vi` | `yman set <id> --body "<text>"` or `--body-file -` |
| `yman add -e` | opens the editor before the first commit | `yman add "<title>" -m "<body>"` |
| `yman comment -e` | opens the editor | `yman comment <id> -m "<text>"`, or pipe on stdin |
| `yman rm <id>` without `-f` | prompts on a terminal | `yman rm <id> -f` |
| `yman tags rm <tag>` without `-f` | prompts on a terminal | `yman tags rm <tag> -f` |

These fail fast rather than hanging. Off a terminal, a removal without `-f`
prints `refusing to remove without -f`, and an editor path with neither
`$VISUAL` nor `$EDITOR` set prints
`no terminal for vi; set $EDITOR, or use -m / --body-file`. Both mean "use the
non-interactive form", not "retry".

## Streams and exit codes

stdout carries data only — listings, `show`, `path`, `log`, `status`, and the
one-line summaries. `error:`, `warning:`, `note:` and pass-through git output
go to stderr. There is no colour.

| Code | Meaning |
| --- | --- |
| `0` | ok |
| `1` | error; stderr says what |
| `2` | usage error, from the argument parser, before anything ran |
| `3` | a sync merge is unresolved |
| `4` | no task has that id (`task <id> not found`) |

Two consequences worth planning for:

- While a merge is unresolved, **every** mutating command exits 3, not just
  `sync`. Reading still works.
- A multi-id verb that exits 4 stopped at the first missing id. The ids before
  it are already committed; do not re-run the whole list blindly.

## Recovering from exit 3

```sh
yman status                  # names the unmerged files under .yman/
# edit them, remove the conflict markers
yman sync --continue         # or: yman sync --abort
```

`yman sync` is the only command that touches the network: it fetches, merges and
pushes. `--no-push` merges without publishing.

## Attribution

Export `YMAN_ACTOR=<agent name>` and comments and attachments are recorded as
yours. It does not change the git committer, which stays whatever git says it is.

## Cost

Plain output is the cheapest thing yman prints. `--json` repeats every key on
every row, so reach for it only when the output goes into `jq`. Cap with `-n`,
use `show <id> -n 0` when you only need the header and body, and never `ls`
without a filter on a large tracker.

## If `.yman` does not exist

Every command except `init`, `hooks`, `git`, `guide`, `completions` and `man` fails with
`.yman is not initialized; run: yman init`. The common case is one command:

```sh
yman init --remote <url>     # omit --remote when the repo already has an origin
yman init                    # no origin at all: a local-only tracker; sync refuses until --remote
```

If `origin` already carries a task history, `init` adopts it — there is no
separate clone step. The full runbook is `docs/setup.md` in the yman-tracker
repository.

`yman guide` prints the same manual straight from the binary, works in any
directory, and needs no repository.
