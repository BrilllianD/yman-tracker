# show prints related by: back-references

An epic cannot see its steps: `show <id>` prints the task's own `related` but not the tasks that relate to it.

- Where: `src/commands/show.rs` (scan `task::list`, pass into print_text and print_json), `src/task.rs` find() doc comment, `docs/commands.md` show section, README, `skills/yman/SKILL.md`, `docs/agents.md`.
- Done when: plain `show` prints `related by: a, b` when non-empty, `--json` carries `related_by` (`[]` when empty), tests pin both, docs updated.
