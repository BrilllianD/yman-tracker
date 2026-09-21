#!/usr/bin/env bash
# The trap: `yman ls --json | jq` reads every field of every task, and
# `yman show <id>` on each task in turn is worse, when `yman ls --assignee -`
# prints exactly the rows that were asked for.
set -u
source "$(dirname "$0")/../../lib.sh"

require_command 'yman[[:space:]]+ls' 'did not list the tracker'
if ! bash_commands | grep -qE "yman[[:space:]]+ls[^|]*--assignee[[:space:]]+-"; then
    fail 'did not filter with `ls --assignee -`'
fi

# One `show` to confirm a single row is fair; walking the tracker task by task
# is the failure this case is about.
shows=$(bash_commands | grep -cE 'yman[[:space:]]+show[[:space:]]' || true)
[ "$shows" -le 1 ] || fail "$shows \`show\` calls: read tasks one by one instead of trusting the filter"
[ "$shows" -eq 0 ] || printf 'NOTE  one `show` past the filter\n'

forbid_command 'yman[[:space:]]+ls[^|]*--json[^|]*\|' 'dumped `ls --json` into a pipeline when the table answered it'

calls=$(yman_call_count)
[ "$calls" -le 3 ] || fail "$calls yman calls for one listing"

answer=$(result_field result)
case $answer in
    *1*) ;;
    *) fail "answer does not name task 1: $answer" ;;
esac
case $answer in
    *2*) ;;
    *) fail "answer does not name task 2: $answer" ;;
esac
case $answer in
    *"Drop the legacy"*) fail "answer includes task 3, which is assigned to mira" ;;
esac

verdict
