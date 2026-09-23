# Renumbering leaves `related` references dangling on other clones

`rewrite_related` (`src/commands/sync.rs:422-456`) walks the renumbering
clone's own tree — the pre-merge one. A reference minted on another clone, or
arriving in the same merge, keeps the old id and ends up pointing at nothing.
The synthetic run used to leave 140 of 142 references dangling, but that was
downstream of a retitle renumbering the task; with that fixed the same run
leaves 0 of 142 dangling and no longer reproduces this at all. The limitation
in `rewrite_related` is unchanged — it needs a genuine offline id collision on
a task another clone already references — so reproducing it now means writing
that case by hand.

- Where: `src/commands/sync.rs`, `docs/commands.md` §6, `tests/cli.rs`
- Done when: references on both sides of the merge are rewritten, and the scope
  sentence added to `docs/commands.md` §6 comes back out.
