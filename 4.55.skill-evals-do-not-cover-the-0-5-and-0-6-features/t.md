# skill evals do not cover the 0.5 and 0.6 features

`skills/yman/evals/cases` holds five cases (body rewrite, claim, close, delete, unassigned query). None exercises `plan`, `--waits-on`, `add --sections`, the task-list procedure or exit-3 recovery, while `README.md` says the evals catch the skill drifting from the CLI.

- Where: `skills/yman/evals/cases`, `README.md` evals section.
- Done when: there is at least one case per new feature and one for the exit-3 recovery path.
