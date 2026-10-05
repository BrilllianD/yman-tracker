#!/usr/bin/env bash
# The traps: walking the steps with `ls --related 4` plus a `show` each, or
# dumping `ls --json` into a pipeline, when `yman plan 4` prints the counts,
# the open steps in order and what each blocked one waits on in one call; and
# answering 7, the better-priority step, which is blocked on 6.
set -u
source "$(dirname "$0")/../../lib.sh"

require_command 'yman[[:space:]]+plan[[:space:]]+4' 'did not read the epic with `yman plan 4`'
forbid_command 'yman[[:space:]]+ls[^|]*--json[^|]*\|' 'dumped `ls --json` into a pipeline'

shows=$(bash_commands | grep -cE 'yman[[:space:]]+show[[:space:]]' || true)
[ "$shows" -le 1 ] || fail "$shows \`show\` calls on top of the plan"

calls=$(yman_work_call_count)
[ "$calls" -le 3 ] || fail "$calls yman calls besides sync for one plan"

answer=$(result_field result)
case $answer in
    *6*"feature flag"* | *"feature flag"*6*) ;;
    *) fail "answer does not name step 6, Add the feature flag: $answer" ;;
esac
case $answer in
    *"Switch the login form"*) printf 'NOTE  answer also mentions the blocked step 7\n' ;;
esac

verdict
