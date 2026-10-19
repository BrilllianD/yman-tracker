#!/usr/bin/env bash
# The trap: the summary is "just a file in .yman", so an agent writes
# `.yman/state.md` itself and commits it with `yman git` or git. The skill's
# way is `yman state set`, one commit with the subject `yman: state`.
set -u
source "$(dirname "$0")/../../lib.sh"

ydir="$WORK/.yman"
[ -f "$ydir/state.md" ] || fail ".yman/state.md does not exist"
state=$(cat "$ydir/state.md" 2>/dev/null)
case $state in
    *next*) ;;
    *) fail "state does not mention the next step: $state" ;;
esac
case $state in
    *progress*) ;;
    *) fail "state does not say what is in progress: $state" ;;
esac

subject=$(git -C "$ydir" log -1 --format=%s)
[ "$subject" = "yman: state" ] || fail "last .yman commit is \"$subject\", not \"yman: state\""

require_command 'yman[[:space:]]+state[[:space:]]+set' 'did not write the summary with `yman state set`'
forbid_command '(>|tee|sed -i)[^|;&]*state\.md' 'wrote .yman/state.md by hand'
forbid_command 'yman[[:space:]]+git[[:space:]]+(add|commit)' 'committed by hand through yman git'
forbid_command 'git[[:space:]]+(-C[[:space:]]+[^[:space:]]+[[:space:]]+)?(add|commit)' 'committed by hand with git'
forbid_command 'yman[[:space:]]+edit' 'reached for the interactive editor'

verdict
