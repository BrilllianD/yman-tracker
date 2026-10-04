# git environment leakage

`yman git` passes the environment through unchanged, so inside a hook or alias it acts on `$GIT_DIR`, not `.yman`. `LEAKY_ENV` in `git.rs` is narrower than `git rev-parse --local-env-vars`: it misses `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_SHALLOW_FILE`, `GIT_PREFIX` and `GIT_CONFIG_*`, and `GIT_NAMESPACE` leaks and rewrites ref names.

- Where: `src/git.rs` (`LEAKY_ENV`), `src/commands/git.rs`, `docs/commands.md` `git`.
- Done when: the stripped set matches git's own list, the passthrough either strips the same set or documents that it does not, and a scenario runs a command with those variables set.
