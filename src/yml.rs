//! `m.yml`: a hand-written reader and writer for the one YAML shape yman
//! stores. The format is flat — scalars, sequences of scalars, and one
//! sequence of fixed-key maps — so a full YAML implementation costs more than
//! the dependency list should pay.
//!
//! The writer is byte-compatible with the `serde_yaml` output this replaced,
//! so upgrading yman never rewrites a task folder on its own. The reader is
//! deliberately more forgiving than the writer: `m.yml` is a file people edit
//! by hand and resolve merge conflicts in.

use crate::task::{Attachment, Meta, Unknown, format_ts};
use anyhow::{Result, bail};
use chrono::{DateTime, Utc};

// ---------------------------------------------------------------- rendering

pub fn render(meta: &Meta) -> String {
    let mut out = String::new();
    scalar(&mut out, "status", &meta.status);
    seq(&mut out, "tags", &meta.tags);
    match &meta.assignee {
        Some(a) => scalar(&mut out, "assignee", a),
        None => out.push_str("assignee: null\n"),
    }
    scalar(&mut out, "created", &format_ts(&meta.created));
    scalar(&mut out, "updated", &format_ts(&meta.updated));
    if meta.attachments.is_empty() {
        out.push_str("attachments: []\n");
    } else {
        out.push_str("attachments:\n");
        for a in &meta.attachments {
            // The first key rides on the `- `, the rest line up under it.
            out.push_str("- path: ");
            out.push_str(&emit(&a.path, 4));
            out.push_str("\n  name: ");
            out.push_str(&emit(&a.name, 4));
            out.push_str("\n  added: ");
            out.push_str(&emit(&format_ts(&a.added), 4));
            out.push_str("\n  by: ");
            out.push_str(&emit(&a.by, 4));
            out.push('\n');
        }
    }
    seq(&mut out, "links", &meta.links);
    seq(&mut out, "related", &meta.related);
    // Keys this version does not know, replayed byte for byte. They come last
    // because the known shape above is what upgrading must not disturb.
    for u in &meta.unknown {
        out.push_str(&u.text);
    }
    out
}

fn scalar(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(": ");
    out.push_str(&emit(value, 2));
    out.push('\n');
}

fn seq(out: &mut String, key: &str, values: &[String]) {
    if values.is_empty() {
        out.push_str(key);
        out.push_str(": []\n");
        return;
    }
    out.push_str(key);
    out.push_str(":\n");
    for v in values {
        out.push_str("- ");
        out.push_str(&emit(v, 2));
        out.push('\n');
    }
}

/// One scalar, as it appears after `key: ` or `- `. Quoted only when the plain
/// form would read back as something else; a value with newlines becomes a
/// literal block indented by `indent`.
fn emit(s: &str, indent: usize) -> String {
    if s.contains('\n') {
        if let Some(block) = literal_block(s, indent) {
            return block;
        }
        return double_quote(s);
    }
    if s.chars().any(is_unprintable) {
        return double_quote(s);
    }
    if plain_is_safe(s) {
        s.to_string()
    } else {
        single_quote(s)
    }
}

/// How much further in each level of the document is indented. The block
/// indentation indicator is relative to the enclosing node, so it is this
/// whatever the absolute column works out to.
const STEP: usize = 2;

/// `|-`, `|` or `|+` plus the indented body. `None` when a literal block would
/// not round-trip: leading indentation and trailing blanks on a line are lost.
fn literal_block(s: &str, indent: usize) -> Option<String> {
    let trailing = s.len() - s.trim_end_matches('\n').len();
    let body = &s[..s.len() - trailing];
    if body.is_empty() {
        // Nothing but newlines: a keep-chomped block of blank lines. The
        // caller writes the newline that ends the header line.
        return Some(format!("|{STEP}+{}", "\n".repeat(trailing)));
    }
    for line in body.split('\n') {
        // A trailing blank on a line does not survive a block scalar.
        if line.ends_with([' ', '\t']) || line.chars().any(is_unprintable) {
            return None;
        }
    }
    let pad = " ".repeat(indent);
    let mut out = String::from("|");
    // A body that opens with a blank line gives the reader nothing to infer
    // the indentation from, so it has to be spelled out.
    if body.starts_with(['\n', ' ', '\t']) {
        out.push_str(&STEP.to_string());
    }
    out.push_str(match trailing {
        0 => "-",
        1 => "",
        _ => "+",
    });
    for line in body.split('\n') {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(&pad);
            out.push_str(line);
        }
    }
    // `|+` keeps every trailing newline; the caller writes the last one.
    for _ in 1..trailing {
        out.push('\n');
    }
    Some(out)
}

