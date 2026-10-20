## 2026-10-20T20:07:01Z — Bronnikov Aleksandr

repro: REPRODUCED: Ry12 ls --json rc 0 comments 0, stderr empty, after a Latin-1 byte 0xE9 (octal 351) was appended to d.md with two comments (was 2); show --json discussion_total 0 (context, blue); Ry16 comment -e rc 1: error: empty comment; editor saved a Latin-1 byte 0xE9 (octal 351), temp file gone, no commit; control UTF-8: rc 0; not run: show treating an unreadable d.md as no discussion (blue)

