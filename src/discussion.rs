//! `d.md` — append-only discussion log, merged with `merge=union`.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::task::{DISCUSSION_FILE, format_ts};

#[derive(Debug, PartialEq)]
pub enum Chunk {
    Comment {
        ts: String,
        author: String,
        text: String,
    },
    /// A chunk we could not parse; kept verbatim so `show` never fails on a
    /// hand-edited or union-merged file.
    Raw(String),
}

/// `d.md` as text for display. Missing: empty, silent. Not UTF-8: decoded with
/// `String::from_utf8_lossy` after a warning. Any other read error: a warning and empty.
pub fn read_for_display(path: &Path, rel: &str) -> String {
    match std::fs::read(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(e) => {
                eprintln!(
                    "warning: {rel}/{DISCUSSION_FILE} is not valid UTF-8; invalid bytes shown as U+FFFD"
                );
                String::from_utf8_lossy(e.as_bytes()).into_owned()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            eprintln!("warning: cannot read {rel}/{DISCUSSION_FILE}: {e}");
            String::new()
        }
    }
}

/// `## {rfc3339} — {author}\n\n{text}\n\n`
pub fn append_entry(path: &Path, ts: DateTime<Utc>, author: &str, text: &str) -> Result<()> {
    let mut f = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)?;
    let pad = missing_separator(&mut f)?;
    write!(
        f,
        "{pad}## {} — {}\n\n{}\n\n",
        format_ts(&ts),
        crate::task::one_line(author),
        escape_body(text)
    )?;
    Ok(())
}

/// What has to go in front of the next header so it starts its own line with a
/// blank line above it. Every entry we write ends in `\n\n`, but a hand-edited
/// tail may not, and gluing a header onto the last line of the previous comment
/// degrades the whole chunk to raw text.
fn missing_separator(f: &mut File) -> Result<&'static str> {
    let len = f.metadata()?.len();
    if len == 0 {
        return Ok("");
    }
    let n = len.min(2);
    f.seek(SeekFrom::End(-(n as i64)))?;
    let mut buf = [0u8; 2];
    let tail = &mut buf[..n as usize];
    f.read_exact(tail)?;
    Ok(if tail.ends_with(b"\n\n") {
        ""
    } else if tail.ends_with(b"\n") {
        "\n"
    } else {
        "\n\n"
    })
}

