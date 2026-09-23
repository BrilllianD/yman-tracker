# second `.yman` from a linked worktree is probably not rejected

`init` adds the worktree with `--detach`, and git only refuses a second checkout of a branch, not of a commit. From a linked worktree of the project the toplevel differs, so a second `.yman` could be created with both HEADs on `refs/yman/local`. The error branch in `init.rs` may never fire. Untested inference from the audit.

- Where: `src/commands/init.rs` (`worktree add --detach`, the rejection branch), `src/repo.rs` (toplevel discovery).
- Done when: an integration scenario runs `init` from a linked worktree and it is refused with a clear message, as `docs/setup.md` and `docs/storage.md` promise.
