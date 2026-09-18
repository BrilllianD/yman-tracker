#!/bin/sh
# Builds a synthetic yman project with a real commit history and drives it, to
# answer what the two-clone suite in tests/cli.rs structurally cannot ask: what
# `add` costs once the history is deep, whether `yman log <id>` stays usable
# after a task has crossed in and out of a status directory several times, and
# what `ls`/`show` cost on a large tree.
#
# Run: sh scripts/synthetic-project.sh [--tasks N] [--force] [dir]
#
# The repository is regenerated, never committed: the default output directory
# is target/synthetic, which the single line in .gitignore already covers. It
# is kept after the run so it can be poked at by hand. This is a pre-release
# check, deliberately not part of CI.
set -eu

fail() { echo "SYNTHETIC FAIL: $*" >&2; exit 1; }

# A finding is behaviour worth writing down, not a reason to stop: the run
# still has to produce its numbers. Hard failures are for a broken tree.
findings=0
finding() { findings=$(( findings + 1 )); echo "finding: $*"; }

usage() {
	cat <<'USAGE'
usage: sh scripts/synthetic-project.sh [--tasks N] [--force] [dir]

  --tasks N   how many tasks to create (default 1000)
  --force     replace a non-empty output directory
  dir         where to build it (default target/synthetic)

  YMAN_BIN=<path>  use this binary instead of building --release
USAGE
}

tasks=1000
force=0
out=""
while [ $# -gt 0 ]; do
	case $1 in
		--tasks) [ $# -ge 2 ] || fail "--tasks needs a number"; tasks=$2; shift 2;;
		--tasks=*) tasks=${1#--tasks=}; shift;;
		--force) force=1; shift;;
		-h|--help) usage; exit 0;;
		--) shift;;
		-*) fail "unknown option: $1";;
		*) out=$1; shift;;
	esac
done
case $tasks in
	''|*[!0-9]*) fail "--tasks needs a number, got: $tasks";;
esac
[ "$tasks" -ge 4 ] || fail "--tasks must be at least 4"

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/.." && pwd)
if [ -z "$out" ]; then
	out=$root/target/synthetic
fi

# Build before the environment is sandboxed: cargo wants the real HOME.
if [ -n "${YMAN_BIN:-}" ]; then
	yman=$YMAN_BIN
	profile=external
else
	( cd "$root" && cargo build --release --quiet )
	yman=$root/target/release/yman
	profile=release
fi
[ -x "$yman" ] || fail "no usable yman binary at $yman"
git_version=$(git --version)
real_git=$(command -v git) || fail "git is not on PATH"

if [ -e "$out" ]; then
	if [ "$force" -eq 1 ]; then
		rm -rf "$out"
	elif [ -n "$(ls -A "$out" 2>/dev/null || true)" ]; then
		fail "$out is not empty; pass --force to replace it"
	fi
fi
mkdir -p "$out"
out=$(CDPATH= cd -- "$out" && pwd)
report=$out/report
mkdir -p "$report"
runlog=$report/run.log
: > "$runlog"

# Same isolation the spike and tests/common/mod.rs each set up for themselves:
# nothing here touches the network or the developer's real git configuration.
home=$out/home
mkdir -p "$home"
cat > "$home/.gitconfig" <<'CFG'
[user]
	name = Synthetic
	email = synthetic@example.invalid
[init]
	defaultBranch = main
[advice]
	detachedHead = false
CFG
HOME=$home
GIT_CONFIG_GLOBAL=$home/.gitconfig
GIT_CONFIG_NOSYSTEM=1
EDITOR=true
VISUAL=""
TERM=dumb
export HOME GIT_CONFIG_GLOBAL GIT_CONFIG_NOSYSTEM EDITOR VISUAL TERM
unset YMAN_AUTHOR
unset GIT_DIR
unset GIT_WORK_TREE

# --- measurement helpers ----------------------------------------------------

# `date +%s%N` is GNU; elsewhere it echoes the format back. Every timing below
# is over a batch, so whole-second resolution still yields a usable number.
if date +%s%N 2>/dev/null | grep -q '^[0-9][0-9]*$'; then
	have_ns=1
else
	have_ns=0
fi
now_ms() {
	if [ "$have_ns" -eq 1 ]; then
		echo $(( $(date +%s%N) / 1000000 ))
	else
		echo $(( $(date +%s) * 1000 ))
	fi
}

