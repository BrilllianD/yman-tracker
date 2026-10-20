# hooks and temp-file edge cases

- A foreign hook that already runs `yman refresh --quiet` stays "not installed" forever.
- `hooks remove` deletes a marked hook that the user added commands to.
- SIGINT during `add -e` or `comment -e` leaves the folder or the temp file, although commands.md:617 says "every path".
- `with_stdin` drops the child unwaited when the write fails.
- Non-UTF-8 repository paths go through lossy conversion with no stated limit.

- Where: src/hooks.rs:191, src/hooks.rs:158, src/commands/add.rs:156, src/commands/comment.rs:49, src/git.rs:215, src/git.rs:181
- Done when: each case has a test or a sentence in the docs stating the behaviour.
