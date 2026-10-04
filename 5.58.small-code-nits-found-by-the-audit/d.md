## 2026-10-04T06:40:54Z — Bronnikov Aleksandr

Found during step 38: `add -s done -e` under an archive config whose editor aborts removes the task folder but can leave an empty status directory behind. Git ignores empty directories, so `.yman` still reads clean; cosmetic.

## 2026-10-04T06:51:26Z — Bronnikov Aleksandr

From step 40: the test-only `tmpfile()` helper in `src/discussion.rs` (~line 186) writes a predictable `temp_dir()/yman-d-<pid>-<tag>.md` through a following write. Test-only, but it should use the `tempfile` dev-dependency.

