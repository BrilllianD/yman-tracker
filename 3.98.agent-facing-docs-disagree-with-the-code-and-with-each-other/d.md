## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ay20 skills/yman-task/SKILL.md:80 says close with yman cancel, no v1 caveat in the 3 agent docs; v1 world: cancel 1 rc 1 'error: no cancel status configured; version 1 has none'; v2 control rc 0; Ay19 README.md:394 Scripts and agents (43 lines): -f/refusing 0 lines, re-show/stop if taken 0 lines; exit codes only as pointers: README.md:398 (the exit codes, in the sentence on docs/agents.md) and README.md:415 ('recovery from each exit-3 case', in a sentence naming skills/yman/), no rule stated; docs/agents.md 1/2/1, skills/yman/SKILL.md 8/6/0 for the same three patterns; Ay21 README.md:195-196 says unknown keys are 'dropped when it rewrites the file'; docs/storage.md:295 says they survive; x-note: keep me, hand-added at line 2 after status, survives set 1 --priority 4 (rc 0) and now sits at line 9 after related: line 8, so README is wrong; Ay18 yman guide: 0 lines match --ours|--theirs; guide:67 says yman git add the side to keep; skills/yman/SKILL.md:275 and README.md:623 say checkout --ours|--theirs first

## 2026-10-22T15:22:11Z — Bronnikov Aleksandr

8851b60: guide names --ours|--theirs, cancel fallback in all copies, README rules

