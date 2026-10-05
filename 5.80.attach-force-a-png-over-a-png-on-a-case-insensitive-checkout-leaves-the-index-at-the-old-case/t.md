# attach --force A.png over a.png on a case-insensitive checkout leaves the index at the old case

On `core.ignorecase=true` (macOS, Windows) the file is removed and recreated, but git's index keeps `f/a.png` while `m.yml` records `f/A.png`; Linux clones then see a dangling attachment. `docs/storage.md` says `--force` "replaces both the file and the entry". Low confidence: reasoned from git's ignorecase behaviour, not reproduced on those systems.

- Where: `src/commands/attach.rs` (the `--force` path), `docs/storage.md`.
- Done when: the rename goes through `git mv` (or `git rm` then `git add`) so the index path matches `m.yml`, or the doc states the limitation.
