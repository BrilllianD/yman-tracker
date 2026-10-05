# init exits 1 after the tracker exists when the refetch in publish_fresh fails

A failed push during `init` is a warning, but a failed refetch inside `publish_fresh` still returns an error. The tracker is then present locally, no hooks are installed, no summary is printed, and the exit code is 1. `docs/setup.md` does not cover this case.

- Where: `src/commands/init.rs` (`publish_fresh`), `docs/setup.md`, `docs/commands.md`.
- Done when: a refetch failure after a successful local init is a warning with the summary still printed, or the docs state that it exits 1 and what to do next.
