# TASKS

Backlog for `yman`. Derived from the as-built specification in `docs/`, the
"Limits and future work" section of `README.md`, and the current state of the
tree at `314ae88`.

Baseline as of 2026-09-18: `cargo test` is green (59 unit, 67 integration),
`docs/` and `README.md` describe the shipped behaviour, and every subcommand
listed in `src/cli.rs` is implemented.

Conventions: each entry names the files it touches and the condition that
closes it. Anything that changes a user-facing string must change
`tests/cli.rs` and `docs/errors.md` in the same commit.

---

## Correctness and data integrity

### Field-wise merge driver for `m.yml`

Two machines editing different fields of the same task produce a textual
conflict that the user resolves by hand, even though `status`, `priority`,
`tags`, `links` and `related` merge unambiguously in most cases.

- Where: new module (`src/merge.rs`), wired from `src/commands/sync.rs`;
  `docs/storage.md` needs a section on the driver and how it is registered.
- Done when: a driver merges non-overlapping field edits without user
  intervention, genuinely conflicting scalar edits still stop the sync with
  exit 3, and `tests/cli.rs` covers both outcomes.
- Open question: registering a merge driver means writing to the repository's
  config or `.gitattributes` inside `.yman`. Decide whether `yman init` does
  that, or whether it is opt-in — and record the decision in `docs/storage.md`
  before writing code.

### Force-add attachments that match `.yman/.gitignore`

`init` writes `*.swp`, `*~`, `.#*` and `*.orig` to `.yman/.gitignore`, and
`attach` stages with `git add -- <task dir>`, which silently skips ignored
paths. Attaching `notes.orig` therefore records the entry in `m.yml`, never
commits the file, and every other clone sees a dangling attachment after
`sync`.

- Where: `src/commands/attach.rs`, `tests/cli.rs`
- Done when: each attached file is staged with `git add -f -- <task>/f/<name>`
  (or such names are rejected with a pinned message), and a two-clone test
  attaches a `*.orig` file and finds it in `git -C .yman ls-files` on both
  sides.

### Preserve unknown `m.yml` keys on rewrite

The reader consumes and drops any key it does not know, and `docs/storage.md`
calls that accepted. There is no per-task schema version, so the day a newer
yman writes a field (`due:`, say), any older yman on the team destroys it on
the next `set`. Hand-added keys are lost the same way.

- Where: `src/yml.rs`, `src/task.rs` (`Meta`), `docs/storage.md`
- Done when: unknown top-level blocks survive a read-modify-write cycle
  verbatim, re-emitted after the known keys, and the doc sentence is
  reversed. Fix the reader's neighbouring inconsistencies in the same pass:
  an empty `status:` currently passes the required-field check as `""`, and
  duplicate keys are last-wins at top level but first-wins inside an
  attachment item.

### Validate the `author` id prefix

