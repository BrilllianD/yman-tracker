# agent-facing docs disagree with the code and with each other

CLAUDE.md requires the guide, the skill and the README to agree, but they do not:
- `docs/agents.md:67` (printed by `yman guide`) tells agents to `yman git add` the side to keep of a markerless file. The skill, the README and the `--continue` error say `yman git checkout --ours|--theirs` first. Followed as written, the guide stages whatever is on disk, which is the one-sided pick `resume` refuses to make.
- `cancel` is listed for agents with no caveat, and yman-task tells agents to close a claim that does not hold with `yman cancel`. On every pre-0.9.0 tracker, this repository's own included, that exits 1.
- README `### Scripts and agents` omits the `-f` refusal, exit 3, and the "re-`show`; stop if taken" step.
- README.md:195 says unknown `m.yml` keys are dropped, but the code and storage.md keep them.

- Where: docs/agents.md:67, skills/yman/SKILL.md:275, skills/yman-task/SKILL.md:80, docs/agents.md:37, README.md:399, README.md:195, src/commands/set.rs:334
- Done when: `yman guide | grep -- '--ours|--theirs'` matches within the 74-line cap; all three copies name the fallback for a tracker without `statuses.cancel`; README Scripts and agents mentions `-f`, exit 3 and the re-show rule; README.md:195 matches storage.md:295.
