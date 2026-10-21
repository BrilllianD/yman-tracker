## 2026-10-20T20:07:00Z — Bronnikov Aleksandr

repro: REPRODUCED: Ry2 init rc 0 rewrote info/exclude (Latin-1 byte 0xE9 (octal 351) in it) to '.yman/' only; *.log rule lost: status now '?? build.log'; control UTF-8 file kept its rules + '.yman/'; Ry4 init --hooks rc 1, stdout only 'hook post-checkout: installed', stderr: error: 1 hook(s) not installed: post-merge; origin refs/tasks/main == refs/yman/local; control without foreign hook: rc 0; Ry3 rerun rc 1: error: refs/yman/local is not a yman history; 2nd rerun rc 1 same; after fault on 'worktree add' refs/yman/local has 0 files, .yman absent; control: delete ref -> init rc 0

## 2026-10-21T19:19:18Z — Bronnikov Aleksandr

0fa10cb: init keeps info/exclude bytes, finishes interrupted inits, warns on foreign hooks

