# `edit`: retitling a closed task moves it out of its status folder

`rename_for_title` builds the new path from `t.folder` alone, dropping the status directory a version 2 repository keeps closed tasks in. `git mv done/5.1.x 5.1.y` lifts the task to the top level, and the following `git add` of `done/5.1.y` fails after the move has happened.

- Where: `src/commands/edit.rs` (`rename_for_title`); `src/commands/set.rs` handles the parent correctly and is the model.
- Done when: retitling a closed task keeps it under its status directory, and `tests/cli.rs` covers it.