fn is_unprintable(c: char) -> bool {
    (c.is_control() && c != '\n') || c == '\u{85}' || c == '\u{2028}' || c == '\u{2029}'
}

fn plain_is_safe(s: &str) -> bool {
    if s.is_empty() || resolves_to_non_string(s) {
        return false;
    }
    if s.starts_with([' ', '\t']) || s.ends_with([' ', '\t']) {
        return false;
    }
    // A leading `---` or `...` reads as a document marker.
    if s.starts_with("---") || s.starts_with("...") {
        return false;
    }
    let first = s.chars().next().expect("non-empty");
    // Indicators that open another YAML construct when they lead a scalar.
    if "#,[]{}&*!|>'\"%@`".contains(first) {
        return false;
    }
    // `-`, `?` and `:` only matter when a space (or nothing) follows.
    if "-?:".contains(first)
        && matches!(
            s[first.len_utf8()..].chars().next(),
            None | Some(' ') | Some('\t')
        )
    {
        return false;
    }
    // `: ` opens a mapping, ` #` opens a comment, a trailing `:` makes a key.
    if s.contains(": ") || s.contains(" #") || s.ends_with(':') {
        return false;
    }
    true
}

/// Would the plain form come back as null, a bool, or a number? This is the
/// YAML 1.2 core schema, which is what `serde_yaml` resolves against — `yes`
/// and `no` are plain strings, not booleans.
fn resolves_to_non_string(s: &str) -> bool {
    if matches!(
        s,
        "" | "~"
            | "null"
            | "Null"
            | "NULL"
            | "true"
            | "True"
            | "TRUE"
            | "false"
            | "False"
            | "FALSE"
    ) {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    if matches!(lower.as_str(), ".inf" | "-.inf" | "+.inf" | ".nan") {
        return true;
    }
    if s.parse::<i64>().is_ok() || s.parse::<u64>().is_ok() || s.parse::<f64>().is_ok() {
        return true;
    }
    // Shapes Rust's parsers reject but a YAML reader accepts.
    let digits = lower.strip_prefix(['-', '+']).unwrap_or(&lower);
    if let Some(hex) = digits.strip_prefix("0x") {
        return !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    if let Some(oct) = digits.strip_prefix("0o") {
        return !oct.is_empty() && oct.bytes().all(|b| b.is_ascii_digit() && b < b'8');
    }
    if let Some(bin) = digits.strip_prefix("0b") {
        return !bin.is_empty() && bin.bytes().all(|b| b == b'0' || b == b'1');
    }
    false
}

fn single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push('\'');
        }
        out.push(ch);
    }
    out.push('\'');
    out
}

