//! `d.md` — append-only discussion log, merged with `merge=union`.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use crate::task::format_ts;

#[derive(Debug, PartialEq)]
pub enum Entry {
    Comment {
        ts: String,
        author: String,
        text: String,
    },
    /// A chunk we could not parse; kept verbatim so `show` never fails on a
    /// hand-edited or union-merged file.
    Raw(String),
}

/// `## {rfc3339} — {author}\n\n{text}\n\n`
pub fn append_entry(path: &Path, ts: DateTime<Utc>, author: &str, text: &str) -> Result<()> {
    let mut f = OpenOptions::new().append(true).create(true).open(path)?;
    write!(
        f,
        "## {} — {}\n\n{}\n\n",
        format_ts(&ts),
        author,
        text.trim_end()
    )?;
    Ok(())
}

pub fn parse(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut chunk: Vec<&str> = Vec::new();
    let flush = |chunk: &mut Vec<&str>, entries: &mut Vec<Entry>| {
        if chunk.is_empty() {
            return;
        }
        let joined = chunk.join("\n");
        chunk.clear();
        if joined.trim().is_empty() {
            return;
        }
        entries.push(parse_chunk(&joined));
    };
    for line in text.lines() {
        if line.starts_with("## ") {
            flush(&mut chunk, &mut entries);
        }
        chunk.push(line);
    }
    flush(&mut chunk, &mut entries);
    entries
}

/// Header is `## {ts} — {author}`; the em dash is the separator.
fn parse_chunk(chunk: &str) -> Entry {
    let mut lines = chunk.lines();
    let Some(head) = lines.next() else {
        return Entry::Raw(chunk.to_string());
    };
    let parsed = head.strip_prefix("## ").and_then(|rest| {
        let (ts, author) = rest.split_once(" — ")?;
        let ts = ts.trim();
        if ts.is_empty() || ts.contains(char::is_whitespace) {
            return None;
        }
        Some((ts.to_string(), author.trim().to_string()))
    });
    match parsed {
        Some((ts, author)) => Entry::Comment {
            ts,
            author,
            text: lines.collect::<Vec<_>>().join("\n").trim().to_string(),
        },
        None => Entry::Raw(chunk.trim_end().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::now;

    #[test]
    fn append_then_parse_round_trip() {
        let tmp = std::env::temp_dir().join(format!("yman-d-{}.md", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let ts = now();
        append_entry(&tmp, ts, "Ivan", "first\nline two").unwrap();
        append_entry(&tmp, ts, "Anna", "second  ").unwrap();
        let text = std::fs::read_to_string(&tmp).unwrap();
        std::fs::remove_file(&tmp).unwrap();

        assert!(text.ends_with("\n\n"));
        let entries = parse(&text);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            Entry::Comment {
                ts: format_ts(&ts),
                author: "Ivan".into(),
                text: "first\nline two".into()
            }
        );
        assert_eq!(
            entries[1],
            Entry::Comment {
                ts: format_ts(&ts),
                author: "Anna".into(),
                text: "second".into()
            }
        );
    }

    #[test]
    fn unparsable_chunks_survive() {
        let text = "hand written note\n\n## 2026-09-16T10:10:00Z — Ivan\n\nreal\n\n## junk header\n\nbody\n";
        let entries = parse(text);
        assert_eq!(entries.len(), 3);
        assert!(matches!(entries[0], Entry::Raw(_)));
        assert!(matches!(entries[1], Entry::Comment { .. }));
        assert!(matches!(entries[2], Entry::Raw(_)));
    }

    #[test]
    fn empty_file_has_no_entries() {
        assert!(parse("").is_empty());
        assert!(parse("\n\n  \n").is_empty());
    }
}
