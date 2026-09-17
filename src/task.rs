//! Task folders: the on-disk representation and everything that reads or
//! rewrites it. The folder *name* is the source of truth for priority and id;
//! `m.yml` never repeats them.

use anyhow::{Result, bail};
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

pub const MD_FILE: &str = "t.md";
pub const META_FILE: &str = "m.yml";
pub const DISCUSSION_FILE: &str = "d.md";
pub const FILES_DIR: &str = "f";

/// Timestamps are RFC 3339, UTC, second precision.
pub fn now() -> DateTime<Utc> {
    Utc::now()
        .with_nanosecond(0)
        .expect("0 is always a valid nanosecond")
}

pub fn format_ts(ts: &DateTime<Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Meta {
    pub status: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub related: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Attachment {
    /// Relative to the task folder, always `f/<name>`.
    pub path: String,
    pub name: String,
    pub added: DateTime<Utc>,
    pub by: String,
}

impl Meta {
    pub fn new(status: impl Into<String>, tags: Vec<String>) -> Meta {
        let ts = now();
        Meta {
            status: status.into(),
            tags,
            assignee: None,
            created: ts,
            updated: ts,
            attachments: Vec::new(),
            links: Vec::new(),
            related: Vec::new(),
        }
    }
}

/// `{priority}.{id}.{slug}`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderName {
    pub priority: u8,
    pub id: String,
    pub slug: String,
}

impl FolderName {
    /// Anchored `^([0-9])\.([^./\\]+)\.(.+)$`, by hand.
    pub fn parse(name: &str) -> Option<FolderName> {
        let mut chars = name.chars();
        let priority = chars.next()?.to_digit(10)? as u8;
        if chars.next()? != '.' {
            return None;
        }
        let rest = &name[2..];
        let dot = rest.find('.')?;
        let id = &rest[..dot];
        let slug = &rest[dot + 1..];
        if id.is_empty() || slug.is_empty() {
            return None;
        }
        if id.contains(['/', '\\']) {
            return None;
        }
        Some(FolderName {
            priority,
            id: id.to_string(),
            slug: slug.to_string(),
        })
    }
}

impl std::fmt::Display for FolderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.priority, self.id, self.slug)
    }
}

/// Lowercase, keep alphanumerics, everything else becomes `-`, collapse and
/// trim runs, truncate at a char boundary, never return an empty string.
pub fn slugify(title: &str, max_bytes: usize) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in title.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch);
        } else {
            pending_dash = true;
        }
    }
    if out.len() > max_bytes {
        let mut cut = max_bytes;
        while cut > 0 && !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        out.push_str("task");
    }
    out
}

/// Split `t.md` into its H1 title and the body below it.
pub fn extract_title(md: &str) -> Result<(String, String)> {
    let mut rest = md;
    let mut title: Option<String> = None;
    loop {
        let (line, tail) = match rest.find('\n') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => (rest, ""),
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if tail.is_empty() && rest.trim().is_empty() {
                break;
            }
            rest = tail;
            continue;
        }
        if let Some(t) = trimmed.strip_prefix('#') {
            // `#` must be followed by whitespace, then a non-empty title.
            if t.starts_with(char::is_whitespace) && !t.trim().is_empty() {
                title = Some(t.trim().to_string());
                rest = tail;
                break;
            }
        }
        bail!("t.md must start with \"# Title\"");
    }
    let Some(title) = title else {
        bail!("t.md must start with \"# Title\"");
    };
    // One blank line directly under the title is separator, not body.
    let body = rest.strip_prefix('\n').unwrap_or(rest);
    Ok((title, body.to_string()))
}

pub fn render_md(title: &str, body: &str) -> String {
    let body = body.trim_matches('\n');
    if body.is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{body}\n")
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    pub folder: FolderName,
    /// Absolute.
    pub dir: PathBuf,
    pub title: String,
    pub body: String,
    pub meta: Meta,
}

// A loaded task is much bigger than a path plus an error string, and that is
// fine: `list()` returns owned entries once, and every caller matches on them
// immediately.
#[allow(clippy::large_enum_variant)]
pub enum Entry {
    Task(Task),
    Broken { dir: PathBuf, error: String },
}

