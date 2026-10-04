# validation gaps: statuses, related ids, id schemes

`statuses.list` entries are never checked, so an empty or blank entry lets `add -s ''` write `status: ''`, which the reader treats as a missing field and the task is committed broken; duplicates produce duplicate keys in `status --json`. `--relate` and `--waits-on` accept the task's own id (it becomes blocked on itself, which `waits_on` filters out, so it is blocked on nothing and never gets a note) and store values untrimmed, so `--relate " 3"` or `--relate ""` never match anything. The `random` scheme loops forever at 100% CPU once the id space is exhausted (`random_len = 2` is allowed and gives 256 ids). The `seq` scheme computes `max + 1` unchecked, so a hand-made id of `u64::MAX` panics in debug and wraps in release.

- Where: `src/config.rs` (statuses validation), `src/commands/set.rs` and `src/commands/add.rs` (related ids), `src/ids.rs` (`next_free`).
- Done when: each input is refused with a clear message, and unit or integration tests cover them.
