## 2026-10-04T06:37:03Z — Bronnikov Aleksandr

Since step 52 the scrub list is copied in two more places: `SCRUBBED_ENV` in `tests/common/mod.rs` and the `unset` line in `scripts/spike-symref.sh`. Widening `LEAKY_ENV` here (GIT_CONFIG_PARAMETERS, GIT_CONFIG_COUNT/KEY_*/VALUE_*, GIT_NAMESPACE, ...) should widen both copies in the same commit.

## 2026-10-04T07:10:04Z — Bronnikov Aleksandr

From step 42: `-c core.hooksPath=/dev/null` on the .yman runner does not stop hooks defined in configuration (`hook.<name>.command` / `hook.<name>.event`, git 2.54+); a configured post-commit still fires in .yman. Blocking them needs a per-hook `-c hook.<name>.enabled=false`, which means enumerating `hook.*` first. The gap is documented in docs/commands.md §2. Also, main-runner `update-ref` and `fetch` fire the user's `reference-transaction` hook (cwd: main repo, not .yman).

## 2026-10-05T14:31:57Z — Bronnikov Aleksandr

dbc8bcb: full local-env-vars + GIT_NAMESPACE stripped, yman git too; 128+sig

