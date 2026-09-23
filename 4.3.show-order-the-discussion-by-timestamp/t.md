# `show`: order the discussion by timestamp

`merge=union` places the other side's lines after ours in a conflicting hunk,
so two comments written concurrently on two clones can display out of time
order. Low priority.

- Where: `src/commands/show.rs`
- Done when: parsed `Comment` entries are shown sorted by their timestamp
  while `Raw` chunks keep their file position, and `docs/commands.md`
  mentions it.
