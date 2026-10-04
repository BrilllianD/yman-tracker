# `comment -e` temp file and `comment` stdin behaviour

`comment -e` writes `temp_dir()/yman-comment-<id>.md`: `fs::write` follows symlinks, so on a shared `/tmp` a planted link makes it truncate another file; two concurrent runs on one id collide; and the file is left behind when the editor fails or is refused, since it is written before `resolve_editor` runs. Separately, `comment <id>` without `-m` reads stdin to EOF whenever stdin is not a terminal, so a harness that leaves stdin open hangs, while `skills/yman/SKILL.md` calls this form safe.

- Where: `src/commands/comment.rs`, `skills/yman/SKILL.md`, `docs/agents.md`, `docs/commands.md` `comment`.
- Done when: the temp file is unique, created without following links and removed on every path, and the docs either state the stdin rule or `-m` becomes required when stdin is not a terminal.
