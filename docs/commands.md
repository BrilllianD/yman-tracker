# Command semantics

Normative, as-built. Companion documents: [storage.md](storage.md),
[errors.md](errors.md).

## 1. What runs before a command body

`main` does three things in order before the body, and one after it (`src/main.rs`):

1. **Discover** the repository. Failure is `not inside a git repository`.
2. **Preflight**, for every command except `init`, `hooks` and `git`:
   `.yman` must be a linked worktree of this repo; its HEAD must be the
   symbolic ref `refs/yman/local`; for *mutating* commands, an unresolved
   sync merge stops the command with exit 3; then `config.toml` must load and
   validate. The merge check comes first so that a merge which conflicted in
   `config.toml` still exits 3, and `sync` is let through an unloadable
   config while a merge is pending — `--abort` and `--continue` are how that
   state ends.
3. **Lazy refresh**, when `yman.refresh` is unset or `lazy`, for every command
   except `init`, `sync`, `refresh`, `status`, `hooks` and `git`. A failure here
   is downgraded to a `warning:` on stderr and never takes the real command
   down.
4. **Autosync**, after a *mutating* command succeeded, when `yman.autosync` is
   `push` and there is an `origin`: one push of `refs/yman/local` to
   `refs/tasks/main`, skipped when `LOCAL` already equals `REMOTE`. It never
   fetches or merges. A push reports `note: pushed N task commit(s)`; a push
   rejected because origin moved (the race in [`sync`](#6-sync) step 8) prints
   `warning: origin has new task commits; run: yman sync`; any other failure,
   a push origin refused included, prints git's stderr and then
   `warning: autosync failed: <why>; run: yman sync`. None of these
   change the exit code: the change is already committed.

Mutating commands, for the purposes of step 2: `add`, `edit`, `set`, `start`,
`done`, `move`, `cancel`, `reopen`, `prio`, `rm`, `attach`, `detach`, `comment`,
`tags rename` and `tags rm`. Bare `yman tags` is a listing and is not
mutating. `sync` is excluded because it handles `MERGE_HEAD` itself. The
same list decides when step 4 runs.

`guide`, `completions` and `man` run none of these steps. They are
documentation compiled into the binary and are answered before the repository
is looked for, so they work from any directory. `merge-driver` skips all
of them as well: git calls it from inside a merge with three temp files, and
preflight would refuse on the `MERGE_HEAD` that is always present then.

## 2. Commit messages

Every mutation commits immediately. The user's own hooks (possibly under
`core.hooksPath`, and knowing nothing about `.yman`) cannot interfere: every
`git` that `yman` runs in `.yman` gets `-c core.hooksPath=/dev/null`, so no
hook is found for its commits, merges and fast-forwards — `post-commit`,
`post-merge`, `post-checkout` and `post-rewrite` included, which `--no-verify`
(also passed) would not stop. The `git worktree add` that creates `.yman` runs
from the main repo and fires `post-checkout` inside it, so that one call gets
the same override. A hook that writes files would otherwise leave `.yman`
dirty, and refresh would back off until the next snapshot committed the junk.
The main repo's runner keeps the user's hooks, so they — and [`yman hooks`](#hooks)
— still fire for the user's own git commands. The override covers hook files
only: hooks a newer git defines in configuration (`hook.<name>.command`) do
not look in `core.hooksPath` and still run. GPG signing is deliberately left
to the user's configuration.

| Command | Subject |
|---|---|
| `init` | `yman: init` (empty root commit, only when `refs/yman/local` does not exist yet), then `yman: init (<scheme>)` |
| `add` | `task({id}): add "{title}"` |
| `edit` | `task({id}): edit` |
| `set`, `start`, `done`, `move`, `cancel`, `reopen`, `prio` | `task({id}): set {pairs}` |
| `rm` | `task({id}): remove "{title}"` |
| `attach` | `task({id}): attach {name}[, {name}…]` |
| `detach` | `task({id}): detach {name}` |
| `comment` | `task({id}): comment` |
| `tags rename` | `yman: tags rename {old} -> {new} ({n} tasks)` |
| `tags rm` | `yman: tags remove {tag} ({n} tasks)` |

In both `tags` subjects `{n} tasks` is `1 task` when there is exactly one.
| `sync` snapshot | `yman: snapshot local changes` |
| `sync` renumber | `yman: renumber 2->3, 7->8 (sync collision)` |
| `sync` merge | `yman: merge origin refs/tasks/main` |
| `sync` rejoin | `yman: rejoin files left under a moved folder` |

`{title}` has `"` replaced by `'`. `{pairs}` is space-joined, e.g.
`status=todo->doing priority=5->2 title tags=+ui,-auth comment` — a changed
title contributes the bare word `title`, a move with no field change
contributes `folder`, list fields contribute `+added,-removed`, a changed
body contributes the bare word `body`, and `-m` contributes the bare word
`comment`.

`Git::commit` treats "nothing to commit" as success: re-applying an identical
change inside the same second stages nothing, and the tree already says what the
caller asked for.

## 3. Reading

### `ls`

Touches no git itself; only the lazy refresh of §1 may. Filters, then sorts by
`(priority, status index, id)`, where the status index is the position in
`config.statuses.list`. Ids order as plain numbers first (numerically), then
`prefix-N` ids by prefix and numeric tail, then everything else lexically.

Default filtering hides tasks in any **terminal** status — `statuses.terminal`,
or the done status when that key is unset; `-a` includes them, and naming one
with `-s` includes it too. `-t` requires **all** the tags given, compared
lowercase on both sides — the tag yman writes is lowercase anyway, and one
written into `m.yml` by hand is still found. An invalid tag at `-t` is the same
error `add` gives.
`--assignee WHO` keeps one assignee (`-` keeps the unassigned), `-p N` one
priority, `--related ID` the tasks whose own `related` list carries that id
(exactly as `--relate` wrote it; an id nothing relates to is an empty listing,
not a warning), and `-q TEXT` the tasks whose title or body contains the text,
compared lowercase; all of these AND together with the status and tag
filters. `-n N` keeps the first N rows **after** sorting. None of the filters
touches git or reads anything `ls` did not already read. Broken folders are
listed regardless of filters. Column headers are printed only when stdout is
a terminal. Trailing empty
columns are omitted, and zero counts render blank rather than `0`.

`-l` prints each task's body under its row, every line indented by four spaces
and blank lines left empty, so a row is still the only line starting in column
0. A task with no body gets no extra lines. The body is the one `ls` already
read for `-q`, so `-l` costs no git either.

`--json` emits one object per task —
`{id, priority, status, title, tags, assignee, links, related, created, updated, attachments, comments, dir}` —
and `{dir, error}` for broken folders. With `-l` each task object also carries
`body`, the same trimmed text `show --json` gives; without it the key is
absent. `dir` is relative to `.yman` and names the status directory for a closed task (`done/5.1.fix-login`). `attachments` and
`comments` are counts here, not lists, and the attachment count comes from
`m.yml` alone: an entry whose file has been deleted outside yman still counts,
because `ls` stats nothing under `f/`. Use `show` to see which one is gone. The
writer is `src/json.rs`: the escaping is hand-rolled and there is no
`serde_json` dependency.

### `show`

Prints the header block, then the body, then `attachments` and `discussion`.
Empty sections are omitted entirely, including `links`, `related` and
`related by`.

`related by:` is the other end of the one-way `related` field: the ids of
every task whose `related` carries this one, in id order, closed tasks
included. It is computed by reading every `m.yml` in the tracker — the one
place `show` pays for a full scan — and is never stored. A broken folder
cannot relate to anything and is skipped, as in `rm`.

An attachment whose file is gone — deleted outside yman, since `detach` takes
the entry with it — is still listed, with ` (missing)` after the
`(added … by …)` parenthesis. The test is a plain existence check on
`f/<name>`, one per attachment and only in `show`; it never errors and never
rewrites `m.yml`. `detach` drops such an entry as usual (§4).

The discussion is shown in timestamp order, not file order: `merge=union`
places the other side's lines after ours in a conflicting hunk, so two
comments written concurrently on two clones can sit in `d.md` out of time
order. Only comments whose timestamp parses as RFC 3339 are reordered, among
their own positions; a chunk `d.md` could not parse, and a comment with an
unreadable timestamp, keep their place in the file. Comments from the same
second keep file order. The file itself is never rewritten.

`-n N` keeps only the last N discussion entries after that ordering (a chunk
`d.md` could not parse counts as one) under the header `discussion (last N of M):`; `-n 0`
drops the section. When N is not smaller than the number of entries the
output is identical to the default, header included.

`--json` emits one object —
`{id, priority, status, title, tags, assignee, links, related, related_by,
created, updated, dir, body, attachments, discussion, discussion_total}`. `attachments` is
`[{name, added, by, missing}]`, where `missing` is the same existence check the
text form marks, always present as `true` or `false`; `discussion` is
`[{ts, author, text}]`, with an
unparsable chunk appearing as `{raw}`; `discussion_total` is the count before
`-n` trimmed anything. Unlike the text form nothing is omitted: an empty
section is an empty array and an absent assignee is `null`, so a script never
has to branch on a missing key. `dir` is relative to `.yman`, as in `ls --json`,
where the text form's `folder:` line is relative to the repository root
(`.yman/2.1.fix-login`).

### `plan`

`plan <id>` reads a plan in one call: the task, how far its steps have got,
and what is left. A step is any task whose `related` carries the id — the set
`ls --related <id>` filters and `show`'s `related by:` lists. It is built from
the same single read of every `m.yml` that `show` makes; broken folders are
skipped.

    30  Ship sync v2
    status: doing   steps: 7   todo 2, doing 1, blocked 1, done 3
    2  32  todo     Rewrite merge driver
    3  34  blocked  Drop old ref            waits on 32

The second line counts **every** step, closed ones included, per status in
`statuses.list` order; a status the config does not list comes last, and a
zero count is left out. With no steps it reads `steps: 0` and no rows follow.
Rows are the open steps, sorted as `ls` sorts (`P ID STATUS TITLE`, no header
line); `-a` adds the closed ones and `-n N` keeps the first N after sorting.
A step in status `blocked` gets a last column, `waits on <ids>`: the tasks in
its own `related` that exist here, are open, and are not tagged `epic` — so
the epic a step belongs to never reads as something it waits on. `-l` prints
each step's body under its row, indented as in `ls -l`.

`--json` emits one object —
`{id, priority, status, title, counts, steps}`, where `counts` maps each
non-zero status to its number (`{}` when there are no steps) and `steps` is
`[{id, priority, status, title, tags, assignee, waits_on, dir}]` in row order;
`waits_on` is `[]` for a step that is not `blocked`, and `-l` adds `body`.
An unknown id is exit 4, as everywhere.

### `status`

Reports and never changes anything: the local ref and its short head, the
remote ref with `ahead`/`behind` when it has been fetched, the refresh policy,
whether the hooks are installed, the number of uncommitted changes under
`.yman`, an unresolved merge with its unmerged files, and the task counts per
configured status.

`--json` emits one object —
`{local: {ref, head}, remote: {ref, fetched, head, ahead, behind, origin}, refresh,
hooks, worktree: {dirty}, merge: {in_progress, unmerged}, tasks: {by_status,
other, broken}}`. Before the first fetch `fetched` is `false`, `head` is `null`
and both counters are `0`. `origin` is `false` for a local-only tracker, whose
text form prints `remote: none (no "origin"; tasks are local only)` instead of
`not fetched yet`. The per-status counts are nested under `by_status`
so a status named `other` or `broken` cannot collide with the two totals beside
it.

### `tags`

An inventory of the tags in use: one row per tag with the number of tasks
carrying it, sorted by name. Touches no git, and counts **every** task on disk
— closed ones included, unlike `ls`, because the question is what tags exist,
not what is open. Tags are folded to lowercase before counting, so a hand-written
`UI` lands in the same row as `ui`, and a task that spells one tag two ways
counts once. Column headers follow the `ls` rule: terminal only.

    TAG   N
    auth  1
    ui    3

`--json` emits `[{tag, tasks}]` in the same order. Folders that would not load
are skipped with `warning: skipped <n> unreadable task folder(s): <rels>` on
stderr; `ls` is the command that lists them properly.

`tags rename` and `tags rm` edit the vocabulary itself; they are under
[§4](#4-writing).

### `path`

Prints the absolute folder path and nothing else, so `cd $(yman path 14)` works.
The lazy refresh still runs, silently.

### `log`

`-n` defaults to 20. With an id, the log is limited to the pathspecs
`:(glob)[0-9].<id>.*/**` and `:(glob)*/[0-9].<id>.*/**`. The id is in the folder
name through a priority change, a retitle and a move into or out of a status
directory, so those keep the task's full history without consulting git's
rename detection, and `-n` stops the walk once enough commits are found.

A sync renumber is the one move that changes the id, and `log` reads it from
the commit subject (`yman: renumber 2->3, …`):

- For a task that *arrived* at its id by a renumber, the log continues from
  the renumber's parent under the old id, and `-n` counts across both.
- For a task that *kept* an id another task was renumbered away from, every
  such renumber commit is excluded with `^<commit>`, together with its
  ancestry: the moved task was not in the merge base, so all it did under the
  id is behind the renumber, and none of the kept task's history is. An
  excluded revision makes git walk the whole range before `-n` applies.

### `guide`

Prints [agents.md](agents.md) — the short manual for scripts and agents — to
stdout, byte for byte, via `include_str!`. Needs no repository. The top-level
`yman --help` ends by pointing at it; the per-command help pages do not.

### `completions`

`yman completions <bash|zsh|fish|elvish|powershell>` prints a completion
script for that shell to stdout, generated by `clap_complete` from the same
`src/cli.rs` definition the binary parses with, so it cannot drift from the
CLI. The hidden `merge-driver` is left out. An unknown shell is a usage error,
exit 2. Needs no repository.

### `man`

`yman man` prints the `yman(1)` page in roff to stdout, generated by
`clap_mangen` from the same definition; its SUBCOMMANDS section refers to
`yman-<name>(1)`. `yman man --dir <DIR>` creates `DIR` if needed and writes
`yman.1` plus one page per visible subcommand, nested ones included
(`yman-tags-rename.1`), and prints nothing. Needs no repository.

## 4. Writing

### `init`

Creates the tracker: the ref, the `.yman` worktree, the repository
configuration and the first commits. Its flags, what it changes and the
local-only case are specified in [setup.md §2](setup.md#2-yman-init) and
[setup.md §3](setup.md#3-what-init-changes).

### `add`

Mints an id (see [storage.md §8](storage.md#8-id-schemes)), validates the status
against the config, normalizes the tags, dedupes them together with the
`--link`s and `--relate`d ids while keeping their order, then writes `t.md` and
`m.yml` with `created == updated`. A tag is trimmed, lowercased, and refused
when it is empty or carries whitespace, a comma or a control character — the
comma separates the `ls` TAGS column and the flow sequence `m.yml` accepts, and
whitespace makes `-t` unusable without quoting. Tags are checked before the
editor opens, before `--body-file` reads stdin and before an id is minted, so a
typo costs neither an id nor a half-written folder. `-t UI -t ui` is one tag.
The title is trimmed and refused, at the same point, when it is empty
(`title must not be empty`) or still contains a control character — a newline,
a carriage return, a tab, an escape, anything `char::is_control` matches — with
`invalid title "<t>"; titles must not contain line breaks or other control
characters`, `<t>` being the trimmed title with those characters escaped
(`\n`, `\t`, `\u{1b}`). A newline would otherwise leave `t.md` with a one-line
title and the rest as body, and split the commit subject. `set --title` and
every `--sections` heading apply the same rule with the same message.
`-a/--assignee` is trimmed; blank means unassigned. `--relate` and
`--waits-on` values are trimmed and a blank one is refused (`related id must
not be empty`); `set` also refuses the task's own id. `--relate` to an id no
folder here carries warns exactly as `set --relate` does, before the id is
minted. `--waits-on ID` (repeatable) is `--relate ID` plus `-s blocked`, and
excludes `-s`; it refuses, before anything else is read, when `blocked` is not
in `statuses.list` (see `set`). The body comes from `-m`,
or from `--body-file PATH` (`-` reads stdin); the two exclude each other and
`-e`. With `-e`, the editor opens before the
first commit, so a title typed there renames the folder by plain rename — the
placeholder never enters git history. If the editor cannot be started, exits
non-zero or is killed, or leaves a `t.md` that no longer parses or whose title
the rule above refuses, the new folder is removed and nothing is committed:
`editor exited with status N; task not added`, or
`t.md invalid after edit: <why>; task not added`. The id is not
spent, and what was typed in the editor is lost with the folder.

`--sections PATH` (`-` reads stdin) creates one task per section of a markdown
file instead of one from a title; it excludes the title, `-m`, `--body-file`
and `-e`, and every other flag applies to each task. A line that is `#` or
starts with `# ` opens a section and is its title, trimmed; the body is
everything up to the next such line, with blank lines at either end dropped.
Inside a fenced block — opened by a line starting, after indentation, with
three or more backticks or `~`, closed only by a run of the same character at
least as long with nothing after it — a `# ` line is body text, and
`##` and deeper headings are always body text. CRLF reads like LF. The whole
file is read and parsed before the first id is minted, and these refuse it,
`<source>` being the path or `stdin`:

- `<source>: line <n>: text before the first "# " heading` — anything but
  blank lines above the first heading;
- `<source>: line <n>: empty title`;
- `<source>: no "# " heading, so no tasks`;
- `<source>: line <n>: unclosed code fence` — `<n>` is the opening line.

Then every title is checked against the control-character rule above, still
before the first id is minted, so a bad heading anywhere in the file creates
no task at all; the refusal names the title, not a line.

Tasks are then created in file order, one commit and one `added <id>  <dir>`
line each, exactly as separate `add` calls would make them, except that the
history is walked for taken ids once for the whole file rather than per task; the first failure
stops the run with the tasks before it committed. Under the `seq` scheme ids
follow file order, so a plan listed with equal priorities reads in that order;
under the other schemes, order steps with `prio` afterwards.

### `edit`

Opens `$VISUAL`, else `$EDITOR`, else `vi`, split on whitespace so
`EDITOR="code --wait"` works. The `vi` fallback is taken only when stdin is a
terminal; otherwise the command fails at once with
`no terminal for vi; set $EDITOR, or use -m / --body-file` rather than leaving
a `vi` waiting on a pipe. The same rule covers `add -e` and `comment -e`. A
non-zero editor exit aborts and leaves the file alone (under `add -e` it
removes the new folder instead, under `comment -e` its temp file; see `add`
and `comment`). If the file no longer parses, or
its title carries a control character (the rule under `add`), the command fails
with `t.md invalid after edit: <why>; fix the file then run: yman edit <id>`,
commits nothing and **does not revert the user's text**; the next `sync`
snapshots it. When nothing changed, it prints
`no changes` and commits nothing. A changed title re-slugs the folder with
`git mv`.

### `set` (and `start`, `done`, `move`, `cancel`, `reopen`, `prio`)

At least one flag is required, enforced by the argument parser. Changes are
applied in memory first, then:

1. A changed priority, slug, **or terminal boundary** renames or moves the
   folder with `git mv`. Crossing the boundary also prints
   `note: task folder is now <rel>` on stderr — stdout stays data, but someone
   who had `cd`'d into the folder needs to hear that it moved.
2. `updated` is touched; `m.yml` is rewritten; `t.md` too when the title or
   the body moved. With `-m`, the text is appended to `d.md` exactly as
   `comment` would, with the same actor.
3. One commit, one printed line per change (`14: status todo -> doing`,
   `14: commented`).

List fields (`--tag/--untag`, `--link/--unlink`, `--relate/--unrelate`) are set
semantics with insertion order preserved: adding a value already present is not
a change, and removing one that was never there is quietly accepted. Both
`--tag` and `--untag` normalize their values the way `add` does, and both refuse
an invalid one — nothing yman wrote can look like that, so such a value is a
typo rather than something waiting to be removed. `--title` is trimmed and
refused exactly as `add` refuses a title — empty, or carrying a control
character — before anything is written. `-a/--assignee` is trimmed the same
way `add` trims it, and a blank value clears the assignee like
`--no-assignee`. When
nothing at all changed, `set` prints `no changes` and commits nothing. `-m`
always counts as a change; an empty message is the `empty comment` error.

`--relate` to an id no folder here carries prints
`warning: task <id> does not exist here; relating anyway` on stderr and commits
regardless. It is not a refusal because a concurrent `add` on another clone can
legitimately race it, and the id becomes real the moment that clone's work
arrives. Re-adding an id the task already relates to is not a change and warns
about nothing.

`--waits-on ID` (repeatable) says the task waits on another: it is `--relate
ID` plus `--status blocked`, in the same commit and with the same printed
lines, and the argument parser refuses it together with `--status`. `blocked`
is a status name, not a config role; when `statuses.list` lacks it the flag
fails with `--waits-on needs a "blocked" status; add it to statuses.list in
.yman/config.toml` before anything is written.

`related` is **one-way**: relating 1 to 2 says nothing about 2. yman maintains
the field in exactly three places — here, the renumber rewrite in §6, and `rm`
below — and nowhere else. In particular, an id written into the prose of `t.md`
or `d.md` is text; it is never checked and never rewritten.

`--body TEXT` and `--body-file PATH` (`-` for stdin) replace the body of
`t.md`; they exclude each other. The comparison ignores leading and trailing
newlines, as `t.md` is rendered that way, so re-applying the same text is
`no changes`. `--body ""` clears the body. The line is `<id>: body updated`
and the token `body`; a body change alone never renames the folder. A source
that cannot be read fails with `cannot read <path>: <why>` before anything is
written.

`start` and `done` resolve to `statuses.start` and `statuses.done`, falling back
to `list[1]` and `list.last()` when those are unset.

The folder's location is recomputed on every `set`, before the "nothing
changed" exit. Setting a task's status to the one it already has therefore
relocates a task that is in the wrong place — a hand edit, a resolved merge, or
a repository that raised its version with closed tasks already on disk — and
reports it as `folder <old> -> <new>`, with `folder` as the commit-subject
token. There is no separate repair command.

The remaining verbs are `set --status` with the status looked up for you:

| Command | Status | When it refuses |
|---|---|---|
| `move <id>... <status>` | the one you name | unknown status, as for `set` |
| `cancel <id>...` | `statuses.cancel` | the key is unset |
| `reopen <id>...` | `statuses.default` | the task is not in a terminal status |

Every verb, and `prio <id>... <0-9>`, takes several ids and `-m <text>` like
`set`. Ids are processed in the order given, one commit each, printing the
same lines `set` would. The first failure stops the run with that error; the
tasks before it are already committed, and what their closes freed is reported
(below) before the error. `set`, `edit`, `show`, `path` and `rm`
take exactly one id.

`cancel` has no fallback on purpose: picking one of several closed statuses by
position is the guesswork the named roles exist to remove.

**What a close changed.** When `set` or a verb moves a task from an open
status into a terminal one, yman reads every task once more and reports on
stderr, after the last id of the call (so `done 32 33` judges a task waiting
on both after both closed) — or, when an id fails, after the last one that
completed (`done 32 99` reports what closing 32 freed, then fails on 99).
Exit status and stdout are unaffected.

- A closed task tagged `epic` that open tasks still relate to gets
  `warning: <id> closed with open tasks relating to it: <ids>` (id order).
  The close is already committed; the warning is there so a plan is not
  closed by accident with work left. Closing it frees nothing: its steps
  relate to it as a container, not a blocker.
- Otherwise, a task in status `blocked` whose `related` carries a closed id,
  and which now relates to nothing open except tasks tagged `epic`, gets
  `note: <id> no longer waits on anything open: yman move <id> <default
  status>`, one per task in id order. Nothing is moved.

A follow-up that relates to an ordinary task is not a step and is never
reported. A `set` that changes nothing, or that leaves the task closed,
reports nothing.

### `tags rename` / `tags rm`

The two edits that only make sense across every task at once. Both normalize
their arguments the way `add` does, match stored tags folded, and touch only
`m.yml` — a tag never names the folder, so nothing is renamed or moved.

`tags rename <old> <new>` replaces `old` in place, keeping its position in the
list. A task already carrying `new` loses `old` rather than gaining a duplicate,
so its line reads `<id>: tags -<old>` where the others read
`<id>: tags +<new> -<old>`. `old` and `new` that normalize to the same value is
`no changes`.

`tags rm <tag>` drops it. On a terminal it prompts
`remove tag "<tag>" from <n> tasks? [y/N] ` (`1 task` for one) on stdout — asked only once the count is
known, since confirming a removal without knowing it touches forty tasks is not
consent. Anything but `y`/`Y` aborts. Without a terminal and without `-f` it
refuses with `refusing to remove without -f`, the same wording as `rm`.

Both are **one** commit for every task they touch: renaming a tag is a single
decision, and a half-applied rename would leave the vocabulary in a state
nobody chose. No match is `no changes` and no commit, as in `set`. A folder
that does not load cannot be rewritten either, so it is reported on stderr with
the same `warning: skipped …` the listing prints rather than silently left out
of the count.

### `rm`

On a terminal, prompts `remove task 14 "Fix login"? [y/N]`; anything but `y`/`Y`
aborts. Without a terminal and without `-f`, it refuses rather than assume
consent.

Every other task's `related` loses the removed id in the **same commit** as the
removal, so a task that is gone and a reference still naming it are never two
states of the repository anyone can observe. The count goes to stderr as
`note: dropped N reference(s) to <id>`, and is omitted when there was nothing to
drop. A folder that does not load cannot be rewritten, so it is reported with
the same `warning: skipped N unreadable task folder(s): …` the listing prints —
a reference inside one survives the removal.

Removing an open task that is not tagged `epic` frees its waiters the way
closing it would: after `removed <id>`, a task in status `blocked` that lost
the reference and now relates to nothing open except epics gets
`note: <id> no longer waits on anything open: yman move <id> <default status>`
(see "What a close changed" under `set`). Nothing is moved. Removing a closed
task or an epic frees nothing, as closing one does not.

### `attach` / `detach`

Each source must be an existing, readable regular file. `--name` applies to a
single file only, and must not contain a path separator. Every name, given or
taken from the source, must also be portable: no `:` `*` `?` `"` `<` `>` `|`
or control character, no trailing `.` or space, not empty — the names a
Windows checkout cannot hold. Names compare ignoring case throughout, since
`a.png` and `A.png` are one file on macOS and Windows. Two sources that would
land under the same name are refused with `attachment "<name>" given more than
once`, `--force` or not. Every source is checked before any is copied, so a
refusal leaves nothing behind: no file under `f/`, no `m.yml` entry, no
commit. The `by` recorded in `m.yml` is
the actor (see `comment`). An existing attachment of the same
name needs `--force` — an `m.yml` entry, even one whose file is gone, or a file
under `f/`. With `--force` the entry is replaced in place, and a file whose
name differs only in case is removed, so the task keeps one of each. Files over 5 MiB produce a warning, never a refusal. An
attachment listed in `m.yml` whose file is already gone can still be detached —
the entry is simply dropped. Each copied file is staged with `git add -f`, so a
name matching `.yman/.gitignore` (`*.swp`, `*~`, `.#*`, `*.orig`) is committed
like any other: naming a file to attach overrides the ignore rule.

### `comment`

Text comes from `-m`, or from `$EDITOR` with `-e`, or — when neither is given
and stdin is not a terminal — from stdin, read until end of file. That last
form waits for EOF: a caller whose stdin is an open pipe it never closes (a
harness, a CI step) blocks, so a non-interactive caller passes `-m`, or gives
stdin a definite end (`< note.md`, a pipe from a command that exits,
`< /dev/null`, which is then an empty comment). At a terminal with neither
flag the command fails with `empty comment` without reading. Empty after
trimming is an error.

Under `-e` the editor is resolved first (the rule under `edit`), then handed a
new, empty file in the temp dir (on unix `$TMPDIR`, else `/tmp`) named
`yman-comment-<id>-<pid>-<nanos>-<n>.md`. It is created exclusively, mode 0600
on unix: an existing path, a symlink included, is never opened, truncated or
followed — another name is tried instead. The file is removed on every path:
after the comment is committed, when the editor fails
(`editor exited with status N; comment not added`), and when the text is
empty. A refused editor creates no file at all. The author is `$YMAN_ACTOR` (trimmed,
non-empty), else `git config user.name`, else `unknown`. This is the display
name written into `d.md` only; the git committer is whatever git resolves.

## 5. Refresh

`refresh(quiet)` never touches the network. In order:

1. Either ref missing, or both equal → nothing to do.
2. `LOCAL` is not an ancestor of `REMOTE` → skip. Only `yman refresh`
   reports it, as `note: local has unpushed commits; run: yman sync`; the lazy
   path stays silent. `yman status` shows the same situation as
   `ahead N, behind M` with `→ run: yman sync`.
3. The worktree is dirty → skip. Only `yman refresh` reports it, as
   `note: .yman has uncommitted changes, refresh skipped`; the lazy path
   stays silent.
4. Otherwise fast-forward (`merge --ff-only`) and report
   `refreshed: N new commit(s)` unless quiet.

`yman refresh` additionally prints `up to date` when there was nothing to do.

| `yman.refresh` | Effect |
|---|---|
| `lazy` (default) | every non-exempt command fast-forwards first |
| `manual` | only `yman refresh` and `yman sync` move `.yman` |

On the lazy path the policy is consulted between steps 2 and 3, not before
step 1: it can only change the outcome for a repository that is actually
behind, and reading it costs a `git` process. A repository that is up to date
or holds unpushed commits reaches the same answer without it.

### Hooks

`yman hooks install` writes `post-merge` and `post-checkout`, each carrying the
marker line `# yman-hook v1`:

```sh
#!/bin/sh
# yman-hook v1
command -v yman >/dev/null 2>&1 && yman refresh --quiet
exit 0
```

The target directory is `core.hooksPath` when set, as git resolves it with
`rev-parse --git-path hooks` (`~/` expanded, a relative value relative to the
toplevel), and the install warning names the resolved path; otherwise
`COMMON/hooks`. An empty `core.hooksPath` counts as unset. A hook that exists
**without** the marker is never touched: the command prints the one line to
add and exits non-zero after processing both hooks. A hook yman cannot read —
permission denied, content that is not UTF-8, a dangling symlink — is refused
the same way, since it cannot tell whose it is: `install` prints
`hook <name>: cannot read <path>: <why>; not replacing it` and counts it among
the hooks not installed, and nothing is written through a dangling link. A
symlink to a readable file is judged by its target. `remove` deletes only
marked files (a marked symlink loses the link, not its target); `status`
reports `installed` / `foreign` / `unreadable` / `absent`, and both say why a
hook is `unreadable` with `warning: cannot read <path>: <why>` on stderr. The hooks fire for the user's own `git
pull` and `git checkout` in the project, never for yman's own merges in
`.yman`, which run without hooks (see [§2](#2-commit-messages)).

## 6. Sync

One of two commands that use the network; the other is `init`, which fetches
and pushes unless given `--offline` (see [setup.md §2](setup.md#2-yman-init)).
Preflight runs with `mutating = false`;
`sync` inspects `MERGE_HEAD` itself. Except for `--abort`, it first requires an
`origin`: a local-only tracker (see [setup.md §2](setup.md#2-yman-init)) fails
with `no "origin" remote; tasks are local only. Connect one: yman init --remote <url>`,
exit 1.

### Normal path

1. A merge already in progress → exit 3.
2. A dirty worktree is committed as `yman: snapshot local changes`, reporting
   `snapshotted N local change(s)`.
3. Fetch `+refs/tasks/main:refs/yman/remote`. A stderr mentioning
   `couldn't find remote ref` means origin simply has no tasks yet; if we had a
   `REMOTE` before, it is deleted and the disappearance is reported. Any other
   failure is fatal, with git's stderr passed through.
4. No `REMOTE` → straight to the push.
5. No merge base → fatal: the histories are unrelated, and the message spells
   out the re-init recipe.
6. `base == remote` → nothing to merge. `base == local` → `merge --ff-only`.
7. Otherwise **renumber collisions** (below), then
   `merge --no-edit --no-verify` with `merge.directoryRenames=true`. `m.yml`
   goes through the field-wise merge driver
   ([storage.md](storage.md#merging-myml)), so edits to different fields of one
   task settle on their own; anything it cannot settle falls back to the text
   merge. Conflicts print the unmerged files and exit 3. A clean merge is
   followed by the **rejoin** below.
8. Push, unless `--no-push`, with `--porcelain`, and read git's verdict from
   the `refs/tasks/main` line on stdout. Four reasons on a `!` line mean
   origin moved after our fetch — the race: `[rejected] (fetch first)`,
   `[rejected] (non-fast-forward)`, and the two ways receive-pack's own
   compare-and-swap fails because the ref changed between its advertisement
   and its update, `[remote rejected] (incorrect old value provided)` and, for
   a push that creates the ref, `[remote rejected] (reference already
   exists)`. Older git reports both as `failed to update ref`. A race loops back
   to step 3, at most three attempts, then fails with `origin keeps moving`.
   Any other `!` — `pre-receive hook declined`, a protected or denied ref, and
   the ambiguous `failed to update ref` and `failed to lock`, which can mean a
   moved ref but also a server-side fault — is a refusal: it is not retried,
   and fails with `push failed` after git's stderr and its
   ` ! [remote rejected] refs/yman/local -> refs/tasks/main (<reason>)` line.
9. Report `synced  pulled N, pushed N, renumbered N   refs/yman/local @ <sha>`.

After a successful push, `REMOTE` is set to `LOCAL`, so `status` and `refresh`
agree with what origin now holds.

### Collision renumbering

`seq` and `author` mint the same id on two machines working offline. The side
that is behind moves, because the other side's id is already published.

- Candidates are ids of tasks **created locally since the merge base** that also
  exist on the remote. All three sets are read from folder names in
  `ls-tree -d -r` — of the base, of `LOCAL` and of `REMOTE` — and an id created
  locally is one `LOCAL` has that the base does not. Nothing here consults
  git's rename detection: adding a comment or an attachment to an existing task
  does not mint an id, and neither does a retitle or a close, both of which move
  the folder while the id stays in its name.
- The replacement comes from the same scheme, avoiding everything on disk, on
  the remote, and everything ever assigned on either ref. For `author`, the
  original prefix is kept.
- Each rename is a `git mv`, the task's `updated` is touched, and one commit
  covers the batch.
- Each move prints `renumbered {old} -> {new}  (id taken on origin)`.
- `related` references to the old id are rewritten to the new one, in the
  same commit as the moves, and `note: rewrote N reference(s) to renumbered
  ids` reports how many changed. A folder that does not load is left alone.
  The rewrite covers the renumbering clone's own tree, which is the pre-merge
  one, and that is every reference that can mean the moved task: a candidate
  is absent from the merge base, so it never reached origin and no other
  clone has seen it. A reference on the remote side, including one arriving
  in the same merge, names the remote's task, which keeps its id.

### Rejoining a moved folder

A retitle or a close moves a task's folder, and a file the other clone added
under the old name belongs in the new one. `merge.directoryRenames=true` has
git move it — but only when something was also added directly in that folder,
so a first comment's `d.md` follows and an attachment on its own, `f/<name>`,
does not. The merge then succeeds with the task split across two folders.

So after the merge, any folder that shares its id with exactly one folder
carrying an `m.yml`, and has none itself, is emptied into that folder with
`git mv`, printing `note: moved N file(s) left under <old> into <rel>`. A file
whose destination already exists stays put, with
`warning: <path> not moved; <dest> already exists`. After a clean merge the
moves are committed as `yman: rejoin files left under a moved folder`; under
`--continue` they go into the merge commit, ahead of its checks.

### `--continue` and `--abort`

Both require a merge in progress.

`--continue` treats a file as unresolved while it still carries conflict
markers — staging is yman's job, since the message tells the user to edit and
rerun, not to run `git add`. The exception is a conflict git never marked up:
a binary file, a change on one side to what the other deleted, or an `m.yml`
whose merge driver could not run. The file on disk is then simply ours, and
taking it would drop theirs. When the merge stops, yman records in
`WT_GITDIR/YMAN_MARKED` which unmerged files carried markers; an unmerged file
that has both sides' versions (or a base and one side) and is not on that list
stays unmerged until the user stages a version, and `--continue` says so:
`still unmerged: <files>; no markers to remove in <files>, stage the version to keep: yman git checkout --ours|--theirs -- <file> && yman git add <file>`.
`sync` prints git's stderr from the failed merge, so a driver's
`No such file or directory` is not lost. It then checks that `config.toml` loads, that
every task folder the merge touched still loads and is marker-free, that no
id has two folders, stages
everything, commits with `--no-edit`, and pushes.

The folder list comes from both `HEAD..MERGE_HEAD` and the unmerged paths,
because a rename conflict names paths that survive in neither tree.

### A task closed to two different statuses

Closing one task to `done` on one clone and `cancelled` on another renames it
two ways, and git resolves that by keeping **both** folders. Both load, and
neither holds a conflict marker, so every other check here passes — the
duplicate-id check is the only thing between that state and a pushed task no
command can touch afterwards. It reports

```
duplicate task id 1: cancelled/5.1.fix-login, done/5.1.fix-login; delete one folder, then: yman sync --continue
```

with exit 3, and `sync` names the same pair on stderr when the merge fails:

```
note: task 1 was closed to two different statuses; keep one of done/5.1.fix-login, cancelled/5.1.fix-login
```

Delete the folder you do not want, then rerun `yman sync --continue`.

`--abort` runs `git merge --abort` and reports `merge aborted`.

## 7. `git` passthrough

`yman git -- <args>` runs git inside `.yman` with inherited stdio and
propagates the exit code. It deliberately bypasses the `Git` wrapper, so the
user's own flags — including colour — behave normally, and so do the user's
hooks: unlike yman's own calls, a `yman git commit` fires `post-commit`.
