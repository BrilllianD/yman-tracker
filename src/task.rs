//! Task folders: the on-disk representation and everything that reads or
//! rewrites it. The folder *name* is the source of truth for priority and id;
//! `m.yml` never repeats them.

use anyhow::{Result, bail};
use chrono::{DateTime, Timelike, Utc};
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

#[derive(Clone, Debug, PartialEq)]
pub struct Meta {
    pub status: String,
    pub tags: Vec<String>,
    pub assignee: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub attachments: Vec<Attachment>,
    pub links: Vec<String>,
    pub related: Vec<String>,
    /// Top-level keys the reader does not know, in the order they were read.
    pub unknown: Vec<Unknown>,
}

/// A top-level `m.yml` key this yman does not know, kept verbatim so that a
/// rewrite by an older binary — or by a person — does not destroy it.
///
/// Invariant, upheld by `yml::parse`, the only producer: `text` starts with
/// the column-zero `key: …` line, every later line is blank or indented or a
/// sequence item, the last line is not blank, and every line ends with `\n`.
/// That is exactly what the reader hands back on the next parse, which is what
/// makes `parse(render(m)) == m` hold.
#[derive(Clone, Debug, PartialEq)]
pub struct Unknown {
    /// Unquoted; used only to collapse a repeated key.
    pub key: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
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
            unknown: Vec::new(),
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
    /// Status directory this folder sits in, `None` at the top level of
    /// `.yman`. See `docs/storage.md` §5.
    pub parent: Option<String>,
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
    Broken {
        dir: PathBuf,
        parent: Option<String>,
        error: String,
    },
}

impl Entry {
    /// Leaf folder name of this entry, parsed or not.
    pub fn dir_name(&self) -> String {
        let dir = match self {
            Entry::Task(t) => &t.dir,
            Entry::Broken { dir, .. } => dir,
        };
        dir.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    /// Path relative to `.yman`, status directory included. What error
    /// messages and `ls` print, since a bare folder name cannot tell two
    /// copies of one id apart.
    pub fn rel(&self) -> String {
        let parent = match self {
            Entry::Task(t) => &t.parent,
            Entry::Broken { parent, .. } => parent,
        };
        match parent {
            Some(p) => format!("{p}/{}", self.dir_name()),
            None => self.dir_name(),
        }
    }
}

/// Load the task folder at `dir`. `ydir` is needed to tell a top-level folder
/// from one archived under a status directory.
pub fn load(ydir: &Path, dir: &Path) -> Result<Task> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(folder) = FolderName::parse(&name) else {
        bail!("folder name does not look like {{priority}}.{{id}}.{{slug}}");
    };
    let parent = parent_of(ydir, dir)?;
    let md = std::fs::read_to_string(dir.join(MD_FILE))
        .map_err(|e| anyhow::anyhow!("cannot read {MD_FILE}: {e}"))?;
    let (title, body) = extract_title(&md)?;
    let yml = std::fs::read_to_string(dir.join(META_FILE))
        .map_err(|e| anyhow::anyhow!("cannot read {META_FILE}: {e}"))?;
    let meta: Meta =
        crate::yml::parse(&yml).map_err(|e| anyhow::anyhow!("invalid {META_FILE}: {e:#}"))?;
    Ok(Task {
        folder,
        parent,
        dir: dir.to_path_buf(),
        title,
        body,
        meta,
    })
}

/// `None` for `.yman/<task>`, `Some(status)` for `.yman/<status>/<task>`.
/// Anything deeper is not a task folder.
fn parent_of(ydir: &Path, dir: &Path) -> Result<Option<String>> {
    let Some(up) = dir.parent() else {
        bail!("task folder has no parent directory");
    };
    if up == ydir {
        return Ok(None);
    }
    if up.parent() == Some(ydir) {
        let name = up
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if crate::config::is_status_dir_name(&name) {
            return Ok(Some(name));
        }
    }
    bail!("task folder is not directly under .yman or a status directory");
}

/// The task folder a git path belongs to, as a path relative to `.yman`, with
/// its parsed name. Accepts both `5.1.x/m.yml` and `done/5.1.x/m.yml`, and
/// rejects root files such as `config.toml`.
///
/// Every place that reads paths out of git — the rename graph, the id history,
/// the collision renumber, the merge check — goes through this, so a task keeps
/// its identity when it moves into or out of a status directory.
pub fn task_path_of(path: &str) -> Option<(String, FolderName)> {
    let segs: Vec<&str> = path.split('/').collect();
    let first = *segs.first()?;
    if segs.len() >= 2
        && let Some(folder) = FolderName::parse(first)
    {
        return Some((first.to_string(), folder));
    }
    if segs.len() >= 3 && crate::config::is_status_dir_name(first) {
        let second = segs[1];
        if let Some(folder) = FolderName::parse(second) {
            return Some((format!("{first}/{second}"), folder));
        }
    }
    None
}

/// Every task folder in `ydir`, at the top level and one directory down, in
/// relative-path order. Non-directories and directories whose name is not a
/// task name are ignored entirely.
///
/// The second level is found by *grammar*, not by consulting the configured
/// terminal set: a task archived under a status that was later removed from
/// `statuses.list` must still be found, or editing the config would make tasks
/// disappear.
pub fn list(ydir: &Path) -> Result<Vec<Entry>> {
    Ok(task_dirs(ydir)?
        .into_iter()
        .map(|(parent, name, _)| {
            let dir = dir_of(ydir, &parent, &name);
            match load(ydir, &dir) {
                Ok(t) => Entry::Task(t),
                Err(e) => Entry::Broken {
                    dir,
                    parent,
                    error: format!("{e:#}"),
                },
            }
        })
        .collect())
}

