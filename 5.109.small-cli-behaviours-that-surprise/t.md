# small CLI behaviours that surprise

- `ls -n 1` can print more than one row because broken entries are appended after the cut, and they carry no `id` in JSON.
- `log <id>` exits 4 for a removed task whose history git still has.
- `comment -m x -e` silently drops `-e`.
- `ls --assignee` does not normalise like `add`/`set` (`""` and padded values).
- `add --waits-on <next id>` can make a task wait on itself.
- `yman.author` beats `$YMAN_AUTHOR`, while `$YMAN_ACTOR` beats `user.name`.

- Where: src/commands/ls.rs:71, src/commands/log.rs:12, src/commands/comment.rs:69, src/commands/ls.rs:44, src/commands/add.rs:36, src/ids.rs:34
- Done when: each has a scenario pinning the chosen behaviour (for example `comment 1 -m x -e` exits 2).
