# linked worktrees of the project break every command and spam hooks

`ROOT` comes from `--show-toplevel`, which is the current worktree, while storage.md defines it as the main toplevel. From a linked worktree every command says `.yman is not initialized; run: yman init`, and `init` then refuses with `already checked out in another worktree`. Hooks live in the shared `COMMON/hooks`, so every checkout or merge there prints that error, even though `refresh --quiet` promises silence. The same happens under a global `core.hooksPath`. `yman_parent` and `ydir_state` are two predicates for "our `.yman`", and `default_wt_gitdir` still builds `worktrees/-yman` by hand. That path is unreachable today, but it breaks the "read, never constructed" rule.

- Where: src/repo.rs:99, src/repo.rs:104, src/repo.rs:128, src/repo.rs:234, src/repo.rs:290, src/hooks.rs:14, src/commands/init.rs:292, docs/storage.md:10, docs/setup.md:17
- Done when: `yman ls` and `yman refresh --quiet` in a linked worktree of the fixture either use the main `.yman` or fail without saying `run: yman init`; `git checkout -b x` there has empty stderr with hooks installed; `grep -n 'join("-yman")' src` is empty.
