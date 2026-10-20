# test isolation leaks the developer's environment

- `GIT_CONFIG_GLOBAL` does not cover `$XDG_CONFIG_HOME/git/ignore` and `attributes`, or `/etc/gitattributes`. On this machine the fixture already sees `~/.config/git/ignore`.
- `GIT_TRACE*` and `GIT_TEMPLATE_DIR` pass through.
- The German-locale probe runs git outside `sandbox()`.
- The `configured_hooks` unit test reads the real global config.
- Nothing checks that `SCRUBBED_ENV` and the `unset` lines in scripts/*.sh cover `LEAKY_ENV`.
- Scenarios that skip (root, no German locale, git without config hooks) report `ok`.

- Where: tests/common/mod.rs:109, tests/common/mod.rs:14, tests/cli.rs:3506, src/git.rs:352, tests/cli.rs:1353, tests/cli.rs:3541, tests/cli.rs:4027, tests/cli.rs:8829
- Done when: `sandbox()` points `XDG_CONFIG_HOME` inside the tempdir and sets `GIT_ATTR_NOSYSTEM=1`; the suite passes with `GIT_TRACE=1 GIT_TEMPLATE_DIR=/nonexistent` and with an XDG `git/ignore` holding `*`; a test asserts every `LEAKY_ENV` name is in `SCRUBBED_ENV` and both scripts' `unset` lists; skipped scenarios are distinguishable from passes.
