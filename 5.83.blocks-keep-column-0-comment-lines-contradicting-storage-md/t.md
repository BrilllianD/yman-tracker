# |+ blocks keep column-0 comment lines, contradicting storage.md

`push_unknown` keeps column-0 comment lines that come before a trailing blank inside a `|+` block, so `notes: |+` / `  k` / (blank) / `# c` / (blank) round-trips with the comment preserved. It is stable, but `docs/storage.md` says comments are still dropped.

- Where: `src/yml.rs` (`push_unknown`), `docs/storage.md`.
- Done when: either the reader drops the comment line or the doc states the exception.
