//! Three-way merge of `m.yml` metadata, one field at a time.
//!
//! Git's text merge works on lines, and `m.yml` writes its keys in one fixed
//! order, so a status change on one clone and a tag change on another land on
//! adjacent lines and conflict. `updated` makes it worse: it changes on every
//! edit on both sides, so it is a permanently-differing line sitting between
//! `created` and `attachments`, dragging its neighbours into conflicts they do
//! not deserve.
//!
//! This module compares whole field values instead. It is deliberately
//! all-or-nothing: a single genuinely conflicting field makes the whole merge
//! fail, and the caller falls back to `git merge-file`, so what the user has to
//! resolve by hand is exactly the file they would have seen before this
//! existed. See `src/commands/merge_driver.rs` for the git side.

use crate::task::{Attachment, Meta, Unknown};
use std::collections::{BTreeMap, BTreeSet};

/// The merged metadata, or `None` when a field was changed differently on both
/// sides.
///
/// Scalars go to the side that moved. The timestamps have an obvious answer
/// and never conflict: `updated` takes the later of the two, `created` the
/// earlier. The list fields are merged entry by entry — see [`merge_list`].
pub fn three_way(base: &Meta, ours: &Meta, theirs: &Meta) -> Option<Meta> {
    Some(Meta {
        status: pick(&base.status, &ours.status, &theirs.status)?,
        tags: merge_list(&base.tags, &ours.tags, &theirs.tags, String::clone)?,
        assignee: pick(&base.assignee, &ours.assignee, &theirs.assignee)?,
        created: ours.created.min(theirs.created),
        updated: ours.updated.max(theirs.updated),
        attachments: merge_list(
            &base.attachments,
            &ours.attachments,
            &theirs.attachments,
            |a: &Attachment| a.path.clone(),
        )?,
        links: merge_list(&base.links, &ours.links, &theirs.links, String::clone)?,
        related: merge_list(&base.related, &ours.related, &theirs.related, String::clone)?,
        unknown: merge_list(
            &base.unknown,
            &ours.unknown,
            &theirs.unknown,
            |u: &Unknown| u.key.clone(),
        )?,
    })
}

/// Merge a list entry by entry, keyed by `key`.
///
/// Every entry is an independent [`pick`] over "is it there, and with what
/// value": added on one side stays, removed on one side goes, and only an
/// entry *changed* differently on both sides conflicts. Two clones each adding
/// a tag therefore merge instead of conflicting, which whole-value comparison
/// could not do.
///
/// The result's order is the one thing this cannot take from either side.
/// Git hands the driver `%A` and `%B` the other way round on the other clone,
/// so anything derived from "ours first" would produce two different orderings
/// of the same set and conflict on the *next* sync. Instead: entries that were
/// already in the base keep the base's order, and everything new is appended
/// sorted by key. That is commutative, so both clones reach the same list.
/// Duplicate keys within one list — only reachable by hand-editing — collapse
/// to their first entry.
fn merge_list<T, K, F>(base: &[T], ours: &[T], theirs: &[T], key: F) -> Option<Vec<T>>
where
    T: Clone + PartialEq,
    K: Ord + Clone,
    F: Fn(&T) -> K,
{
    let index = |list: &[T]| -> BTreeMap<K, T> {
        let mut m = BTreeMap::new();
        for item in list {
            m.entry(key(item)).or_insert_with(|| item.clone());
        }
        m
    };
    let (b, o, t) = (index(base), index(ours), index(theirs));

    // `None` is "absent", which is a value like any other: a removal on one
    // side against an untouched other side is a removal.
    let mut keys: BTreeSet<K> = BTreeSet::new();
    keys.extend(b.keys().chain(o.keys()).chain(t.keys()).cloned());
    let mut merged: BTreeMap<K, T> = BTreeMap::new();
    for k in keys {
        let entry = pick(
            &b.get(&k).cloned(),
            &o.get(&k).cloned(),
            &t.get(&k).cloned(),
        )?;
        if let Some(v) = entry {
            merged.insert(k, v);
        }
    }

    let mut out: Vec<T> = Vec::new();
    for item in base {
        if let Some(v) = merged.remove(&key(item)) {
            out.push(v);
        }
    }
    // BTreeMap iteration is key order, which is the commutative part.
    out.extend(merged.into_values());
    Some(out)
}

