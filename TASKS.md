# TASKS

Backlog for `yman`. Derived from the as-built specification in `docs/`, the
"Limits and future work" section of `README.md`, and the current state of the
tree at `314ae88`.

Baseline as of 2026-09-17: `cargo test` is green (41 unit, 54 integration),
`docs/` and `README.md` describe the shipped behaviour, and every subcommand
listed in `src/cli.rs` is implemented.

Conventions: each entry names the files it touches and the condition that
closes it. Anything that changes a user-facing string must change
`tests/cli.rs` and `docs/errors.md` in the same commit.

---

## Correctness and data integrity

### Field-wise merge driver for `m.yml`

Two machines editing different fields of the same task produce a textual
conflict that the user resolves by hand, even though `status`, `priority`,
`tags`, `links` and `related` merge unambiguously in most cases.

- Where: new module (`src/merge.rs`), wired from `src/commands/sync.rs`;
  `docs/storage.md` needs a section on the driver and how it is registered.
- Done when: a driver merges non-overlapping field edits without user
  intervention, genuinely conflicting scalar edits still stop the sync with
  exit 3, and `tests/cli.rs` covers both outcomes.
- Open question: registering a merge driver means writing to the repository's
  config or `.gitattributes` inside `.yman`. Decide whether `yman init` does
  that, or whether it is opt-in — and record the decision in `docs/storage.md`
  before writing code.

### Force-add attachments that match `.yman/.gitignore`

`init` writes `*.swp`, `*~`, `.#*` and `*.orig` to `.yman/.gitignore`, and
`attach` stages with `git add -- <task dir>`, which silently skips ignored
paths. Attaching `notes.orig` therefore records the entry in `m.yml`, never
commits the file, and every other clone sees a dangling attachment after
`sync`.

- Where: `src/commands/attach.rs`, `tests/cli.rs`
- Done when: each attached file is staged with `git add -f -- <task>/f/<name>`
  (or such names are rejected with a pinned message), and a two-clone test
  attaches a `*.orig` file and finds it in `git -C .yman ls-files` on both
  sides.

### Preserve unknown `m.yml` keys on rewrite

The reader consumes and drops any key it does not know, and `docs/storage.md`
calls that accepted. There is no per-task schema version, so the day a newer
yman writes a field (`due:`, say), any older yman on the team destroys it on
the next `set`. Hand-added keys are lost the same way.

- Where: `src/yml.rs`, `src/task.rs` (`Meta`), `docs/storage.md`
- Done when: unknown top-level blocks survive a read-modify-write cycle
  verbatim, re-emitted after the known keys, and the doc sentence is
  reversed. Fix the reader's neighbouring inconsistencies in the same pass:
  an empty `status:` currently passes the required-field check as `""`, and
  duplicate keys are last-wins at top level but first-wins inside an
  attachment item.

### Validate the `author` id prefix

`author_prefix` returns `yman.author` or `$YMAN_AUTHOR` trimmed but otherwise
unchecked. A prefix containing `.` produces a folder such as `5.v.i-1.slug`,
which `FolderName::parse` reads as id `v` with slug `i-1.slug`; `/`, `\` or
whitespace make the folder unparsable, so the new task is invisible or
mis-identified.

- Where: `src/ids.rs`, `tests/cli.rs`, `docs/errors.md`, `docs/storage.md` §8
- Done when: a prefix that is empty or contains `.`, `/`, `\` or whitespace
  fails `add` with a pinned message, and the accepted grammar is written
  down next to the id schemes.

### Keep `related` references consistent

`set --relate` accepts an id that does not exist, and `rm` leaves dangling
references in every other task. The sync renumber path already rewrites
`related` entries, so the machinery exists.

- Where: `src/commands/set.rs`, `src/commands/rm.rs`, `docs/commands.md`
- Done when: relating to an unknown id prints a `warning:` on stderr but
  still commits (a concurrent `add` on another clone can legitimately race),
  `rm` drops references to the removed task in the same commit and reports
  the count, and the doc states that `related` is one-way and that id
  mentions inside `t.md` or `d.md` text are never rewritten.

---

## Robustness

### Keep a dangling attachment from breaking `show`

`detach` already tolerates an entry in `m.yml` whose file has disappeared
(`docs/commands.md` §4). Confirm `show` and `ls --json` behave the same way
rather than erroring, and add a regression test either way.

- Where: `src/commands/show.rs`, `src/commands/ls.rs`, `tests/cli.rs`
- Done when: a task whose attachment file was deleted outside yman renders with
  the entry marked as missing, and the behaviour is pinned in `docs/commands.md`.

### `d.md`: guard the entry header on write and on read

The parser starts a new entry at any line beginning with `## `, so a comment
whose text contains a markdown heading is split into a `Comment` plus a
`Raw` chunk and inflates the comment count in `ls` and `ls --json`. And
`append_entry` does not check that the file already ends with a blank line,
so a hand-edited tail glues the next header onto the previous line and the
whole chunk degrades to raw text.

- Where: `src/discussion.rs`, `docs/storage.md` §6
- Done when: the writer ensures a trailing blank line before appending and
  neutralises text lines that start with `#` (a leading space is enough; the
  reader trims), the reader only treats `## <timestamp> — ` with a
  whitespace-free timestamp as a header, and both cases have unit tests.

