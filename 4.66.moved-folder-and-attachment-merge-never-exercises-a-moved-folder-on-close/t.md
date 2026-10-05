# moved_folder_and_attachment_merge never exercises a moved folder on close

The "close" case of `moved_folder_and_attachment_merge` runs on a v1 config, where closing does not move the folder, so the moved-on-close path is never tested.

- Where: `tests/cli.rs`.
- Done when: the case uses `v2_config` and asserts that the folder moved and the attachment followed it.
