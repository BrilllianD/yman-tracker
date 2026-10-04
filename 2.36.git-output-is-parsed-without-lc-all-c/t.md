# git output is parsed without `LC_ALL=C`

`Git::command` sets no locale, but control flow matches English git messages: `couldn't find remote ref` in `init` and `sync`, `nothing to commit` and `Please tell me who you are` in `Git::commit`, `already checked out` and `is already used by worktree` in `init`. Under a localized git, `init` and `sync` against an empty origin fail with `fetch failed (see above); use --offline to skip`, re-applying an identical change becomes an error, and the identity hint is lost. The test fixture does not pin a locale either, so CI never sees it.

- Where: `src/git.rs` (`command`, `commit`), `src/commands/sync.rs` (`fetch_tasks`, `push`), `src/commands/init.rs` (`fetch_tasks`, `add_worktree`), `tests/common/mod.rs`.
- Done when: every git child runs with `LC_ALL=C` (or the parsing moves to exit codes and `--porcelain`), the fixture pins the locale, and a test runs the fixture under `LANG=de_DE.UTF-8` where that locale is available.
