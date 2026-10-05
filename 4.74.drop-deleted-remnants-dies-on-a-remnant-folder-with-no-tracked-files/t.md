# drop_deleted_remnants dies on a remnant folder with no tracked files

`yman rm` runs `git rm -rq`, which leaves ignored and untracked files (`*.swp`, `*~`, `*.orig`, a note the user dropped in) behind, so the folder survives with neither `t.md` nor `m.yml`. At the next three-way merge it qualifies as a remnant, `ls-files` returns nothing, and `git rm -r -q -- <rel>` fails with `pathspec … did not match any files`; sync exits 1 after the merge commit. The same can happen in `--continue`. Introduced by eb011db.

- Where: `src/commands/sync.rs` (`drop_deleted_remnants`), `tests/cli.rs`.
- Done when: a remnant with zero tracked files is tidied without calling `git rm`, and a test with a remnant holding only an ignored file syncs cleanly.
