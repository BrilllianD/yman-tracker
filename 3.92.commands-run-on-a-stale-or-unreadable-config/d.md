## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ay14 during config.toml conflict (sync rc 3): status rc 1: error: invalid .yman/config.toml: TOML parse error at line 5, column 1; ls rc 1; show 1 rc 1; state rc 1; Ry5 after git fetch, done 1 rc 0 with config now version = 2: task left at b/.yman/5.1.fix-login (status: done), not done/5.1.fix-login; control with yman refresh first: done/5.1.fix-login; Ay10 with strat = doing (typo of start) under [statuses]: start 1 rc 0 -> status: review; no output mentions strat; control start = doing: start 2 -> status: doing

## 2026-10-21T20:15:33Z — Bronnikov Aleksandr

84081a5: reload config after refresh/merge, read-only commands work in a config.toml conflict, warn on unknown keys

