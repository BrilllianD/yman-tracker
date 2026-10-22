# yman-tracker state (2026-10-22)

## In progress
Audit backlog, epic #86. Outline with per-slice loop, global constraints and lessons:
`~/.claude/plans/any-tasks-remains-distributed-bear.md` (Handoff section is current). Slice
index with every plan, report and review: `~/mem/projects/yman/notes/2026-10-21-audit-fix-run.md`.

- S1-S10 merged into `main` (155424a), pushed to origin 2026-10-22.
- S11 docs (#98, #107, #108) next: branch off `main`, designer writes the S11 plan.

## Next
1. S11 docs (#98, #107, #108), plus CLAUDE.md serde_yaml qualifier and test counts.
2. S12 CLI/JSON contract (#99): ask the owner item by item first. Then `yman done 86`.

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

## Blockers
None. Open: epic #86 and #98 #99 #107 #108.
