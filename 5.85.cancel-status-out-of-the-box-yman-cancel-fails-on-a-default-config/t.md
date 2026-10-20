# Cancel status out of the box: yman cancel fails on a default config

Problem: `yman init` writes `statuses.list = ["todo", "doing", "done"]` with `cancel: None` (`src/config.rs:101-119`), so `yman cancel <id>` on any freshly initialised tracker fails with `error: no cancel status configured; set statuses.cancel in .yman/config.toml`. Hit in vue-form-chad when the owner wanted to drop two tasks without deleting them; the only options left were `rm` (loses history) or `done` (misleading).

Wanted: cancelling works without hand-editing config.

Options to weigh:
- `yman init` writes a `cancel` status into `statuses.list` and sets `statuses.cancel = "cancel"` (and `terminal` so `ls` hides it like `done`).
- Existing trackers: either a fallback when the list already contains `cancel`/`cancelled`, or a clear hint in the error naming the two lines to add. Keep the "no silent fallback" rule from `config.rs:64` unless dropping it is decided explicitly.
- `ls` default filter and `plan` progress must treat the cancel status as closed.

Done when: `yman init` in a scratch repo, `yman add x`, `yman cancel 1 -m why` succeeds, `yman ls` hides the task, `yman show 1` shows status cancel and the comment; docs and CHANGELOG mention it.
