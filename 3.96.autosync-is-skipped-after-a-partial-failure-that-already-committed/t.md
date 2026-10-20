# autosync is skipped after a partial failure that already committed

`dispatch(...)?` returns before autosync runs. `done 32 99` (exit 4) and an `add --sections` that stops mid-file have already committed the earlier tasks (errors.md:53). Under `yman.autosync = push` those commits stay local, and nothing says `run: yman sync`.

- Where: src/main.rs:102, src/main.rs:104
- Done when: with autosync `push`, `yman done <ok> 99` exits 4 and origin's `refs/tasks/main` holds the close of `<ok>`, or stderr carries `run: yman sync`.
