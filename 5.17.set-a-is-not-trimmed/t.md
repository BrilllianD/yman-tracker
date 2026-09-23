# `set -a` is not trimmed

`add -a` trims the value and treats blank as unassigned; `set -a` stores it as given, so `set -a ""` writes an empty assignee.

- Where: `src/commands/set.rs`.
- Done when: `set -a` trims like `add -a`, blank means unassigned (or is refused), and `docs/commands.md` says which.
