# spike script only warns on the properties it claims to prove

`docs/storage.md` §2 says `scripts/spike-symref.sh` proves that committing moves `refs/yman/local` and that HEAD stays symbolic. The script prints `FALLBACK` and `<detached>` and carries on in those cases; only the invisibility checks and the clean main worktree fail the run. It also cites `YMAN_PLAN.md §8`, which does not exist.

- Where: `scripts/spike-symref.sh`, `docs/storage.md` §2.
- Done when: the script exits non-zero when either claimed property fails (or the doc stops claiming it), and the stale reference is gone.
