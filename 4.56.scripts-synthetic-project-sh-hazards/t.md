# `scripts/synthetic-project.sh` hazards

With `--force` the script runs `rm -rf "$out"` on whatever positional directory it was given, with no check that it lives under `target/`, so `--force .` wipes the checkout. It also does not unset `YMAN_ACTOR`, unlike the tests, so a user's environment leaks into the generated history.

- Where: `scripts/synthetic-project.sh`.
- Done when: `--force` refuses a target outside `target/`, and the script strips the same variables as the test fixture.
