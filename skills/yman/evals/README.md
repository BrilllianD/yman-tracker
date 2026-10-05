# Evals for the `yman` skill

Each case builds a throwaway tracker, hands one prompt to a headless agent
(`claude -p`), and grades two things: what the agent did to the repository, and
which commands it reached for on the way. The point is not that the task gets
done — a capable model manages that without any skill — but that it is done the
non-interactive way the skill prescribes, in the number of calls it prescribes.

```sh
./run.sh                        # every case, one run each
./run.sh --case delete-task     # one case
./run.sh --runs 3               # agents are stochastic; three runs is a fairer read
./run.sh --no-skill             # ablation arm: identical cases, skill not copied in
./run.sh --keep                 # keep the fixtures and transcripts for inspection
```

Runs cost real money on the logged-in account — roughly $0.25 per case with the
skill loaded. `run.sh` prints the per-run cost and the total, and exits non-zero
if any case failed.

## The cases

| Case | The trap it sets |
| --- | --- |
| `body-rewrite` | `yman edit` is the obvious verb and it opens `$EDITOR`; the skill's answer is `set --body` / `--body-file -` |
| `claim-task` | three calls (`--status`, then `-a`, then `comment`) where one `set --status doing -a agent -m` is a single commit; and claiming without the `yman sync` that must come first |
| `close-task` | `yman done` closes the task locally and looks finished; the skill says to sync before the status change and again after it, so origin must carry the close |
| `delete-task` | `yman rm` without `-f` refuses off a terminal |
| `unassigned-query` | reading every task, or dumping `ls --json` into a pipeline, when `ls --assignee -` answers it |
| `plan-epic` | walking an epic's steps with `ls --related` and a `show` each, when `yman plan <id>` answers in one call; and naming the better-priority step that is blocked |
| `waits-on` | `set --relate` plus `set --status blocked`, two commits, where `set --waits-on` is one |
| `add-sections` | one `yman add` per section of a markdown file, retyping it, where `add --sections <file>` makes them all |
| `task-list` | the skill's task-list procedure: `ls -l`, a markdown table, `show` for at most three candidates |
| `exit-3-recovery` | a sync merge stopped on a conflict: `yman git merge --abort` or deleting `.yman` instead of fixing the file and running `yman sync --continue` |

Each case is a directory holding `prompt.md` (what the agent is asked) and
`grade.sh` (what makes it a pass). It may also hold `setup.sh`, run in the
working copy after the shared fixture with the run directory and the binary as
arguments (a version 2 config through `v2-config.sh`, a second clone, a file
to read), and `allowed-tools`, one extra `--allowedTools` entry per line
(`exit-3-recovery` adds `Edit` and `Write` so the agent can fix the
conflicted file). A grader sources `../../lib.sh` and reads
three variables: `WORK` (the agent's working copy), `TRANSCRIPT` (the
stream-json log) and `YMAN` (the binary under test). It calls `fail <reason>`
for each problem and ends with `verdict`. Lines it prints starting with `NOTE`
are not failures — they record a weaker-but-acceptable path, like claiming a
task in two calls instead of one.

## The fixture

`fixture.sh` makes a bare remote and a working copy in a tempdir, runs
`yman init --offline`, and seeds three tasks: a stale bug report, an unassigned
chore, and a task assigned to somebody else, so "unassigned" is a real filter.

`HOME` is deliberately left alone — the agent under test needs its own
credentials — so git is isolated with `GIT_CONFIG_GLOBAL`,
`GIT_CONFIG_NOSYSTEM` and a repo-local identity instead. `EDITOR` and `VISUAL`
are unset for the run, so a case that reaches for an editor fails visibly
rather than hanging.

The agent runs with `--permission-mode default` and a three-entry allowlist:
`Bash(yman:*)`, `Bash(ls:*)` and `Bash(YMAN_ACTOR=*)` — the last one because an
agent that sets the actor inline writes `YMAN_ACTOR=agent yman set ...`, which
no `yman`-prefixed rule matches. Everything else is denied, which is itself
informative: a denial in the transcript means the agent went somewhere the
skill did not send it.

## Measured

One run per case, `claude-opus-5-5`, 2026-10-05, for the five cases added
that day. Each grader was first checked against a scripted ideal run (pass)
and a scripted wrong one (fail).

| Case | Arm | Verdict | Cost |
| --- | --- | --- | --- |
| `plan-epic` | skill | **FAIL** — two `show` calls on top of `yman plan 4`; the grader allows one | $0.15 |
| `waits-on` | skill | PASS | $0.16 |
| `add-sections` | skill | PASS | $0.17 |
| `task-list` | skill | PASS | $0.17 |
| `exit-3-recovery` | skill | PASS | $0.21 |

The `plan-epic` failure is kept as a finding, not loosened: the skill already
says `plan` answers the question, and the agent still re-read steps it had
just been shown.

One run per case, `claude-opus-5`, 2026-09-21:

| Case | Arm | Verdict | Cost |
| --- | --- | --- | --- |
| `body-rewrite` | skill | PASS | $0.23 |
| `body-rewrite` | no-skill | **FAIL** — body never rewritten | $0.95 |
| `claim-task` | skill | PASS | $0.23 |
| `delete-task` | skill | PASS | $0.30 |
| `unassigned-query` | skill | PASS | $0.23 |

The ablation row is the one worth keeping an eye on: without the skill the same
prompt cost four times as much and still left the task unchanged.

## Porting to `claude plugin eval`

`claude plugin eval <skill dir>` runs cases with a no-plugin baseline arm and
LLM graders, which is where this should end up — the layout here (cases below
the skill, one directory each, `prompt.md` per case) is already what it expects.
It is in early access and refuses to run on this account, so the bash harness
stands in. When access lands, each `grade.sh` becomes a grader file and
`fixture.sh` becomes the case's `scaffold_script`; `--no-skill` becomes
`--ablation with-without`.
