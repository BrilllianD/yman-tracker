# declared Rust minimum is 1.85 but the code needs 1.88

`Cargo.toml` says `rust-version = "1.85"`, yet the code uses let-chains (`if … && let Some(…)`) in `src/task.rs`, `src/commands/merge_driver.rs`, `src/commands/ls.rs` and `src/commands/sync.rs`, which are stable only since Rust 1.88. A 1.85–1.87 toolchain passes the `rust-version` gate and then fails with E0658. The same number is repeated in `docs/setup.md`, `README.md`, the `CHANGELOG.md` entry and the comment in `.github/workflows/ci.yml`. Introduced by 201904c.

- Where: `Cargo.toml`, `docs/setup.md`, `README.md`, `CHANGELOG.md`, `.github/workflows/ci.yml`.
- Done when: all five say 1.88 (or let-chains are rewritten and 1.85 is true), in one `ci:` commit.
