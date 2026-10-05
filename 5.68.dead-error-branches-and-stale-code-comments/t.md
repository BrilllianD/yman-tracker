# dead error branches and stale code comments

`cannot attach <path>: no file name` can never be reached: `file_name()` is `None` only for a root or a path ending in `..`, both directories, and the `not a regular file` check runs first. `statuses.terminal marks every status terminal` (`src/config.rs` ~360) cannot be reached either: in v2 the default status must be listed and non-terminal, and that check fails first. The test-only `tmpfile()` in `src/discussion.rs` (~185) writes a predictable path in `temp_dir()` instead of using the `tempfile` dev-dependency. The comment in `src/commands/completions.rs` (~19) about `generate` panicking on a closed pipe now holds only on non-unix, since on unix SIGPIPE kills the process first.

- Where: `src/commands/attach.rs`, `src/config.rs`, `src/discussion.rs`, `src/commands/completions.rs`, `docs/errors.md`.
- Done when: the two branches and their `docs/errors.md` rows are gone (or made reachable and tested), `tmpfile()` uses `tempfile`, and the completions comment matches the code.
