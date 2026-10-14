---
name: yman-task
description: Turn the user's own prompt into yman tasks, marked as written by the user and not yet verified, so an agent re-checks each one before working on it. Use when the user invokes /yman-task, or asks to file, record or write down their list of bugs, wishes or ideas as tasks in a repository that has `.yman/`. Filing only - no investigation, no fixes.
---

# yman-task: file the user's words as unverified tasks

The text after `/yman-task` is the user's own list of problems or wishes. File
each one as a yman task, marked as user-written and not yet checked by an
agent. Do not fix anything and do not investigate the code: filing only.
Checking each claim is the job of the agent that later picks the task up.

The `yman` skill has the full command reference; this one only adds the
procedure and the marking.

## The marking

Every task filed here carries:

- tags `user-request` (the user wrote it) and `needs-agent` (an agent has to
  re-check it before work starts);
- no assignee, so `yman ls --assignee -` offers it to the next agent;
- the user's original words, quoted verbatim, and an `Origin:` line that tells
  the next agent what to do first.

## Steps

1. `yman status` must succeed. If `.yman` is not initialized, stop and say so;
   do not run `yman init` unasked.
2. `yman tags`: reuse the existing tags (`bug`, `feature`, `ui`, ...) next to
   the two markers; never invent a near-duplicate of one already in use.
3. Split the prompt into tasks, one per independent problem or wish. When it
   is unclear whether two points are one task, keep them together. When
   nothing in the prompt is actionable, ask and file nothing.
4. Look for duplicates: `yman ls -q "<key word>" -n 5` per task. A clear open
   duplicate is not added again; instead
   `yman comment <id> -m "User reported again: <their words>"`, and report it.
5. Add each task. Title in English, imperative, at most 70 characters. The
   heredoc keeps multi-line text and quotes intact:

   ```sh
   yman add "<Title>" -t user-request -t needs-agent -t <bug|feature|...> \
     --body-file - <<'EOF'
   <the problem or wish restated in English, 1-5 lines>

   User wrote:
   > <the user's original words for this task, verbatim, in their language>

   Origin: written by the user, not verified. Agent: before working, re-check
   it against the code - reproduce or confirm the claim, find the place,
   rewrite the body if it is wrong - then claim it.
   EOF
   ```

   - No `-a`: the task stays unassigned.
   - `-p` only when the user states urgency (0-2 for urgent); otherwise the
     tracker's default priority.
   - Tasks from the same prompt that truly depend on each other: `--relate`
     the later one to the earlier one.
6. `yman sync` once at the end (`no "origin" remote` means a local-only
   tracker: nothing to publish). Under `autosync: push` there is nothing to
   run.
7. Report a table `ID | Title | Tags` and any duplicates commented instead of
   added. Nothing else.

## Rules

- Restate, never invent: no causes, file paths, fixes or acceptance criteria
  the user did not give. Guesses belong to the re-check, not the filing.
- The quoted original is the source of truth for the re-check; never shorten
  or translate it.
- Never `yman edit`, `add -e` or `rm`.

## Picking such a task up later

An agent that takes a `needs-agent` task re-checks it first: reproduce or
confirm the claim in the code, then rewrite the body with what it found
(`yman set <id> --body-file -`, keeping the quote), drop the marker with
`yman set <id> --untag needs-agent -m "checked: <one line>"`, and only then
claim it. A claim that does not hold is closed with `yman cancel <id> -m "<why>"`.
