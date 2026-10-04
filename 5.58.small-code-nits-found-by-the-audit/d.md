## 2026-10-04T06:40:54Z — Bronnikov Aleksandr

Found during step 38: `add -s done -e` under an archive config whose editor aborts removes the task folder but can leave an empty status directory behind. Git ignores empty directories, so `.yman` still reads clean; cosmetic.

