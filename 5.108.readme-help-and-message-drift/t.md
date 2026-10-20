# README, help and message drift

- The README transcripts predate 0.9.0: no `cancelled 0`, no `note: unmerged files:`, no `(or: yman sync --abort)`.
- The README command reference omits `state`.
- commands.md:487's one-id list misses five commands.
- The paste block in docs/agents.md:11 ends in an unterminated heredoc.
- "claim" is `set --status doing` in the guide but `yman start` in CLAUDE.md.
- `after_help` is a fourth copy of the agent contract.
- The no-terminal editor message offers `-m / --body-file` to `edit`, which has neither.
- `state set --body-file` lacks `ValueHint::FilePath`.
- `snapshotted N local change(s)` is progress printed on stdout.
- Task paths print relative to three bases (repo root, `.yman`, absolute).

- Where: README.md:371, README.md:610, README.md:353, docs/commands.md:487, docs/agents.md:11, docs/agents.md:8, src/cli.rs:12, src/commands/edit.rs:24, src/cli.rs:469, src/commands/sync.rs:467, src/commands/show.rs:86
- Done when: each README block matches a fresh 0.9.0 tracker's output; `yman completions zsh` marks `state set --body-file` with `_files`; the snapshot line is a `note:` on stderr (or the doc says stdout); one base for printed task paths is named in commands.md.
