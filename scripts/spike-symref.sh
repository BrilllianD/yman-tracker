#!/bin/sh
# Phase-0 spike (docs/storage.md §2): does `git commit` inside a linked worktree
# whose HEAD is a symbolic ref outside refs/heads/ move that ref?
#
# Run: sh scripts/spike-symref.sh
set -eu

# Run from a hook or with an exported identity, these would point the spike's
# git calls at another repository or another author. Same set the test
# fixture scrubs (tests/common/mod.rs).
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR \
	GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_AUTHOR_DATE \
	GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL GIT_COMMITTER_DATE \
	GIT_DEFAULT_REF_FORMAT

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_GLOBAL="$tmp/gitconfig"
cat > "$GIT_CONFIG_GLOBAL" <<'CFG'
[user]
	name = Spike
	email = spike@example.invalid
[init]
	defaultBranch = main
CFG

fail() { echo "SPIKE FAIL: $*" >&2; exit 1; }

git init -q "$tmp/main"
cd "$tmp/main"
echo hello > README
git add -A
git commit -q -m "initial"

empty_tree=$(printf '' | git mktree)
root=$(git commit-tree "$empty_tree" -m "yman: init")
git update-ref refs/yman/local "$root"

printf '.yman/\n' >> .git/info/exclude
git worktree add -q --detach .yman refs/yman/local
git -C .yman symbolic-ref HEAD refs/yman/local

# --- assertion 1: commit moves refs/yman/local
mkdir -p .yman/5.1.first
printf '# First\n' > .yman/5.1.first/t.md
git -C .yman add -A
git -C .yman commit -q --no-verify -m "task(1): add"

ref=$(git rev-parse refs/yman/local)
head=$(git -C .yman rev-parse HEAD)
[ "$ref" != "$root" ] || fail "ref did not advance at all"
[ "$ref" = "$head" ] || fail "ref $ref != HEAD $head; commit did not move refs/yman/local"
echo "commit: git commit moved refs/yman/local"

# ...and HEAD is still the symbolic ref after committing.
sym=$(git -C .yman symbolic-ref -q HEAD || echo "<detached>")
[ "$sym" = refs/yman/local ] || fail "HEAD is $sym after commit, not refs/yman/local"
echo "head:    $sym"

# --- assertion 2: the ref is invisible to branch listing
branches=$(git branch -a)
case "$branches" in *yman*) fail "git branch -a mentions yman: $branches";; esac
echo "branch:  hidden (git branch -a shows only: $(echo "$branches" | tr -d ' *' | tr '\n' ' '))"

# --- assertion 3: ff-only merge of a ref built outside the worktree
blob=$(printf '# Second\n' | git hash-object -w --stdin)
sub=$(printf '100644 blob %s\tt.md\n' "$blob" | git mktree)
tree=$(printf '040000 tree %s\t5.1.first\n040000 tree %s\t5.2.second\n' \
	"$(git rev-parse "$head":5.1.first)" "$sub" | git mktree)
next=$(git commit-tree "$tree" -p "$head" -m "task(2): add")
git update-ref refs/yman/remote "$next"
git -C .yman merge -q --ff-only refs/yman/remote
[ -f .yman/5.2.second/t.md ] || fail "ff-only merge did not bring in the new task"
[ "$(git rev-parse refs/yman/local)" = "$next" ] || fail "ff-only merge did not move refs/yman/local"
echo "merge:   ff-only OK (refs/yman/local advanced to $next)"

# --- assertion 4: the main worktree never sees any of it
cd "$tmp/main"
[ -z "$(git status --porcelain)" ] || fail "main worktree is dirty: $(git status --porcelain)"
echo "status:  main worktree clean"

echo "SPIKE OK"
