## 2026-10-20T20:07:00Z — Bronnikov Aleksandr

repro: REPRODUCED: Rr3 show --json returns tags and related as one-element lists '[a, b]' and '[]' from 'tags: [a, b]  # note' and 'related: [] # x'; set rc 0 stores - '[a, b]' and - '[]'; PyYAML: ['a', 'b'] []; Ry11 show --json assignee keeps a blank line (two newlines) between a and b for 'assignee: >-' over a, blank, b; YAML folding gives one newline (PyYAML: 'a\nb'); src/yml.rs fold() adds a second newline; Rr2 ls rc 101: panicked at src/yml.rs:515:19: start byte index 3 is not a char boundary (status: |3 over '  я'); merge-driver rc 101: panicked at src/yml.rs:515:19; not run: Ry13 (copied item loop, out of scope)

## 2026-10-21T15:37:19Z — Bronnikov Aleksandr

7235f77: m.yml reader: indicator error, flow-seq comments, YAML folding, shared next_item

