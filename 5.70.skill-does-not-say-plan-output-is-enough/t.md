# skill does not say plan output is enough

The `plan-epic` eval fails: after `yman plan 4` the agent still runs `show` on steps whose titles and status `plan` already printed. The skill says `plan` answers the question but does not say that no further `show` is needed.

- Where: `skills/yman/SKILL.md`, `docs/agents.md` (72-line cap), the README "Scripts and agents" section.
- Done when: all three say it, and a fresh `./run.sh --case plan-epic` passes.
