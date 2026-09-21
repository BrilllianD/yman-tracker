#!/usr/bin/env bash
# Build a throwaway repository with a yman tracker and three seeded tasks.
#
# Usage: fixture.sh <dir> <yman-binary>
# Leaves the agent's working copy at <dir>/wk.
#
# HOME is deliberately NOT pinned: the agent under test needs its own
# credentials. Git is isolated through GIT_CONFIG_GLOBAL / GIT_CONFIG_NOSYSTEM
# and a repo-local identity instead, so the run cannot read or write the
# developer's real git config.
set -eu

dir=$1
yman=$2

export GIT_CONFIG_GLOBAL="$dir/gitconfig"
export GIT_CONFIG_NOSYSTEM=1
: > "$GIT_CONFIG_GLOBAL"

git init -q --bare "$dir/bare"
git init -q "$dir/wk"
cd "$dir/wk"
git config user.name "Eval Fixture"
git config user.email fixture@example.invalid
git config init.defaultBranch main
git remote add origin "$dir/bare"
printf 'placeholder\n' > README.md
git add README.md
git commit -qm "init"

"$yman" init --offline >/dev/null

# Task 1: the one every case touches. Its body is deliberately stale.
"$yman" add "Login redirects to /null" \
    -m "Users land on /null after signing in. Cause unknown." \
    -t auth -p 2 >/dev/null
# Task 2: unassigned, lower priority. Distractor for the query case.
"$yman" add "Rotate the staging credentials" -m "Quarterly chore." -t ops -p 4 >/dev/null
# Task 3: assigned, so "unassigned" is a real filter and not the whole list.
"$yman" add "Drop the legacy /v1 endpoints" -m "After the clients migrate." \
    -a mira -p 5 >/dev/null
