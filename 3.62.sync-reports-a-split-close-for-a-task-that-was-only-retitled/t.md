# sync reports a split close for a task that was only retitled

While task 53 was trying out a setup for the "not moved" warning, `sync` printed `note: task 1 was closed to two different statuses; keep one of …` after an attachment had been left behind under a retitled folder. The task had been retitled, not closed on two clones, so the note is wrong and points the user at a fix that does not apply. `report_split_closes` seems to treat any two folders that carry one id as a split close, without checking that both sit under terminal statuses.

- Where: `src/commands/sync.rs` (`report_split_closes`), `tests/cli.rs`.
- Done when: a scenario with a retitled folder plus a leftover attachment prints no split-close note, and the real split-close scenario still prints it.
