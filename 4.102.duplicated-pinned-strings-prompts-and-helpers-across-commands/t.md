# duplicated pinned strings, prompts and helpers across commands

The confirm-or-refuse prompt behind `refusing to remove without -f` is 12 identical lines in rm and tags. The `unknown status` check appears three times, and the `skipped N unreadable task folder(s)` warning three times. `rev-list --count … unwrap_or(0)` appears four times, and the porcelain dirty count twice next to `Git::is_dirty`. The task JSON object is assembled separately in ls, show and plan. The "related by" rule exists in three copies and they have already drifted: `ls --related` lists a self-related task, while `show` and `plan` exclude it. `plan` imports `ls` internals, and add and comment import `edit` internals.

- Where: src/commands/rm.rs:14, src/commands/tags.rs:191, src/commands/add.rs:69, src/commands/set.rs:43, src/commands/ls.rs:13, src/commands/sync.rs:870, src/autosync.rs:44, src/commands/status.rs:77, src/commands/ls.rs:52, src/commands/show.rs:42, src/plan.rs:66
- Done when: each pinned string occurs once in src/; `grep -c '"rev-list", "--count"' src` totals 1; a self-related task gives the same ids from `ls --related X --json` and `plan X -a --json`.
