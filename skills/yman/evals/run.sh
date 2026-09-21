#!/usr/bin/env bash
# Run the yman skill evals: build a throwaway tracker per run, hand the case's
# prompt to a headless agent, and grade what it did to the repository and which
# commands it reached for.
#
#   ./run.sh                          every case, one run each, skill loaded
#   ./run.sh --case delete-task       one case
#   ./run.sh --runs 3                 three runs per case; agents are stochastic
#   ./run.sh --no-skill               ablation arm: same cases, skill not copied
#   ./run.sh --keep                   leave the fixtures and transcripts behind
#
# Each run spends real tokens on the logged-in account. The per-case cost is
# reported, and `--runs` multiplies it.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)

runs=1
filter='*'
with_skill=1
keep=0
model=''
timeout_s=300

while [ $# -gt 0 ]; do
    case $1 in
        --case) filter=$2; shift 2 ;;
        --runs) runs=$2; shift 2 ;;
        --model) model=$2; shift 2 ;;
        --timeout) timeout_s=$2; shift 2 ;;
        --no-skill) with_skill=0; shift ;;
        --keep) keep=1; shift ;;
        -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
        *) echo "unknown flag: $1" >&2; exit 2 ;;
    esac
done

command -v claude >/dev/null || { echo "claude not in PATH" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 not in PATH" >&2; exit 1; }

YMAN=${YMAN:-$repo/target/debug/yman}
if [ ! -x "$YMAN" ]; then
    echo "building yman ..." >&2
    (cd "$repo" && cargo build -q)
fi

out=$(mktemp -d)
trap '[ "$keep" -eq 1 ] || rm -rf "$out"' EXIT
[ "$keep" -eq 0 ] || echo "artifacts: $out" >&2

arm=$([ "$with_skill" -eq 1 ] && echo "skill" || echo "no-skill")
total_cost=0
total_fail=0
total_run=0
report=""

for case_dir in "$here"/cases/$filter/; do
    [ -f "$case_dir/prompt.md" ] || continue
    name=$(basename "$case_dir")

    for run in $(seq 1 "$runs"); do
        work="$out/$name-$run"
        mkdir -p "$work"
        bash "$here/fixture.sh" "$work" "$YMAN" >/dev/null

        # The binary must answer to `yman`, the name every prompt and the skill
        # itself use.
        mkdir -p "$work/bin"
        ln -sf "$YMAN" "$work/bin/yman"

        if [ "$with_skill" -eq 1 ]; then
            mkdir -p "$work/wk/.claude/skills"
            cp -r "$here/.." "$work/wk/.claude/skills/yman"
            rm -rf "$work/wk/.claude/skills/yman/evals"
        fi

        transcript="$work/transcript.jsonl"
        set +e
        (
            cd "$work/wk"
            export PATH="$work/bin:$PATH"
            export GIT_CONFIG_GLOBAL="$work/gitconfig"
            export GIT_CONFIG_NOSYSTEM=1
            export YMAN_ACTOR=agent
            # No editor anywhere: a case that reaches for one must fail loudly
            # rather than block on a terminal that is not there.
            unset EDITOR VISUAL
            # YMAN_ACTOR= is allowlisted separately: an agent that sets the
            # actor inline writes `YMAN_ACTOR=agent yman set ...`, which does
            # not match a `Bash(yman:*)` prefix rule and would be denied.
            # --permission-mode default: the developer's own default may be
            # `plan`, and a planning agent writes a plan file instead of
            # touching the tracker, which grades as a failure it did not earn.
            timeout "$timeout_s" claude -p "$(cat "$case_dir/prompt.md")" \
                --output-format stream-json --verbose \
                --permission-mode default \
                --allowedTools 'Bash(yman:*)' 'Bash(ls:*)' 'Bash(YMAN_ACTOR=*)' \
                ${model:+--model "$model"} \
                < /dev/null > "$transcript"
        )
        agent_rc=$?
        set -e

        cost=$(WORK="$work/wk" TRANSCRIPT="$transcript" YMAN="$YMAN" \
               bash -c 'source "$0"; result_field total_cost_usd' "$here/lib.sh")
        [ -n "$cost" ] || cost=0
        cost=$(python3 -c "print(f'{float(\"$cost\"):.4f}')")
        total_cost=$(python3 -c "print(f'{float(\"$total_cost\") + float(\"$cost\"):.4f}')")

        if [ "$agent_rc" -ne 0 ]; then
            verdict="ERROR (agent exited $agent_rc)"
            detail=""
            rc=1
        else
            set +e
            detail=$(WORK="$work/wk" TRANSCRIPT="$transcript" YMAN="$YMAN" \
                     bash "$case_dir/grade.sh" 2>&1)
            rc=$?
            set -e
            verdict=$([ "$rc" -eq 0 ] && echo PASS || echo FAIL)
        fi

        total_run=$((total_run + 1))
        [ "$rc" -eq 0 ] || total_fail=$((total_fail + 1))
        report="$report$(printf '%-18s %-8s run %s  %-6s $%s' \
            "$name" "$arm" "$run" "$verdict" "$cost")
"
        [ -z "$detail" ] || report="$report$(echo "$detail" | sed 's/^/    /')
"
    done
done

printf '\n%s' "$report"
printf '%d run(s), %d failed, $%s total\n' "$total_run" "$total_fail" "$total_cost"
[ "$total_fail" -eq 0 ]
