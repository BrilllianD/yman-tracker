# stdout writes panic on a closed pipe (yman ls | head)

`yman ls | head -1` panics with `failed printing to stdout: Broken pipe (os error 32)` and a Rust backtrace hint on stderr. `println!` panics when stdout is a closed pipe, and nothing in `main` handles it, so any listing piped into `head`, `grep -q` or a pager that quits early ends in a panic instead of a quiet exit. That breaks the "stdout is for data, pipeable" contract in `CLAUDE.md`. Found while starting the work on epic 31.

- Where: `src/main.rs` (no SIGPIPE / EPIPE handling), every command that prints with `println!` (`ls`, `show`, `log`, `plan`, ...).
- Done when: `yman ls | head -1` exits quietly with no panic text, and an integration scenario closes stdout early and asserts empty stderr.
