# `add --sections` walks history per section and mis-handles fences

`create` calls `ids::new_id` for every section, which runs `taken_ids` and `ever_assigned`, a `git log --name-only` over all of LOCAL and REMOTE plus a readdir, so k sections cost k full history walks. In `sections.rs`, an unclosed fence silently swallows every later heading into one task where the other malformed inputs are refused, and a closing fence with an info string (a ```` ```sh ```` line inside an open block) closes it, so `# ` lines in a nested example become spurious tasks.

- Where: `src/commands/add.rs` (`create`, `run` for `--sections`), `src/sections.rs`, `src/ids.rs` (`new_id`), `docs/commands.md` `add --sections`.
- Done when: the taken set is computed once and extended locally per section, an unclosed fence is refused with a `<source>: line <n>: ...` message, closing fences carry no info string, and tests cover all three.