`author_prefix` returns `yman.author` or `$YMAN_AUTHOR` trimmed but otherwise
unchecked. A prefix containing `.` produces a folder such as `5.v.i-1.slug`,
which `FolderName::parse` reads as id `v` with slug `i-1.slug`; `/`, `\` or
whitespace make the folder unparsable, so the new task is invisible or
mis-identified.

- Where: `src/ids.rs`, `tests/cli.rs`, `docs/errors.md`, `docs/storage.md` §8
- Done when: a prefix that is empty or contains `.`, `/`, `\` or whitespace
  fails `add` with a pinned message, and the accepted grammar is written
  down next to the id schemes.

### A retitle can change a task's id on the next sync

`sync` decides a task was created locally by looking for an added
`{folder}/t.md` in `git diff --name-only --diff-filter=A -M <base> LOCAL`
(`src/commands/sync.rs:340-351`). But a retitle is a `git mv` plus a rewritten
first line of a short file, and git scores that rename as low as 10% — under
its default 50% threshold — so the pair reads as a delete plus an add. The task
then looks newly created, its own id counts as "taken on origin", and sync
renumbers it. Retitle offline, sync against a moved remote, and the id silently
changes. `scripts/synthetic-project.sh` reproduces it on every run.

- Where: `src/commands/sync.rs`, `tests/cli.rs`
- Done when: a retitled task keeps its id through a divergent sync, and a test
  pins it. The id is in the folder name at both ends, so the fix need not lean
  on git's rename scoring at all.

### Renumbering leaves `related` references dangling on other clones

`rewrite_related` (`src/commands/sync.rs:422-456`) walks the renumbering
clone's own tree — the pre-merge one. A reference minted on another clone, or
arriving in the same merge, keeps the old id and ends up pointing at nothing.
The synthetic run left 140 of 142 references dangling, because the entry above
made renumbering common.

- Where: `src/commands/sync.rs`, `docs/commands.md` §6, `tests/cli.rs`
- Done when: references on both sides of the merge are rewritten, and the scope
  sentence added to `docs/commands.md` §6 comes back out.

### Keep `related` references consistent

`set --relate` accepts an id that does not exist, and `rm` leaves dangling
references in every other task. The sync renumber path already rewrites
`related` entries, so the machinery exists.

- Where: `src/commands/set.rs`, `src/commands/rm.rs`, `docs/commands.md`
- Done when: relating to an unknown id prints a `warning:` on stderr but
  still commits (a concurrent `add` on another clone can legitimately race),
  `rm` drops references to the removed task in the same commit and reports
  the count, and the doc states that `related` is one-way and that id
  mentions inside `t.md` or `d.md` text are never rewritten.

---

## Robustness

### Keep a dangling attachment from breaking `show`

`detach` already tolerates an entry in `m.yml` whose file has disappeared
(`docs/commands.md` §4). Confirm `show` and `ls --json` behave the same way
rather than erroring, and add a regression test either way.

- Where: `src/commands/show.rs`, `src/commands/ls.rs`, `tests/cli.rs`
- Done when: a task whose attachment file was deleted outside yman renders with
  the entry marked as missing, and the behaviour is pinned in `docs/commands.md`.

### `d.md`: guard the entry header on write and on read

The parser starts a new entry at any line beginning with `## `, so a comment
whose text contains a markdown heading is split into a `Comment` plus a
`Raw` chunk and inflates the comment count in `ls` and `ls --json`. And
`append_entry` does not check that the file already ends with a blank line,
so a hand-edited tail glues the next header onto the previous line and the
whole chunk degrades to raw text.

- Where: `src/discussion.rs`, `docs/storage.md` §6
- Done when: the writer ensures a trailing blank line before appending and
  neutralises text lines that start with `#` (a leading space is enough; the
  reader trims), the reader only treats `## <timestamp> — ` with a
  whitespace-free timestamp as a header, and both cases have unit tests.

### A retitle and a comment on two clones conflict

`**/d.md merge=union` settles concurrent comments, but only while the folder
keeps its name. Retitle on one clone, comment on another, sync: the comment was
written against the old path, so it arrives as modify/delete and the merge stops
with exit 3 on a file the union driver is never consulted about. Recorded as a
limit in `docs/storage.md` §6; whether it is worth more than that is the open
question.

- Where: `src/commands/sync.rs`, `docs/storage.md` §6
- Done when: either the merge resolves it (the discussion is append-only, so the
  content is mergeable once the paths are matched up), or the limit is accepted
  and this entry goes.

### `show`: order the discussion by timestamp

`merge=union` places the other side's lines after ours in a conflicting hunk,
so two comments written concurrently on two clones can display out of time
order. Low priority.

- Where: `src/commands/show.rs`
- Done when: parsed `Comment` entries are shown sorted by their timestamp
  while `Raw` chunks keep their file position, and `docs/commands.md`
  mentions it.

### Document the `m.yml` reader's flow-sequence limit

The flow-sequence branch splits on a bare `,`, so a hand-written
`tags: ['a,b']` fails with `unterminated single-quoted string`. The writer
never emits flow form, so this is a reader limit, not a bug to fix.

