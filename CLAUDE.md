# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`yman` is a git-native task tracker. Its whole design rests on one trick: task
history lives in `refs/yman/local`, a ref outside `refs/heads/`, checked out as
a linked worktree at `.yman/`. Read `docs/storage.md` before changing anything
that touches refs, the worktree, or the on-disk layout.

## Commands

```sh
cargo test                                  # 88 unit + 98 integration
cargo clippy --all-targets -- -D warnings   # must be clean; CI-equivalent gate
cargo fmt                                   # run before committing
sh scripts/spike-symref.sh                  # re-proves the git invariant below
cargo test --test cli <name>                # one integration scenario
```

## Gotchas

- **Every git call goes through `Git` in `src/git.rs`.** It strips `GIT_DIR`,
  `GIT_WORK_TREE` and friends from the environment (so running inside a hook
  does not leak state) and forces `core.quotePath=false` and `color.ui=never`.
  The one deliberate exception is `yman git`, which is a raw passthrough so the
  user's own flags survive.
- **User-facing strings are part of the contract.** `tests/cli.rs` asserts many
  of them verbatim, and `docs/errors.md` lists the ones the spec pins down.
  Changing wording means changing the test and the doc in the same commit.
- **stdout is for data, stderr for everything else.** `cd $(yman path 14)` and
  `yman ls --json` must stay pipeable, so notes, warnings and progress go to
  stderr.
- **Exit code 3 means a sync merge is unresolved.** It travels as the
  `MergePending` marker error in `src/errors.rs` and is recognised in `main` by
  downcasting — returning a plain `anyhow` error loses it.
- **The worktree's private git dir is `worktrees/-yman`, not `worktrees/yman`** —
  git sanitizes the leading dot. Always resolve it by reading `.yman/.git`
  (`repo::read_gitdir_file`); constructing the path by hand breaks every command.
- **`Git::commit` treats "nothing to commit" as success.** Re-applying an
  identical change inside the same second stages nothing, and that is a no-op,
  not a failure.
- **Integration tests must never touch the network or real git config.** The
  fixture in `tests/common/mod.rs` builds a bare remote plus two clones in a
  tempdir and pins `HOME`, `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_NOSYSTEM`. Add
  scenarios there rather than reaching for a live repo.

## Dependencies

The dependency list is deliberately short and should stay that way. There is no
`regex` (folder names, `t.md` titles and `d.md` headers are parsed by hand), no
`serde_json` (every `--json` output is written by `src/json.rs`, which escapes
and assembles it by hand) and no YAML crate (`m.yml`
goes through `src/yml.rs`, whose writer is byte-compatible with the
`serde_yaml` output it replaced). Adding a crate needs a reason beyond
convenience.

## Git conventions

- Conventional Commits: `feat:`, `fix:`, `test:`, `docs:`, `style:`. Subject in
  the imperative; body explains *why*, not what the diff already shows.
- Work on a feature branch and merge it — do not commit straight to `main`.
- Commit each task from `TASKS.md` as soon as it is finished, before starting
  the next one: run the gate (`.claude/skills/verify`), commit the code, the
  tests, the docs and the `TASKS.md` edit together, and say so. One task per
  commit keeps the history reviewable and the working tree honest — do not let
  finished tasks pile up uncommitted.
- A finished task is deleted from `TASKS.md`, not annotated as done. The commit
  that implements it removes its entry in the same diff, so the file only ever
  lists open work. The history is the record of what was done; leaving completed
  entries behind duplicates it and makes the backlog read longer than it is.
- `.git/hooks/pre-commit` rejects a commit when `cargo fmt --check` fails.

## Specification

`docs/` is the normative, as-built specification: `docs/storage.md` for the ref
and on-disk contract, `docs/commands.md` for per-command semantics, and
`docs/errors.md` for exit codes and pinned messages. They describe what the code
does, so a disagreement between them is a bug in one or the other — say which
you think it is instead of silently picking a side.
