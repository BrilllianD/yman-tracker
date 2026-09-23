# Errors, exit codes and output streams

Normative, as-built. Companion documents: [storage.md](storage.md),
[commands.md](commands.md).

## Streams

| Stream | Carries |
|---|---|
| stdout | data only — task listings, `show`, `path`, `log`, `status`, summaries |
| stderr | `error: `, `warning: `, `note: `, and git's own output when passed through |

This split is a contract, not a style choice: `cd $(yman path 14)` and
`yman ls --json \| jq` must stay usable. Nothing decorative goes to stdout, and
there is no colour anywhere.

## Exit codes

| Code | Meaning | Produced by |
|---|---|---|
| `0` | success | |
| `1` | error | any `anyhow` error reaching `main` |
| `2` | usage error | the argument parser, before anything runs |
| `3` | a sync merge is unresolved | the `MergePending` marker error |
| `4` | no task has the given id | the `NotFound` marker error, from `task::find` |

Messages that carry code 3, beyond the preflight and merge ones: a
`sync --continue` that would leave two folders for one id reports
`duplicate task id <id>: <relA>, <relB>; delete one folder, then: yman sync --continue`.

Code 4 is only `task <id> not found`. A folder that exists but is broken or
duplicated is a repository problem, not a missing task, and stays at code 1.
A verb given several ids exits 4 at the first missing one, with the tasks
before it already committed.

Codes 3 and 4 travel as distinct error types (`src/errors.rs`) and are
recognised in `main` by downcasting. Returning the same text as a plain
`anyhow` error would silently degrade it to code 1, which scripts cannot
distinguish from a genuine failure.

Every error prints as `error: {message}`, using anyhow's `{:#}` so the whole
context chain is shown.

## Pinned messages

These strings are part of the interface. Several are asserted verbatim in
`tests/cli.rs`; changing one means changing the test and this table in the same
commit.

| Situation | Message |
|---|---|
| not in a repository | `not inside a git repository` |
| no origin, no `--remote` | `main repo has no "origin" remote; pass --remote <url>` |
| `.yman` missing | `.yman is not initialized; run: yman init` |
| `.yman` HEAD wrong | `.yman worktree is not on refs/yman/local; run: yman init` |
| `.yman` is someone else's directory | `.yman exists and is not a yman worktree; move it aside and rerun` |
| `.yman` is a standalone repo | `.yman is a standalone git repository, not a worktree; move it aside and rerun` |
| ref already checked out | `refs/yman/local is already checked out in another worktree of this repo; only one .yman per clone is supported` |
| unknown task | `task <id> not found` |
| task present but unloadable | `task <id> is broken: <dir>: <why>` |
| duplicate folders for one id | `duplicate task id <id>: <relA>, <relB>` — paths relative to `.yman`, so the two may differ only in their status directory |
| bad status | `unknown status "<s>"; allowed: <list joined by ", ">` |
| unsupported config version | `invalid .yman/config.toml: unsupported version <n> (this yman understands 1 and 2)` |
| a status role on a version 1 config | `invalid .yman/config.toml: statuses.<key> needs version = 2; bump version in .yman/config.toml` |
| a status role outside the list | `invalid .yman/config.toml: statuses.<start\|done\|cancel> "<s>" is not in statuses.list` |
| a terminal entry outside the list | `invalid .yman/config.toml: statuses.terminal entry "<s>" is not in statuses.list` |
| a terminal entry unusable as a directory | `invalid .yman/config.toml: statuses.terminal entry "<s>" is not a usable directory name; use letters, digits, "_" and "-"` |
| a repeated terminal entry | `invalid .yman/config.toml: statuses.terminal lists "<s>" twice` |
| terminal entries differing only in case | `invalid .yman/config.toml: statuses.terminal entries "<a>" and "<b>" differ only in case; they would collide on a case-insensitive filesystem` |
| several terminal statuses and no named done | `invalid .yman/config.toml: statuses.terminal lists more than one closed status, so statuses.done must say which one \`yman done\` means` |
| the done status is not terminal | `invalid .yman/config.toml: statuses.done "<s>" is not in statuses.terminal` |
| the cancel status is not terminal | `invalid .yman/config.toml: statuses.cancel "<s>" is not in statuses.terminal` |
| `default` or the start status is terminal | `invalid .yman/config.toml: statuses.terminal must not contain statuses.default "<s>"` / `… must not contain the start status "<s>"` |
| every status is terminal | `invalid .yman/config.toml: statuses.terminal marks every status terminal; at least one must stay open` |
| empty title | `title must not be empty` |
| empty tag | `tag must not be empty` |
| tag with a separator or a control character | `invalid tag "<t>"; tags must not contain whitespace, commas or control characters` — `<t>` is the trimmed value |
| unknown attachment | `no attachment "<name>" on task <id>` |
| attachment name already used | `attachment "<name>" already exists on task <id>; use --force` |
| `--name` with several files | `--name only works with a single file` |
| attachment source unusable | `cannot attach <path>: <why>` / `… not a regular file` |
| attachment name with a separator | `attachment name "<name>" must not contain a path separator` |
| `add` onto an existing folder | `folder already exists: <dir>` |
| empty comment (`comment`, or `-m` on `set` and the verbs) | `empty comment` |
| `--body-file` cannot be read | `cannot read <path>: <why>` — `<why>` is the OS error text |
| `cancel` with no cancel status | `no cancel status configured; set statuses.cancel in .yman/config.toml` |
| `reopen` on an open task | `task <id> is not closed (status "<s>"); closed statuses: <terminal joined by ", ">` |
| `rm` or `tags rm` with no terminal and no `-f` | `refusing to remove without -f` |
| editor failed | `editor exited with status N; file left as is` (or `editor was killed by a signal; …`) |
| no editor resolvable | `no editor configured; set $EDITOR` |
| neither `$VISUAL` nor `$EDITOR` set, stdin not a terminal | `no terminal for vi; set $EDITOR, or use -m / --body-file` |
| `rm` or `tags rm` prompt declined | `aborted` |
| `t.md` unparsable | `t.md must start with "# Title"` |
| `t.md` unparsable after an edit | `t.md invalid after edit: <why>; fix the file then run: yman edit <id>` |
| invalid config | `invalid .yman/config.toml: <why>` |
| origin holds a non-yman history | `refs/tasks/main on origin is not a yman history (missing or invalid config.toml): <why>` |
| unrelated histories | `task history unrelated to origin refs/tasks/main; re-init from remote:  rm -rf .yman && git update-ref -d refs/yman/local && yman init` |
| author prefix unresolvable | `author prefix unknown; run: git config yman.author <prefix>  (or set YMAN_AUTHOR)` |
| author prefix not a legal id prefix | `author prefix "<p>" from <source> must be letters, digits, "_" or "-"` — `<source>` is `yman.author`, `$YMAN_AUTHOR` or `user.name initials` |
| git identity missing | `git identity missing; run: git config --global user.name "…" && git config --global user.email "…"` |
| git binary absent | `git not found in PATH` |
| any other git failure | `git <subcommand> failed: <trimmed stderr>` |
| push rejected three times | `origin keeps moving; retry yman sync` |
| push rejected while finishing a merge | `origin moved while finishing the merge; run: yman sync` |
| network step failed | `fetch failed` / `push failed` / `merge failed`, with git's stderr printed above |
| `init` fetch failed | `fetch failed (see above); use --offline to skip` |
| `m.yml` merge driver could not fall back | `git merge-file failed`, with git's stderr printed above — reaches the user through git's own merge output |

