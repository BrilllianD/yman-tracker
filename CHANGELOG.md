# Changelog

All notable changes to `yman` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). How a release is cut is described
under [Releasing](README.md#releasing) in the README.

## [Unreleased]

### Fixed

- `yman init` no longer rewrites a `.git/info/exclude` it cannot decode as UTF-8
  down to the single line `.yman/`; it appends and keeps every other byte.
- A `yman init` that failed after creating `refs/yman/local` (`git worktree
  add` failed, its commit failed) no longer leaves every rerun refusing with
  "refs/yman/local is not a yman history"; the rerun finishes it.
- `yman init --hooks` with a hook yman does not own (husky, lefthook) now
  finishes with exit 0 and its summary, reporting the hook on stderr.
- Commands run in a linked worktree of the project (`git worktree add`) now
  use the main clone's `.yman` (or their own, when only they have one)
  instead of asking for `yman init`, and yman's
  hooks stay silent there.
- `yman refresh --quiet`, which the hooks run, no longer prints an error in a
  repository without a tracker (for example under a global `core.hooksPath`).
- Outside a work tree (a bare repository, a directory git refuses under
  `safe.directory`) every command now shows git's reason under "not inside a
  git repository" instead of hiding it.
- `comment -e` no longer discards text the editor saved in a non-UTF-8
  encoding as `empty comment`: it fails, names the error and keeps the file;
  `ls` and `show` no longer report 0 comments for a `d.md` that is not valid
  UTF-8 or cannot be read, they warn on stderr.
- A mutating command whose commit fails (a failing GPG signature, a held
  `index.lock`) no longer leaves its files staged for the next command to commit
  under its own subject; `.yman` is put back as the last commit left it, `add` no
  longer leaves a half-made task for a retry to duplicate, `attach` no longer
  reports files it did not commit, and `rm` of a hand-edited task no longer stages
  the rewrite of other tasks' `related`.
- Under `yman.autosync = push`, a command that committed some tasks and then
  failed (`done 3 99`, `add --sections` stopping part-way) now pushes what it
  committed instead of leaving it local.
- Commands that write the task history on one clone now run one at a time,
  so parallel `yman add` calls (an agent's parallel tool calls) no longer mint
  the same id; a waiting command prints a note. A filesystem that cannot lock
  (Linux `ENOLCK`, as on NFS without lockd) only warns instead of failing.
- A clean `sync` merge can no longer publish one task split across two
  folders. The duplicate-id check and the rejoin of attachments left under a
  moved folder now run on every merge before it is committed; a file present
  on both sides stops the merge (exit 3) instead of being left behind; a fetch
  running alongside can no longer slip unrenumbered ids into the merge; and a
  failure part-way leaves the merge for `sync --continue` instead of having
  the next sync commit half of it. The separate rejoin and drop commits are
  gone; those repairs are part of the merge commit. `sync --continue` stops
  with exit 3, instead of removing it, when a removed task's folder holds a
  file you kept or staged by hand or an uncommitted edit.
- A failure after `sync --continue` started its work exits 3 instead of 1.
- `init` in a clone with no local tracker no longer adopts a
  `refs/yman/remote` that origin has since deleted; like `sync`, it drops the
  stale ref with a warning and creates a fresh tracker.
- Two clones attaching the same name in different case (`A.png` and `a.png`)
  now conflict on `sync` instead of merging into two entries that `attach`
  itself would refuse, and an attachment `path` that a hand edit changed is
  read as `f/<name>`, the file `show` and `detach` actually use. An attachment
  `name` such as `../t.md` can no longer make `show` or `detach` reach outside
  `f/`: `show` lists it as missing and `detach` refuses it.
- A hand-edited `m.yml` no longer crashes `ls` and the merge driver with a
  block indentation indicator larger than the text's indentation (`|3`);
  `tags: [a, b]  # note` reads as two tags instead of storing `'[a, b]'` on
  the next write (`related: [] # x` likewise); and folded blocks (`>`) no
  longer add a blank line at each paragraph break.
- The user's git configuration no longer changes what yman's own git calls do:
  `color.*`, `log.decorate` and `log.showSignature` cannot leak into
  `yman log`, `status.showUntrackedFiles=no` no longer hides hand-made files
  from `status` and the sync snapshot, and `merge.ff=only` and
  `merge.verifySignatures` no longer fail every diverged `sync`. Task and
  attachment paths are matched literally (`detach 1 '[ab].png'` no longer
  removes `a.png`), `GIT_LITERAL_PATHSPECS` no longer empties `yman log <id>`,
  and a diff3 `|||||||` line left in a resolved file keeps `sync --continue`
  pending.
- `sync --continue` no longer mistakes a bare `=======` line in task content (a
  setext heading, a pasted separator) for a conflict marker; an identical
  `state set` or `attach --force` no longer fails when other files in `.yman`
  are untracked or unstaged; a failure while reporting a stopped merge now
  exits 3 instead of 1.

### Changed

- Building needs Rust 1.89 (`std::fs::File::lock`).

## [0.9.0] - 2026-10-20

### Changed

- `yman init` writes `version = 2` with a `cancelled` status, so `yman cancel`
  works out of the box. New trackers archive closed tasks under `done/` and
  `cancelled/`, and a yman older than version 2 cannot read them. Existing
  version 1 trackers are unchanged; `yman cancel` there now says how to opt in.

## [0.8.0] - 2026-10-19

### Added

- `yman state` prints a project state summary, `.yman/state.md`, and
  `yman state set -m <text> | --body-file <path|->` rewrites it in one commit,
  so an agent can leave the next session a short account of where things
  stand. It syncs like any file; the yman skill and `yman guide` cover it.
- `skills/yman-task/`, a second agent skill: `/yman-task <text>` files the
  user's own words as tasks tagged `user-request` and `needs-agent`, quoting
  the original, so the agent that picks one up re-checks it before working.

## [0.7.0] - 2026-10-13

### Added

- `Cargo.toml` declares `rust-version = "1.88"`, the first release with
  let-chains, which the code uses.

### Changed

- `yman guide` and the skill say that `yman plan`'s rows are enough to pick
  the next step, so agents stop running `show` on every listed step.
- Titles and tags refuse the Unicode line and paragraph separators and the
  bidi override and isolate characters, which could make `ls` output
  misleading, with the existing control-character errors; a refused tag is
  shown escaped.
- Git's multi-line stderr quoted in `git <sub> failed: …` (and so in
  `warning: refresh failed: …`) has its continuation lines indented by two
  spaces and blank lines dropped, and `sync`'s push-refusal line is indented
  two spaces, so every stderr line yman writes carries a prefix or continues
  one.
- Every stderr line now carries its prefix or continues one: the `rm` and
  `tags rm` prompts moved from stdout to stderr, `refreshed: N new commit(s)`
  became `note: refreshed N new commit(s)`, the hook advice is a `note:` and
  an unreadable hook a `warning:`, and `sync` introduces its list of
  conflicting files with `note: unmerged files:`.
- `yman add --sections` walks the task history for taken ids once per run
  instead of once per section.

### Removed

- The `cannot attach <path>: no file name` and `statuses.terminal marks every
  status terminal` errors, which could never fire: the `not a regular file`
  check and the rule that `statuses.default` is not terminal refuse first.
- The `no editor configured; set $EDITOR` error, which could never fire: the
  editor resolver always falls back to `vi` or refuses earlier.

### Fixed

- A fresh `yman init` whose push lost the race and whose refetch then failed no
  longer exits 1 with the tracker half set up: it installs hooks, prints the
  summary and warns `not published to origin (fetch failed); run: yman sync`.
- `add --sections` reads a line like ```` ```x``` inline ```` as text, as
  CommonMark does, instead of opening a fence and refusing the file with
  `unclosed code fence`.
- `attach --force A.png` over `a.png` on a case-insensitive checkout (macOS,
  Windows) records `A.png` in git, matching `m.yml`, instead of leaving the
  index at `a.png` and every Linux clone with a dangling attachment.
- `attach` refuses Windows reserved device names (`CON`, `NUL`, `COM1`,
  `LPT1`… with or without an extension, any case), which Git for Windows
  cannot check out, with the existing `is not portable` error.
- Hooks defined in git configuration (`hook.<name>.command`, git 2.54+) no
  longer fire for yman's commits, merges and checkouts in `.yman`, and yman's
  own `update-ref`, `fetch` and `push` no longer run the user's
  `reference-transaction` hook. `yman hooks` refuses an empty
  `core.hooksPath`, which turns git's hooks off, instead of installing into
  `.git/hooks` and reporting `installed`, and no longer trims a
  whitespace-only one.
- A comment line after an unknown `|+` block in `m.yml` is dropped like every
  other comment, instead of surviving when a blank line followed it. An
  indented `#` line inside an unknown key's block is content and is kept.
- An `m.yml` value with a line of only Unicode whitespace (a U+00A0, say, from
  `set --link`) is written double-quoted, so it no longer reads back empty or
  chomped.
- A blank or `-` `assignee` in `m.yml` reads as unassigned (`ls --assignee -`
  lists it, `--json` prints `null`), and `set -a -` / `add -a -` unassign
  instead of storing a literal `-`.
- `rm`, `set` and the status verbs no longer exit non-zero after their commit
  has landed when reading the tasks back for the "no longer waits" report
  fails; that is now a warning, so a script that retries on failure does not
  apply the change twice.
- An aborted `yman add -e` into a closed status under an archive config also
  removes the status directory it created, and a copy failing part way
  through `yman attach` (a full disk) restores `f/` as it was, including files
  `--force` had already replaced, instead of leaving the earlier copies behind.
- A stopped `sync` no longer says a task "was closed to two different
  statuses" when the second folder is only a file left under the task's old
  name after a retitle or close; the note now needs two folders that carry
  the task under two different closed statuses.
- A `t.md` with CRLF line endings no longer keeps a stray `\r\n` at the top
  of its body, and a UTF-8 BOM no longer breaks `t.md` (`must start with
  "# Title"`) or hides `m.yml`'s first key.
- The comment count in `ls` and `ls --json` leaves out unparsable `d.md`
  chunks.
- `yman tags rename` replaces every spelling of the old tag on a task, not
  only the first (`[UI, ui]` renamed to `x` gave `[x, ui]`).
- `yman tags --json rename …` and `yman sync --abort --no-push` are refused
  as usage errors instead of silently ignoring the extra flag.
- Shell completions offer file paths for `--body-file` and `--sections`.
- `rm` on one clone and a first comment or attachment on the same task on the
  other no longer merge into a folder holding only `d.md` (or `f/`) that no
  command could load: sync drops it with
  `note: dropped <rel>: task <id> was removed on one side; …`.
- `yman sync` no longer dies after the merge with `Not a valid object name
  HEAD^2` when one merge needs both a rejoin and a remnant drop, which left
  the unloadable remnant for the next sync to push. Nor does it die with
  `pathspec … did not match any files` on a removed task's folder that `rm`
  left holding only an ignored file such as `*.swp`; there is nothing to drop
  there, and the sync goes through.
- `yman detach` matches the attachment name ignoring case, as `attach` does:
  `detach 3 A.png` removes an `a.png` (under its stored spelling) instead of
  failing with `no attachment "A.png" on task 3`.
- `yman init` whose first push fails no longer stops before `--hooks` and the
  summary: the tracker exists locally, so it finishes and warns
  `not published to origin (<why>); run: yman sync`.
- `yman init` names the history it found broken (`refs/yman/local` or
  `refs/tasks/main on origin`) instead of always blaming origin.
- The merge driver's path is single-quoted in git config, so a binary under a
  path containing `"`, `$`, a backtick or backslashes still runs.
- Repository discovery works with git older than 2.31, which echoes
  `--path-format=absolute` back instead of honouring it.
- `yman sync` pushes the commit `refs/yman/local` held when the push began,
  by sha, and records that commit as published. A commit made by another
  command during the push used to be marked published without reaching
  origin, and autosync then never pushed it.
- A sync renumber that fails part-way resets `.yman` to the commit before it
  (`renumber aborted; .yman restored`) instead of leaving half-moved folders
  for the next sync to snapshot.
- `yman sync --continue` reports how many commits the merge pulled instead
  of `pulled 0`, gives the git-identity hint when committing fails for want
  of one, and no longer follows symlinks while scanning for conflict markers
  (a committed symlink loop recursed until it failed).
- `yman sync` and `yman init` no longer overwrite the main repository's
  `FETCH_HEAD`.
- Run from a git hook or alias, yman strips every variable
  `git rev-parse --local-env-vars` lists, plus `GIT_NAMESPACE`, instead of
  five of them, and `yman git` strips the same set, so it acts on `.yman`
  rather than on the repository the hook was running for. A `yman git` whose
  git is killed by a signal exits `128 + signal` instead of 1.
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
- `--help` for `edit`, `add -e` and `comment -e` names `$VISUAL` before
  `$EDITOR`, the order the editor is looked up in, and `yman --help` lists
  every editor-free way to give a body (`add -m`, `set --body-file` were
  missing). The agent manual and the skill list `plan` among the commands
  that take `--json` and say that `ls` and `plan` carry `body` only with
  `-l`; the docs add the `autosync` key of `status --json`, the
  `yman.autosync` setting `init` writes, and the `git worktree add failed`
  error.

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

[Unreleased]: https://github.com/BrilllianD/yman-tracker/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/BrilllianD/yman-tracker/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/BrilllianD/yman-tracker/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/BrilllianD/yman-tracker/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/BrilllianD/yman-tracker/releases/tag/v0.2.0
