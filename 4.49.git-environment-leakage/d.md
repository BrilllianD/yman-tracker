## 2026-10-04T06:37:03Z — Bronnikov Aleksandr

Since step 52 the scrub list is copied in two more places: `SCRUBBED_ENV` in `tests/common/mod.rs` and the `unset` line in `scripts/spike-symref.sh`. Widening `LEAKY_ENV` here (GIT_CONFIG_PARAMETERS, GIT_CONFIG_COUNT/KEY_*/VALUE_*, GIT_NAMESPACE, ...) should widen both copies in the same commit.

