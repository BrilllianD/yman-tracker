# TASKS

Backlog for `yman`. Derived from the as-built specification in `docs/`, the
"Limits and future work" section of `README.md`, and the current state of the
tree at `314ae88`.

Baseline as of 2026-09-18: `cargo test` is green (59 unit, 81 integration),
`docs/` and `README.md` describe the shipped behaviour, and every subcommand
listed in `src/cli.rs` is implemented.

Conventions: each entry names the files it touches and the condition that
closes it. Anything that changes a user-facing string must change
`tests/cli.rs` and `docs/errors.md` in the same commit.

---

## Correctness and data integrity

### Renumbering leaves `related` references dangling on other clones

`rewrite_related` (`src/commands/sync.rs:422-456`) walks the renumbering
clone's own tree — the pre-merge one. A reference minted on another clone, or
arriving in the same merge, keeps the old id and ends up pointing at nothing.
The synthetic run used to leave 140 of 142 references dangling, but that was
downstream of a retitle renumbering the task; with that fixed the same run
leaves 0 of 142 dangling and no longer reproduces this at all. The limitation
in `rewrite_related` is unchanged — it needs a genuine offline id collision on
a task another clone already references — so reproducing it now means writing
that case by hand.

- Where: `src/commands/sync.rs`, `docs/commands.md` §6, `tests/cli.rs`
- Done when: references on both sides of the merge are rewritten, and the scope
  sentence added to `docs/commands.md` §6 comes back out.

---

## Robustness

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
