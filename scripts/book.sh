#!/bin/sh
# Build the documentation site into target/book.
#
# The pages are the files GitHub already renders: README.md, docs/*.md,
# CHANGELOG.md and the agent skill. They stay where they are; this copies them
# into target/book-src, rewrites the few links that only make sense in the
# repository layout, runs mdBook, and then fails on any local link or anchor
# in the result that leads nowhere.
#
# Run: sh scripts/book.sh      (needs `mdbook` on PATH; `mdbook serve` after)
set -eu

cd "$(dirname "$0")/.."
repo=https://github.com/BrilllianD/yman-tracker
src=target/book-src
out=target/book

command -v mdbook >/dev/null 2>&1 || { echo "book: mdbook not found in PATH" >&2; exit 1; }

rm -rf "$src" "$out"
mkdir -p "$src"
cp docs/*.md "$src/"

# README links into docs/ become siblings; skills/ and LICENSE are not pages,
# so they go to GitHub. The hand-written Contents list is dropped because the
# sidebar replaces it.
sed -e 's#](docs/#](#g' \
	-e "s#](skills/#]($repo/tree/main/skills/#g" \
	-e "s#](LICENSE)#]($repo/blob/main/LICENSE)#g" \
	-e '/^## Contents$/,/^---$/d' \
	README.md > "$src/README.md"

# `## [0.4.0] - …` headings are links on GitHub; in mdBook every heading is
# already wrapped in an anchor, and a link inside one nests <a> in <a>. The
# README is the book's index page, so links to it follow.
sed -e 's/^## \[\([^]]*\)\]/## \1/' \
	-e 's#](README.md#](index.md#g' \
	CHANGELOG.md > "$src/CHANGELOG.md"

# The skill's YAML front matter is for the harness, not for readers.
awk 'NR == 1 && $0 == "---" { fm = 1; next }
	fm && $0 == "---" { fm = 0; next }
	!fm' skills/yman/SKILL.md > "$src/skill.md"

mdbook build

# Every relative href must reach a file, and every #fragment an id in it.
# Absolute paths (the 404 page's, under site-url) and external URLs are
# skipped; print.html rewrites cross-page links into fragments of itself.
bad="$src/.broken"
: > "$bad"
find "$out" -name '*.html' ! -name print.html ! -name 404.html | while IFS= read -r page; do
	dir=$(dirname "$page")
	grep -o 'href="[^"]*"' "$page" | sed 's/^href="//; s/"$//' | sort -u |
		while IFS= read -r link; do
			case "$link" in
			'' | /* | *://* | mailto:*) continue ;;
			esac
			path=${link%%#*}
			target=$page
			[ -z "$path" ] || target=$dir/$path
			if [ ! -f "$target" ]; then
				echo "${page#"$out"/}: $link (no such page)" >> "$bad"
				continue
			fi
			case "$link" in
			*'#'*)
				frag=${link#*#}
				grep -q "id=\"$frag\"" "$target" ||
					echo "${page#"$out"/}: $link (no such anchor)" >> "$bad"
				;;
			esac
		done
done
if [ -s "$bad" ]; then
	echo "book: broken links:" >&2
	sed 's/^/  /' "$bad" >&2
	exit 1
fi
echo "book: $out/index.html"
