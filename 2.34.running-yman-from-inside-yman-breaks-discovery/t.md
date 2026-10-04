# running yman from inside `.yman` breaks discovery

`discover` takes `rev-parse --show-toplevel`, which inside the linked worktree is `.yman` itself, so the tracker directory becomes `.yman/.yman` and every command fails with `.yman is not initialized; run: yman init`. Following that advice runs `worktree add --detach .yman/.yman refs/yman/local`, which git allows, leaving two worktrees on the same symref; the outer index goes stale and the next snapshot reverts the other worktree's commits. `docs/setup.md` says any subdirectory will do, and `docs/commands.md` advertises `cd $(yman path <id>)`. Same root cause as task 13, different and more likely trigger.

- Where: `src/repo.rs` (`discover`, `discover_separately`), `src/commands/init.rs` (`add_worktree`), `docs/setup.md` §1.
- Done when: a command run from a task folder resolves the real repository root, `init` refuses to run from inside `.yman`, and a scenario runs `yman show` from `$(yman path <id>)`.
