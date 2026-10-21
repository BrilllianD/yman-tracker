## 2026-10-20T20:07:00Z — Bronnikov Aleksandr

repro: REPRODUCED: Ry8 merge renumbered 1 -> 2 (id taken on origin); origin id 2 holds 5.2.fix-logout and 5.2.third-task; A show 2: error: duplicate task id 2: 5.2.fix-logout, 5.2.third-task; Ry9 first sync rc 1 on an injected git mv fault (error: git mv failed: exit status 1); rerun rc 0 pushed 3; origin left 5.1.fix-login/f/b.txt; show 1: error: duplicate task id 1: 5.1.fix-login, 5.1.repair-login; Rr1 B sync rc 0: warning: 5.1.fix-login/f/trace.txt not moved; 5.1.repair-login/f/trace.txt already exists; A show 1 rc 1: error: duplicate task id 1: 5.1.fix-login, 5.1.repair-login

## 2026-10-21T16:09:46Z — Bronnikov Aleksandr

bdeaab2: sync merges are finished (rejoin, drop, duplicate-id gate) before commit; failures exit 3

