# multi-id and multi-file verbs that fail part-way

Three places stop early and leave the user without the follow-up they were promised. `each` in `set.rs` returns on the first failing id, so `done 32 99` commits 32 but never prints its epic warning or unblock notes, while `docs/commands.md` §4 says the report comes after the last id. `rm` drops `related` references but never calls `report_closed`, so a `blocked` task whose only blocker was removed stays blocked on nothing with no `no longer waits` note. `attach` copies each file before checking the next, so `attach 3 a/x.png b/x.png` copies the first and bails on the second, leaving an untracked `f/x.png` with no `m.yml` entry for the next sync to snapshot.

- Where: `src/commands/set.rs` (`each`), `src/commands/rm.rs`, `src/commands/attach.rs`, `src/plan.rs` (`report_closed`).
- Done when: the close report is printed for the ids that did complete, `rm` reports what the removal freed, `attach` validates every source before copying any, and scenarios cover each.
