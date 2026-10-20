# CLI and JSON contract inconsistencies to settle before 1.0

Each of these is an API break to fix, so the owner has to decide; record the decision before 1.0.
- JSON: `attachments` is an int in `ls --json` but an array in `show --json`. `ls`'s `comments` skips raw `d.md` chunks while `show`'s `discussion_total` counts them. `tasks` is an int in `tags --json` but an object in `status --json`. `worktree.dirty` is a count named like a boolean.
- `-m` sets the body on `add` but appends a comment on `set` and the verbs, so `yman set 5 -m "new description"` adds a comment and exits 0.
- Short flags: `-a` is `--all` on ls/plan but `--assignee` on add/set. `set` lacks `-s`/`-p`/`-t`. `-n` maps to three long names. `--force` means "skip prompt" on rm (with `-f`) but "overwrite" on attach (without `-f`).

- Where: src/commands/ls.rs:203, src/commands/show.rs:175, src/commands/tags.rs:46, src/commands/status.rs:213, src/cli.rs:180, src/cli.rs:349, src/cli.rs:214, src/cli.rs:189, src/cli.rs:313
- Done when: each item has a recorded decision (change before 1.0, or document as intended in commands.md), and a test over `Cli::command()` pins the short-flag map.
