# sync fails with "Not a valid object name" when rejoin and drop both fire

After `merge_remote`, `rejoin_split_folders` may commit `yman: rejoin files left under a moved folder`. HEAD is then that single-parent commit, not the merge, so `drop_deleted_remnants(ctx, &base, "HEAD^1", "HEAD^2")` runs `ls-tree HEAD^2` and fails. Sync exits 1 after the merge and rejoin commits, before the push; the next sync sees `base == remote` and never runs the drop again, so an unloadable remnant folder is pushed. Scenario: clone A retitles task X while clone B attaches a file to X, and in the same sync task Y was removed on one side and commented on the other. Introduced by eb011db.

- Where: `src/commands/sync.rs` (`run`, around the rejoin and drop calls), `tests/cli.rs`.
- Done when: the merge sha is taken with `rev-parse HEAD` right after `merge_remote` and `{sha}^1` / `{sha}^2` are passed (or both steps are staged and committed once), and a test runs rejoin and drop in one sync and pushes.