### `show`: order the discussion by timestamp

`merge=union` places the other side's lines after ours in a conflicting hunk,
so two comments written concurrently on two clones can display out of time
order. Low priority.

- Where: `src/commands/show.rs`
- Done when: parsed `Comment` entries are shown sorted by their timestamp
  while `Raw` chunks keep their file position, and `docs/commands.md`
  mentions it.

### Document the `m.yml` reader's flow-sequence limit

The flow-sequence branch splits on a bare `,`, so a hand-written
`tags: ['a,b']` fails with `unterminated single-quoted string`. The writer
never emits flow form, so this is a reader limit, not a bug to fix.

- Where: `docs/storage.md` §6
- Done when: the limit is listed next to the other accepted-but-not-written
  forms.

### Windows support

`README.md` claims Windows "should work with `core.longpaths`" but nothing
tests it. Either verify it or stop claiming it.

- Where: path handling in `src/task.rs` (slugs, folder names) and `src/repo.rs`
  (`read_gitdir_file`); `tests/common/mod.rs` for the fixture.
- Done when: either the integration suite runs on Windows in CI, or the README
  sentence is downgraded to "untested".

Two more non-Linux filesystem cases belong to the same entry. On a
case-insensitive filesystem slugs cannot collide because `slugify` lowercases,
but attachment names (`attach` checks an exact `exists()`) and `author`
prefixes can. On macOS the filesystem stores NFD, so a Cyrillic slug minted on
Linux and `git mv`'d on macOS can diverge unless `core.precomposeunicode` is
set. Record both in `docs/storage.md` §5 even if nothing else changes.

---

## Performance

### Stop spawning a `git` process per ref read

Every ref lookup shells out (`Git::rev_parse`, `src/git.rs:124`), including on
the lazy refresh path that runs before nearly every command
(`src/refresh.rs:40`). That is two or more process spawns before `yman ls`
prints a line, and `ls` otherwise touches no git at all.

- Where: `src/git.rs`, `src/refresh.rs`
- Approach: batch the reads that happen together into one `git for-each-ref`
  call, or read the loose ref and `packed-refs` files directly and keep the
  spawn as a fallback. Do not add a git library dependency for this — see the
  dependency rule in `CLAUDE.md`.
- Done when: `yman ls` in a repository that needs no refresh spawns at most one
  `git` process, and `scripts/spike-symref.sh` still passes.

### `find` by folder name only

`task::find` calls `list`, which reads `t.md` and `m.yml` for every folder in
`.yman/`, then keeps the one whose id matches. Every `show`, `set`, `comment`
and `attach` therefore pays for every done task on disk.

- Where: `src/task.rs`
- Done when: `find` parses directory names with `FolderName::parse`, loads
  only the matching folder, still reports `duplicate task id …` from the
  names alone, and the broken-folder error path is unchanged.

### Measure the cost of `ever_assigned` before deciding anything

Every `add` runs `git log --diff-filter=A --name-only` over `LOCAL` and
`REMOTE` to collect every id ever assigned, so `add` grows linearly with the
history. That is the price of "ids are never reused"; a tombstone file would
be a format change and is not wanted.

- Where: `src/ids.rs`, `README.md` (limits section)
- Done when: the cost is measured on a synthetic history of a few thousand
  commits and either recorded as acceptable in the README, or a new task
  proposes a cache that adds no file to the format.

Also doc-only: a plain `ls .yman/` orders `5.10.x` before `5.2.x` because
the listing is lexical. `docs/storage.md` §5 only promises priority-first
order, which holds; zero-padding ids would change the id contract and is not
on the table. Say so in the doc.

---

## Dependencies and tooling

### Add a CI workflow

`CLAUDE.md` calls `cargo clippy --all-targets -- -D warnings` the
"CI-equivalent gate", but there is no `.github/` directory — the gate only runs
when someone remembers to run it.

- Where: new `.github/workflows/ci.yml`
- Done when: pushes and pull requests run `cargo fmt --check`, the clippy gate,
  `cargo test` and `sh scripts/spike-symref.sh` — the same four steps as the
  `verify` skill in `.claude/skills/verify/SKILL.md`.

---

## Distribution

### Shell completions and a man page

`clap` can generate both from the existing `src/cli.rs` derives.

- Where: `src/cli.rs`, a new `yman completions <shell>` subcommand or a build
  script; `docs/commands.md` if it becomes a subcommand.
- Done when: bash, zsh and fish completions are generated from the same
  definition the binary uses, so they cannot drift.

### Release process

The crate is at `0.1.0` with no changelog and no tagged release.

- Done when: there is a `CHANGELOG.md`, a documented tagging convention, and a
  decision on whether the crate is published to crates.io. The binary is `yman`
  while the package is `yman-tracker`; if publishing, check the name is free
  before committing to it.

---

## Explicitly out of scope

Recorded so they are not re-proposed. Reopening any of these is a design
decision, not a task.

- git-lfs integration for attachments. Attachments go straight into git; the
  5 MiB warning is the whole policy.
- More than one `.yman` worktree per clone.
- Colour output, a TUI, or a web UI.
- A GitHub Issues bridge.
- New dependencies added for convenience. There is no `regex` and no
  `serde_json` on purpose; adding a crate needs a reason.
