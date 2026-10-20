# model and code nits

- `cmp_id` and `ids::prefix_of`/`parse_all_digits` parse the id shape twice, and neither knows the random scheme (`t-0042` reads as author `t`).
- `FolderName.priority` accepts values above 9.
- `blocked` is a literal outside the status roles. That is deliberate, but storage.md does not record it.
- `Task.parent` should be named after the spec's "status directory".
- Two types are both called `Entry`.
- The rel path is built three ways, and `Task::rel` carries a stale doc line.
- The yml.rs byte-compat claim lists one difference, but likely misses more: number-like strings such as `nan`, `inf` and `0X1F` are quoted, `\a\b\v\f\N` are written as `\x..`, and non-BMP characters are written plain. This needs a one-off differential run against serde_yaml 0.9.34.
- In tests/cli.rs the shared setup block appears 31 times inline, and the `state`/hooks scenarios sit under the "flags and pinned messages" header.

- Where: src/task.rs:536, src/ids.rs:172, src/task.rs:84, src/plan.rs:17, src/task.rs:241, src/task.rs:459, src/task.rs:482, src/yml.rs:6, src/yml.rs:211, src/yml.rs:253, tests/cli.rs:921, tests/cli.rs:8806
- Done when: one id classifier; `FolderName` refuses priority > 9; the yml.rs:6 comment lists every byte difference confirmed against serde_yaml; tests/cli.rs uses a shared-setup helper and correct section headers.
