# yman-tracker state (2026-10-21, session paused by the user)

## In progress
Audit backlog, epic #86. Outline with per-slice loop, global constraints and lessons:
`~/.claude/plans/any-tasks-remains-distributed-bear.md`. Slice index with every plan, report
and review: `~/mem/projects/yman/notes/2026-10-21-audit-fix-run.md`.

- S1-S5 merged into local `main` (f358af8). `main` is NOT pushed to origin; ask before pushing.
- S6 (setup & discovery) on branch `fix/audit-s6-setup`, plan
  `~/mem/projects/yman/notes/2026-10-21-audit-s6-plan.md` (3 tasks, no split):
  - Task 1 #106 discover item: done, 621ea94 (#106 stays open for S10, comment added).
  - Task 2 #94: done, 7b11fc7, `yman done 94`.
  - Task 3 #90: NOT started.

## Next
1. `git switch fix/audit-s6-setup && yman sync`.
2. Implementer for S6 Task 3 (#90). Brief = S6 plan `### Task 3`; report
   `2026-10-21-audit-s6-task3-report.md` (frontmatter `status: active`). Interfaces: Task 2 made
   `Context.wt_gitdir` an `Option` with a `wt_gitdir()` accessor that panics before `.yman` is
   known ours; `default_wt_gitdir` is gone.
3. One reviewer over `f358af8..HEAD`. Ask it to grep every `wt_gitdir()` caller (panic risk).
   Then the review note, one fix wave, one scoped re-review, and merge `--no-ff` into `main`.
4. Then S7 to S12 in outline order: designer per slice, implementers one at a time, reviewer,
   fix wave, re-review, merge. S12 (#99) starts by asking the owner item by item.

## Decisions already taken
- yman honours the user's `commit.gpgSign`.
- Clone lock: `File::lock`, MSRV 1.89, no timeout, `refresh` unlocked.
- S6: `refresh --quiet` is silent with no tracker. Git's reason is indented under the error.
  `init --hooks` with a foreign hook warns and exits 0.

## Blockers
None. 14 open steps + epic: #90 #92 #93 #98 #99 #101 #102 #106 #107 #108 #109 #110 #111 #112.
