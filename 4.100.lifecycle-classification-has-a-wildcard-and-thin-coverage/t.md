# lifecycle classification has a wildcard and thin coverage

`is_mutating` decides two things: the exit-3 refusal during an unresolved merge, and whether autosync runs. It ends in `_ => matches!(...)`, so a new `Cmd` variant compiles as non-mutating without anyone deciding that. `dispatch` also repeats the doc commands in arms that never run. Only `add` and `state set` are tested for the refusal, out of 16 mutating commands. `exit_code_for` has no unit test for an error wrapped in `.context()`, although that is a CLAUDE.md contract.

- Where: src/cli.rs:572, src/commands/mod.rs:153, src/errors.rs:57, tests/cli.rs:1236
- Done when: `is_mutating` is an exhaustive match; a table-driven scenario asserts exit 3 with the full preflight message for every command in commands.md §1; a unit test asserts codes 3 and 4 survive `.context()`.
