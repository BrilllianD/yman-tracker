# Store an agent-written project state summary in .yman

Keep a short, agent-written summary of the project's current situation in
`.yman/state.md`, apart from the tasks.

User wrote:
> Нужно хранить текущее состояние проекта, как небольшая сводка по текущей ситуации, она должна записываться агентом. Храниться отдельно от задач, но в папке .yman

Checked: no such file or command exists. Design:

- File `.yman/state.md` at the top level of the worktree; free-form markdown,
  overwritten whole on each write, history in `yman log`. Its name is already
  ignored by task discovery (`FolderName::parse`, `task_path_of`,
  `is_status_dir_name`); pin that in the reserved-name tests.
- `yman state` prints it verbatim; absent file: empty stdout, exit 0,
  `note: no project state yet; write one with yman state set -m <text>`.
- `yman state set (-m <text> | --body-file <path|->)`, exactly one; trimmed,
  one trailing newline; empty -> `empty state` (exit 1); commit `yman: state`;
  stdout `state updated`. Mutating only with the subcommand, so bare `state`
  stays readable during an unresolved merge.
- Synced like everything else; concurrent edits conflict as a plain text
  merge (exit 3 path).
- Docs: storage.md, commands.md, errors.md, agents.md, SKILL.md (+ eval case),
  README, CHANGELOG.

Where: src/cli.rs, new src/commands/state.rs, src/commands/mod.rs, tests/cli.rs.
Done when: the commands above work and are tested, the gate is green, docs and
the three agent surfaces describe `yman state`.
