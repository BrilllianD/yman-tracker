# `log <id>` rebuilds the whole history's rename graph

`historical_names` (`src/commands/log.rs:35-78`) runs an uncapped
`git log --diff-filter=R -M` over the entire history to find the paths one task
ever had, then passes them all to a second `git log`. The cost tracks the
number of renames in the repository, not in the task: measured at 420 ms
against 3802 rename records, for a task with 17 commits.

- Where: `src/commands/log.rs`
- Done when: the walk is bounded — by `-n`, by restricting the first log to the
  task's own paths, or by giving the final log `--follow` and dropping the graph
  — and `scripts/synthetic-project.sh` reports a smaller number.
