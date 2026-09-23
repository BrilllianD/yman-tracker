# Documentation website

Publish README, docs/ and the changelog as an mdBook site on GitHub Pages.

- Where: `book.toml`, `docs/SUMMARY.md`, `scripts/book.sh`, `.github/workflows/pages.yml`.
- Done when: `sh scripts/book.sh` builds with no broken local links and the Pages workflow builds on every push and deploys from main.
