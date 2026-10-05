# literal-block lines of only Unicode whitespace do not round-trip in m.yml

`read_block` treats a line as blank with `line.trim().is_empty()`, while `literal_block` in the writer guards only `' '` and `'\t'`. A line holding only U+00A0 or U+3000 is written as a literal block but reads back empty or chomped. `set 3 --link $'a\n '` is written as `|-\n  a\n   ` and reads back as `"a"`. This is the same gap 9720798 closed for single-line values.

- Where: `src/yml.rs` (`read_block`, `literal_block`), unit tests in the same file.
- Done when: the reader and writer agree on what counts as blank, and a round-trip test with a U+00A0-only line passes.
