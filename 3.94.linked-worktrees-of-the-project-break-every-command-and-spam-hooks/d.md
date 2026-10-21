## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ay8 linked worktree (common dir has .yman): ls rc 1 'error: .yman is not initialized; run: yman init'; init rc 1 'error: refs/yman/local is already checked out in another worktree of this repo; only one .yman per clone is supported'; main worktree: ls rc 0; not run: a global core.hooksPath variant; the yman_parent/ydir_state predicates and default_wt_gitdir (code shape)

## 2026-10-21T17:44:16Z — Bronnikov Aleksandr

7b11fc7: ROOT from main toplevel in linked worktrees, one our_worktree_gitdir predicate, refresh --quiet silent without tracker

