## 2026-10-05T14:40:02Z — Bronnikov Aleksandr

Root cause is not timing: `comment` touches `updated`, and when that leaves m.yml byte-identical (same second) the commit holds only the d.md add, which merges cleanly against `rm`. When m.yml does change, git raises a modify/delete conflict (exit 3) instead. The deterministic repro writes d.md by hand and lets the snapshot commit it.

