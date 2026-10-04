# concurrent `rm` and `comment` can leave a folder holding only `d.md`

Running `rm` on one clone and `comment` on the same task on the other clone, both within the same second, then syncing, merged cleanly and left a task folder that holds only `d.md` (no `t.md`, no `m.yml`). The tree merge sees a delete on one side and an add of a new file on the other, which git resolves without conflict, so the result is a folder no command can load. Seen once during session 1 of epic 31; it is timing-dependent and was not reproduced on demand.

- Where: `src/commands/sync.rs` merge handling, `src/commands/rm.rs`, `src/commands/comment.rs`; the folder loader that would have to reject or repair a folder without `t.md`.
- Done when: a deterministic integration scenario (both commits in the same second, e.g. with pinned `GIT_*_DATE`) reproduces the case, and sync either reports it as a conflict or drops the orphaned `d.md` with a note on stderr; `docs/storage.md` says which.
