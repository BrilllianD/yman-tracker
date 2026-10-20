## 2026-10-20T20:07:00Z — Bronnikov Aleksandr

repro: REPRODUCED: Ry14 add X rc 1 (gpg failed to sign the data) left A 5.2.x/m.yml and A 5.2.x/t.md staged; they were committed under the next add Y (rc 0); set --title rc 1, attach rc 1; Rr4 (flaky: this run REPRODUCED, 10 of 14 runs reproduced) round 5: add 1 rc 0 'added 1 5.1.task-1' and add 7 rc 0 'added 1 5.1.task-7'; collided 1/5; sequential control: ids 1-8 distinct; Ry15 rm -f 1 rc 1: error: git rm failed: error: the following file has local modifications:; 5.2.other/m.yml left staged (task 2's related: 1 dropped); index empty before rm; not run: the held index.lock trigger of a failed commit

