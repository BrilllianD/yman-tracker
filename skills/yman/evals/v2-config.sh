#!/usr/bin/env bash
# Sourced by a case's setup.sh: switch the fixture's tracker to a version 2
# config whose statuses include `blocked`, which `--waits-on` and the `waits
# on` column of `plan` need. Expects $yman and the working copy as cwd.
cat > .yman/config.toml <<'CFG'
version = 2

[ids]
scheme = "seq"
random_len = 4

[statuses]
list = ["todo", "doing", "blocked", "done", "cancelled"]
default = "todo"
start = "doing"
done = "done"
cancel = "cancelled"
terminal = ["done", "cancelled"]

[priorities]
default = 5

[slug]
max_bytes = 200
CFG
"$yman" git -- commit -q -m "yman: config version 2 with blocked" -- config.toml
