//! Tag values: what a tag may look like, and what tags are in use.
//!
//! A tag is a token, not free text. It is trimmed, lowercased, and refused
//! outright when it carries whitespace, a comma or a control character — a
//! comma is the separator of the `ls` TAGS column and of the flow sequence the
//! `m.yml` reader accepts, and whitespace makes `-t` unusable without quoting.
//! The check is the same shape as `ids::checked_prefix`: a user token is
//! validated before it can reach disk, not after.
//!
//! Reading is deliberately more forgiving than writing, as everywhere else in
//! yman: a `m.yml` edited by hand may hold anything, so nothing here is
//! applied to a stored value on the way in. What reads a stored tag folds it
//! instead, with `fold`.

use anyhow::{Result, bail};

/// A tag as typed on the command line: trimmed, checked, lowercased.
pub fn normalize(raw: &str) -> Result<String> {
    let t = raw.trim();
    if t.is_empty() {
        bail!("tag must not be empty");
    }
    if t.chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == ',')
    {
        bail!(
            "invalid tag \"{t}\"; tags must not contain whitespace, commas or control characters"
        );
    }
    Ok(t.to_lowercase())
}

/// Several tags at once. First occurrence wins, order kept — the rule `add`
/// and `set` apply to every list field. Deduping happens after normalizing, so
/// `-t UI -t ui` is one tag.
pub fn normalize_all(raw: &[String]) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for v in raw {
        let tag = normalize(v)?;
        if !out.contains(&tag) {
            out.push(tag);
        }
    }
    Ok(out)
}

/// How a stored tag compares: lowercased, because a hand-written `m.yml` is
/// not bound by what `normalize` accepts.
pub fn fold(stored: &str) -> String {
    stored.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_trims_and_lowercases() {
        for (raw, want) in [
            ("ui", "ui"),
            ("UI", "ui"),
            ("  Ui  ", "ui"),
            ("needs-review", "needs-review"),
            ("Первая", "первая"),
        ] {
            assert_eq!(normalize(raw).unwrap(), want, "for {raw:?}");
        }
    }

    #[test]
    fn normalize_refuses_an_empty_tag() {
        for raw in ["", "   ", "\t"] {
            let err = normalize(raw).unwrap_err().to_string();
            assert_eq!(err, "tag must not be empty", "for {raw:?}");
        }
    }

    #[test]
    fn normalize_refuses_separators_and_control_characters() {
        for raw in ["a,b", "needs review", "a\tb", "a\u{1}b"] {
            let err = normalize(raw).unwrap_err().to_string();
            assert!(err.starts_with("invalid tag "), "for {raw:?}, got {err:?}");
            assert!(
                err.ends_with("; tags must not contain whitespace, commas or control characters")
            );
        }
    }

    #[test]
    fn normalize_all_dedupes_after_folding() {
        let raw = ["UI".to_string(), "ui".to_string(), "auth".to_string()];
        assert_eq!(normalize_all(&raw).unwrap(), ["ui", "auth"]);
    }

    #[test]
    fn normalize_all_stops_at_the_first_bad_tag() {
        let raw = ["ui".to_string(), "a,b".to_string()];
        assert!(normalize_all(&raw).is_err());
    }
}
