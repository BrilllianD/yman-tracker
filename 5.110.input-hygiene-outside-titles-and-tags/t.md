# input hygiene outside titles and tags

- `check_portable` and the assignee/link paths refuse only `is_control`, so attachment names, assignees and links keep bidi overrides and U+2028/2029. `show` prints them raw.
- `$YMAN_ACTOR` with a newline can forge a second `d.md` header.
- `is_unsafe_char` lets direction marks and zero-width characters through.
- A BOM breaks `d.md` and `--sections` (but not `t.md`/`m.yml`).
- `render_md` trims LF but not CRLF blank lines.
- The H1 rule differs between `t.md` (any indent, any whitespace) and `--sections`.
- `slugify` splits at combining marks.

- Where: src/commands/attach.rs:292, src/commands/mod.rs:64, src/commands/mod.rs:116, src/discussion.rs:35, src/task.rs:130, src/discussion.rs:103, src/sections.rs:24, src/task.rs:228, src/sections.rs:33, src/task.rs:161
- Done when: unit tests pin each choice: `attach --name $'a‮b.txt'` refused, an author with a newline yields one comment, `discussion::parse("\u{feff}## …")` yields one comment, `render_md("T", "\r\nbody\r\n")` has no stray `\r` line.
