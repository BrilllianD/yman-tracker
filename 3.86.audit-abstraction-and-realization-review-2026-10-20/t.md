# Audit: abstraction and realization review 2026-10-20

Two-pass review of yman-tracker @ 4ba6d5e (2026-10-20): abstraction (CLI contract, storage and git seams, domain model, command layer and agent surface) and realization (storage core, sync and merge, parsers, task commands, entry point and tests). Nine read-only parts. Every 🔴 and 🟡 was re-checked against the cited lines by the main agent.

Abstraction: 0 🔴, 21 🟡, about 30 🔵. Realization: 4 🔴, 22 🟡, about 35 🔵. Steps below are grouped by theme, with priority = severity (2 wrong value written or task unusable, 3 wrong visible result, 4 robustness and tests, 5 drift and nits).

Tagged owner: the step "CLI and JSON contract inconsistencies to settle before 1.0" needs a decision, because every fix there breaks consumers.

Notes: ~/mem/projects/yman/notes/yman-tracker-abstraction-review.md and ~/mem/projects/yman/notes/yman-tracker-realization-review.md.

- Where: see steps (yman plan <this id> -a)
- Done when: every related step is done or cancelled.
