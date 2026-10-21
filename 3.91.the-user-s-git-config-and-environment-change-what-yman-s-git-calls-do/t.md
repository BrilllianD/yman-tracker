# the user's git config and environment change what yman's git calls do

The `Git` wrapper pins only `core.quotePath` and `color.ui`. User settings that override or reshape output reach every call:
- `color.diff=always`, `log.decorate=full` and `log.showSignature=true` put ANSI escapes, ref names or gpg lines into `yman log` stdout.
- `status.showUntrackedFiles=no` hides hand-made task folders from `is_dirty`, `status`, `reattach_head` and the sync snapshot.
- `merge.ff=only` and `merge.verifySignatures=true` make every diverged `sync` fail with `merge failed`.
- `merge.conflictStyle=diff3` leaves `|||||||` sections that `is_marker` does not catch.
- `commit.gpgSign` is honoured by design (owner decision 2026-10-21): yman's commits are signed when the user's config says so. A failed signature is handled by #88.

Pathspecs are not pinned either. Literal task and attachment paths go to git as glob pathspecs (`git rm -- f/[ab].png` also matches `a.png`), while `log <id>` needs `:(glob)`, which `GIT_LITERAL_PATHSPECS=1` from the environment turns off.

- Where: src/git.rs:149, src/git.rs:255, src/git.rs:59, src/commands/log.rs:10, src/commands/log.rs:54, src/commands/sync.rs:500, src/commands/sync.rs:343, src/commands/attach.rs:343
- Done when: with a fixture global config setting `color.diff=always`, `log.decorate=full`, `status.showUntrackedFiles=no` and `merge.ff=only`, `yman log` stdout is byte-identical to a plain run, a hand-made folder counts as dirty, and a diverged `yman sync` exits 0; `GIT_LITERAL_PATHSPECS=1 yman log <id>` lists the task's commits.
