# Audit 2026-10: defects found in the post-0.6.0 review

A whole-codebase review after the 0.6.0 release, run on 2026-10-04 over the storage and sync core, the task model and per-task commands, and the CLI, agent-facing docs, tests and tooling. Every step below is one defect or one tight group of defects from the same file, with the lines that show it and what fixing it means. Severity became priority: 2 for data loss or a bricked tracker, 3 for a wrong result the user will notice, 4 for robustness and test gaps, 5 for drift and nits.

Open tasks 12 to 18 came from the earlier doc audit and are not repeated here; step 3 relates to task 13 because it shares its root cause.

- Where: everything under `src/`, `tests/`, `docs/`, `scripts/`, `skills/`.
- Done when: `yman plan` lists no open steps.
