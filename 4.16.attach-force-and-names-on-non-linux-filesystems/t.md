# `attach`: `--force` and names on non-Linux filesystems

Three related gaps. `--force` is required when the file exists on disk, not when the `m.yml` entry exists, so a listed attachment whose file is missing is overwritten silently. Entries are matched exactly, so on a case-insensitive filesystem `attach --force a.png` over `A.png` overwrites one file but adds a second entry. Names are only checked for `/`, `\`, `.` and `..`, so `:*?"<>|` pass and break a Windows checkout.

- Where: `src/commands/attach.rs`.
- Done when: the overwrite check and the entry match agree, Windows-illegal names are refused, and `docs/storage.md` §5 is updated.
