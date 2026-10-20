# storage, setup and errors docs lag the code

- storage.md §3 says the main runner keeps the user's hooks. Since #65, fetch, push and every `update-ref` run without hooks.
- The §4 tree shows only flat task folders, although v2 makes `done/` and `cancelled/` the default. It also omits `worktrees/-yman/YMAN_MARKED`.
- storage.md:88 says write paths ask git, but `push` reads `LOCAL` through `refs::resolve`.
- §6 never defines `related`, the `epic` tag or the `blocked` reading, and never says that richer relations or per-attachment fields need a new top-level key.
- setup.md §3 omits the ref and the worktree registration.
- errors.md's "every stderr line is prefixed" rule ignores clap's `Usage:` lines.
- "No colour anywhere" is false for clap's help and errors.
- CLAUDE.md claims 141 + 222 tests (grep finds 142 + 231) and names a `## Scripts and agents` heading that is `###`.

- Where: docs/storage.md:133, docs/storage.md:148, docs/storage.md:88, docs/storage.md:282, docs/setup.md:88, docs/errors.md:19, docs/errors.md:15, CLAUDE.md:15, CLAUDE.md:112
- Done when: each named passage matches the code it describes.
