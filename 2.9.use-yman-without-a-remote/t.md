# Use yman without a remote

init refuses when the repo has no "origin", even with --offline, although only sync needs one.

- Where: `src/commands/init.rs` (`resolve_remote`, `summary`), `src/commands/sync.rs`, `src/commands/status.rs`; `docs/setup.md`, `docs/errors.md`, `docs/commands.md`, the agent surface (agents.md, SKILL.md, README).
- Done when: `yman init` with no origin sets up a local-only tracker; sync says clearly that there is no origin; `status` reports it; a later `yman init --remote <url>` connects it and the first sync pushes.
