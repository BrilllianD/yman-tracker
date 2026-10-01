#!/usr/bin/env bash
# The traps: three separate calls (set --status, set -a, comment) where one
# `set --status doing -a agent -m ...` does the job in one commit; and
# claiming without a `yman sync` first, which can overwrite another clone's
# claim the agent never saw.
set -u
source "$(dirname "$0")/../../lib.sh"

[ "$(task_field 1 status)" = doing ] || fail "status is $(task_field 1 status), not doing"
[ "$(task_field 1 assignee)" = agent ] || fail "assignee is $(task_field 1 assignee), not agent"

comments=$(task_json 1 | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["discussion"]))')
[ "$comments" -ge 1 ] || fail "no comment was left"

require_before 'yman[[:space:]]+sync' 'yman[[:space:]]+(set|start)[[:space:]]+1' \
    'did not sync before changing the status'

calls=$(yman_work_call_count)
[ "$calls" -le 3 ] || fail "$calls yman calls besides sync; one \`set --status doing -a agent -m\` covers it"
if ! bash_commands | grep -qE 'yman[[:space:]]+set[[:space:]]+1[^|]*--status[^|]*-a[^|]*-m'; then
    printf 'NOTE  claimed in %s call(s) rather than one combined `set`\n' "$calls"
fi

verdict
