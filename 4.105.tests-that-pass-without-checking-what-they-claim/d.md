## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ry7 mutation src/git.rs:274 (commit without LC_ALL=C): a_localized_git_does_not_change_behaviour ok when attaches straddle a second; same-second 5/5 FAILED, 0 ok; unmutated ok; Ry20 mutation src/yml.rs:72: test awkward_field_values_survive_a_rewrite ... ok; its second write set 1 --title 'Fix login' rc 0 writes nothing (m.yml same, .yman commits 4 -> 4); Ry22 mutation src/git.rs:105 (hooks kept on): reference_transaction_hook_never_sees_yman_refs ok with hook unable to fire; hook firing: FAILED (saw refs/yman/local); Ry21 mutation src/commands/init.rs:160,165 (re-init ignores explicit --refresh/--autosync): init_takes_explicit_refresh_and_autosync_values ... ok; mutated gives refresh=lazy, frozen refresh=manual; R105-1352 uid 0 in userns (not real root): a_failed_renumber_restores_yman ... ok with 'skipped: running as root, read-only files are writable' (vacuous early return at tests/cli.rs:1352-1355); not run: the stale-driver scenario, log_follows_renames order, hook_environment_does_not_leak, the preflight prefix assertion, Fx::runs

## 2026-10-21T13:36:06Z — Bronnikov Aleksandr

4fa8674: vacuous test scenarios now check what they claim

