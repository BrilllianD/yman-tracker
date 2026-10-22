## 2026-10-22T00:50:50Z — Bronnikov Aleksandr

 # Handoff: continue the audit fix run in a new session (S10 … S12)

 ## Context

Epic #86 (audit of 4ba6d5e). The user paused after the S10 Task 1 dispatch. That implementer
stopped before editing anything (plan mode came on): the tree is clean and nothing is committed.
It had already run `yman start 106`, so #106 shows `doing`. Outline, per-slice loop, global
constraints and lessons live in `~/.claude/plans/any-tasks-remains-distributed-bear.md`. The
slice index with every plan, report and review is
`~/mem/projects/yman/notes/2026-10-21-audit-fix-run.md`.

State at pause (2026-10-22):
- Local `main` = bf779b2 holds S1–S9, but is NOT pushed to origin (28+ commits ahead). Ask
  before pushing.
- Merges: S6 b8d15cc (#90, #94, #106 discover), S7 d264970 (#92, #93), S8 cbdff58 (#101,
   #102), S9 bf779b2 (#110, #111, #112).
- Current branch: `fix/audit-s10-scenarios` off bf779b2, no commits.
- S10 plan: `~/mem/projects/yman/notes/2026-10-22-audit-s10-plan.md`. ΣW 6, no split. Task 1
  is #106 rest (tests plus "on Unix" docs wording). Task 2 is #109 plus the `add` discard gap
  carried from S9a. Owner decisions are recorded at the end of the plan, all designer
  defaults.
- Open: epic #86, #98, #99, #106 (doing), #107, #108, #109.

 ## Steps 1–3: housekeeping right after approval (this session)

1. Copy this handoff into the outline's "Handoff" section, replacing the old 2026-10-21 one.
   Add an S10-in-progress line to its Progress section.
2. Rewrite `yman state` with `yman state set --body-file -` (same content in short form), then
   `yman sync`.
3. Commit `~/mem` notes if any changed.

 ## Steps 4–9: new session

4. `cd /pro/tools/yman-tracker && git switch fix/audit-s10-scenarios && yman sync`. #106 is
   already `doing`; no need to start it again.
5. **S10 Task 1:** one implementer.
   - Brief: S10 plan `### Task 1`.
   - Report: `2026-10-22-audit-s10-task1-report.md`.
   - Most scenarios are characterization tests: say per test whether it was RED.
   - Stop rule: if the v1→v2 opt-in race loses data, mark that test `#[ignore = "..."]` and
     report it rather than pin the loss.
   - Use the `Fx::shared`/`shared_titles` helpers.
   - Close with `yman done 106`.
6. **S10 Task 2:** one implementer. Brief: S10 plan `### Task 2`; close with `yman done 109`.
   The owner decided:
   - `comment -m x -e` exits 2.
   - `log <id>` for a removed task prints its history and exits 0.
   - Broken folders in `ls --json` carry `id` (key order `id, dir, error`).
   - `ls --assignee ""` / `" - "` lists the unassigned tasks.
   - A self-relation in `add` (including `--sections`) is refused before anything is written.
   - `add` calls `discard` on a write or relocate failure.
7. **S10 review loop:**
   - One reviewer over `bf779b2..HEAD`; the main session writes the review note.
   - One fix wave, one scoped re-review.
   - Run the `verify` gate, then `git merge --no-ff` into `main`.
   - Update the slice index, the outline Progress section and `yman state`.
8. **S11, docs (#98, #107, #108):**
   - Designer, implementers, reviewer, fix wave, re-review, merge.
   - Extra items carried into S11:
     - CLAUDE.md:70 serde_yaml "byte-compatible" claim needs "except the differences listed
       in `src/yml.rs`".
     - CLAUDE.md test counts (now 186 unit + 308 cli, more after S10).
     - Keep the three agent-doc copies in agreement within the 74-line guide cap.
9. **S12, CLI/JSON contract (#99):**
   - First ask the owner item by item: JSON shapes, `-m` semantics, short flags, `--force`.
   - The designer plans only the breaks the owner accepts.
   - Then `yman done 86`.
   - Finally, ask before pushing `main`.

 ## Lessons for briefs (beyond the outline's)

- Reviewers are read-only. The main session writes `…-review.md` with YAML frontmatter.
- For cosmetic nits, fix them directly, re-run the gate and record that in the review note.
- Check doc claims against the code ("every path", "unconditionally"). Fix waves tend to
  reword docs without making them as-built.
- Run fix waves in the same branch. Amend only a local, unmerged commit.
- After the S8 rule, each pinned string occurs once in `src/`. Doc comments must not quote a
  pinned string verbatim.

 ## Verification

- Per task: the "Done when" scenario plus the `verify` skill (fmt, clippy `-D warnings`,
  `cargo test`, the symref spike, `scripts/book.sh`).
- End of the run:
  - `yman ls` shows no `audit` task.
  - `yman plan 86 -a` shows every step done or cancelled.
  - CHANGELOG `[Unreleased]` lists every user-visible fix.

