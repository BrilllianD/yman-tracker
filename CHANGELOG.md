# Changelog

All notable changes to `yman` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). How a release is cut is described
under [Releasing](README.md#releasing) in the README.

## [Unreleased]

### Changed

- `yman add --sections` walks the task history for taken ids once per run
  instead of once per section.

### Removed

- The `no editor configured; set $EDITOR` error, which could never fire: the
  editor resolver always falls back to `vi` or refuses earlier.

### Fixed

- `yman attach` needs `--force` whenever the task already lists the name,
  even when its file is gone, and compares names ignoring case: `--force
  A.png` over `a.png` replaces the file and the entry rather than adding a
  second entry. Names a Windows checkout cannot hold (`:*?"<>|`, control
  characters, a trailing `.` or space) are refused.
- `yman add --sections` refuses a code fence that is never closed
  (`<source>: line <n>: unclosed code fence`) instead of swallowing every
  later heading, and a fence line with an info string (```` ```sh ````) inside
  an open block no longer closes it. Fences follow CommonMark: a longer fence
  holds shorter ones.
- `m.yml` round trips: a value with Unicode whitespace at an edge (U+00A0)
  is quoted so it reads back intact; a comment after a quoted scalar
  (`status: 'todo'  # x`) no longer fails as unterminated; the libyaml
  escapes `\a \b \v \f \N \_ \L \P` are read; and an unknown `|+` block keeps
  its trailing blank lines on rewrite.
- `.yman/config.toml` refuses a blank, edge-whitespace or repeated
  `statuses.list` entry. A blank status used to let `add -s ''` commit a task
  whose `m.yml` read back as broken.
- `--relate` and `--waits-on` trim their values and refuse a blank one, and
  `set` refuses a task relating to (or waiting on) itself.
- Id generation reports an exhausted id space instead of looping forever
  (`ids.scheme = "random"` with every `random_len` id taken) or overflowing
  (`seq`/`author` past `u64::MAX`).
- Ids are sorted by a total order: plain numbers, then `prefix-N` by prefix
  and number, then the rest lexically. The old pairwise rules could form a
  cycle (`9 < 10 < 5-a < 9`), on which Rust's sort may panic. The one visible
  change is that a `prefix-N` id such as `z-1` now sorts before a random id
  such as `t-7f3a`.
- `yman refresh` on a dirty `.yman` prints one note,
  `note: .yman has uncommitted changes, refresh skipped`, instead of following
  it with a second `note: worktree has uncommitted changes`.
- `yman ls --help` describes `-a` as including closed tasks (any terminal
  status) rather than "the final status", which predates terminal sets.

- `yman set -a` now trims its value and treats a blank one as unassigned,
  as `yman add -a` always has; it used to store `" bob "` or `"  "` verbatim.

## [0.6.1] - 2026-10-04

### Fixed

- `yman ls | head -1`, and any other output piped into a reader that quits
  early, now ends quietly the way git and coreutils do (killed by SIGPIPE, shell
  status 141) instead of panicking with `failed printing to stdout: Broken
  pipe` on stderr.
- A multi-id verb that fails part-way (`yman done 32 99`) now prints the close
  report — epic warning, `no longer waits` notes — for the ids it did commit
  before returning the error; it used to drop it. `yman rm` of an open
  blocker now prints the `no longer waits` note for a blocked task it freed,
  as closing the blocker would. `yman attach` checks every source before
  copying any, so a missing second file or two sources with the same name
  (now `attachment "<name>" given more than once`) no longer leave an
  untracked copy of the first one under `f/`.
- `yman hooks` now finds the hooks directory the way git does, through
  `rev-parse --git-path hooks`. A `core.hooksPath` of `~/.githooks` used to be
  taken literally, so the hooks landed in `<project>/~/.githooks` where git
  never runs them. A hook yman cannot read — permission denied, content that is
  not UTF-8, a dangling symlink — used to count as absent: `install` overwrote
  it, or wrote through the link and created its target. It is now refused like
  any foreign hook (`hook <name>: cannot read <path>: <why>; not replacing it`)
  and `hooks status` reports it as `unreadable`.
- The user's `post-commit`, `post-merge`, `post-checkout` and `post-rewrite`
  hooks no longer run for yman's own commits, merges and fast-forwards in
  `.yman`, nor for the `git worktree add` that creates it. `--no-verify` only
  skipped the pre-hooks, so a husky, lefthook or LFS hook that wrote files
  left `.yman` dirty: refresh then backed off for good and the next snapshot
  committed the junk. Every git call in `.yman` now runs with
  `core.hooksPath=/dev/null`; hooks in the project itself, `yman hooks`
  included, fire as before.
- A push that origin refuses — a `pre-receive` hook declining it, a protected
  or denied ref — is no longer mistaken for origin having moved. `sync` used to
  retry it three times and end in `origin keeps moving`, autosync used to say
  `origin has new task commits`, and `init` then failed resetting to a
  `refs/yman/remote` that did not exist. The push now runs with `--porcelain`
  and only `fetch first`, `non-fast-forward`, `incorrect old value provided`
  and `reference already exists` count as the race; anything else fails at once with `push failed`
  and git's reason on stderr. An `init` whose push loses the race but finds
  nothing to adopt on refetch pushes again instead of resetting.
- A title containing a newline, a carriage return, a tab or another control
  character is refused by `add`, `set --title`, `add --sections`, `add -e` and
  `edit` with `invalid title "<t>"; titles must not contain line breaks or
  other control characters`; `edit` leaves the text on disk to be fixed, as it
  does for a file that does not parse. A newline used to be split into a title and a body line
  in `t.md` and to break the commit subject over several lines.
- An `add -e` whose editor fails, or whose edited `t.md` does not parse, no
  longer leaves an untracked task folder behind for `ls` to list and the next
  `sync` to publish. The folder is removed and the error says `task not added`.
- `comment -e` no longer writes a fixed `yman-comment-<id>.md` in the temp
  dir. On a shared `/tmp` a symlink planted at that path made it truncate
  another file, and two runs on one id shared the file. The temp file now gets
  a fresh name, is created exclusively with mode 0600 and never through a
  link, and is removed on every path; a refused editor creates none. A failing
  editor says `comment not added` instead of `file left as is`.
- The docs no longer call `comment <id>` with stdin safe for scripts: without
  `-m` it reads stdin to end of file whenever stdin is not a terminal, so a
  caller that leaves stdin open blocks. `docs/commands.md`, `yman guide` and
  the skill now say to pass `-m` or redirect stdin.
- An id a sync renumber moved a task to is no longer handed out again after
  that task is removed. The history scan read the renumber's `git mv` as a
  rename, not an add.
- A sync merge that conflicts in `config.toml` no longer locks out every
  command: `sync --abort` and `sync --continue` still run, mutating commands
  exit 3 as documented instead of 1, and `--continue` refuses a resolution
  that does not load.
- `yman init` re-attaches a detached or misplaced `.yman` HEAD to
  `refs/yman/local`, as the preflight error advises, instead of failing with
  that same error. It refuses when HEAD has commits or edits the ref lacks.
- Commands run from inside `.yman` — `cd $(yman path 14)` — find the project
  instead of failing with `.yman is not initialized`, and `init` there no
  longer nests a second worktree at `.yman/.yman`.
- `yman init` from a linked worktree of the project refuses to create a
  second `.yman` on the same ref; git does not guard a `--detach` checkout.
- Under a localized git, `init` and `sync` against an origin with no tasks
  no longer fail with `fetch failed`, and re-applying an identical change is
  no longer an error. Every git child now runs with `LC_ALL=C`.
- `sync --continue` no longer takes a conflict git could not mark up (a
  binary attachment, modify/delete, or an `m.yml` whose merge driver path is
  gone) as resolved, which kept ours and dropped theirs without a word. Such a
  file must be staged by hand, and `sync` now prints git's stderr from the
  failed merge, so a stale driver's `No such file or directory` is visible.

## [0.6.0] - 2026-10-04

### Added

- `yman plan <id>`: a task, a per-status count over every task relating to
  it, and the open ones sorted like `ls`, with `waits on <ids>` on a blocked
  step. `-a`, `-n`, `-l` and `--json` as on `ls`.
- `--waits-on <id>` on `add` and `set`: relate to a blocker and set status
  `blocked` in one commit. Needs `blocked` in `statuses.list`.
- Closing a task (`done`, `cancel`, `move`/`set --status`) notes on stderr
  each blocked task that now waits on nothing open, with the `yman move` that
  frees it. Nothing is moved automatically.
- `add --sections <path|->`: one task per `# Title` section of a markdown
  file, body up to the next heading, every other flag applied to each. A
  heading inside a code fence is body text; a bad file fails before any id
  is minted.
- Closing a task tagged `epic` while tasks relating to it are still open
  warns on stderr and names them; the close still goes through.

## [0.5.1] - 2026-10-01

### Added

- Published to crates.io as `yman-tracker`: `cargo install yman-tracker`.
  `Cargo.toml` gained the registry metadata and an `exclude` list, and the
  release steps in the README end with `cargo publish`.

### Changed

- The README says where a `cargo install` leaves the skill (the crate's
  unpacked source under `~/.cargo/registry/src/`) and how to copy it into a
  project or into `~/.claude/skills/`.

## [0.5.0] - 2026-10-01

### Added

- `yman ls --related <id>` keeps the tasks whose `related` list carries that
  id, so the steps of a plan can be listed from their epic without editing
  it. It ANDs with the other filters; an id nothing relates to lists nothing.
- `yman show` prints `related by:`, the tasks whose `related` carries this
  one, closed tasks included, and `show --json` always carries `related_by`.
  `related` stays one-way and nothing is stored; `show` reads every `m.yml`
  once to find them.
- The agent manual (`yman guide`, the skill and the README's "Scripts and
  agents") describes plans: an epic task, steps that `--relate` it, priority
  for order, and `blocked` plus a relation to the blocker for a step that
  waits. `ls --related` and `related by:` read the one-way relation from
  either end, so an epic is never edited when a step is added. The guide's
  line cap in `tests/cli.rs` rose from 60 to 72 to fit the section.
- `yman init --autosync push` (git config `yman.autosync`): every command that
  changes a task pushes it to origin once it has committed, and reports
  `note: pushed N task commit(s)` on stderr. It never fetches or merges; when
  origin has moved it warns `origin has new task commits; run: yman sync`, and
  a failed push is a warning, never the command's exit code. `yman status`
  shows `autosync: push` when it is on, and `status --json` always carries
  `"autosync"`.
- The agent manual (`yman guide`, the skill and the README's "Scripts and
  agents") now says when to run `yman sync`: before any status change, so an
  agent sees another clone's claim or close before overwriting it, and after
  claiming and closing, to publish. Before, it described `sync` but never
  asked for it, so agents left task history unpushed. The `claim-task` and
  `close-task` skill evals check both.

- A documentation site, built with mdBook from the README, `docs/` and the
  changelog and published to GitHub Pages:
  <https://brillliand.github.io/yman-tracker/>. `scripts/book.sh` builds it
  locally and fails on broken links or anchors.

### Fixed

- `docs/commands.md` and `docs/errors.md` were checked against the code and
  corrected where they had drifted: the mutating-command list, the `init`
  commits, `refresh` notes, exit codes after `yman git` and for
  `no merge in progress`, and messages the tables were missing.
- `docs/storage.md` and `docs/setup.md` were corrected the same way: what the
  spike script actually enforces, how `t.md` and `m.yml` are written, where
  hooks go, which commands use the network, and the filesystem notes added in
  the previous entry, which overstated one gap and missed the Windows ones.
- The three copies of the agent manual (`yman guide`, the skill and the
  README's "Scripts and agents") agree again: the task-list rule no longer
  asks for an assignee `ls` does not print, `rm -f` and the duplicate-id
  recovery are in all of them, and the README no longer says `init` adds one
  config line or that `sync` is the only command using the network.
- `yman edit` retitling a closed task in a version 2 repository no longer
  fails halfway with `pathspec ... did not match any files`; the task is
  renamed inside its status directory instead of being moved to the top of
  `.yman`.
- `yman sync` renumbering a colliding task that is closed, in a version 2
  repository, keeps it in its status directory. It used to move the task to
  the top of `.yman` while `m.yml` still said it was closed.

### Changed

- The README now calls Windows untested, and `docs/storage.md` §5 records
  the known case-insensitive and Unicode-normalization pitfalls on macOS and
  Windows.

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

[Unreleased]: https://github.com/BrilllianD/yman-tracker/compare/v0.6.1...HEAD
[0.6.1]: https://github.com/BrilllianD/yman-tracker/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/BrilllianD/yman-tracker/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/BrilllianD/yman-tracker/releases/tag/v0.2.0
