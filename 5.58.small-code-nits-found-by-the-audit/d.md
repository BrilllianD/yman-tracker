## 2026-10-04T06:40:54Z — Bronnikov Aleksandr

Found during step 38: `add -s done -e` under an archive config whose editor aborts removes the task folder but can leave an empty status directory behind. Git ignores empty directories, so `.yman` still reads clean; cosmetic.

## 2026-10-04T06:51:26Z — Bronnikov Aleksandr

From step 40: the test-only `tmpfile()` helper in `src/discussion.rs` (~line 186) writes a predictable `temp_dir()/yman-d-<pid>-<tag>.md` through a following write. Test-only, but it should use the `tempfile` dev-dependency.

## 2026-10-04T07:16:31Z — Bronnikov Aleksandr

From step 43: an empty `core.hooksPath` turns hooks off in git, yet `yman hooks install` writes into `.git/hooks` and reports `installed`; a whitespace-only value is trimmed by `get_cfg` and treated as unset, while git treats it as a relative directory. Both predate step 43.

## 2026-10-04T07:22:00Z — Bronnikov Aleksandr

From step 44: (1) `attach` still leaves earlier copies behind if a `std::fs::copy` fails part-way (disk full), including files overwritten with `--force`; validation cannot catch that, it needs a rollback. (2) `rm` and `set::run` return a report-loading error after the commit has landed, so a committed change can exit non-zero.

## 2026-10-04T07:25:34Z — Bronnikov Aleksandr

From step 59: the comment in `src/commands/completions.rs` (~line 19, "`generate` panics on a closed pipe, and a plain write turns that into an ordinary error") now holds only on non-unix; on unix SIGPIPE kills the process first.

## 2026-10-05T14:46:07Z — Bronnikov Aleksandr

2abbb27: CRLF/BOM, comment_count, tags rename, -yman fallback, value hints, refused flags

