#!/usr/bin/env bash
# Grader helpers. Sourced by cases/*/grade.sh, which run with:
#   WORK       the agent's working copy (a git repo with .yman)
#   TRANSCRIPT the stream-json transcript, one JSON object per line
#   YMAN       the yman binary under test
#
# A grader reports by calling `fail <reason>` (repeatable) and exits through
# `verdict`. Anything a grader prints on stdout is quoted in the report.

FAILURES=0

fail() {
    printf 'FAIL  %s\n' "$*"
    FAILURES=$((FAILURES + 1))
}

verdict() {
    [ "$FAILURES" -eq 0 ] || exit 1
    exit 0
}

# Every command the agent passed to the Bash tool, one per line. Newlines
# inside a single command become spaces so one call stays one line.
bash_commands() {
    python3 - "$TRANSCRIPT" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    line = line.strip()
    if not line:
        continue
    try:
        ev = json.loads(line)
    except json.JSONDecodeError:
        continue
    msg = ev.get("message")
    content = msg.get("content") if isinstance(msg, dict) else None
    for block in content if isinstance(content, list) else []:
        if isinstance(block, dict) and block.get("type") == "tool_use" and block.get("name") == "Bash":
            cmd = (block.get("input") or {}).get("command", "")
            print(" ".join(cmd.split()))
PY
}

# Names of every tool the agent invoked, one per line, in order.
tools_used() {
    python3 - "$TRANSCRIPT" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    line = line.strip()
    if not line:
        continue
    try:
        ev = json.loads(line)
    except json.JSONDecodeError:
        continue
    msg = ev.get("message")
    content = msg.get("content") if isinstance(msg, dict) else None
    for block in content if isinstance(content, list) else []:
        if isinstance(block, dict) and block.get("type") == "tool_use":
            print(block.get("name", ""))
PY
}

# A field of the terminating `result` event: total_cost_usd, num_turns, ...
result_field() {
    python3 - "$TRANSCRIPT" "$1" <<'PY'
import json, sys
want = sys.argv[2]
val = ""
for line in open(sys.argv[1]):
    line = line.strip()
    if not line:
        continue
    try:
        ev = json.loads(line)
    except json.JSONDecodeError:
        continue
    if ev.get("type") == "result" and want in ev:
        val = ev[want]
print(val)
PY
}

# Fails when any Bash command matches the extended regex.
forbid_command() {
    local pattern=$1 why=$2 hit
    hit=$(bash_commands | grep -E -- "$pattern" | head -3 || true)
    [ -z "$hit" ] || fail "$why: $hit"
}

# Fails unless some Bash command matches the extended regex.
require_command() {
    local pattern=$1 why=$2
    bash_commands | grep -qE -- "$pattern" || fail "$why (no Bash call matched /$pattern/)"
}

# How many Bash calls invoked yman at all.
yman_call_count() {
    bash_commands | grep -cE '(^|[;&|[:space:]])yman[[:space:]]' || true
}

# One task as JSON, straight out of the tracker under test.
task_json() {
    (cd "$WORK" && "$YMAN" show "$1" --json)
}

# A scalar field of `show --json`, without a JSON parser in the shell.
task_field() {
    task_json "$1" | python3 -c 'import json,sys; print(json.load(sys.stdin)[sys.argv[1]])' "$2"
}
