#!/usr/bin/env bash
# The skill's task-list procedure: one `ls -l` (bodies included), the rows as
# a markdown table, a `show` for at most the top few candidates, and a
# recommendation. The traps: `show` on every task, `ls --json` through a
# pipeline, or a prose list instead of the table.
set -u
source "$(dirname "$0")/../../lib.sh"

require_command 'yman[[:space:]]+ls[^|]*(-l|--long)' 'did not list with `ls -l`'
forbid_command 'yman[[:space:]]+ls[^|]*--json[^|]*\|' 'dumped `ls --json` into a pipeline'
shows=$(bash_commands | grep -cE 'yman[[:space:]]+show[[:space:]]' || true)
[ "$shows" -le 3 ] || fail "$shows \`show\` calls; the skill reads at most three candidates"

answer=$(result_field result)
rows=$(printf '%s\n' "$answer" | grep -cE '^[[:space:]]*\|.*\|' || true)
[ "$rows" -ge 4 ] || fail "answer has no markdown table (header, rule and three rows): $answer"
for title in "Login redirects to /null" "Rotate the staging credentials" "Drop the legacy /v1 endpoints"; do
    case $answer in
        *"$title"*) ;;
        *) fail "table is missing \"$title\"" ;;
    esac
done

verdict