- Where: `docs/storage.md` §6
- Done when: the limit is listed next to the other accepted-but-not-written
  forms.

### Windows support

`README.md` claims Windows "should work with `core.longpaths`" but nothing
tests it. Either verify it or stop claiming it.

- Where: path handling in `src/task.rs` (slugs, folder names) and `src/repo.rs`
  (`read_gitdir_file`); `tests/common/mod.rs` for the fixture.
- Done when: either the integration suite runs on Windows in CI, or the README
  sentence is downgraded to "untested".

Two more non-Linux filesystem cases belong to the same entry. On a
case-insensitive filesystem slugs cannot collide because `slugify` lowercases,
but attachment names (`attach` checks an exact `exists()`) and `author`
prefixes can. On macOS the filesystem stores NFD, so a Cyrillic slug minted on
Linux and `git mv`'d on macOS can diverge unless `core.precomposeunicode` is
set. Record both in `docs/storage.md` §5 even if nothing else changes.

---

## Performance

### Stop spawning a `git` process per ref read

Every ref lookup shells out (`Git::rev_parse`, `src/git.rs:124`), including on
the lazy refresh path that runs before nearly every command
(`src/refresh.rs:40`). That is two or more process spawns before `yman ls`
prints a line, and `ls` otherwise touches no git at all.

- Where: `src/git.rs`, `src/refresh.rs`
- Approach: batch the reads that happen together into one `git for-each-ref`
  call, or read the loose ref and `packed-refs` files directly and keep the
  spawn as a fallback. Do not add a git library dependency for this — see the
  dependency rule in `CLAUDE.md`.
- Measured baseline (`scripts/synthetic-project.sh`, 1000 tasks): `yman ls` 6
  processes and 32 ms, `yman show` 6 and 23 ms, `yman add` 11.
- Done when: `yman ls` in a repository that needs no refresh spawns at most one
  `git` process, and `scripts/spike-symref.sh` still passes.

### `find` by folder name only

`task::find` calls `list`, which reads `t.md` and `m.yml` for every folder in
`.yman/` and in every status directory, then keeps the one whose id matches.
Every `show`, `set`, `comment` and `attach` therefore pays for every closed task
on disk.

Archiving closed tasks took the edge off this — the common case is an open task,
and `list` could stop descending into the status directories once it has a hit
at the top level — but it did not fix it, and it added a second level to walk.

- Where: `src/task.rs`
- Measured baseline: 23 ms for `yman show <id>` at 1014 tasks.
- Done when: `find` parses directory names with `FolderName::parse` at both
  levels, loads only the matching folder, still reports `duplicate task id …`
  from the names alone (relative paths, since two copies can share a leaf
  name), and the broken-folder error path is unchanged.

### `log <id>` rebuilds the whole history's rename graph

`historical_names` (`src/commands/log.rs:35-78`) runs an uncapped
`git log --diff-filter=R -M` over the entire history to find the paths one task
ever had, then passes them all to a second `git log`. The cost tracks the
number of renames in the repository, not in the task: measured at 420 ms
against 3802 rename records, for a task with 17 commits.

- Where: `src/commands/log.rs`
- Done when: the walk is bounded — by `-n`, by restricting the first log to the
  task's own paths, or by giving the final log `--follow` and dropping the graph
  — and `scripts/synthetic-project.sh` reports a smaller number.

---

## Dependencies and tooling

### Add a CI workflow

`CLAUDE.md` calls `cargo clippy --all-targets -- -D warnings` the
"CI-equivalent gate", but there is no `.github/` directory — the gate only runs
when someone remembers to run it.

- Where: new `.github/workflows/ci.yml`
- Done when: pushes and pull requests run `cargo fmt --check`, the clippy gate,
  `cargo test` and `sh scripts/spike-symref.sh` — the same four steps as the
  `verify` skill in `.claude/skills/verify/SKILL.md`.

---

