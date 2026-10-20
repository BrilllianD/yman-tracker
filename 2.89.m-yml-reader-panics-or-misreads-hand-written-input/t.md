# m.yml reader panics or misreads hand-written input

The reader is meant to be forgiving, because `m.yml` is edited by hand and in merge conflicts (storage.md §6), but three shapes break it:
- An explicit block indentation indicator (`|3`) larger than the real indentation slices `l[base..]` mid-character and panics. That takes down `ls` for the whole tracker and the merge driver; with ASCII content it silently drops characters instead.
- `tags: [a, b]  # note` misses `strip_suffix(']')` and falls through to the bare-scalar branch, so it reads as the single tag `[a, b]`, and the next write stores `- '[a, b]'` permanently. `related: [] # x` becomes `["[]"]`.
- Folded blocks add a newline at each paragraph break (`a`, blank, `b` reads `"a\n\nb"`; YAML says `"a\nb"`).

The item loop in `seq_value` and `attachments_value` is the same 16 lines twice, so a fix to one will miss the other.

- Where: src/yml.rs:515, src/yml.rs:565, src/yml.rs:575, src/yml.rs:549, src/yml.rs:581, src/yml.rs:615
- Done when: unit tests show `status: |3\n  я` returns `Err` (no panic), `tags: [a, b]  # c` gives `["a", "b"]`, `related: [] # x` gives `[]`, `>-` over `a`/blank/`b` gives `"a\nb"`, and one item iterator serves both sequence readers.
