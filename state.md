# yman-tracker state (2026-10-22)

## In progress
Audit backlog, epic #86. Outline with per-slice loop, global constraints and lessons:
`~/.claude/plans/any-tasks-remains-distributed-bear.md` (Handoff section is current). Slice
index with every plan, report and review: `~/mem/projects/yman/notes/2026-10-21-audit-fix-run.md`.

- S1-S10 pushed to origin (155424a). S11 merged into local `main` (698f988), NOT pushed; ask before pushing.
- S12 CLI/JSON contract (#99) next: ask the owner item by item before any design.

## Next
1. S12 CLI/JSON contract (#99): ask the owner item by item first. Then `yman done 86`.

## Decisions already taken
- yman honours the user's `commit.gpgSign`.
- Clone lock: `File::lock`, MSRV 1.89, no timeout, `refresh` unlocked.
- S6: `refresh --quiet` is silent with no tracker. Git's reason is indented under the error.
  `init --hooks` with a foreign hook warns and exits 0.
- S7: unknown `config.toml` keys warn; invalid `yman.refresh`/`yman.autosync` fall back with a
  warning; read-only commands run on the HEAD config during a `config.toml` conflict; a clean
  merge with an unloadable config exits 3; `sync` does not check the merge driver.
- S10: `comment -m x -e` exits 2; `log <id>` of a removed task prints history, exit 0; broken
  folders in `ls --json` carry `id` (keys `id, dir, error`); `ls --assignee ""`/`" - "` lists
  unassigned; self-relation in `add` refused before any write; `add` discards on failure.
- S11: clap colour off; printed task paths `.yman`-relative except `yman path`; no `cancel`
  status means `done -m "cancelled: <why>"`; split-close recipe: delete one folder, settle the
  kept m.yml, `sync --continue` (docs fixed, code unchanged).

## Blockers
None. Open: epic #86 and #99.
