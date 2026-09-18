# yman for scripts and agents: the whole manual, for a reader that pays per byte

## Paste into the project's CLAUDE.md

    yman ls -n 10 --assignee -                        # pick: unassigned, best priority first
    yman show <id> -n 3                               # read: header, body, last 3 comments
    yman set <id> --status doing -a <me> -m "on it"   # claim: status, assignee, note, one commit
    yman done <id> -m "what changed"                  # close and explain in one call
    yman add "Title" -m "body" -a <me> --relate <id>  # follow-up, linked to its parent
    Never `edit` or `-e` (needs a terminal); change a body with `yman set <id> --body "..."`.

## Reading

`ls` prints one row per task, no header unless stdout is a terminal:

    P  ID  STATUS  TITLE  TAGS  F  C

`P` is priority 0 (highest) to 9, `F` attachments, `C` comments; zero is
blank and trailing empty columns are dropped. Filters AND together:
`-s <status>` `-t <tag>` `--assignee <who|->` `-p <0-9>` `-q <text>`, and
`-n <N>` caps the rows after sorting. Closed tasks need `-a`.

`show <id>` prints a header block, the body, then `attachments:` and
`discussion:`; empty sections are omitted. `-n <N>` keeps the last N entries.

## Writing

`start`, `done`, `cancel`, `reopen`, `move <id>... <status>` and
`prio <id>... <0-9>` take several ids, one commit each, and `-m <text>` to
comment in the same commit. `set <id>` changes any field: `--status`
`--priority` `--title` `-a/--assignee` `--no-assignee` `--tag/--untag`
`--link/--unlink` `--relate/--unrelate` `--body <text>` `--body-file <path|->`
`-m <text>`. Every mutation is its own commit; there is nothing to save.

## Streams and exit codes

stdout carries data only. Errors (`error: ...`), warnings and notes go to
stderr.

    0  ok
    1  error, stderr says what
    2  usage error, from the argument parser
    3  a sync merge is unresolved
    4  no task has that id

On 3: `yman status` names the unmerged files under `.yman/`; edit them, remove
the markers, then `yman sync --continue` (or `yman sync --abort`). Until then
every mutating command exits 3.

## Attribution

Set `YMAN_ACTOR=<name>` and comments and attachments are recorded as yours.
The git committer stays whoever git says it is.

## Cost

Plain output is the cheapest; `--json` repeats every key on every row, so use
it only when the output goes into `jq`. Cap with `-n`, and prefer one verb
with `-m` over a verb followed by `comment`.
