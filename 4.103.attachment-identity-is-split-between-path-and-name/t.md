# attachment identity is split between path and name

storage.md says `path` is always `f/<name>`, but nothing enforces that on read. `show` locates the file by `name`, attach and detach match by case-folded `name`, and the merge driver keys entries by exact `path`. As a result, two clones attaching `A.png` and `a.png` merge into two entries that `attach` itself would have refused.

- Where: src/task.rs:518, src/commands/attach.rs:336, src/merge.rs:36, src/yml.rs:672
- Done when: `yml::parse` refuses or normalises an attachment whose `path` is not `f/<name>`, and the merge driver keys on the same identity as `attach`, with a unit test.
