# add and attach leave partial state behind on failure

`add -s done -e` under an archive config whose editor aborts removes the task folder but can leave an empty status directory behind. Git ignores empty directories, so `.yman` still reads clean, but the directory stays on disk. Separately, `attach` leaves the earlier copies behind if a `std::fs::copy` fails part-way (disk full), including files it already overwrote with `--force`. Validation cannot catch that; it needs a rollback.

- Where: `src/commands/add.rs`, `src/commands/attach.rs`.
- Done when: an aborted `add -e` removes every directory it created, and a failed `attach` leaves `f/` and `m.yml` exactly as they were.
