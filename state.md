# yman-tracker state (2026-10-21)

## In progress
Audit backlog, epic #86. Outline with per-slice loop, global constraints and lessons:
`~/.claude/plans/any-tasks-remains-distributed-bear.md`. Slice index with every plan, report
and review: `~/mem/projects/yman/notes/2026-10-21-audit-fix-run.md`.

- S1-S7 merged into local `main` (d264970). `main` is NOT pushed to origin; ask before pushing.
- S8 (layout and duplication refactor: #101 part 2, #102) next.

## Next
1. `yman sync`; branch `fix/audit-s8-layout` off `main`; designer writes
   `~/mem/projects/yman/notes/2026-10-21-audit-s8-plan.md`.
2. Implementers one at a time, reviewer, fix wave, re-review, merge `--no-ff`.
3. Then S9 to S12 in outline order. S12 (#99) starts by asking the owner item by item.

## Decisions already taken
- yman honours the user's `commit.gpgSign`.
- Clone lock: `File::lock`, MSRV 1.89, no timeout, `refresh` unlocked.
- S6: `refresh --quiet` is silent with no tracker. Git's reason is indented under the error.
  `init --hooks` with a foreign hook warns and exits 0.
- S7: unknown `config.toml` keys warn; invalid `yman.refresh`/`yman.autosync` fall back with a
  warning; all read-only commands run on the HEAD config during a `config.toml` conflict; a
  clean merge with an unloadable config exits 3; `sync` does not check the merge driver.

## Blockers
None. Open: epic #86 and #98 #99 #101 #102 #106 #107 #108 #109 #110 #111 #112.
