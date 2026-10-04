# aborted `add -e` leaves a valid untracked task behind

`create` writes `t.md` and `m.yml`, then `open_editor(..)?` and `task::load(..)?` return early on failure. The untracked folder stays: `ls` shows it, the lazy refresh is skipped as dirty, and the next `sync` snapshots and publishes it. `docs/commands.md` §4 says a non-zero editor exit aborts.

- Where: `src/commands/add.rs` (`create`, the `spec.edit` branch).
- Done when: the folder is removed when the editor fails or the result does not parse, and a scenario with `EDITOR=false` leaves `.yman` clean.
