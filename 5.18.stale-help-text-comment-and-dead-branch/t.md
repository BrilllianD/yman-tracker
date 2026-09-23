# stale help text, comment and dead branch

Three small leftovers the doc audit found.

- `ls -a` help says "Include tasks in the final status"; version 2 has a terminal set (`src/cli.rs`).
- The preflight comment in `src/repo.rs` lists the exempt commands as `init`, `hooks`, `git`; `guide`, `completions`, `man` and `merge-driver` are exempt too.
- `no editor configured; set $EDITOR` in `src/commands/edit.rs` cannot fire, because `resolve_editor` always falls back to `vi`. Remove it (and its line in `docs/errors.md`) or make it reachable.
- Done when: all three are gone and the gate is green.
