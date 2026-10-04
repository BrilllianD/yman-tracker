# `add --sections <path|->`: many tasks from one markdown file

Steps are created one `add` per call.

- Where: new `src/sections.rs`, `src/commands/add.rs` (create split), `src/cli.rs`, docs.
- Done when: each `# Title` section becomes a task with its body, fenced headings are not split, bad input fails before an id is minted, tests and docs in place.
