#!/usr/bin/env bash
# Epic 4 with three steps: 5 done, 6 free, 7 blocked waiting on 6.
set -eu
yman=$2
source "$(dirname "$0")/../../v2-config.sh"
"$yman" add "Ship the login revamp" -t epic -p 2 -m "Done when: yman plan 4 lists no steps" >/dev/null
"$yman" add "Write the session migration" --relate 4 -p 2 >/dev/null
"$yman" done 5 >/dev/null
"$yman" add "Add the feature flag" --relate 4 -p 3 >/dev/null
"$yman" add "Switch the login form" --relate 4 --waits-on 6 -p 2 >/dev/null