# A `git` earlier on PATH that records every spawn, then execs the real one.
# Process count is how TASKS.md states the performance goals, so it is what
# gets reported next to the wall-clock numbers.
shim=$out/shim
mkdir -p "$shim"
cat > "$shim/git" <<SHIM
#!/bin/sh
echo x >> "\$GIT_SPAWN_LOG"
exec "$real_git" "\$@"
SHIM
chmod 755 "$shim/git"
spawnlog=$report/spawns
spawns() { # spawns <clone> <yman args...>  -> number of git processes
	sp_c=$1; shift
	: > "$spawnlog"
	( cd "$out/$sp_c" && PATH="$shim:$PATH" GIT_SPAWN_LOG="$spawnlog" "$yman" "$@" ) \
		>/dev/null 2>>"$runlog" || true
	wc -l < "$spawnlog" | tr -d ' \t'
}

# --- the binary, quietly ----------------------------------------------------

y() { # y <clone> <args...>  -> stdout, stderr to the run log
	y_c=$1; shift
	( cd "$out/$y_c" && "$yman" "$@" ) 2>>"$runlog"
}
yq() { y "$@" >/dev/null; }

syncs=$report/syncs.txt
: > "$syncs"
sync_one() {
	so_out=$(y "$1" sync) || fail "sync failed in clone $1; see $runlog"
	echo "$so_out" >> "$syncs"
}
# Twice around: the first pass publishes, the second lets everyone see it. A
# clone that starts a phase stale turns an ordinary edit into a conflict.
# Ids are not stable: a clone that minted them offline gets renumbered on the
# next sync, so what `add` printed is not necessarily what is on disk.
id_list() { y a ls -a --json | grep -o '"id":"[^"]*"' | sed 's/.*:"//;s/"//'; }
sync_all() {
	for sa_c in $clones; do sync_one "$sa_c"; done
	for sa_c in $clones; do sync_one "$sa_c"; done
}

# --- phase 1: four clones of one bare remote, at version 2 ------------------

git init -q --bare -b main "$out/remote.git"
git clone -q "$out/remote.git" "$out/seed"
printf '# synthetic project\n' > "$out/seed/README.md"
git -C "$out/seed" add -A
git -C "$out/seed" commit -q -m "initial"
git -C "$out/seed" push -q origin main
rm -rf "$out/seed"

clones="a b c d"
for c in $clones; do
	git clone -q "$out/remote.git" "$out/$c"
	git -C "$out/$c" config user.name "Synth $c"
	git -C "$out/$c" config user.email "$c@example.invalid"
done

yq a init
# There is no CLI for the config by design (docs/storage.md §7): raising the
# version and naming the status roles is a hand edit, committed like any other.
cat > "$out/a/.yman/config.toml" <<'CFG'
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
git -C "$out/a/.yman" add -- config.toml
git -C "$out/a/.yman" commit -q --no-verify -m "yman: config"
sync_one a
for c in b c d; do
	yq "$c" init
done

echo "binary:  $yman ($profile), $git_version"
echo "output:  $out"
echo "init:    version 2, 4 clones, scheme seq"

# --- phase 2: bulk add, timed in blocks -------------------------------------

words="fix add drop tune port audit purge cache index queue parse render"
subjects="the login form|the sync path|ref reads|the merge driver|folder names|the discussion file|attachment staging|id allocation"
tagset="bug chore perf docs infra"

pick() { # pick <space-or-pipe list> <sep> <n> <count>
	echo "$1" | cut -d"$2" -f$(( $3 % $4 + 1 ))
}

title_for() {
	tf_n=$1
	case $(( tf_n % 17 )) in
		0) echo "Первая задача $tf_n";;
		7) echo "Task $tf_n: \"quoted\", odd — chars & all";;
		*) echo "$(pick "$words" ' ' "$tf_n" 12) $(pick "$subjects" '|' "$(( tf_n / 12 ))" 8) $tf_n";;
	esac
}

ids=$report/ids.txt
: > "$ids"
blocks=$report/add-blocks.tsv
printf 'block\ttasks_before\tadds\tms_total\tms_per_add\n' > "$blocks"