/// Indent body lines that open with `#` by one space, so a comment quoting a
/// markdown heading can never be read back as an entry header. One space is
/// enough for us and still renders as a heading everywhere else; [`unescape`]
/// is its exact inverse.
fn escape_body(text: &str) -> String {
    text.trim_end()
        .lines()
        .map(|line| {
            if line.trim_start().starts_with('#') {
                format!(" {line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn unescape(line: &str) -> &str {
    match line.strip_prefix(' ') {
        Some(rest) if rest.trim_start().starts_with('#') => rest,
        _ => line,
    }
}

pub fn parse(text: &str) -> Vec<Chunk> {
    // An editor's BOM is not part of the first header.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut entries = Vec::new();
    let mut chunk: Vec<&str> = Vec::new();
    let flush = |chunk: &mut Vec<&str>, entries: &mut Vec<Chunk>| {
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
        if parse_header(line).is_some() {
            flush(&mut chunk, &mut entries);
        }
        chunk.push(line);
    }
    flush(&mut chunk, &mut entries);
    entries
}

/// Put comments in timestamp order. `merge=union` appends the other side's
/// lines after ours in a conflicting hunk, so two comments written
/// concurrently on two clones can land in the file out of time order.
///
/// Only comments whose timestamp parses move, and only among their own
/// positions: a raw chunk, or a comment with a hand-mangled timestamp, keeps
/// its place in the file because there is nothing to order it by. The sort
/// is stable, so comments from the same second keep file order.
pub fn sort_by_time(entries: Vec<Chunk>) -> Vec<Chunk> {
    let key = |e: &Chunk| match e {
        Chunk::Comment { ts, .. } => DateTime::parse_from_rfc3339(ts).ok(),
        Chunk::Raw(_) => None,
    };
    let mut dated = Vec::new();
    let mut slots: Vec<Option<Chunk>> = Vec::new();
    for e in entries {
        if key(&e).is_some() {
            dated.push(e);
            slots.push(None);
        } else {
            slots.push(Some(e));
        }
    }
    dated.sort_by_cached_key(key);
    let mut dated = dated.into_iter();
    slots
        .into_iter()
        .map(|slot| {
            slot.or_else(|| dated.next())
                .expect("one dated entry per empty slot")
        })
        .collect()
}

/// Header is `## {ts} — {author}`; the em dash is the separator, and the
/// timestamp carries no whitespace. A `## ` line that does not match this is
/// prose — a quoted heading inside a comment — and stays part of its chunk.
fn parse_header(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## ")?;
    let (ts, author) = rest.split_once(" — ")?;
    let ts = ts.trim();
    if ts.is_empty() || ts.contains(char::is_whitespace) {
        return None;
    }
    Some((ts.to_string(), author.trim().to_string()))
}

fn parse_chunk(chunk: &str) -> Chunk {
    let mut lines = chunk.lines();
    let Some(head) = lines.next() else {
        return Chunk::Raw(chunk.to_string());
    };
    match parse_header(head) {
        Some((ts, author)) => Chunk::Comment {
            ts,
            author,
            text: lines
                .map(unescape)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string(),
        },
        None => Chunk::Raw(chunk.trim_end().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::now;

    /// A `d.md` path in a fresh directory that is removed when the guard drops.
    fn tmpfile() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("d.md");
        (dir, path)
    }

    #[test]
    fn read_for_display_missing_valid_and_invalid() {
        let (_dir, tmp) = tmpfile();
        assert_eq!(read_for_display(&tmp, "5.1.x"), "");
        std::fs::write(&tmp, "plain \u{e9}\n").unwrap();
        assert_eq!(read_for_display(&tmp, "5.1.x"), "plain \u{e9}\n");
        std::fs::write(&tmp, b"a\xe9b").unwrap();
        assert_eq!(read_for_display(&tmp, "5.1.x"), "a\u{fffd}b");
    }

    #[test]
    fn append_then_parse_round_trip() {
        let (_dir, tmp) = tmpfile();
        let ts = now();
        append_entry(&tmp, ts, "Ivan", "first\nline two").unwrap();
        append_entry(&tmp, ts, "Anna", "second  ").unwrap();
        let text = std::fs::read_to_string(&tmp).unwrap();

        assert!(text.ends_with("\n\n"));
        let entries = parse(&text);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            Chunk::Comment {
                ts: format_ts(&ts),
                author: "Ivan".into(),
                text: "first\nline two".into()
            }
        );
        assert_eq!(
            entries[1],
            Chunk::Comment {
                ts: format_ts(&ts),
                author: "Anna".into(),
                text: "second".into()
            }
        );
    }

    #[test]
    fn an_author_with_a_line_break_cannot_forge_a_comment() {
        let (_dir, tmp) = tmpfile();
        let forged = "Ivan\n## 2026-01-01T00:00:00Z — Mallory";
        append_entry(&tmp, now(), forged, "hi").unwrap();
        let entries = parse(&std::fs::read_to_string(&tmp).unwrap());
        assert_eq!(entries.len(), 1, "{entries:?}");
        let Chunk::Comment { author, .. } = &entries[0] else {
            panic!("not a comment: {entries:?}");
        };
        assert_eq!(author, "Ivan ## 2026-01-01T00:00:00Z — Mallory");
    }

    #[test]
    fn a_leading_bom_does_not_hide_the_first_comment() {
        let entries = parse("\u{feff}## 2026-09-16T10:10:00Z — Ivan\n\nhi\n");
        assert_eq!(
            entries,
            [Chunk::Comment {
                ts: "2026-09-16T10:10:00Z".into(),
                author: "Ivan".into(),
                text: "hi".into()
            }]
        );
    }

    #[test]
    fn unparsable_chunks_survive() {
        // `## junk header` is not a header — no em dash, no timestamp — so it
        // stays inside Ivan's comment instead of opening a third entry.
        let text = "hand written note\n\n## 2026-09-16T10:10:00Z — Ivan\n\nreal\n\n## junk header\n\nbody\n";
        let entries = parse(text);
        assert_eq!(entries.len(), 2);
        assert!(matches!(entries[0], Chunk::Raw(_)));
        let Chunk::Comment { text, .. } = &entries[1] else {
            panic!("expected a comment, got {:?}", entries[1]);
        };
        assert_eq!(text, "real\n\n## junk header\n\nbody");
    }

    #[test]
    fn a_heading_in_a_comment_does_not_split_the_entry() {
        let (_dir, tmp) = tmpfile();
        let ts = now();
        // Both a plain heading and a line shaped exactly like our own header.
        let body = "before\n## Heading\n## 2026-09-16T10:10:00Z — Mallory\nafter";
        append_entry(&tmp, ts, "Ivan", body).unwrap();
        let text = std::fs::read_to_string(&tmp).unwrap();

        // On disk both lines are indented by one space, so nothing downstream
        // of us can mistake them for a header either.
        assert!(text.contains("\n ## Heading\n"), "{text}");
        assert!(
            text.contains("\n ## 2026-09-16T10:10:00Z — Mallory\n"),
            "{text}"
        );

        let entries = parse(&text);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            Chunk::Comment {
                ts: format_ts(&ts),
                author: "Ivan".into(),
                text: body.into()
            }
        );
    }

    #[test]
    fn an_already_indented_heading_round_trips() {
        let (_dir, tmp) = tmpfile();
        let ts = now();
        append_entry(&tmp, ts, "Ivan", " # spaced").unwrap();
        let entries = parse(&std::fs::read_to_string(&tmp).unwrap());
        assert_eq!(entries.len(), 1);
        let Chunk::Comment { text, .. } = &entries[0] else {
            panic!("expected a comment, got {:?}", entries[0]);
        };
        // `trim` eats the leading space of the first body line, as it always
        // has; what matters is that escape and unescape did not stack up.
        assert_eq!(text, "# spaced");
    }

    #[test]
    fn a_hand_edited_tail_gets_its_separator_back() {
        for tail in ["no trailing newline", "one newline\n", "two newlines\n\n"] {
            let (_dir, tmp) = tmpfile();
            std::fs::write(&tmp, format!("## 2026-09-16T10:10:00Z — Ivan\n\n{tail}")).unwrap();
            let ts = now();
            append_entry(&tmp, ts, "Anna", "second").unwrap();
            let text = std::fs::read_to_string(&tmp).unwrap();

            assert!(
                text.contains(&format!("\n\n## {} — Anna\n", format_ts(&ts))),
                "{text}"
            );
            let entries = parse(&text);
            assert_eq!(entries.len(), 2, "{tail:?} produced {entries:?}");
            assert!(matches!(&entries[0], Chunk::Comment { author, .. } if author == "Ivan"));
            assert!(matches!(&entries[1], Chunk::Comment { author, .. } if author == "Anna"));
        }
    }

    #[test]
    fn comments_sort_by_time_around_fixed_chunks() {
        // A union merge put B's 10:05 comment after A's 10:07 one. The raw
        // chunk and the comment with an unreadable timestamp stay put.
        let text = "\
note on top

## 2026-09-16T10:07:00Z — A

later

## not-a-time — C

undated

## 2026-09-16T10:05:00Z — B

earlier

## 2026-09-16T10:05:00Z — D

same second, after B
";
        let order: Vec<String> = sort_by_time(parse(text))
            .into_iter()
            .map(|e| match e {
                Chunk::Comment { author, .. } => author,
                Chunk::Raw(raw) => raw,
            })
            .collect();
        assert_eq!(order, ["note on top", "B", "C", "D", "A"]);
    }

    #[test]
    fn empty_file_has_no_entries() {
        assert!(parse("").is_empty());
        assert!(parse("\n\n  \n").is_empty());
    }
}
