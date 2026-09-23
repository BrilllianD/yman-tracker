# Changelog

All notable changes to `yman` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). How a release is cut is described
under [Releasing](README.md#releasing) in the README.

## [Unreleased]

## [0.4.0] - 2026-09-23

### Added

- `yman completions <shell>` prints a completion script for bash, zsh, fish,
  elvish or PowerShell, and `yman man [--dir <dir>]` prints the man page or
  writes every page to a directory. Both are generated from the CLI definition
  itself and work outside a repository.
- `yman init` in a repository with no `origin` sets up a local-only tracker
  instead of refusing. `status` reports it, `sync` says to connect a remote,
  and `yman init --remote <url>` does so later.

### Fixed

- `show` prints the discussion in timestamp order, so two comments written
  concurrently on two clones no longer appear out of order after a sync.
  Unparsable chunks keep their place; `d.md` itself is unchanged.

### Changed

- The yman skill formats a task list as a table and suggests what to take next.

## [0.3.0] - 2026-09-23

### Added

- `yman ls -l` prints each task's body, indented under its row; `--json` gains
  `body` only under `-l`.

### Fixed

- `sync` keeps a moved task's new files with the task: a first comment or an
  attachment added on another clone under the old folder name follows a retitle
  or a close instead of stopping with exit 3 or being orphaned.

### Performance

- `yman log <id>` finds a task's history by a pathspec on its id rather than by
  rebuilding the rename graph, and no longer shows unrelated tasks' commits
  when git paired two folders as a rename. About 400 ms down to 125 ms on the
  synthetic 1000-task repository.

## [0.2.0] - 2026-09-23

First tagged release.

### Added

- The tracker itself: task history in `refs/yman/local`, checked out at
  `.yman/`; `init`, `add`, `ls`, `show`, `edit`, `set`, `rm`, `comment`,
  `attach`, `log`, `status`, `path`, `sync`, `refresh`, the `git` passthrough
  and the optional git hooks.
- `move`, `cancel` and `reopen`; state verbs take several ids.
- Named status roles and a terminal set in the config; closed tasks are
  archived into a per-status folder, and `ls` hides every terminal status.
- The `tags` command, which renames and removes a tag across all tasks; tags
  are validated and lowercased, and `ls -t` matches case-insensitively.
- `--json` output for `show` and `status`.
- `yman guide`, and a description for every flag.
- Bodies set without an editor on `add` and `set`, `set -m` appending a comment
  in the same commit, and `edit` refusing the `vi` fallback without a terminal.
- `add` accepts an assignee, links and related tasks.
- `show -n` caps the discussion; `ls` filters and limits without reading the
  whole backlog.
- `comment` and `attach` take the author from `YMAN_ACTOR`.
- Exit 4 when a task id does not exist.
- `sync` merges `m.yml` field by field and union-merges its list fields, and
  rewrites related references when it renumbers a task.
- The yman agent skill and its evals, and CI running the quality gate.

### Fixed

- `sync` keeps a task's id through a retitle, only renumbers ids minted
  locally, and follows tasks into status directories.
- Related references stay consistent.
- An attachment whose file is gone is marked as such.
- Attachments matching `.yman/.gitignore` are staged.
- The `d.md` entry header is guarded on write and on read.
- Unknown `m.yml` keys survive a rewrite.
- An author prefix that would break folder names is rejected.
- `statuses.done` is required once more than one status is closed.

### Performance

- Refs are read from disk instead of by spawning `git`.
- A task is looked up from folder names alone.

[Unreleased]: https://github.com/BrilllianD/yman-tracker/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/BrilllianD/yman-tracker/releases/tag/v0.2.0
