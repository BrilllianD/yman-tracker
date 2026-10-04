# the user's post-* hooks run inside `.yman`

`--no-verify` skips only pre-hooks. `post-merge`, `post-checkout` and `post-commit` fire for yman's own fast-forwards, the sync merge, `worktree add` and every commit, while `docs/commands.md` §2 says the user's hooks cannot interfere. A husky, lefthook or LFS hook that writes files makes `.yman` dirty, refresh is then skipped forever and the next snapshot commits the junk.

- Where: `src/git.rs` (the worktree runner), `src/refresh.rs`, `src/commands/sync.rs`, `src/commands/init.rs`, `docs/commands.md` §2.
- Done when: the worktree runner passes `-c core.hooksPath=<empty dir>` (or equivalent), and a scenario installs a noisy `post-commit` hook and shows `.yman` stays clean.
