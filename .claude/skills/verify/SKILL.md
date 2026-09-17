---
name: verify
description: Run the full quality gate for yman-tracker - rustfmt check, clippy with warnings denied, the unit and integration test suites, and the symref spike - then report exactly what failed. Use before committing, before opening a PR, or whenever asked to check that the repo is green.
---

Run these four checks in order. Do not stop at the first failure — run all four,
so the report covers everything at once.

```sh
cargo fmt --check          # the pre-commit hook rejects a dirty tree
cargo clippy --all-targets -- -D warnings
cargo test                 # unit tests, then the two-clone integration suite
sh scripts/spike-symref.sh # the git invariant the whole design rests on
```

## What each failure means

- **`cargo fmt --check`** — `cargo fmt` fixes it. Nothing else to think about.
- **clippy** — must be clean at `-D warnings`; that is the gate, not a
  suggestion. Prefer fixing the code over `#[allow]`; when an allow really is
  right, it needs a comment saying why.
- **`cargo test`** — note *which* suite failed. Unit tests (`src/**`) point at
  pure logic: slugs, folder names, config validation, id generation.
  `tests/cli.rs` drives two real clones against a bare remote, so a failure
  there is usually about git behaviour or an exact output string. Re-run one
  scenario with `cargo test --test cli <name>`.
- **`scripts/spike-symref.sh`** — this failing is serious: it means committing
  inside the `.yman` worktree no longer moves `refs/yman/local`, or the ref
  became visible to `git branch`. Say so loudly rather than working around it;
  `src/git.rs` documents why the primary code path depends on it.

## Reporting

State plainly which of the four passed and which failed, quote the shortest
decisive line from each failure rather than dumping output, and do not claim the
repo is green unless all four exited zero.

If a user-facing string changed, check whether `tests/cli.rs` asserts it
verbatim and whether `docs/errors.md` lists it — both need to move in the same
commit as the code.
