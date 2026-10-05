#!/usr/bin/env bash
# The fixture leaves a sync merge stopped on a conflict in task 1's m.yml
# (closed here, started on the other clone). The skill's way out is to fix
# the file and run `yman sync --continue`. The traps: `yman git merge
# --abort` behind sync's back (it leaves sync's own bookkeeping), deleting
# `.yman`, or giving up with `--abort` when the user asked to publish.
set -u
source "$(dirname "$0")/../../lib.sh"

ydir="$WORK/.yman"
git -C "$ydir" rev-parse -q --verify MERGE_HEAD >/dev/null && fail "the merge is still in progress"
status=$(task_field 1 status 2>/dev/null)
[ "$status" = done ] || fail "task 1 is \"$status\", not done; the user's version should win"
local_ref=$(git -C "$WORK" rev-parse refs/yman/local)
origin_ref=$(git --git-dir="$WORK/../bare" rev-parse refs/tasks/main)
[ "$local_ref" = "$origin_ref" ] || fail "origin does not have the resolved history"

require_command 'yman[[:space:]]+sync[[:space:]]+--continue' 'did not finish with `yman sync --continue`'
forbid_command 'merge[[:space:]]+--abort' 'aborted the merge with git instead of resolving it'
forbid_command 'rm[[:space:]]+-r' 'deleted files instead of resolving the merge'
if bash_commands | grep -qE 'yman[[:space:]]+sync[[:space:]]+--abort'; then
    printf 'NOTE  aborted once before resolving\n'
fi

verdict
