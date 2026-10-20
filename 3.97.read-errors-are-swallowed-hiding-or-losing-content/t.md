# read errors are swallowed, hiding or losing content

`comment -e` reads the temp file with `unwrap_or_default`. If an editor saved Latin-1, the result is `empty comment`, and the temp file holding the user's text is deleted. `Task::comment_count` returns 0 on any read error, so `ls` shows 0 comments. `show` treats an unreadable `d.md` as no discussion, so `--json` says `discussion_total: 0`. Only NotFound should mean empty.

- Where: src/commands/comment.rs:76, src/task.rs:507, src/commands/show.rs:16
- Done when: an `$EDITOR` writing `caf\xe9` makes `comment 1 -e` fail with the read error, and a non-UTF-8 `d.md` makes `ls` and `show` warn on stderr.
