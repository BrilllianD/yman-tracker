# `cmp_id` is not a total order

`cmp_id` compares numerically when both ids are numbers, by numeric tail when they share a `prefix-`, and lexically otherwise. The three rules can contradict each other: `9 < 10` numerically, `"10" < "5-a"` and `"5-a" < "9"` lexically, a cycle. It is reachable with an author prefix starting with a digit mixed with seq ids, or with a changed `random_len`. Since Rust 1.81 `sort_by` may panic on a non-total order, and this affects `ls`, `plan`, `show` and `report_closed`.

- Where: `src/task.rs` (`cmp_id`).
- Done when: ids sort by a single key that is a total order (for example a tuple of class, prefix and numeric tail), and a unit test sorts a mixed set without panicking.