block=25
made=0
bi=0
while [ "$made" -lt "$tasks" ]; do
	n=$block
	rem=$(( tasks - made ))
	if [ "$rem" -lt "$n" ]; then
		n=$rem
	fi
	c=$(pick "$clones" ' ' "$bi" 4)
	t0=$(now_ms)
	i=0
	while [ "$i" -lt "$n" ]; do
		k=$(( made + i ))
		line=$(y "$c" add "$(title_for "$k")" \
			-p $(( k % 10 )) \
			-t "$(pick "$tagset" ' ' "$k" 5)" \
			-m "Synthetic body for task $k.")
		echo "$line" | awk 'NR==1{print $2}' >> "$ids"
		i=$(( i + 1 ))
	done
	t1=$(now_ms)
	printf '%d\t%d\t%d\t%d\t%d\n' "$bi" "$made" "$n" "$(( t1 - t0 ))" "$(( (t1 - t0) / n ))" >> "$blocks"
	made=$(( made + n ))
	sync_one "$c"
	bi=$(( bi + 1 ))
done
sync_all
id_list > "$ids"
commits=$(git -C "$out/a/.yman" rev-list --count HEAD)
echo "add:     $made tasks in $bi blocks of $block, $commits commits"

# --- phase 3: churn ---------------------------------------------------------

mkdir -p "$out/blobs"
printf 'attachment payload\n' > "$out/blobs/note.txt"
printf 'second payload\n' > "$out/blobs/trace.log"

retitled=0
commented=0
attached=0
related=0
prev=""
ci=0
for id in $(awk 'NR % 7 == 1' "$ids"); do
	# One clone owns a given task's churn. Two clones editing the same task
	# offline is a real case, but it is a conflict, not churn: probe P3 does
	# that deliberately with one task rather than letting it stop the run.
	c=$(pick "$clones" ' ' "$ci" 4)
	yq "$c" set "$id" --title "$(title_for "$(( retitled + 10000 ))") (revised)"
	retitled=$(( retitled + 1 ))
	yq "$c" comment "$id" -m "Synthetic comment on task $id."
	commented=$(( commented + 1 ))
	if [ $(( retitled % 2 )) -eq 0 ]; then
		yq "$c" prio "$id" $(( retitled % 10 ))
		yq "$c" attach "$id" "$out/blobs/note.txt"
		attached=$(( attached + 1 ))
	fi
	if [ -n "$prev" ]; then
		yq "$c" set "$id" --relate "$prev"
		related=$(( related + 1 ))
	fi
	prev=$id
	ci=$(( ci + 1 ))
	if [ $(( ci % 5 )) -eq 0 ]; then
		sync_one "$c"
	fi
done
sync_all
churn_renumbered=$(grep -c '^renumbered ' "$syncs" || true)
id_list > "$ids"
echo "churn:   $retitled retitled, $commented comments, $attached attachments, $related relations"

# --- phase 4: close several hundred ----------------------------------------

closed=0
cancelled=0
for id in $(awk 'NR % 3 == 0' "$ids"); do
	yq a done "$id"
	closed=$(( closed + 1 ))
done
for id in $(awk 'NR % 11 == 5 && NR % 3 != 0' "$ids"); do
	yq b cancel "$id"
	cancelled=$(( cancelled + 1 ))
done
sync_all
echo "close:   $closed done, $cancelled cancelled"

# --- phase 5: cross the status boundary, repeatedly -------------------------

crossers=$(awk 'NR % 7 == 3' "$ids" | head -n 5)
laps=3
for id in $crossers; do
	lap=0
	while [ "$lap" -lt "$laps" ]; do
		yq a done "$id"
		yq a reopen "$id"
		yq a set "$id" --title "Crossed $id lap $lap"
		yq a cancel "$id"
		yq a reopen "$id"
		lap=$(( lap + 1 ))
	done
	yq a done "$id"
done
sync_all
n_crossers=$(echo "$crossers" | wc -w | tr -d ' \t')
echo "cross:   $n_crossers tasks x $laps laps through done/reopen/cancel/reopen"

# --- phase 6: offline collisions across four clones -------------------------

offline=0
for c in $clones; do
	i=0
	while [ "$i" -lt 3 ]; do
		yq "$c" add "Offline $c $i" -p 4
		offline=$(( offline + 1 ))
		i=$(( i + 1 ))
	done
done
sync_all
renumbered=$(grep -c '^renumbered ' "$syncs" || true)
echo "collide: $offline offline adds, $renumbered renumber lines"

