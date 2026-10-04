# `init` cannot repair a wrong `.yman` HEAD

Preflight says `.yman worktree is not on refs/yman/local; run: yman init`, but the repair path in `init` starts with `ctx.check_worktree_head()?`, so it prints the same error it is meant to fix. Nothing ever re-runs `symbolic-ref HEAD refs/yman/local`.

- Where: `src/commands/init.rs` (`repair`), `src/repo.rs` (`check_worktree_head`).
- Done when: `init` on a worktree with a detached or wrong HEAD restores the symref and reports it, and a test detaches HEAD in `.yman` and recovers with `yman init`.
