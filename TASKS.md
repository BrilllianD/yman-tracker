# TASKS

Backlog for `yman`. Derived from the as-built specification in `docs/`, the
"Limits and future work" section of `README.md`, and the current state of the
tree at `534397c`.

Baseline as of 2026-09-17: `cargo test` is green (33 unit, 52 integration),
`docs/` and `README.md` describe the shipped behaviour, and every subcommand
listed in `src/cli.rs` is implemented.

Conventions: each entry names the files it touches and the condition that
closes it. Anything that changes a user-facing string must change
`tests/cli.rs` and `docs/errors.md` in the same commit.

---

## Correctness and data integrity

### Rewrite `related` references when sync renumbers an id

A collision renumber moves a task's id but leaves every `related:` entry that
pointed at the old id dangling. Today the user gets a note
(`src/commands/sync.rs:351`) telling them to fix it by hand.

- Where: `src/commands/sync.rs` (`renumber_collisions`), `src/task.rs`
- Done when: after a renumber batch, no task on `refs/yman/local` refers to a
  renumbered id by its old value; the manual-update note is gone from the code,
  `docs/commands.md` §6 and `README.md`; an integration scenario in
  `tests/cli.rs` creates a cross-referenced pair, forces a collision on the
  second clone and asserts the reference followed the move.
- Note: the renamed ids and the rewritten references must land in the *same*
  commit as the `git mv` batch, or a mid-sync failure leaves the tree
  inconsistent.

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

---

## Robustness

### Keep a dangling attachment from breaking `show`

`detach` already tolerates an entry in `m.yml` whose file has disappeared
(`docs/commands.md` §4). Confirm `show` and `ls --json` behave the same way
rather than erroring, and add a regression test either way.

- Where: `src/commands/show.rs`, `src/commands/ls.rs`, `tests/cli.rs`
- Done when: a task whose attachment file was deleted outside yman renders with
  the entry marked as missing, and the behaviour is pinned in `docs/commands.md`.

### Windows support

`README.md` claims Windows "should work with `core.longpaths`" but nothing
tests it. Either verify it or stop claiming it.

- Where: path handling in `src/task.rs` (slugs, folder names) and `src/repo.rs`
  (`read_gitdir_file`); `tests/common/mod.rs` for the fixture.
- Done when: either the integration suite runs on Windows in CI, or the README
  sentence is downgraded to "untested".

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
- Done when: `yman ls` in a repository that needs no refresh spawns at most one
  `git` process, and `scripts/spike-symref.sh` still passes.

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