# --- probes -----------------------------------------------------------------

echo ""

# P1: what does `add` cost on a deep history? ids::ever_assigned walks the
# whole history of both refs on every add, with no -n and no -M.
echo "P1 add cost by history depth (ms per add):"
awk -F'\t' 'NR>1 { rows[n++] = $2 "\t" $5 }
	END {
		step = int(n / 6); if (step < 1) step = 1
		for (i = 0; i < n; i += step) { split(rows[i], f, "\t"); printf "           after %6d tasks: %5d ms\n", f[1], f[2] }
		split(rows[n-1], f, "\t"); printf "           after %6d tasks: %5d ms\n", f[1], f[2]
	}' "$blocks"
first=$(awk -F'\t' 'NR==2{print $5}' "$blocks")
last=$(awk -F'\t' 'END{print $5}' "$blocks")
echo "           first block $first ms/add, last block $last ms/add   ($blocks)"

# P2: does `yman log <id>` survive several status crossings?
probe_id=$(echo "$crossers" | head -n 1)
t0=$(now_ms)
i=0
while [ "$i" -lt 5 ]; do
	logout=$(y a log "$probe_id" -n 200)
	i=$(( i + 1 ))
done
t1=$(now_ms)
renames=$(git -C "$out/a/.yman" log --format= --name-status --diff-filter=R -M refs/yman/local -- . | wc -l | tr -d ' \t')
if ! echo "$logout" | grep -q "task($probe_id): add"; then
	fail "yman log $probe_id lost the commit that created the task"
fi
echo "P2 log:  task $probe_id, $(echo "$logout" | wc -l | tr -d ' \t') commits, $(( (t1 - t0) / 5 )) ms, $renames rename records in history"

# P3: `.gitattributes` marks `**/d.md merge=union` so concurrent comments
# merge without a conflict — but only while the folder keeps its name. A
# retitle is a `git mv`, so a comment written on another clone against the old
# name arrives as modify/delete and the merge stops. Exercised on one task,
# then unwound, because it needs hands to resolve.
conf_id=$(y a add "Conflict probe" -p 5 | awk '{print $2}')
sync_all
yq a set "$conf_id" --title "Conflict probe renamed by a"
sync_one a
yq b comment "$conf_id" -m "Comment written on b against the old folder name."
set +e
( cd "$out/b" && "$yman" sync ) >/dev/null 2>>"$runlog"
conf_rc=$?
set -e
if [ "$conf_rc" -ne 3 ]; then
	fail "expected exit 3 from the retitle-versus-comment merge, got $conf_rc"
fi
conf_files=$(cd "$out/b/.yman" && git diff --name-only --diff-filter=U | wc -l | tr -d ' \t')
yq b sync --abort
# Drop b's side outright: the point was the conflict, not resolving it. HEAD in
# the worktree is a symref to refs/yman/local, so this moves the ref too.
git -C "$out/b/.yman" reset -q --hard refs/yman/remote
sync_all
finding "retitle on one clone + comment on another = exit $conf_rc, $conf_files unmerged path(s);"
echo "           merge=union settles concurrent comments only while the folder keeps its name"

# P4: what do ls / show / find cost on the big tree?
bench() { # bench <clone> <reps> <yman args...>
	b_c=$1; b_reps=$2; shift 2
	b_t0=$(now_ms)
	b_i=0
	while [ "$b_i" -lt "$b_reps" ]; do
		yq "$b_c" "$@"
		b_i=$(( b_i + 1 ))
	done
	b_t1=$(now_ms)
	echo $(( (b_t1 - b_t0) / b_reps ))
}
ms_ls=$(bench a 5 ls)
ms_lsa=$(bench a 5 ls -a)
ms_show=$(bench a 5 show "$probe_id")
sp_ls=$(spawns a ls)
sp_show=$(spawns a show "$probe_id")
sp_add=$(spawns a add "Spawn-counted add")
open_n=$(y a ls | wc -l | tr -d ' \t')
all_n=$(y a ls -a | wc -l | tr -d ' \t')
echo "P4 read: ls $ms_ls ms / $sp_ls git spawns, ls -a $ms_lsa ms, show $ms_show ms / $sp_show spawns, add $sp_add spawns"
echo "           $open_n open rows, $all_n rows with -a"

