# `hooks` mishandles `core.hooksPath` and unreadable hooks

`hooks_dir` reads `core.hooksPath` with `config --get` and no `--type=path`, so `~/.githooks` is not expanded and yman installs into `<root>/~/.githooks`. `hooks::state` returns `Absent` on any read error, so a non-UTF-8 or unreadable foreign hook is overwritten and a dangling symlink is written through, against `docs/commands.md` §5.

- Where: `src/hooks.rs` (`hooks_dir`, `state`).
- Done when: the hooks directory comes from `rev-parse --git-path hooks`, a hook that cannot be read is refused rather than replaced, and scenarios cover both.
