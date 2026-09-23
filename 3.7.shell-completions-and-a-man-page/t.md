# Shell completions and a man page

`clap` can generate both from the existing `src/cli.rs` derives.

- Where: `src/cli.rs`, a new `yman completions <shell>` subcommand or a build
  script; `docs/commands.md` if it becomes a subcommand.
- Done when: bash, zsh and fish completions are generated from the same
  definition the binary uses, so they cannot drift.
