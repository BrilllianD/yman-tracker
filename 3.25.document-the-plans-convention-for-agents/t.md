# document the plans convention for agents

The agent manual says nothing about splitting big work into tasks. With `ls --related` and `show`'s `related by` a one-way convention is enough: an epic task, steps that `--relate` it, priority for order, `blocked` plus a relation to the blocker for dependencies.

- Where: `skills/yman/SKILL.md` (new `## Plans` section, task-list step 3), `docs/agents.md` (condensed, raise the 60-line cap in tests/cli.rs), README `### Scripts and agents`, CHANGELOG.
- Done when: all three agent copies describe the convention, `yman guide` still passes its test, book.sh is green.