/// Every task folder in `ydir` as `(status directory, leaf name, parsed name)`,
/// in relative-path order — the names only, nothing read from inside them.
/// `list` loads all of these; `find` and `fs_ids` never do.
pub fn task_dirs(ydir: &Path) -> Result<Vec<(Option<String>, String, FolderName)>> {
    let mut rels: Vec<(Option<String>, String, FolderName)> = Vec::new();
    for entry in std::fs::read_dir(ydir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(folder) = FolderName::parse(&name) {
            rels.push((None, name, folder));
        } else if crate::config::is_status_dir_name(&name) {
            for nested in std::fs::read_dir(entry.path())? {
                let nested = nested?;
                if !nested.file_type()?.is_dir() {
                    continue;
                }
                let leaf = nested.file_name().to_string_lossy().into_owned();
                if let Some(folder) = FolderName::parse(&leaf) {
                    rels.push((Some(name.clone()), leaf, folder));
                }
            }
        }
    }
    // Top-level tasks first, then each status directory, both by name.
    rels.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    Ok(rels)
}

fn dir_of(ydir: &Path, parent: &Option<String>, name: &str) -> PathBuf {
    match parent {
        Some(p) => ydir.join(p).join(name),
        None => ydir.join(name),
    }
}

/// Look a task up by id. Broken folders with the right id still error out.
///
/// The id is in the folder name, so the match is decided from directory names
/// alone and only the winning folder is read. `list` would read `t.md` and
/// `m.yml` for every task on disk, which every `show`, `set`, `comment` and
/// `attach` would then pay for.
pub fn find(ydir: &Path, id: &str) -> Result<Task> {
    let mut hits: Vec<(Option<String>, String)> = task_dirs(ydir)?
        .into_iter()
        .filter(|(_, _, folder)| folder.id == id)
        .map(|(parent, name, _)| (parent, name))
        .collect();
    match hits.len() {
        0 => Err(crate::errors::NotFound::new(format!("task {id} not found")).into()),
        1 => {
            let (parent, name) = hits.pop().expect("length checked");
            let rel = rel_of(&parent, &name);
            load(ydir, &dir_of(ydir, &parent, &name))
                .map_err(|e| anyhow::anyhow!("task {id} is broken: {rel}: {e:#}"))
        }
        _ => {
            // Relative paths, not folder names: after a bad merge both copies
            // can share a leaf name and differ only in their status directory.
            let names: Vec<String> = hits
                .iter()
                .map(|(parent, name)| rel_of(parent, name))
                .collect();
            bail!("duplicate task id {id}: {}", names.join(", "))
        }
    }
}

fn rel_of(parent: &Option<String>, name: &str) -> String {
    match parent {
        Some(p) => format!("{p}/{name}"),
        None => name.to_string(),
    }
}

impl Task {
    pub fn write_md(&self) -> Result<()> {
        std::fs::write(self.dir.join(MD_FILE), render_md(&self.title, &self.body))?;
        Ok(())
    }

    pub fn write_meta(&self) -> Result<()> {
        let yml = crate::yml::render(&self.meta);
        std::fs::write(self.dir.join(META_FILE), yml)?;
        Ok(())
    }

    pub fn touch(&mut self) {
        self.meta.updated = now();
    }

    /// Folder name, i.e. the path relative to `.yman`.
    /// Path relative to `.yman` — what every git call and `ls --json` use.
    pub fn rel(&self) -> String {
        match &self.parent {
            Some(p) => format!("{p}/{}", self.folder),
            None => self.folder.to_string(),
        }
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

    /// The file behind an entry in `m.yml`. It can be absent: nothing stops a
    /// user deleting it, and the entry outlives it until `detach` runs.
    pub fn attachment_path(&self, at: &Attachment) -> PathBuf {
        self.dir.join(FILES_DIR).join(&at.name)
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

    /// A dotted id is mis-read, not refused — the id field ends at the first
    /// dot. This is why `ids::author_prefix` validates its prefix: nothing
    /// downstream would complain about the folder it produces.
    #[test]
    fn folder_name_splits_at_the_first_dot() {
        let f = FolderName::parse("5.v.i-1.fix-login").expect("parses");
        assert_eq!(f.id, "v");
        assert_eq!(f.slug, "i-1.fix-login");
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
        for md in [
            "no heading\n# late\n",
            "## sub only\n",
            "#nospace\n",
            "",
            "   \n",
        ] {
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

    #[test]
    fn rel_includes_the_status_directory() {
        let folder = FolderName::parse("5.1.fix-login").unwrap();
        let mut t = Task {
            folder,
            parent: None,
            dir: PathBuf::from("/tmp/.yman/5.1.fix-login"),
            title: "Fix login".to_string(),
            body: String::new(),
            meta: Meta::new("todo".to_string(), Vec::new()),
        };
        assert_eq!(t.rel(), "5.1.fix-login");
        t.parent = Some("done".to_string());
        assert_eq!(t.rel(), "done/5.1.fix-login");
    }

    #[test]
    fn parent_is_derived_from_the_path() {
        let ydir = Path::new("/repo/.yman");
        assert_eq!(parent_of(ydir, &ydir.join("5.1.x")).unwrap(), None);
        assert_eq!(
            parent_of(ydir, &ydir.join("done").join("5.1.x")).unwrap(),
            Some("done".to_string())
        );
        // Two levels down is not a task folder, and neither is a parent whose
        // name could never be a status.
        assert!(parent_of(ydir, &ydir.join("a").join("b").join("5.1.x")).is_err());
        assert!(parent_of(ydir, &ydir.join("5.9.other").join("5.1.x")).is_err());
    }
}
