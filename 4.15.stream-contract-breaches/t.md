# stream contract breaches

`docs/errors.md` promises stdout is data only and stderr lines carry `error: `, `warning: ` or `note: `. Exceptions today: the `rm` and `tags rm` confirmation prompts go to stdout; `refreshed: N new commit(s)`, the hook-install advice and the conflicting-file list go to stderr unprefixed.

- Where: `src/commands/rm.rs`, `src/commands/tags.rs`, `src/refresh.rs`, `src/hooks.rs`, `src/commands/sync.rs`.
- Done when: prompts go to stderr, each stderr line fits the documented shape (or `docs/errors.md` names the exceptions), and tests pin it.
