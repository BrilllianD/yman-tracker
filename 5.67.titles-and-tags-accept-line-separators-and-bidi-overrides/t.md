# titles and tags accept line separators and bidi overrides

The title rule is `char::is_control`, which does not match U+2028/U+2029 (line and paragraph separators) or the bidi override characters. Either can make `ls` output misleading in a terminal. This is a design decision as much as a fix.

- Where: the title and tag validation in `src/`, `docs/errors.md`, `tests/cli.rs`.
- Done when: it is decided whether titles and tags refuse these characters, and the code, docs and tests match the decision.
