# sync --continue and "nothing to commit" misread ordinary content

`is_marker` treats any line that is exactly `=======` as a conflict marker. It scans every file in every task folder that the merge touched, so a setext heading or a pasted separator in `d.md` or `t.md` blocks `--continue` until the user edits their own content. `Git::commit` counts only `nothing to commit` as success. When other files in `.yman` are untracked or unstaged, git says `nothing added to commit` or `no changes added to commit` instead, so an identical `state set` or `attach --force` fails. A few failures after a stopped merge (`unmerged_paths`, the `YMAN_MARKED` write, `report_split_closes`) exit 1 instead of 3.

- Where: src/commands/sync.rs:343, src/commands/sync.rs:366, src/git.rs:281, src/commands/sync.rs:517, src/commands/sync.rs:533
- Done when: a merge touching a task whose `d.md` contains `Summary\n=======` completes `sync --continue` once the real conflict is resolved; with an untracked `.yman/scratch.txt`, a repeated `yman state set -m X` exits 0; a `YMAN_MARKED` write failure exits 3.
