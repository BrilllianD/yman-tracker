# check_portable omits Windows reserved device names

`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9` and `LPT1`–`LPT9`, with or without an extension, pass `check_portable`, yet `docs/commands.md` and `docs/errors.md` promise to refuse "the names a Windows checkout cannot hold". `attach 3 x --name nul.txt` is accepted and Git for Windows will not check it out.

- Where: `src/commands/attach.rs` (`check_portable`), `docs/commands.md`, `docs/errors.md`, `tests/cli.rs`.
- Done when: either the reserved names are refused with the existing `is not portable` message and tested, or the docs list exactly what is checked.