### Exit-code-3 messages

| Situation | Message |
|---|---|
| mutating command during a merge | `sync merge in progress; resolve conflicts then run: yman sync --continue  (or: yman sync --abort)` |
| `sync` during a merge | `merge in progress; resolve then: yman sync --continue  (or --abort)` |
| merge produced conflicts | `conflicts in N file(s); edit them, remove markers, then: yman sync --continue  (or: yman sync --abort)` |
| `--continue` with markers left | `still unmerged: <files>` |
| `--continue` with an unloadable task | `conflict markers or invalid task in <dir>: <why>` |
| `--continue`/`--abort` with nothing to finish | `no merge in progress` |

## Warnings and notes

Never fatal, always stderr:

| Message | When |
|---|---|
| `warning: id scheme is "<s>" (from config.toml); --id-scheme ignored` | `init` on an existing history with a conflicting flag |
| `warning: origin already points at <url>; --remote ignored` | `init --remote` where origin already exists |
| `warning: remote already had tasks; adopted remote state` | two clones initialized the tracker at once |
| `warning: refs/tasks/main disappeared from origin; will recreate it` | the ref was deleted server-side |
| `warning: <name> is N MiB; git is not great at large binaries` | attaching a file over 5 MiB |
| `warning: installing into core.hooksPath=<p>` | hooks redirected away from `.git/hooks` |
| `warning: task <id> does not exist here; relating anyway` | `set --relate` naming an id no folder here carries |
| `warning: skipped N unreadable task folder(s): <rels>` | `tags`, `tags rename`, `tags rm` and `rm` walking every task |
| `warning: <path> not moved; <dest> already exists` | `sync` rejoining a split folder found the same file on both sides |
| `note: added remote "origin" -> <url>` | `init --remote` created the remote |
| `note: .yman has uncommitted changes, refresh skipped` | refresh backed off |
| `note: rewrote N reference(s) to renumbered ids` | a collision renumber moved ids other tasks related to |
| `note: moved N file(s) left under <old> into <rel>` | a merge left files under a folder the other side had moved |
| `note: dropped N reference(s) to <id>` | `rm` cleared the removed task out of other tasks' `related` |
| `note: task <id> was closed to two different statuses; keep one of <relA>, <relB>` | a merge renamed one task into two status directories |
| `note: task folder is now <rel>` | a status change moved the folder into or out of a status directory |

## Deliberate non-errors

- Committing when nothing is staged — an identical re-application is a no-op.
- Removing a tag, link or relation that was never there.
- A `status` value that is no longer in `config.statuses.list`: reported, so a
  config change cannot brick existing tasks.
- A task folder sitting somewhere other than where its status says it belongs:
  it is listed correctly, and the next `set` moves it.
- A fetch that fails only because origin has no `refs/tasks/main` yet.
- A task folder that fails to load: it becomes a *broken* entry that `ls` shows
  and other commands refuse individually, rather than an error that hides every
  other task.
