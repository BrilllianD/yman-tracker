# detach matches attachment names exactly while attach case-folds

`run_detach` finds the entry with `x.name == a.name`, but `attach` now compares names ignoring case and `docs/commands.md` says "Names compare ignoring case throughout". `attach 3 a.png` followed by `detach 3 A.png` fails with `no attachment "A.png" on task 3`. Introduced by 5450af1.

- Where: `src/commands/attach.rs` (`run_detach`), `tests/cli.rs`.
- Done when: `detach` finds the entry ignoring case and removes the file under its stored spelling, with a test for `attach a.png` / `detach A.png`.