impl Entry {
    /// Folder name of this entry, parsed or not.
    pub fn dir_name(&self) -> String {
        let dir = match self {
            Entry::Task(t) => &t.dir,
            Entry::Broken { dir, .. } => dir,
        };
        dir.file_name().unwrap_or_default().to_string_lossy().into_owned()
    }
}

pub fn load(dir: &Path) -> Result<Task> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(folder) = FolderName::parse(&name) else {
        bail!("folder name does not look like {{priority}}.{{id}}.{{slug}}");
    };
    let md = std::fs::read_to_string(dir.join(MD_FILE))
        .map_err(|e| anyhow::anyhow!("cannot read {MD_FILE}: {e}"))?;
    let (title, body) = extract_title(&md)?;
    let yml = std::fs::read_to_string(dir.join(META_FILE))
        .map_err(|e| anyhow::anyhow!("cannot read {META_FILE}: {e}"))?;
    let meta: Meta =
        serde_yaml::from_str(&yml).map_err(|e| anyhow::anyhow!("invalid {META_FILE}: {e}"))?;
    Ok(Task {
        folder,
        dir: dir.to_path_buf(),
        title,
        body,
        meta,
    })
}

/// Every task folder in `ydir`, in folder-name order. Non-directories and
/// directories whose name is not a task name are ignored entirely.
pub fn list(ydir: &Path) -> Result<Vec<Entry>> {
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(ydir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if FolderName::parse(&name).is_some() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names
        .into_iter()
        .map(|name| {
            let dir = ydir.join(&name);
            match load(&dir) {
                Ok(t) => Entry::Task(t),
                Err(e) => Entry::Broken {
                    dir,
                    error: format!("{e:#}"),
                },
            }
        })
        .collect())
}

/// Look a task up by id. Broken folders with the right id still error out.
pub fn find(ydir: &Path, id: &str) -> Result<Task> {
    let mut hits: Vec<Entry> = Vec::new();
    for entry in list(ydir)? {
        let name = entry.dir_name();
        if FolderName::parse(&name).map(|f| f.id == id).unwrap_or(false) {
            hits.push(entry);
        }
    }
    match hits.len() {
        0 => bail!("task {id} not found"),
        1 => match hits.pop().expect("length checked") {
            Entry::Task(t) => Ok(t),
            Entry::Broken { dir, error } => {
                let name = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
                bail!("task {id} is broken: {name}: {error}")
            }
        },
        _ => {
            let names: Vec<String> = hits.iter().map(|e| e.dir_name()).collect();
            bail!("duplicate task id {id}: {}", names.join(", "))
        }
    }
}

impl Task {
    pub fn write_md(&self) -> Result<()> {
        std::fs::write(self.dir.join(MD_FILE), render_md(&self.title, &self.body))?;
        Ok(())
    }

    pub fn write_meta(&self) -> Result<()> {
        let yml = serde_yaml::to_string(&self.meta)?;
        std::fs::write(self.dir.join(META_FILE), yml)?;
        Ok(())
    }

    pub fn touch(&mut self) {
        self.meta.updated = now();
    }

    /// Folder name, i.e. the path relative to `.yman`.
    pub fn rel(&self) -> String {
        self.folder.to_string()
    }

    pub fn id(&self) -> &str {
        &self.folder.id
    }

    pub fn priority(&self) -> u8 {
        self.folder.priority
    }

    pub fn comment_count(&self) -> usize {
        let path = self.dir.join(DISCUSSION_FILE);
        match std::fs::read_to_string(path) {
            Ok(text) => crate::discussion::parse(&text).len(),
            Err(_) => 0,
        }
    }

    pub fn attachment_count(&self) -> usize {
        self.meta.attachments.len()
    }
}