fn double_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            '\u{1b}' => out.push_str("\\e"),
            c if is_unprintable(c) => {
                let n = c as u32;
                if n <= 0xff {
                    out.push_str(&format!("\\x{n:02X}"));
                } else {
                    out.push_str(&format!("\\u{n:04X}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ------------------------------------------------------------------ parsing

pub fn parse(text: &str) -> Result<Meta> {
    let mut status: Option<String> = None;
    let mut tags: Vec<String> = Vec::new();
    let mut assignee: Option<String> = None;
    let mut created: Option<DateTime<Utc>> = None;
    let mut updated: Option<DateTime<Utc>> = None;
    let mut attachments: Vec<Attachment> = Vec::new();
    let mut links: Vec<String> = Vec::new();
    let mut related: Vec<String> = Vec::new();
    let mut unknown: Vec<Unknown> = Vec::new();

    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if is_blank(line) {
            i += 1;
            continue;
        }
        if line.starts_with([' ', '\t']) || line.starts_with("- ") || line == "-" {
            bail!("unexpected indentation on line {}", i + 1);
        }
        if line == "---" || line == "..." {
            i += 1;
            continue;
        }
        let start = i;
        let (key, rest) = split_key(line, i + 1)?;
        i += 1;
        match key.as_str() {
            "status" => status = Some(scalar_value(rest, &lines, &mut i)?),
            "assignee" => assignee = opt_scalar_value(rest, &lines, &mut i)?,
            "created" => {
                created = Some(timestamp(&scalar_value(rest, &lines, &mut i)?, "created")?)
            }
            "updated" => {
                updated = Some(timestamp(&scalar_value(rest, &lines, &mut i)?, "updated")?)
            }
            "tags" => tags = seq_value(rest, &lines, &mut i)?,
            "links" => links = seq_value(rest, &lines, &mut i)?,
            "related" => related = seq_value(rest, &lines, &mut i)?,
            "attachments" => attachments = attachments_value(rest, &lines, &mut i)?,
            // Unknown keys are kept verbatim and re-emitted after the known
            // ones, so a newer yman's fields survive an older one's rewrite.
            _ => {
                skip_block(&lines, &mut i);
                push_unknown(&mut unknown, key, &lines[start..i]);
            }
        }
    }

    // A bare `status:`, `status: null` or `status: '  '` is not a status; this
    // used to read back as the empty string and pass the check below.
    let Some(status) = status.filter(|s| !s.trim().is_empty()) else {
        bail!("missing field `status`");
    };
    let Some(created) = created else {
        bail!("missing field `created`");
    };
    let Some(updated) = updated else {
        bail!("missing field `updated`");
    };
    Ok(Meta {
        status,
        tags,
        assignee,
        created,
        updated,
        attachments,
        links,
        related,
        unknown,
    })
}

/// Keep the lines an unknown key owns, so `render` can replay them.
fn push_unknown(out: &mut Vec<Unknown>, key: String, block: &[&str]) {
    // `skip_block` also eats the blank and comment lines that follow the
    // value; they belong to nobody and are not preserved anywhere else.
    let end = block
        .iter()
        .rposition(|l| !is_blank(l))
        .map_or(0, |p| p + 1);
    let mut text = String::new();
    for line in &block[..end] {
        text.push_str(line);
        text.push('\n');
    }
    // A repeated key is last-wins, the same as every known key. Replacing
    // rather than pushing also keeps the file from growing on every rewrite.
    out.retain(|u| u.key != key);
    out.push(Unknown { key, text });
}

fn is_blank(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// `key: rest` on one line. `rest` is the remainder, possibly empty.
fn split_key(line: &str, lineno: usize) -> Result<(String, &str)> {
    let line = line.trim();
    match line.find(": ") {
        Some(colon) => Ok((unquote_key(&line[..colon]), line[colon + 2..].trim_start())),
        None => match line.strip_suffix(':') {
            Some(k) => Ok((unquote_key(k), "")),
            None => bail!("line {lineno} is not `key: value`"),
        },
    }
}

fn unquote_key(k: &str) -> String {
    let k = k.trim();
    let quoted = k.len() >= 2
        && ((k.starts_with('\'') && k.ends_with('\'')) || (k.starts_with('"') && k.ends_with('"')));
    if quoted {
        return read_scalar(k).unwrap_or_else(|_| k.to_string());
    }
    k.to_string()
}

fn scalar_value(rest: &str, lines: &[&str], i: &mut usize) -> Result<String> {
    Ok(opt_scalar_value(rest, lines, i)?.unwrap_or_default())
}

/// The value of a key: inline on its own line, or a literal/folded block
/// carried by the more-indented lines that follow.
fn opt_scalar_value(rest: &str, lines: &[&str], i: &mut usize) -> Result<Option<String>> {
    if let Some(block) = block_header(rest) {
        return Ok(Some(read_block(block, lines, i, 0)));
    }
    if rest.trim().is_empty() {
        skip_block(lines, i);
        return Ok(None);
    }
    if is_null_literal(rest) {
        return Ok(None);
    }
    Ok(Some(read_scalar(rest)?))
}

fn is_null_literal(rest: &str) -> bool {
    matches!(rest.trim(), "null" | "~" | "Null" | "NULL")
}

#[derive(Clone, Copy)]
struct BlockHeader {
    folded: bool,
    /// `-` strips every trailing newline, `+` keeps them all, absent keeps one.
    chomp: Option<char>,
    /// An explicit indentation indicator, relative to the enclosing node.
    indent: Option<usize>,
}

fn block_header(rest: &str) -> Option<BlockHeader> {
    let rest = strip_comment(rest.trim()).trim_end();
    let mut chars = rest.chars();
    let folded = match chars.next()? {
        '|' => false,
        '>' => true,
        _ => return None,
    };
    let mut chomp = None;
    let mut indent = None;
    for c in chars {
        match c {
            '-' | '+' => chomp = Some(c),
            '1'..='9' => indent = Some(c as usize - '0' as usize),
            _ => return None,
        }
    }
    Some(BlockHeader {
        folded,
        chomp,
        indent,
    })
}

/// Every following line more indented than `parent`, dedented by the smallest
/// indentation among them — which is the block's own indentation, whether or
/// not the header spelled it out.
fn read_block(header: BlockHeader, lines: &[&str], i: &mut usize, parent: usize) -> String {
    let mut raw: Vec<&str> = Vec::new();
    while *i < lines.len() {
        let line = lines[*i];
        if line.trim().is_empty() {
            raw.push("");
            *i += 1;
            continue;
        }
        if indent_of(line) <= parent {
            break;
        }
        raw.push(line);
        *i += 1;
    }
    // An explicit indicator wins: it is there precisely because the body's own
    // indentation cannot be inferred.
    let base = match header.indent {
        Some(n) => parent + n,
        None => raw
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| indent_of(l))
            .min()
            .unwrap_or(0),
    };
    let mut blanks = 0;
    while raw.last().is_some_and(|l| l.trim().is_empty()) {
        raw.pop();
        blanks += 1;
    }
    let body: Vec<String> = raw
        .iter()
        .map(|l| l[base.min(l.len())..].to_string())
        .collect();
    let mut text = if header.folded {
        fold(&body)
    } else {
        body.join("\n")
    };
    // `-` strips every trailing newline, `+` keeps the blank lines that
    // followed the body, and a bare header keeps exactly one.
    match header.chomp {
        Some('-') => {}
        Some('+') => {
            let keep = blanks + usize::from(!text.is_empty());
            for _ in 0..keep {
                text.push('\n');
            }
        }
        _ => {
            if !text.is_empty() {
                text.push('\n');
            }
        }
    }
    text
}

/// Folded style: a single newline between non-empty lines becomes a space.
fn fold(body: &[String]) -> String {
    let mut out = String::new();
    for (n, line) in body.iter().enumerate() {
        if n == 0 {
            out.push_str(line);
            continue;
        }
        if line.is_empty() || out.ends_with('\n') {
            out.push('\n');
            out.push_str(line);
        } else {
            out.push(' ');
            out.push_str(line);
        }
    }
    out
}

fn seq_value(rest: &str, lines: &[&str], i: &mut usize) -> Result<Vec<String>> {
    let rest = rest.trim();
    if rest == "[]" {
        return Ok(Vec::new());
    }
    if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return inner
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(read_scalar)
            .collect();
    }
    if !rest.is_empty() {
        // A bare scalar where a list belongs; take it as a one-element list.
        return Ok(vec![read_scalar(rest)?]);
    }
    let mut out = Vec::new();
    // A block sequence may be written flush with its key or indented under it;
    // whichever the first item uses is what the rest have to match.
    let mut item_indent: Option<usize> = None;
    while *i < lines.len() {
        let line = lines[*i];
        if is_blank(line) {
            *i += 1;
            continue;
        }
        let ind = indent_of(line);
        if ind != *item_indent.get_or_insert(ind) {
            break;
        }
        let bare = line.trim_start();
        let item = match bare.strip_prefix("- ") {
            Some(item) => item,
            None if bare == "-" => "",
            None => break,
        };
        *i += 1;
        match block_header(item) {
            Some(h) => out.push(read_block(h, lines, i, ind)),
            None => out.push(read_scalar(item)?),
        }
    }
    Ok(out)
}

fn attachments_value(rest: &str, lines: &[&str], i: &mut usize) -> Result<Vec<Attachment>> {
    if rest.trim() == "[]" {
        return Ok(Vec::new());
    }
    if !rest.trim().is_empty() {
        bail!("`attachments` must be a list");
    }
    let mut out = Vec::new();
    let mut item_indent: Option<usize> = None;
    while *i < lines.len() {
        let line = lines[*i];
        if is_blank(line) {
            *i += 1;
            continue;
        }
        let ind = indent_of(line);
        if ind != *item_indent.get_or_insert(ind) {
            break;
        }
        let bare = line.trim_start();
        let first = match bare.strip_prefix("- ") {
            Some(f) => f,
            None if bare == "-" => "",
            None => break,
        };
        *i += 1;
        let mut fields: Vec<(String, String)> = Vec::new();
        if !first.trim().is_empty() {
            let (k, v) = split_key(first, *i)?;
            // The mapping inside a `- ` item starts one level in, so that is
            // what a block scalar here has to out-indent.
            let value = match block_header(v) {
                Some(h) => read_block(h, lines, i, ind + STEP),
                None => read_scalar(v)?,
            };
            fields.push((k, value));
        }
        // Continuation keys of this item are indented past the `-`; anything
        // at or before it starts the next item, or the next top-level key.
        while *i < lines.len() {
            let cont = lines[*i];
            if is_blank(cont) {
                *i += 1;
                continue;
            }
            if indent_of(cont) <= ind {
                break;
            }
            let (k, v) = split_key(cont, *i + 1)?;
            let cont_indent = indent_of(cont);
            *i += 1;
            let value = match block_header(v) {
                Some(h) => read_block(h, lines, i, cont_indent),
                None => read_scalar(v)?,
            };
            fields.push((k, value));
        }
        // Last-wins, matching the top level: whoever resolved a conflict by
        // hand most likely kept the lower half.
        let get = |name: &str| {
            fields
                .iter()
                .rev()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        };
        let (Some(path), Some(name)) = (get("path"), get("name")) else {
            bail!("an attachment is missing `path` or `name`");
        };
        let Some(added) = get("added") else {
            bail!("attachment {name} is missing `added`");
        };
        out.push(Attachment {
            path,
            name,
            added: timestamp(&added, "added")?,
            by: get("by").unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Consume whatever an unrecognised key owns: every following indented or
/// `- ` line.
fn skip_block(lines: &[&str], i: &mut usize) {
    while *i < lines.len() {
        let line = lines[*i];
        if is_blank(line) {
            *i += 1;
            continue;
        }
        if indent_of(line) > 0 || line == "-" || line.starts_with("- ") {
            *i += 1;
            continue;
        }
        break;
    }
}

/// One scalar: plain (comment stripped), single-quoted, or double-quoted.
fn read_scalar(raw: &str) -> Result<String> {
    let s = raw.trim();
    if let Some(inner) = s.strip_prefix('\'') {
        let Some(inner) = inner.strip_suffix('\'') else {
            bail!("unterminated single-quoted string: {s}");
        };
        return Ok(inner.replace("''", "'"));
    }
    if let Some(inner) = s.strip_prefix('"') {
        let Some(inner) = inner.strip_suffix('"') else {
            bail!("unterminated double-quoted string: {s}");
        };
        return unescape_double(inner);
    }
    Ok(strip_comment(s).trim_end().to_string())
}

/// A `#` opens a comment at the start of a scalar or after whitespace.
fn strip_comment(s: &str) -> &str {
    let bytes = s.as_bytes();
    for (idx, &b) in bytes.iter().enumerate() {
        if b == b'#' && (idx == 0 || bytes[idx - 1] == b' ' || bytes[idx - 1] == b'\t') {
            return &s[..idx];
        }
    }
    s
}

fn unescape_double(s: &str) -> Result<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(esc) = chars.next() else {
            bail!("trailing backslash in double-quoted string");
        };
        match esc {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '0' => out.push('\0'),
            'e' => out.push('\u{1b}'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            ' ' => out.push(' '),
            'x' | 'u' | 'U' => {
                let width = match esc {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let mut code = String::with_capacity(width);
                for _ in 0..width {
                    let Some(h) = chars.next() else {
                        bail!("truncated \\{esc} escape");
                    };
                    code.push(h);
                }
                let Ok(n) = u32::from_str_radix(&code, 16) else {
                    bail!("bad \\{esc} escape: {code}");
                };
                let Some(c) = char::from_u32(n) else {
                    bail!("bad \\{esc} escape: {code}");
                };
                out.push(c);
            }
            other => bail!("unknown escape \\{other}"),
        }
    }
    Ok(out)
}

fn timestamp(s: &str, field: &str) -> Result<DateTime<Utc>> {
    match DateTime::parse_from_rfc3339(s.trim()) {
        Ok(ts) => Ok(ts.with_timezone(&Utc)),
        Err(e) => bail!("`{field}` is not an RFC 3339 timestamp: {s}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::now;

    fn meta() -> Meta {
        let ts = DateTime::parse_from_rfc3339("2026-09-16T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        Meta {
            status: "doing".into(),
            tags: vec!["auth".into(), "bug".into()],
            assignee: Some("Ivan".into()),
            created: ts,
            updated: ts,
            attachments: vec![Attachment {
                path: "f/screenshot.png".into(),
                name: "screenshot.png".into(),
                added: ts,
                by: "Ivan".into(),
            }],
            links: vec!["https://example.com/issues/12".into()],
            related: vec!["5".into()],
            unknown: Vec::new(),
        }
    }

    #[test]
    fn renders_the_documented_shape() {
        assert_eq!(
            render(&meta()),
            "status: doing\n\
             tags:\n\
             - auth\n\
             - bug\n\
             assignee: Ivan\n\
             created: 2026-09-16T10:00:00Z\n\
             updated: 2026-09-16T10:00:00Z\n\
             attachments:\n\
             - path: f/screenshot.png\n\
             \x20 name: screenshot.png\n\
             \x20 added: 2026-09-16T10:00:00Z\n\
             \x20 by: Ivan\n\
             links:\n\
             - https://example.com/issues/12\n\
             related:\n\
             - '5'\n"
        );
    }

    #[test]
    fn empty_fields_render_flow_style() {
        let ts = now();
        let m = Meta::new("todo", vec![]);
        assert_eq!(
            render(&m),
            format!(
                "status: todo\ntags: []\nassignee: null\ncreated: {ts}\nupdated: {ts}\nattachments: []\nlinks: []\nrelated: []\n",
                ts = format_ts(&m.created)
            )
        );
        assert_eq!(parse(&render(&m)).unwrap(), m);
        let _ = ts;
    }

    #[test]
    fn round_trips_awkward_values() {
        for value in [
            "5",
            "0x1f",
            "0b101",
            "true",
            "null",
            "~",
            "",
            " padded ",
            "a: b",
            "a #b",
            "#lead",
            "- lead",
            "---",
            "...",
            "it's",
            "say \"hi\"",
            "back\\slash",
            "два слова",
            "line\nbreak",
            "trailing\n",
            "trailing\n\n",
            "\nleading",
            " indented\nlines",
            "tab\there",
            "esc\u{1b}ape",
        ] {
            let mut m = meta();
            // A blank `status` reads back as missing, so it cannot round trip
            // and is covered by `an_empty_status_is_missing` instead.
            if !value.trim().is_empty() {
                m.status = value.to_string();
            }
            m.tags = vec![value.to_string()];
            m.assignee = Some(value.to_string());
            m.attachments[0].name = value.to_string();
            m.links = vec![value.to_string()];
            let text = render(&m);
            let back = parse(&text).unwrap_or_else(|e| panic!("{value:?}: {e:#}\n{text}"));
            assert_eq!(back, m, "{value:?} round trip\n{text}");
        }
    }

    #[test]
    fn reads_what_a_person_would_write() {
        let text = "\
# a task someone edited by hand
status: doing      # still working on it
tags: [auth, bug]
assignee: 'Ivan'
created: 2026-09-16T10:00:00+02:00
updated: 2026-09-16T10:00:00Z
attachments:
  - path: f/a.png
    name: a.png
    added: 2026-09-16T10:00:00Z
    by: Ivan
links:
  -  https://example.com/issues/12
related: []
priority: 3
";
        let m = parse(text).unwrap();
        assert_eq!(m.status, "doing");
        assert_eq!(m.tags, ["auth", "bug"]);
        assert_eq!(m.assignee.as_deref(), Some("Ivan"));
        assert_eq!(format_ts(&m.created), "2026-09-16T08:00:00Z");
        assert_eq!(m.attachments.len(), 1);
        assert_eq!(m.attachments[0].name, "a.png");
        assert_eq!(m.links, ["https://example.com/issues/12"]);
        assert!(m.related.is_empty());
        // `priority` lives in the folder name, so the reader does not act on
        // it — but it does keep it, rather than eating it on the next write.
        assert_eq!(m.unknown.len(), 1);
        assert_eq!(m.unknown[0].key, "priority");
        assert_eq!(m.unknown[0].text, "priority: 3\n");
        assert!(render(&m).ends_with("priority: 3\n"));
    }

    #[test]
    fn reads_folded_and_literal_blocks() {
        let text = "\
status: |-
  one
  two
tags:
- >-
  folded
  onto one line
assignee: |
  keeps one newline
created: 2026-09-16T10:00:00Z
updated: 2026-09-16T10:00:00Z
";
        let m = parse(text).unwrap();
        assert_eq!(m.status, "one\ntwo");
        assert_eq!(m.tags, ["folded onto one line"]);
        assert_eq!(m.assignee.as_deref(), Some("keeps one newline\n"));
    }

    #[test]
    fn missing_required_fields_are_named() {
        let err = parse("tags: []\n").unwrap_err().to_string();
        assert_eq!(err, "missing field `status`");
        let err = parse("status: todo\n").unwrap_err().to_string();
        assert_eq!(err, "missing field `created`");
        let err = parse("status: todo\ncreated: 2026-09-16T10:00:00Z\n")
            .unwrap_err()
            .to_string();
        assert_eq!(err, "missing field `updated`");
    }

    #[test]
    fn a_bad_timestamp_says_which_field() {
        let err = parse("status: todo\ncreated: yesterday\nupdated: 2026-09-16T10:00:00Z\n")
            .unwrap_err()
            .to_string();
        assert!(
            err.starts_with("`created` is not an RFC 3339 timestamp: yesterday"),
            "{err}"
        );
    }

    #[test]
    fn conflict_markers_are_not_yaml() {
        let text = "\
status: todo
<<<<<<< HEAD
tags:
- a
=======
tags:
- b
>>>>>>> origin
created: 2026-09-16T10:00:00Z
updated: 2026-09-16T10:00:00Z
";
        assert!(parse(text).is_err());
    }

    #[test]
    fn unknown_blocks_survive_a_round_trip() {
        let text = "\
due: 2026-12-01
status: doing
custom:
  a: 1
  nested:
    deep: true
created: 2026-09-16T10:00:00Z
updated: 2026-09-16T10:00:00Z
estimate: |-
  two
  days
\"quoted\": 1
reviewers:
- ana
- bo

";
        let m = parse(text).unwrap();
        let keys: Vec<&str> = m.unknown.iter().map(|u| u.key.as_str()).collect();
        assert_eq!(keys, ["due", "custom", "estimate", "quoted", "reviewers"]);
        // The trailing blank line belongs to nobody and is not kept.
        assert_eq!(m.unknown[4].text, "reviewers:\n- ana\n- bo\n");

        let out = render(&m);
        let tail = out.split_once("related: []\n").unwrap().1;
        assert_eq!(
            tail,
            "due: 2026-12-01\n\
custom:\n  a: 1\n  nested:\n    deep: true\n\
estimate: |-\n  two\n  days\n\
\"quoted\": 1\n\
reviewers:\n- ana\n- bo\n"
        );
        assert_eq!(parse(&out).unwrap(), m);
    }

    #[test]
    fn unknown_duplicate_keys_are_last_wins() {
        let text = "\
status: todo
due: 1
created: 2026-09-16T10:00:00Z
updated: 2026-09-16T10:00:00Z
due: 2
";
        let m = parse(text).unwrap();
        assert_eq!(m.unknown.len(), 1);
        assert_eq!(m.unknown[0].text, "due: 2\n");
        assert_eq!(parse(&render(&m)).unwrap(), m);
    }

    #[test]
    fn an_attachment_takes_the_last_duplicate_key() {
        let text = "\
status: todo
created: 2026-09-16T10:00:00Z
updated: 2026-09-16T10:00:00Z
attachments:
- path: f/a.png
  name: a.png
  added: 2026-09-16T10:00:00Z
  by: ana
  by: bo
";
        let m = parse(text).unwrap();
        assert_eq!(m.attachments[0].by, "bo");
    }

    #[test]
    fn an_empty_status_is_missing() {
        for rest in ["", " null", " ~", " ''", " '  '"] {
            let text = format!(
                "status:{rest}\ncreated: 2026-09-16T10:00:00Z\nupdated: 2026-09-16T10:00:00Z\n"
            );
            let err = parse(&text).unwrap_err().to_string();
            assert_eq!(err, "missing field `status`", "status:{rest}");
        }
    }

    #[test]
    fn a_conflict_inside_an_unknown_block_still_fails() {
        let text = "\
status: todo
created: 2026-09-16T10:00:00Z
updated: 2026-09-16T10:00:00Z
reviewers:
<<<<<<< HEAD
- ana
=======
- bo
>>>>>>> origin
";
        assert!(parse(text).is_err());
    }
}