# P5: did renumbering keep the tree consistent?
json=$(y a ls -a --json)
echo "$json" | grep -o '"id":"[^"]*"' | sed 's/.*:"//;s/"//' | sort > "$report/ids-final.txt"
total=$(wc -l < "$report/ids-final.txt" | tr -d ' \t')
uniq_n=$(sort -u "$report/ids-final.txt" | wc -l | tr -d ' \t')
if [ "$total" -ne "$uniq_n" ]; then
	fail "duplicate task ids after sync: $total folders, $uniq_n distinct ids"
fi
if echo "$json" | grep -q '"error":'; then
	fail "broken task folders after sync; run: yman ls -a in $out/a"
fi
( cd "$out/a/.yman" && find . -name m.yml -print ) > "$report/metas.txt"
: > "$report/related.txt"
while read -r f; do
	awk '/^related:/ { g = 1; next } g && /^- / { print $2 } g && !/^- / { g = 0 }' \
		"$out/a/.yman/$f" | tr -d "'\"" >> "$report/related.txt"
done < "$report/metas.txt"
rel_n=$(wc -l < "$report/related.txt" | tr -d ' \t')
sort -u "$report/related.txt" | comm -23 - "$report/ids-final.txt" > "$report/dangling.txt"
dangling=$(wc -l < "$report/dangling.txt" | tr -d ' \t')
echo "P5 ids:  $total tasks, all ids distinct, $rel_n related reference(s)"
if [ "$dangling" -ne 0 ]; then
	# sync rewrites `related` across the tasks the renumbering clone can see,
	# which is the pre-merge tree. A reference that arrives in the same merge
	# keeps the old id and is left pointing at nothing.
	finding "$dangling related reference(s) dangle after renumbering: $(tr '\n' ' ' < "$report/dangling.txt")"
	echo "           docs/commands.md §6 says renumbering rewrites related references; it rewrites"
	echo "           only the local side, so a reference merged in from another clone is missed"
fi

# P6: a plain `ls .yman/` is lexical, so 5.10.x sorts before 5.2.x. §5 of
# docs/storage.md only promises priority-first order, which still holds.
inversions=$(ls "$out/a/.yman" | awk -F. '
	/^[0-9]\./ {
		if (prio == $1 && prev != "" && prev + 0 > $2 + 0) n++
		prio = $1; prev = $2
	}
	END { print n + 0 }')
sample=$(ls "$out/a/.yman" | awk -F. '/^[0-9]\./ && $1 == 5' | head -n 4 | tr '\n' ' ')
echo "P6 sort: $inversions numeric inversions in a plain ls of .yman/"
echo "           $sample"

# P7: `sync` decides a task was created locally by looking for an added
# `{folder}/t.md` in `git diff --diff-filter=A -M <base> LOCAL`. A retitle is a
# `git mv` plus a one-line content change, and on a short t.md that lands under
# git's 50% rename threshold — so the task reads as new, its id is "taken on
# origin" by itself, and sync renumbers it.
p7_id=$(y a add "Rename threshold probe with a fairly long original title" -m "Body." | awk '{print $2}')
sync_all
yq b add "Unrelated, so a's next sync is a real merge" -m "Body."
sync_one b
yq a set "$p7_id" --title "Completely different wording here now"
# Score the rename the way sync will see it, before the sync moves the base.
git -C "$out/a" fetch -q origin '+refs/tasks/main:refs/yman/remote'
p7_base=$(git -C "$out/a/.yman" merge-base refs/yman/local refs/yman/remote)
p7_sim=$(git -C "$out/a/.yman" diff --name-status --find-renames=1% "$p7_base" refs/yman/local \
	| awk '/t\.md$/ && /^R/ { print $1; exit }')
sync_one a
if y a show "$p7_id" >/dev/null 2>&1; then
	echo "P7 retitle: id $p7_id survived the retitle"
else
	finding "an offline retitle renumbered task $p7_id: sync read the renamed folder as a new task"
	echo "           git scores that t.md rename ${p7_sim:-below R050}, under the default 50%, so the"
	echo "           --diff-filter=A -M probe in sync.rs sees an added t.md and mints a new id"
fi

echo ""
echo "kept:    $out   (regenerate with --force; never committed, target/ is ignored)"
if [ "$findings" -gt 0 ]; then
	echo "SYNTHETIC OK   ($findings finding(s) above)"
else
	echo "SYNTHETIC OK"
fi
