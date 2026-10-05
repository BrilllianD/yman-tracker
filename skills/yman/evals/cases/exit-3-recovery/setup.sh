#!/usr/bin/env bash
# Publish the fixture, have a second clone start task 1 and publish that, then
# close task 1 here and sync: both sides changed `status`, so the sync stops
# with exit 3 and the merge is left for the agent.
set -eu
work=$1
yman=$2
"$yman" sync >/dev/null
git clone -q "$work/bare" "$work/other" 2>/dev/null
(
    cd "$work/other"
    git config user.name "Other Clone"
    git config user.email other@example.invalid
    "$yman" init >/dev/null
    "$yman" set 1 --status doing >/dev/null
    "$yman" sync >/dev/null
)
"$yman" set 1 --status done >/dev/null
if "$yman" sync >/dev/null 2>&1; then
    echo "expected the sync to stop on a conflict" >&2
    exit 1
fi
