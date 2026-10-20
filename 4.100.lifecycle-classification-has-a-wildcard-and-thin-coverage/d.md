## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ay16 structural, no runtime symptom today: src/cli.rs:572, the wildcard arm (_ => matches macro call), is the last arm of is_mutating (:565); 15 of 30 Cmd variants are non-mutating only through it, so a new variant compiles as non-mutating; not run: dispatch arms for doc commands, exit-3 refusal coverage, exit_code_for under .context() (blue)

