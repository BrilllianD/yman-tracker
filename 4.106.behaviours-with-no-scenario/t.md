# behaviours with no scenario

No scenario reaches:
- the non-conflict `merge failed` path;
- a v1→v2 opt-in on one clone while the other edits a closed task;
- `editor was killed by a signal`;
- `git worktree add failed`;
- `state`'s `cannot read/write`;
- `state set` in the completions test;
- `discover` in a bare repo or under `safe.directory`. Today `discover` replaces git's reason with `not inside a git repository`, so it is also a defect.

- Where: src/commands/sync.rs:541, src/commands/edit.rs:50, src/commands/init.rs:314, src/repo.rs:138, tests/cli.rs:6003
- Done when: each has a scenario, and `yman ls` in a bare repo prints git's reason under the error.
