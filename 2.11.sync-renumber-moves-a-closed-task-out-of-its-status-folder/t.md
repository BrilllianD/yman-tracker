# sync renumber moves a closed task out of its status folder

Collision renumbering in `sync` builds `new_rel` from `t.folder` alone, so a colliding closed task in a version 2 repository lands at the top of `.yman` while `m.yml` still says it is closed.

- Where: `src/commands/sync.rs` (renumber loop, `new_rel = t.folder.to_string()`).
- Done when: a renumbered closed task stays under its status directory, with an integration scenario.
