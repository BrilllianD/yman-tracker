# `m.yml` reader and writer round-trip gaps

Four cases where `parse(render(m)) != m` or a valid file is refused. `plain_is_safe` only checks ASCII space and tab at the edges while the reader trims all Unicode whitespace, so `assignee: Ivan` followed by U+00A0 reads back trimmed. A quoted scalar followed by a comment, `status: 'todo'  # x`, fails with `unterminated single-quoted string` because quotes are stripped before comments, though `docs/storage.md` §6 promises both. The escapes `\a \b \v \f \N \_ \L \P` are refused although libyaml (behind the serde_yaml this replaced) emits some of them, which also contradicts the byte-compatibility claim at the top of the file. Unknown `|+` blocks lose their trailing blank lines on rewrite despite the verbatim claim.

- Where: `src/yml.rs` (`plain_is_safe`, the comment stripping in the scalar reader, the escape table, the `|+` block reader).
- Done when: each case round-trips in a unit test and the byte-compatibility comment is accurate.
