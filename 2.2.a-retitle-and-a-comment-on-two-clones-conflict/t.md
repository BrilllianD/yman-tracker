# A retitle and a comment on two clones conflict

`**/d.md merge=union` settles concurrent comments, but only while the folder
keeps its name. Retitle on one clone, comment on another, sync: the comment was
written against the old path, so it arrives as modify/delete and the merge stops
with exit 3 on a file the union driver is never consulted about. Recorded as a
limit in `docs/storage.md` §6; whether it is worth more than that is the open
question.

- Where: `src/commands/sync.rs`, `docs/storage.md` §6
- Done when: either the merge resolves it (the discussion is append-only, so the
  content is mergeable once the paths are matched up), or the limit is accepted
  and this entry goes.
