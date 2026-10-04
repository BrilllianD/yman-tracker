# `sync --continue` treats marker-less unmerged files as resolved

`resume` lists unmerged files and keeps only those that still carry conflict markers, so an unmerged file without markers counts as resolved and is committed as ours. That is what git leaves behind when the `merge.ymanmeta.driver` path is stale (an absolute `current_exe` path from a Cellar, a Nix store or `target/debug` that is gone after an upgrade), when the driver errors, for binary attachment conflicts and for modify/delete conflicts. Theirs is dropped silently. `merge_remote` does not print the captured git stderr on the conflict path, so `sh: ...: not found` is swallowed too. `docs/storage.md` says an unregistered driver cannot corrupt a file; a stale one can.

- Where: `src/commands/sync.rs` (`resume`, `file_has_markers`, `merge_remote`), `docs/storage.md` merge driver section.
- Done when: `--continue` refuses while `diff --diff-filter=U` is non-empty unless the file was staged by the user, git stderr from the merge is surfaced, the stale-driver case is documented, and a scenario covers a binary or modify/delete conflict.
