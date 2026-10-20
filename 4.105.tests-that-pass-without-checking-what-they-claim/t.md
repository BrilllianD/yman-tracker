# tests that pass without checking what they claim

- tests/cli.rs:4574: "a second write must not drift" repeats the current title, so `set` writes nothing.
- tests/cli.rs:7782: the re-init switch test never asserts the intermediate `manual`/`push`.
- tests/cli.rs:8864: `reference_transaction_hook_never_sees_yman_refs` has no positive control.
- tests/cli.rs:3561: the localized-git "nothing to commit" check only holds when both attaches fall in the same second.
- tests/cli.rs:1352: `a_failed_renumber_restores_yman` passes vacuously as root.
- The stale-driver scenario ends at exit status.
- `log_follows_renames` never checks order.
- `hook_environment_does_not_leak` omits `GIT_INDEX_FILE` and `GIT_OBJECT_DIRECTORY`.
- The preflight message is asserted only by prefix.
- `Fx::runs` reads any error as zero runs.

- Where: tests/cli.rs:4574, tests/cli.rs:7782, tests/cli.rs:8864, tests/cli.rs:3561, tests/cli.rs:1352, tests/cli.rs:1742, tests/cli.rs:903, tests/cli.rs:2960, tests/cli.rs:1239, tests/common/mod.rs:410
- Done when: each listed test fails when the behaviour it names is broken, for example a real second write; `Fx::runs` panics on errors other than NotFound.