## Distribution

### Shell completions and a man page

`clap` can generate both from the existing `src/cli.rs` derives.

- Where: `src/cli.rs`, a new `yman completions <shell>` subcommand or a build
  script; `docs/commands.md` if it becomes a subcommand.
- Done when: bash, zsh and fish completions are generated from the same
  definition the binary uses, so they cannot drift.

### Release process

The crate is at `0.1.0` with no changelog and no tagged release.

- Done when: there is a `CHANGELOG.md`, a documented tagging convention, and a
  decision on whether the crate is published to crates.io. The binary is `yman`
  while the package is `yman-tracker`; if publishing, check the name is free
  before committing to it.

---

## Agents and scripts

Agents drive `yman` through a shell, one call at a time, and pay per byte
they read back. The entries below cut calls and bytes without moving any
human default, table layout or pinned string. All are additive and opt-in.

### `edit` refuses the `vi` fallback without a terminal

With neither `$VISUAL` nor `$EDITOR` set, `edit` spawns `vi` even when stdin
is a pipe. In an agent's shell that hangs until a timeout.

- Where: `open_editor` in `src/commands/edit.rs`; `docs/errors.md`,
  `docs/commands.md` §4.
- Done when: the fallback bails with
  `no terminal for vi; set $EDITOR, or use -m / --body-file` before spawning,
  an explicit `EDITOR` still works, and a test removes both variables.

### Exit code 4 for an unknown task id

`task 99 not found` exits 1 like every other error, so a script cannot tell
a typo from a broken repository without parsing stderr.

- Where: a `NotFound` marker in `src/errors.rs` next to `MergePending`,
  raised from `task::find`; `docs/errors.md`, `README.md` exit tables.
- Done when: every command that looks up an id exits 4 on a missing one,
  `broken` and `duplicate` still exit 1, and the message is unchanged.

### Describe every flag; add `yman guide`

`yman set --help` shows bare flags, and the only documentation is a 19 KB
README.

- Where: doc comments on every bare field in `src/cli.rs`, a short
  `after_help` on the top-level command; new `docs/agents.md` (60 lines at
  most) printed by `src/commands/guide.rs` via `include_str!`; `src/main.rs`
  handles `guide` before `repo::discover()`; `docs/commands.md` §1 and §3,
  `README.md`.
- Done when: `yman guide` works outside any repository and prints the file
  byte for byte, `--help` points at it, and every flag has a one-line
  description.

### Move the JSON writer into `src/json.rs`

`ls --json` escapes and assembles its output inline in `src/commands/ls.rs`.
A second JSON producer would copy that code.

- Where: new `src/json.rs` with `string`/`opt_string`/`strings`/`array` and
  an ordered object builder; `print_json` in `src/commands/ls.rs` rebuilt on
  it; `CLAUDE.md` dependencies note.
- Done when: `ls --json` output is byte-identical, and unit tests cover
  escaping, key order and empty collections. No `serde_json`.

### `--json` for `show` and `status`; `links` and `related` in `ls --json`

Scripts that want a task's links or the sync state have to parse text.

- Where: `src/commands/show.rs`, `src/commands/status.rs` (a shared report
  struct so the text path stays byte-identical), `Cmd::Status` grows args;
  `docs/commands.md` §3 shapes, `README.md`.
- Done when: `show --json` is one object including attachments and the
  discussion (honouring `-n`), `status --json` reports refs, ahead/behind,
  merge state and counts, and `ls --json` gains `links` and `related` after
  `assignee`.

---

## Explicitly out of scope

Recorded so they are not re-proposed. Reopening any of these is a design
decision, not a task.

- git-lfs integration for attachments. Attachments go straight into git; the
  5 MiB warning is the whole policy.
- More than one `.yman` worktree per clone.
- Colour output, a TUI, or a web UI.
- A GitHub Issues bridge.
- New dependencies added for convenience. There is no `regex` and no
  `serde_json` on purpose; adding a crate needs a reason.
