#!/usr/bin/env bash
# The trap: `yman edit 1` is the obvious verb and it opens $EDITOR. With no
# editor set and no terminal it fails, so an agent that reaches for it either
# gives up or flails.
set -u
source "$(dirname "$0")/../../lib.sh"

forbid_command 'yman[[:space:]]+edit' 'reached for the interactive editor'
forbid_command 'yman[[:space:]]+(add|set)[^|]*(^|[[:space:]])-e([[:space:]]|$)' \
    'passed -e, which opens the editor'
require_command 'yman[[:space:]]+set[[:space:]]+1[^|]*--body' \
    'did not rewrite the body with `yman set --body`'

body=$(task_field 1 body)
case $body in
    *next*) ;;
    *) fail "body does not mention the cause: $body" ;;
esac
case $body in
    *"Cause unknown"*) fail "the stale sentence is still there" ;;
esac

[ "$(task_field 1 status)" = todo ] || fail "status moved; the prompt asked for a body change only"
[ "$(task_field 1 priority)" = 2 ] || fail "priority moved; the prompt asked for a body change only"

verdict
