# titles accept newlines and control characters

`add` and `set --title` only trim. `yman add $'a\nb'` writes `# a` followed by `b`, which reads back as title `a` with body `b`; the commit subject becomes multi-line because `quote_title` only replaces `"`.

- Where: `src/commands/add.rs`, `src/commands/set.rs` (title handling), `src/commands/mod.rs` (`quote_title`), `docs/errors.md`.
- Done when: a title containing `\n`, `\r` or another control character is refused with a pinned message, and a scenario covers it.
