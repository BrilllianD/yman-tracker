# hooks in .yman: config-defined hooks still fire, blank core.hooksPath misread

`-c core.hooksPath=/dev/null` on the `.yman` runner does not stop hooks defined in configuration (`hook.<name>.command` / `hook.<name>.event`, git 2.54+), so a configured post-commit still fires in `.yman`. Blocking them needs a per-hook `-c hook.<name>.enabled=false`, which means enumerating `hook.*` first. The main-runner `update-ref` and `fetch` also fire the user's `reference-transaction` hook. Separately, an empty `core.hooksPath` turns hooks off in git, yet `yman hooks install` writes into `.git/hooks` and reports `installed`. A whitespace-only value is trimmed by `get_cfg` and treated as unset, while git treats it as a relative directory.

- Where: `src/git.rs`, `src/hooks.rs`, `docs/commands.md` §2.
- Done when: no configured hook fires for a `.yman` commit, and `hooks install` either honours an empty or blank `core.hooksPath` the way git does or refuses with a clear message.
