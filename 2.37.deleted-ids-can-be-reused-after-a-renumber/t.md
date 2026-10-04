# deleted ids can be reused after a renumber

`ever_assigned` runs `git log --diff-filter=A --name-only` without `--no-renames`. Rename detection is on by default for `log`, so the `git mv` a sync renumber makes shows as R, not A, and the renumbered id is never recorded as assigned. After `rm` of that id, `seq` hands it out again, which `docs/storage.md` §8 says never happens. The same function also skips paths git still quotes (names with `"`, `\`, tab or newline), since `core.quotePath=false` only covers non-ASCII.

- Where: `src/ids.rs` (`ever_assigned`), `tests/cli.rs` (the single `ever_assigned` scenario covers only the plain case).
- Done when: the log runs with `--no-renames`, quoted paths are unquoted or documented as unsupported, and a scenario renumbers, removes and re-adds without reusing the id.
