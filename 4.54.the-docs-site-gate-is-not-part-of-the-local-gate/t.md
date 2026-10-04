# the docs site gate is not part of the local gate

`.github/workflows/pages.yml` runs `scripts/book.sh` on every push and pull request, but that step is in neither `.claude/skills/verify/SKILL.md` nor README "Releasing" step 4, so a broken doc link passes the local gate and fails CI. `CLAUDE.md` says changing a gate means changing both. `ci.yml` also uses an unpinned `stable` toolchain with `-D warnings`, so a new clippy lint can break CI on an unrelated commit.

- Where: `.github/workflows/ci.yml`, `.github/workflows/pages.yml`, `.claude/skills/verify/SKILL.md`, `README.md` "Releasing", `CLAUDE.md` commands.
- Done when: `book.sh` is in the verify skill and the README, and the toolchain is pinned or the policy of tracking stable is written down.
