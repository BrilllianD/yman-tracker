# init overwrites info/exclude and strands half-made state

`exclude_add` reads `info/exclude` with `unwrap_or_default`, so a non-UTF-8 or unreadable file becomes `""` and is then overwritten with the single line `.yman/`. This runs on every `init`, including repair. A fresh `init` points `refs/yman/local` at the empty root commit before `worktree add` and before `config.toml` exists. If anything after that fails, every rerun then refuses with `refs/yman/local is not a yman history`, and there is no recovery hint. `init --hooks` with a foreign hook (husky, lefthook) bails after the tracker is created and published: exit 1, no summary.

- Where: src/repo.rs:186, src/repo.rs:200, src/commands/init.rs:62, src/commands/init.rs:80, src/commands/init.rs:109, src/hooks.rs:126
- Done when: `init` over an `info/exclude` holding a Latin-1 line keeps those bytes; a scenario that makes `git worktree add` fail once and then reruns `yman init` ends with a working tracker; and `init --hooks` with a foreign `post-merge` exits 0, prints the summary with `hooks:    not installed`, and puts the note on stderr.
