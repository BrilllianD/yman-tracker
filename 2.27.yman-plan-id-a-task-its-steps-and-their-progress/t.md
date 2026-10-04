# `yman plan <id>`: a task, its steps and their progress

Reading a plan takes two calls (`show <epic>` + `ls --related <epic> -a`) and gives no progress count.

- Where: new `src/plan.rs` (steps_of, waits_on), new `src/commands/plan.rs`, `src/cli.rs`, `src/commands/ls.rs` (share table code), docs.
- Done when: `yman plan <id>` prints the header, per-status counts and open steps (`-a -n -l --json`), tests cover it, docs in all three agent copies.
