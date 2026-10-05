# synthetic-project.sh --force can still delete outside target/

The guard resolves the target with a logical `cd` and `pwd -P`, but `rm -rf "$out"` lets the kernel resolve the path. With `target/link` a symlink to `/x/sub`, and both `target/victim` and `/x/victim` existing, `--force target/link/../victim` passes the check (logical `cd` lands in `target/victim`) and `rm -rf` deletes `/x/victim`. Contrived, but the guard's whole purpose is that this cannot happen. Introduced by 1fe34f2.

- Where: `scripts/synthetic-project.sh` (the `--force` guard).
- Done when: the guard uses `cd -P` or `rm -rf "$abs"` so the path checked is the path removed.
