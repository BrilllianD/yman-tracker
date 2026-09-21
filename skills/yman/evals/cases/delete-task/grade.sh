#!/usr/bin/env bash
# The trap: `yman rm 2` without -f refuses off a terminal with
# `refusing to remove without -f`. The skill says to pass -f up front.
set -u
source "$(dirname "$0")/../../lib.sh"

(cd "$WORK" && "$YMAN" show 2 >/dev/null 2>&1) && fail "task 2 still exists"
(cd "$WORK" && "$YMAN" show 1 >/dev/null 2>&1) || fail "task 1 was removed too"
(cd "$WORK" && "$YMAN" show 3 >/dev/null 2>&1) || fail "task 3 was removed too"

require_command 'yman[[:space:]]+rm[[:space:]]' 'did not use `yman rm`'
forbid_command 'rm[[:space:]]+-rf' 'deleted the folder behind the tracker'

attempts=$(bash_commands | grep -cE 'yman[[:space:]]+rm[[:space:]]' || true)
[ "$attempts" -le 1 ] || printf 'NOTE  %s `rm` attempts; -f was missing on the first\n' "$attempts"

verdict
