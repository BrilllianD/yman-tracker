## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ry1 identical state set with untracked scratch.txt rc 1: git commit failed: nothing added to commit but untracked files present; clean-tree control rc 0; Ry10 --continue rc 3: error: conflict markers or invalid task in 5.1.fix-login: d.md still has merge markers; the '=======' line comes from a comment; control after replacing it: rc 0; not run: failures after a stopped merge that exit 1 instead of 3 (blue)

## 2026-10-21T15:22:17Z — Bronnikov Aleksandr

e52e8c5: bare ======= only counts as leftover separator; nothing-staged variants succeed; stopped-merge failures exit 3

