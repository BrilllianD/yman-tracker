# `refresh` on a dirty `.yman` prints two notes

The lazy refresh prints `note: .yman has uncommitted changes, refresh skipped` and returns a skip reason, which `yman refresh` then prints again as `note: worktree has uncommitted changes`.

- Where: `src/refresh.rs` (the dirty check), `src/commands/refresh.rs`.
- Done when: one note per event, and `docs/commands.md` §5 matches.
