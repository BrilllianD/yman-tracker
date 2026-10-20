# transport and storage layout leak into command modules

`push`, `PushResult` and `PUSH_ATTEMPTS` live in `commands/sync.rs`, but core `autosync` and `init` import them. `init::fetch_tasks` duplicates `sync::fetch` and has already drifted from it: only sync clears a stale `refs/yman/remote`, so `init` with no LOCAL can adopt a tracker that origin has deleted. Storage layout is also rebuilt by hand in commands:
- the `{p}.{id}.{slug}` name, in add.rs;
- `<status>/<name>` rel paths, three times in sync.rs, while `task::rel_of` is private;
- `f/<name>` attachment paths;
- folder relocation, separately in set, edit and add.

`Task.dir` is stored state that every mutation must remember to recompute. The renumber commit subject is built in sync.rs and parsed back in log.rs through separate literals.

- Where: src/autosync.rs:11, src/commands/init.rs:8, src/commands/init.rs:266, src/commands/sync.rs:471, src/commands/add.rs:191, src/commands/sync.rs:562, src/commands/set.rs:179, src/commands/edit.rs:96, src/task.rs:243, src/commands/sync.rs:803, src/commands/log.rs:24
- Done when: `grep -n 'commands::' src/*.rs` matches only main.rs; `grep -rn '"--no-write-fetch-head"' src` has one hit; no command module formats a folder name or assigns `t.dir`; the renumber subject is built and parsed by one pair of functions, with a round-trip unit test.
