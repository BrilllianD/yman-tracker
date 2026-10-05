# blank assignee from a hand edit is not normalised on read

626830f made `set -a ""` mean unassigned, but the `m.yml` reader still loads `assignee: ''` as `Some("")`. `ls --assignee -` then skips the task, `--json` prints `""` instead of `null`, and `show` prints an empty field. Separately, `set -a -` stores the literal `-`, which `show` prints like unassigned while `ls --assignee -` excludes it (pre-existing).

- Where: `src/yml.rs` or `src/task.rs` (meta loading), `src/commands/set.rs`, `docs/commands.md`, `tests/cli.rs`.
- Done when: a blank or whitespace-only assignee on disk reads as unassigned everywhere, and it is decided and documented whether `-` is a value or a sentinel.
