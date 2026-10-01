#!/usr/bin/env bash
# The traps: `yman done 1 -m ...` closes the task locally and stops there,
# where the skill says to publish with `yman sync`, so origin must end up
# carrying it; and closing without a `yman sync` first to see whether another
# clone already moved the task.
set -u
source "$(dirname "$0")/../../lib.sh"

[ "$(task_field 1 status)" = done ] || fail "status is $(task_field 1 status), not done"

local_head=$(git -C "$WORK" rev-parse refs/yman/local)
origin_head=$(git -C "$WORK/../bare" rev-parse --verify -q refs/tasks/main || true)
[ "$origin_head" = "$local_head" ] || fail "origin does not carry the close; \`yman sync\` never ran"

require_command 'yman[[:space:]]+done[[:space:]]+1' 'did not close with `yman done`'
require_before 'yman[[:space:]]+sync' 'yman[[:space:]]+done[[:space:]]+1' \
    'did not sync before changing the status'

verdict