/// Sort ids the way a human reads them: numerically when they are numbers,
/// by numeric tail when they share a `prefix-`, lexically otherwise.
pub fn cmp_id(a: &str, b: &str) -> Ordering {
    let num = |s: &str| -> Option<u64> {
        if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) {
            s.parse().ok()
        } else {
            None
        }
    };
    if let (Some(x), Some(y)) = (num(a), num(b)) {
        return x.cmp(&y);
    }
    fn split(s: &str) -> Option<(&str, u64)> {
        let i = s.rfind('-')?;
        let tail = s[i + 1..].parse::<u64>().ok().filter(|_| {
            let t = &s[i + 1..];
            !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit())
        })?;
        Some((&s[..i], tail))
    }
    if let (Some((pa, ta)), Some((pb, tb))) = (split(a), split(b))
        && pa == pb
    {
        return ta.cmp(&tb);
    }
    a.cmp(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_name_round_trip() {
        for name in ["2.14.fix-login", "0.t-7f3a.a", "9.ab-12.чинить-вход"] {
            let f = FolderName::parse(name).expect(name);
            assert_eq!(f.to_string(), name);
        }
        let f = FolderName::parse("2.14.fix-login").unwrap();
        assert_eq!(f.priority, 2);
        assert_eq!(f.id, "14");
        assert_eq!(f.slug, "fix-login");
    }

    #[test]
    fn folder_name_rejects_junk() {
        for name in [
            "config.toml",
            ".git",
            "14.fix-login",
            "2..slug",
            "2.14.",
            "2.14",
            "",
            "x.14.slug",
            "2.1/4.slug",
        ] {
            assert!(FolderName::parse(name).is_none(), "accepted {name}");
        }
    }

    #[test]
    fn slug_keeps_only_alphanumerics() {
        assert_eq!(slugify("Fix login!", 200), "fix-login");
        assert_eq!(slugify("  Fix   LOGIN  ", 200), "fix-login");
        assert_eq!(slugify("a/b\\c", 200), "a-b-c");
        assert_eq!(slugify("2 + 2 = 4", 200), "2-2-4");
    }

    #[test]
    fn slug_keeps_unicode() {
        assert_eq!(slugify("Первая задача", 200), "первая-задача");
        assert_eq!(slugify("日本語 タスク", 200), "日本語-タスク");
    }

    #[test]
    fn slug_truncates_on_char_boundary() {
        let title = "я".repeat(300); // 2 bytes per char
        let s = slugify(&title, 200);
        assert!(s.len() <= 200, "{} bytes", s.len());
        assert!(s.chars().all(|c| c == 'я'));
        assert_eq!(s.len(), 200);

        let odd = slugify(&"я".repeat(300), 199);
        assert_eq!(odd.len(), 198, "must stop at a char boundary");
    }

    #[test]
    fn slug_never_ends_with_dash_after_truncation() {
        let s = slugify("aaaa bbbb cccc", 5);
        assert_eq!(s, "aaaa");
    }

    #[test]
    fn slug_empty_falls_back() {
        assert_eq!(slugify("", 200), "task");
        assert_eq!(slugify("!!! ???", 200), "task");
        assert_eq!(slugify("...", 3), "task");
    }

    #[test]
    fn title_from_plain_file() {
        let (t, b) = extract_title("# Fix login\n\nFree body.\n").unwrap();
        assert_eq!(t, "Fix login");
        assert_eq!(b, "Free body.\n");
    }

    #[test]
    fn title_skips_leading_blanks() {
        let (t, b) = extract_title("\n\n   \n# Fix login\n").unwrap();
        assert_eq!(t, "Fix login");
        assert_eq!(b, "");
    }

    #[test]
    fn title_body_keeps_markdown() {
        let md = "# T\n\n---\n\n## sub\n\ntext\n";
        let (t, b) = extract_title(md).unwrap();
        assert_eq!(t, "T");
        assert_eq!(b, "---\n\n## sub\n\ntext\n");
        assert_eq!(render_md(&t, &b), md);
    }

    #[test]
    fn title_missing_h1_errors() {
        for md in ["no heading\n# late\n", "## sub only\n", "#nospace\n", "", "   \n"] {
            let err = extract_title(md).unwrap_err().to_string();
            assert_eq!(err, "t.md must start with \"# Title\"", "for {md:?}");
        }
    }

    #[test]
    fn render_md_shapes() {
        assert_eq!(render_md("T", ""), "# T\n");
        assert_eq!(render_md("T", "body"), "# T\n\nbody\n");
        assert_eq!(render_md("T", "\n\nbody\n\n"), "# T\n\nbody\n");
    }

    #[test]
    fn id_order() {
        let mut ids = vec!["10", "2", "1", "t-b", "t-a", "ab-10", "ab-2"];
        ids.sort_by(|a, b| cmp_id(a, b));
        assert_eq!(ids, ["1", "2", "10", "ab-2", "ab-10", "t-a", "t-b"]);
    }
}
