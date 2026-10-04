# push rejection is classified too broadly and the retry is untested

`push` treats any `rejected` in stderr as `Rejected`, so `[remote rejected] (pre-receive hook declined)` and protected-ref denials are retried three times and end in `origin keeps moving`; autosync says `origin has new task commits`; and `init` then runs `reset --hard refs/yman/remote` on a ref that may not exist. The scenario `push_rejected_retry` never reaches the retry: `sync` fetches at the top of every attempt, so the push is never rejected, and the loop and the `origin keeps moving` message have no test.

- Where: `src/commands/sync.rs` (`push`, `PushResult`, the retry loop), `src/commands/init.rs` (the `Rejected` branch), `tests/cli.rs` (`push_rejected_retry`).
- Done when: the push uses `--porcelain` and only `!` with `fetch first` or `non-fast-forward` counts as rejected, and a scenario forces a real race (a pre-receive hook on the bare remote that advances the ref) and asserts the retry.