/// The one rule: a side that did not move yields to the side that did.
fn pick<T: PartialEq + Clone>(base: &T, ours: &T, theirs: &T) -> Option<T> {
    if ours == theirs {
        Some(ours.clone())
    } else if ours == base {
        Some(theirs.clone())
    } else if theirs == base {
        Some(ours.clone())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{Attachment, Unknown};
    use chrono::{DateTime, Utc};

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn meta() -> Meta {
        Meta {
            status: "todo".into(),
            tags: vec!["api".into()],
            assignee: None,
            created: ts("2026-01-01T00:00:00Z"),
            updated: ts("2026-01-01T00:00:00Z"),
            attachments: Vec::new(),
            links: Vec::new(),
            related: Vec::new(),
            unknown: Vec::new(),
        }
    }

    #[test]
    fn pick_yields_to_the_side_that_moved() {
        assert_eq!(pick(&"a", &"a", &"a"), Some("a"));
        assert_eq!(pick(&"a", &"b", &"a"), Some("b"));
        assert_eq!(pick(&"a", &"a", &"b"), Some("b"));
        // The same edit on both sides is not a conflict.
        assert_eq!(pick(&"a", &"b", &"b"), Some("b"));
        assert_eq!(pick(&"a", &"b", &"c"), None);
    }

    #[test]
    fn disjoint_field_edits_merge() {
        let base = meta();
        let mut ours = base.clone();
        ours.status = "doing".into();
        ours.updated = ts("2026-01-02T00:00:00Z");
        let mut theirs = base.clone();
        theirs.tags = vec!["api".into(), "urgent".into()];
        theirs.updated = ts("2026-01-03T00:00:00Z");

        let m = three_way(&base, &ours, &theirs).expect("disjoint fields merge");
        assert_eq!(m.status, "doing");
        assert_eq!(m.tags, vec!["api".to_string(), "urgent".to_string()]);
    }

    #[test]
    fn the_same_field_changed_twice_conflicts() {
        let base = meta();
        let mut ours = base.clone();
        ours.status = "doing".into();
        let mut theirs = base.clone();
        theirs.status = "done".into();
        assert!(three_way(&base, &ours, &theirs).is_none());
    }

    #[test]
    fn a_tag_added_on_each_side_keeps_both() {
        let base = meta();
        let mut ours = base.clone();
        ours.tags = vec!["api".into(), "ours".into()];
        let mut theirs = base.clone();
        theirs.tags = vec!["api".into(), "theirs".into()];

        let m = three_way(&base, &ours, &theirs).expect("add/add unions");
        // Base entries keep the base's order; new ones are appended by key.
        assert_eq!(m.tags, ["api", "ours", "theirs"]);
    }

    /// The property the ordering rule exists for: git hands the driver the two
    /// sides the other way round on the other clone, so a merge that is not
    /// commutative produces two orderings of one set and conflicts on the next
    /// sync.
    #[test]
    fn swapping_the_two_sides_gives_the_same_list() {
        let base = meta();
        let mut ours = base.clone();
        ours.tags = vec!["api".into(), "zeta".into()];
        let mut theirs = base.clone();
        theirs.tags = vec!["api".into(), "alpha".into()];

        let a = three_way(&base, &ours, &theirs).unwrap();
        let b = three_way(&base, &theirs, &ours).unwrap();
        assert_eq!(a.tags, b.tags);
        assert_eq!(a.tags, ["api", "alpha", "zeta"]);
    }

    #[test]
    fn a_removal_beats_an_untouched_side() {
        let mut base = meta();
        base.tags = vec!["api".into(), "stale".into()];
        let mut ours = base.clone();
        ours.tags = vec!["api".into()];
        let mut theirs = base.clone();
        theirs.tags = vec!["api".into(), "stale".into(), "new".into()];

        let m = three_way(&base, &ours, &theirs).expect("remove/add is not a conflict");
        assert_eq!(m.tags, ["api", "new"]);
    }

    #[test]
    fn one_entry_changed_differently_still_conflicts() {
        // Same attachment path, different metadata on each side.
        let mut base = meta();
        let att = |by: &str| Attachment {
            path: "f/log.txt".into(),
            name: "log.txt".into(),
            added: ts("2026-01-02T00:00:00Z"),
            by: by.into(),
        };
        base.attachments = vec![att("ann")];
        let mut ours = base.clone();
        ours.attachments = vec![att("bo")];
        let mut theirs = base.clone();
        theirs.attachments = vec![att("cy")];
        assert!(three_way(&base, &ours, &theirs).is_none());
    }

    #[test]
    fn an_unknown_block_is_keyed_by_its_key() {
        let base = meta();
        let blk = |key: &str, v: &str| Unknown {
            key: key.into(),
            text: format!("{key}: {v}\n"),
        };
        let mut ours = base.clone();
        ours.unknown = vec![blk("estimate", "3d")];
        let mut theirs = base.clone();
        theirs.unknown = vec![blk("owner", "ops")];

        let m = three_way(&base, &ours, &theirs).expect("different keys union");
        assert_eq!(m.unknown.len(), 2);

        // The same key with different text is a conflict, though.
        let mut theirs = base.clone();
        theirs.unknown = vec![blk("estimate", "5d")];
        assert!(three_way(&base, &ours, &theirs).is_none());
    }

    #[test]
    fn updated_takes_the_later_value_and_created_the_earlier() {
        let base = meta();
        let mut ours = base.clone();
        ours.created = ts("2026-01-05T00:00:00Z");
        ours.updated = ts("2026-01-09T00:00:00Z");
        let mut theirs = base.clone();
        theirs.created = ts("2026-01-04T00:00:00Z");
        theirs.updated = ts("2026-01-07T00:00:00Z");

        let m = three_way(&base, &ours, &theirs).expect("timestamps never conflict");
        assert_eq!(m.created, ts("2026-01-04T00:00:00Z"));
        assert_eq!(m.updated, ts("2026-01-09T00:00:00Z"));
    }

    #[test]
    fn attachments_and_unknown_blocks_take_part() {
        let base = meta();
        let mut ours = base.clone();
        ours.attachments = vec![Attachment {
            path: "f/log.txt".into(),
            name: "log.txt".into(),
            added: ts("2026-01-02T00:00:00Z"),
            by: "ann".into(),
        }];
        let mut theirs = base.clone();
        theirs.unknown = vec![Unknown {
            key: "estimate".into(),
            text: "estimate: 3d\n".into(),
        }];

        let m = three_way(&base, &ours, &theirs).expect("different fields, no conflict");
        assert_eq!(m.attachments.len(), 1);
        assert_eq!(m.unknown.len(), 1);
    }
}
