# Windows support

`README.md` claims Windows "should work with `core.longpaths`" but nothing
tests it. Either verify it or stop claiming it.

- Where: path handling in `src/task.rs` (slugs, folder names) and `src/repo.rs`
  (`read_gitdir_file`); `tests/common/mod.rs` for the fixture.
- Done when: either the integration suite runs on Windows in CI, or the README
  sentence is downgraded to "untested".

Two more non-Linux filesystem cases belong to the same entry. On a
case-insensitive filesystem slugs cannot collide because `slugify` lowercases,
but attachment names (`attach` checks an exact `exists()`) and `author`
prefixes can. On macOS the filesystem stores NFD, so a Cyrillic slug minted on
Linux and `git mv`'d on macOS can diverge unless `core.precomposeunicode` is
set. Record both in `docs/storage.md` §5 even if nothing else changes.
