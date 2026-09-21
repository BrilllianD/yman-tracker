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

use crate::task::Meta;

/// The merged metadata, or `None` when a field was changed differently on both
/// sides.
///
/// Every field is compared as a whole value — a `tags` list edited on both
/// sides conflicts even when the two edits would union cleanly. The two
/// exceptions are the timestamps, which have an obvious answer and never
/// conflict: `updated` takes the later of the two, `created` the earlier.
pub fn three_way(base: &Meta, ours: &Meta, theirs: &Meta) -> Option<Meta> {
    Some(Meta {
        status: pick(&base.status, &ours.status, &theirs.status)?,
        tags: pick(&base.tags, &ours.tags, &theirs.tags)?,
        assignee: pick(&base.assignee, &ours.assignee, &theirs.assignee)?,
        created: ours.created.min(theirs.created),
        updated: ours.updated.max(theirs.updated),
        attachments: pick(&base.attachments, &ours.attachments, &theirs.attachments)?,
        links: pick(&base.links, &ours.links, &theirs.links)?,
        related: pick(&base.related, &ours.related, &theirs.related)?,
        unknown: pick(&base.unknown, &ours.unknown, &theirs.unknown)?,
    })
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
    fn a_list_edited_on_both_sides_conflicts() {
        // Deliberate: lists are compared whole, not unioned.
        let base = meta();
        let mut ours = base.clone();
        ours.tags = vec!["api".into(), "ours".into()];
        let mut theirs = base.clone();
        theirs.tags = vec!["api".into(), "theirs".into()];
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
