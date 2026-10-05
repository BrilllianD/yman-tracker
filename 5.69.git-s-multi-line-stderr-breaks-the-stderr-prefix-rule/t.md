# git's multi-line stderr breaks the stderr prefix rule

`warning: refresh failed:` and every `git <sub> failed: <stderr>` error span several lines, with git's own text unprefixed. The streams section of `docs/errors.md` arguably allows that ("git's own output"), but the rule that each stderr line carries a prefix or a two-space indent does not hold for those lines.

- Where: `src/errors.rs` or the callers that embed git stderr, `docs/errors.md` streams section.
- Done when: the continuation lines are indented, or the doc states the exception plainly.
