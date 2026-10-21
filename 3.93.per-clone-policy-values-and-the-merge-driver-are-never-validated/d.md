## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ay5 status echoes '(refresh: lazy, autosync: Push, hooks: not installed)'; add under Push rc 0, not pushed (origin lacks 5.2.second); control push: note: pushed; B ls under Lazy misses Third (behind 2); Ay9 stale driver: status rc 0 silent, sync rc 3 on m.yml; control init driver: sync rc 0; hooks: checkout without yman on PATH left refs/yman/local behind, hooks status says installed

## 2026-10-21T20:26:21Z — Bronnikov Aleksandr

d70e823: typed refresh/autosync policy readers with warnings; status reports stale merge driver and yman missing from PATH

