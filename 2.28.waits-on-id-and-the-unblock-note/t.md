# `--waits-on <id>` and the unblock note

Waiting is a three-part ritual (relate, status blocked, a body line) and closing a blocker tells nobody what got unblocked.

- Where: `src/cli.rs` (add, set), `src/commands/set.rs` (apply/report split), `src/commands/add.rs`, `src/plan.rs` (report_closed), docs.
- Done when: `add`/`set --waits-on` relate and block in one commit, closing the last open blocker prints a note naming the move command, tests and docs in place.
