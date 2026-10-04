# preflight loads `config.toml` before the `MERGE_HEAD` check

`preflight` calls `Config::load` and only then looks for `MERGE_HEAD`, and `sync` is not preflight-exempt. When a sync merge conflicts in `config.toml`, every command fails with `invalid .yman/config.toml`, including `sync --abort` and `sync --continue`, so the only way out is `yman git merge --abort`. Mutating commands exit 1 instead of the documented 3.

- Where: `src/repo.rs` (`preflight`, the order of `Config::load` and `merge_in_progress`), `src/cli.rs` (`skips_preflight`), `docs/errors.md` "mutating command during a merge".
- Done when: the `MERGE_HEAD` check runs before the config is loaded, `sync --abort` and `sync --continue` work with a conflicted `config.toml`, and an integration scenario pins exit 3.
