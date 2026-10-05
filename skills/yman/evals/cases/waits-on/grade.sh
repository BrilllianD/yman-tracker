#!/usr/bin/env bash
# The trap: `set 1 --relate 2` followed by `set 1 --status blocked`, two
# commits and a status nobody reading `plan` can trace, when `--waits-on 2`
# does both in one.
set -u
source "$(dirname "$0")/../../lib.sh"

[ "$(task_field 1 status)" = blocked ] || fail "task 1 is $(task_field 1 status), not blocked"
related=$(task_json 1 | python3 -c 'import json,sys; print(" ".join(json.load(sys.stdin)["related"]))')
case " $related " in
    *" 2 "*) ;;
    *) fail "task 1 does not relate to 2 (related: $related)" ;;
esac

require_command 'yman[[:space:]]+(set|add)[^|]*--waits-on[[:space:]]+2' 'did not use `--waits-on 2`'
forbid_command 'yman[[:space:]]+set[^|]*--status[[:space:]]+blocked' 'set the status by hand instead of through --waits-on'

calls=$(yman_work_call_count)
[ "$calls" -le 3 ] || fail "$calls yman calls besides sync for one dependency"

verdict
