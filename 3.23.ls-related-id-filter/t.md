# ls --related <id> filter

Plans need a way to list the steps that relate to an epic. `related` is one-way, so today the epic must be edited for every step.

- Where: `src/cli.rs` (LsArgs), `src/commands/ls.rs` (filter chain), `docs/commands.md` ls section, README ls block, `skills/yman/SKILL.md` flag table, `docs/agents.md`.
- Done when: `yman ls --related <id>` keeps only tasks whose related list contains <id>, ANDs with the other filters, is tested, and all four docs name it.
