# yman for scripts and agents: the short manual, for a reader that pays per token

## Paste into the project's CLAUDE.md

    yman ls -n 10 --assignee -                        # pick: unassigned, best priority first
    yman show <id> -n 3                               # read: header, body, last 3 comments
    yman set <id> --status doing -a <me> -m "on it"   # claim: status, assignee, note, one commit
    yman done <id> -m "what changed"                  # close and explain in one call
    yman add "Title" -m "body" -a <me> --relate <id>  # follow-up, linked to its parent
    Never `edit` or `-e` (opens an editor); change a body with `yman set <id> --body "..."`.
    `rm` and `tags rm` need `-f` off a terminal, or refuse: `refusing to remove without -f`.
    Task list asked for: `yman ls -l -n 20`, render as a table (P ID Status Title Tags),
    `show <id> -n 0` the best 1-3 (it has the assignee; `ls` does not), recommend one.

## Reading

`ls` prints one row per task, header only on a terminal; `-l` adds the bodies:

    P  ID  STATUS  TITLE  TAGS  F  C

`P` is priority 0 (highest) to 9, `F` attachments, `C` comments; zero is
blank and trailing empty columns are dropped. Filters AND together, `-t` and
`-q` comparing lowercase: `-s <status>` `-t <tag>` `--assignee <who|->`
`-p <0-9>` `-q <text>`; `-n <N>` caps rows after sorting; closed tasks need `-a`
or `-s <closed status>`.

`show <id>` prints a header, the body, then `attachments:` and `discussion:`;
empty sections are omitted, `-n <N>` keeps the last N.
`tags` lists every tag in use with its task count, closed tasks included. `ls`,
`show`, `status` and `tags` take `--json`, which omits nothing: `[]` and `null`.

## Writing

`start`, `done`, `cancel`, `reopen`, `move <id>... <status>` and
`prio <id>... <0-9>` take several ids, one commit each, and `-m <text>` to
comment in the same commit. `set <id>` changes any field: `--status`
`--priority` `--title` `-a/--assignee` `--no-assignee` `--tag/--untag`
`--link/--unlink` `--relate/--unrelate` `--body <text>` `--body-file <path|->`
`-m <text>`. Every mutation is its own commit; nothing to save.

## Streams and exit codes

stdout carries data only. Errors (`error: ...`), warnings and notes go to stderr.

    0  ok
    1  error, stderr says what
    2  usage error, from the argument parser
    3  a sync merge is unresolved
    4  no task has that id

On 3: `yman status` names the unmerged files under `.yman/`; edit them, remove
the markers (or delete one of the folders a `duplicate task id` error names),
then `yman sync --continue` (or `--abort`). Until then mutating commands exit 3.

## Cost and attribution

Plain output is cheapest; `--json` repeats every key, so keep it for `jq`. Cap
with `-n` (`show <id> -n 0` drops the discussion); one verb with `-m` beats a
verb then `comment`. `YMAN_ACTOR=<name>` records comments and attachments as
yours; the git committer stays whoever git says it is.
