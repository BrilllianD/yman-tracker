#!/usr/bin/env bash
# The trap: three `yman add "<title>" -m "<body>"` calls, retyping the file,
# when `yman add --sections rollout.md` makes one task per `# ` heading with
# the section as its body.
set -u
source "$(dirname "$0")/../../lib.sh"

for pair in "4:Add the rollout flag" "5:Migrate existing sessions" "6:Remove the old login page"; do
    id=${pair%%:*}
    want=${pair#*:}
    got=$(task_field "$id" title 2>/dev/null || true)
    [ "$got" = "$want" ] || fail "task $id is \"$got\", not \"$want\""
done
body=$(task_field 4 body 2>/dev/null || true)
case $body in
    *"Gate the new login"*) ;;
    *) fail "task 4 lost its section body: $body" ;;
esac
(cd "$WORK" && "$YMAN" show 7 >/dev/null 2>&1) && fail "a fourth task was created"

require_command 'yman[[:space:]]+add[^|]*--sections' 'did not use `add --sections`'
adds=$(bash_commands | grep -oE 'yman[[:space:]]+add[[:space:]]' | wc -l)
[ "$adds" -le 1 ] || fail "$adds \`yman add\` calls for one file"

verdict
