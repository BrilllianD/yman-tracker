# rm and set can exit non-zero after their commit landed

`rm` and `set::run` load a report after committing, and an error there is returned, so a change that is already committed can exit non-zero. A script that retries on failure would then apply the change twice, or report a failure that did not happen.

- Where: `src/commands/rm.rs`, `src/commands/set.rs`.
- Done when: once the commit has landed, a later reporting failure is a warning on stderr and the exit code is 0.
