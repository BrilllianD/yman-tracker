# two eval graders are looser than their prompts

`waits-on/grade.sh` forbids only `set … --status blocked`, so `set 1 --relate 2` plus `move 1 blocked` passes the ban (it still needs `--waits-on` somewhere to pass `require_command`). `plan-epic/grade.sh` matches the answer with the glob `*6*"feature flag"*`, which any answer containing a `6` and "feature flag" satisfies; mentioning step 7 only prints a NOTE and does not fail.

- Where: `skills/yman/evals/cases/waits-on/grade.sh`, `skills/yman/evals/cases/plan-epic/grade.sh`.
- Done when: `move … blocked` is forbidden too, and the plan-epic answer check anchors on the step id as a word and fails when step 7 is named.
