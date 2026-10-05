#!/usr/bin/env bash
# A plan file in the working copy, three `# ` sections.
set -eu
cat > rollout.md <<'MD'
# Add the rollout flag

Gate the new login behind a flag, off by default.

# Migrate existing sessions

Move old sessions to the new store before the flag flips.

# Remove the old login page

After a week of the flag at 100%.
MD
