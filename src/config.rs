//! `.yman/config.toml` — committed, shared by everyone on the project.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CONFIG_FILE: &str = "config.toml";

/// Hard upper bound on `slug.max_bytes`, independent of what the file says.
pub const SLUG_MAX_BYTES_CAP: usize = 240;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub version: u32,
    pub ids: IdsCfg,
    pub statuses: StatusesCfg,
    #[serde(default)]
    pub priorities: PrioCfg,
    #[serde(default)]
    pub slug: SlugCfg,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IdsCfg {
    pub scheme: Scheme,
    #[serde(default = "default_random_len")]
    pub random_len: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Random,
    Seq,
    Author,
}

impl Scheme {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scheme::Random => "random",
            Scheme::Seq => "seq",
            Scheme::Author => "author",
        }
    }
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StatusesCfg {
    pub list: Vec<String>,
    pub default: String,
    /// Status `yman start` moves to. `None` falls back to `list[1]`.
    #[serde(default)]
    pub start: Option<String>,
    /// Status `yman done` moves to. `None` falls back to `list.last()`.
    #[serde(default)]
    pub done: Option<String>,
    /// Status `yman cancel` moves to. No fallback; the command needs it set.
    #[serde(default)]
    pub cancel: Option<String>,
    /// Closed statuses: hidden by `ls`, and archived one directory down.
    /// `None` means "just the done status", which is what version 1 did.
    #[serde(default)]
    pub terminal: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PrioCfg {
    pub default: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SlugCfg {
    pub max_bytes: usize,
}

fn default_random_len() -> u8 {
    4
}

impl Default for PrioCfg {
    fn default() -> Self {
        PrioCfg { default: 5 }
    }
}

impl Default for SlugCfg {
    fn default() -> Self {
        SlugCfg { max_bytes: 200 }
    }
}

impl Config {
    /// The config written by `yman init`.
    pub fn new(scheme: Scheme) -> Config {
        Config {
            version: 1,
            ids: IdsCfg {
                scheme,
                random_len: default_random_len(),
            },
            statuses: StatusesCfg {
                list: vec!["todo".into(), "doing".into(), "done".into()],
                default: "todo".into(),
                start: None,
                done: None,
                cancel: None,
                terminal: None,
            },
            priorities: PrioCfg::default(),
            slug: SlugCfg::default(),
        }
    }

    /// Status that `yman start` moves a task to: `statuses.start`, or the
    /// second entry in the list.
    pub fn start_status(&self) -> &str {
        match &self.statuses.start {
            Some(s) => s,
            None => self
                .statuses
                .list
                .get(1)
                .expect("validated: list has >= 2 entries"),
        }
    }

    /// Status that `yman done` moves a task to: `statuses.done`, or the last
    /// entry in the list.
    pub fn done_status(&self) -> &str {
        match &self.statuses.done {
            Some(s) => s,
            None => self
                .statuses
                .list
                .last()
                .expect("validated: list has >= 2 entries"),
        }
    }

    /// Status that `yman cancel` moves a task to. There is no fallback: a
    /// project that wants the verb says which status it means.
    pub fn cancel_status(&self) -> Option<&str> {
        self.statuses.cancel.as_deref()
    }

    /// Is this a closed status? `ls` hides these, and version 2 archives them.
    pub fn is_terminal(&self, s: &str) -> bool {
        match &self.statuses.terminal {
            Some(t) => t.iter().any(|x| x == s),
            None => s == self.done_status(),
        }
    }

    pub fn has_status(&self, s: &str) -> bool {
        self.statuses.list.iter().any(|x| x == s)
    }

    /// Index of a status in the configured order; unknown statuses sort last.
    pub fn status_index(&self, s: &str) -> usize {
        self.statuses
            .list
            .iter()
            .position(|x| x == s)
            .unwrap_or(self.statuses.list.len())
    }

    pub fn statuses_joined(&self) -> String {
        self.statuses.list.join(", ")
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 && self.version != 2 {
            bail!(
                "invalid .yman/config.toml: unsupported version {} (this yman understands 1 and 2)",
                self.version
            );
        }
        if self.statuses.list.len() < 2 {
            bail!("invalid .yman/config.toml: statuses.list needs at least 2 entries");
        }
        if !self.has_status(&self.statuses.default) {
            bail!(
                "invalid .yman/config.toml: statuses.default \"{}\" is not in statuses.list",
                self.statuses.default
            );
        }
        self.validate_statuses()?;
        if self.priorities.default > 9 {
            bail!(
                "invalid .yman/config.toml: priorities.default {} is not in 0..=9",
                self.priorities.default
            );
        }
        if self.slug.max_bytes < 1 || self.slug.max_bytes > SLUG_MAX_BYTES_CAP {
            bail!(
                "invalid .yman/config.toml: slug.max_bytes {} is not in 1..={SLUG_MAX_BYTES_CAP}",
                self.slug.max_bytes
            );
        }
        if self.ids.random_len < 2 || self.ids.random_len > 16 {
            bail!(
                "invalid .yman/config.toml: ids.random_len {} is not in 2..=16",
                self.ids.random_len
            );
        }
        Ok(())
    }

    /// The `[statuses]` rules that arrived with version 2. Split out because
    /// `validate` was already long enough.
    fn validate_statuses(&self) -> Result<()> {
        // The roles and the terminal set are a version 2 feature. A version 1
        // config must keep meaning exactly what it meant before they existed.
        for (key, set) in [
            ("start", self.statuses.start.is_some()),
            ("done", self.statuses.done.is_some()),
            ("cancel", self.statuses.cancel.is_some()),
            ("terminal", self.statuses.terminal.is_some()),
        ] {
            if set && self.version < 2 {
                bail!(
                    "invalid .yman/config.toml: statuses.{key} needs version = 2; \
                     bump version in .yman/config.toml"
                );
            }
        }

        for (key, value) in [
            ("start", &self.statuses.start),
            ("done", &self.statuses.done),
            ("cancel", &self.statuses.cancel),
        ] {
            if let Some(v) = value
                && !self.has_status(v)
            {
                bail!("invalid .yman/config.toml: statuses.{key} \"{v}\" is not in statuses.list");
            }
        }

        if let Some(terminal) = &self.statuses.terminal {
            for (i, e) in terminal.iter().enumerate() {
                if !self.has_status(e) {
                    bail!(
                        "invalid .yman/config.toml: statuses.terminal entry \"{e}\" \
                         is not in statuses.list"
                    );
                }
                if !is_status_dir_name(e) {
                    bail!(
                        "invalid .yman/config.toml: statuses.terminal entry \"{e}\" is not a \
                         usable directory name; use letters, digits, \"_\" and \"-\""
                    );
                }
                for prev in &terminal[..i] {
                    if prev == e {
                        bail!("invalid .yman/config.toml: statuses.terminal lists \"{e}\" twice");
                    }
                    // Two names differing only in case are one directory on
                    // macOS and Windows, so the tasks would silently merge.
                    if prev.eq_ignore_ascii_case(e) {
                        bail!(
                            "invalid .yman/config.toml: statuses.terminal entries \"{prev}\" and \
                             \"{e}\" differ only in case; they would collide on a \
                             case-insensitive filesystem"
                        );
                    }
                }
            }
            if !self.is_terminal(self.done_status()) {
                bail!(
                    "invalid .yman/config.toml: statuses.done \"{}\" is not in statuses.terminal",
                    self.done_status()
                );
            }
        }
        if let Some(c) = self.cancel_status()
            && !self.is_terminal(c)
        {
            bail!("invalid .yman/config.toml: statuses.cancel \"{c}\" is not in statuses.terminal");
        }

        // Only version 2 archives, and only version 2 can name these roles, so
        // gating here keeps every legal version 1 config legal. A list of
        // ["todo", "done"] derives start == done == the terminal status, which
        // would trip all three rules below while working perfectly well.
        if self.version >= 2 {
            if self.is_terminal(&self.statuses.default) {
                bail!(
                    "invalid .yman/config.toml: statuses.terminal must not contain \
                     statuses.default \"{}\"",
                    self.statuses.default
                );
            }
            if self.is_terminal(self.start_status()) {
                bail!(
                    "invalid .yman/config.toml: statuses.terminal must not contain the start \
                     status \"{}\"",
                    self.start_status()
                );
            }
            if self.statuses.list.iter().all(|s| self.is_terminal(s)) {
                bail!(
                    "invalid .yman/config.toml: statuses.terminal marks every status terminal; \
                     at least one must stay open"
                );
            }
        }
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Config> {
        let cfg: Config = match toml::from_str(text) {
            Ok(c) => c,
            Err(e) => bail!("invalid .yman/config.toml: {}", first_line(&e.to_string())),
        };
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn load(ydir: &Path) -> Result<Config> {
        let path = ydir.join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                bail!("invalid .yman/config.toml: file is missing")
            }
            Err(e) => bail!("invalid .yman/config.toml: {e}"),
        };
        Config::parse(&text)
    }

    pub fn render(&self) -> String {
        // Hand-rolled so the committed file keeps its comments and section
        // order; `toml::to_string` would drop both.
        format!(
            "version = {}\n\n\
             [ids]\n\
             scheme = \"{}\"        # \"random\" | \"seq\" | \"author\"\n\
             random_len = {}        # hex chars, random scheme only\n\n\
             [statuses]\n\
             list = [{}]\n\
             default = \"{}\"\n{}\n\
             [priorities]\n\
             default = {}           # 0..=9\n\n\
             [slug]\n\
             max_bytes = {}         # hard cap {}\n",
            self.version,
            self.ids.scheme,
            self.ids.random_len,
            self.statuses
                .list
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", "),
            self.statuses.default,
            self.render_status_roles(),
            self.priorities.default,
            self.slug.max_bytes,
            SLUG_MAX_BYTES_CAP,
        )
    }

    /// The optional `[statuses]` keys, as lines to append after `default`.
    /// Empty for version 1, so a version 1 file renders byte for byte as it
    /// did before these keys existed.
    fn render_status_roles(&self) -> String {
        if self.version < 2 {
            return String::new();
        }
        let mut out = String::new();
        if let Some(v) = &self.statuses.start {
            out.push_str(&format!(
                "start = \"{v}\"        # default: second entry in list\n"
            ));
        }
        if let Some(v) = &self.statuses.done {
            out.push_str(&format!(
                "done = \"{v}\"         # default: last entry in list\n"
            ));
        }
        if let Some(v) = &self.statuses.cancel {
            out.push_str(&format!(
                "cancel = \"{v}\"       # no default; `yman cancel` needs it\n"
            ));
        }
        if let Some(v) = &self.statuses.terminal {
            let joined = v
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "terminal = [{joined}]  # closed; archived under .yman/<status>/\n"
            ));
        }
        out
    }

    pub fn save(&self, ydir: &Path) -> Result<()> {
        std::fs::write(ydir.join(CONFIG_FILE), self.render())?;
        Ok(())
    }
}

/// A terminal status doubles as a directory name under `.yman`, so it has to
/// survive being one. Checked by hand; there is no `regex` dependency.
///
/// The grammar also does the work of a reserved-name list: `config.toml`,
/// `.git`, `.gitignore`, `t.md` and any `{priority}.{id}.{slug}` folder all
/// contain `.`, which this rejects. A leading `-` is out because `git mv -x`
/// would read it as a flag.
pub fn is_status_dir_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> String {
        Config::new(Scheme::Seq).render()
    }

    #[test]
    fn round_trip_default() {
        let cfg = Config::parse(&base()).unwrap();
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.ids.scheme, Scheme::Seq);
        assert_eq!(cfg.ids.random_len, 4);
        assert_eq!(cfg.statuses.default, "todo");
        assert_eq!(cfg.start_status(), "doing");
        assert_eq!(cfg.done_status(), "done");
        assert_eq!(cfg.priorities.default, 5);
        assert_eq!(cfg.slug.max_bytes, 200);
    }

    #[test]
    fn optional_sections_default() {
        let cfg = Config::parse(
            "version = 1\n[ids]\nscheme = \"random\"\n\
             [statuses]\nlist = [\"a\", \"b\"]\ndefault = \"a\"\n",
        )
        .unwrap();
        assert_eq!(cfg.priorities.default, 5);
        assert_eq!(cfg.slug.max_bytes, 200);
        assert_eq!(cfg.ids.random_len, 4);
    }

    fn reject(text: &str, needle: &str) {
        let err = Config::parse(text).unwrap_err().to_string();
        assert!(
            err.starts_with("invalid .yman/config.toml: "),
            "bad prefix: {err}"
        );
        assert!(err.contains(needle), "{err} does not mention {needle}");
    }

    #[test]
    fn rejects_bad_version() {
        reject(&base().replace("version = 1", "version = 3"), "version 3");
    }

    #[test]
    fn rejects_short_status_list() {
        reject(
            &base().replace(
                "list = [\"todo\", \"doing\", \"done\"]",
                "list = [\"todo\"]",
            ),
            "at least 2",
        );
    }

    #[test]
    fn rejects_default_outside_list() {
        reject(
            &base().replace("default = \"todo\"", "default = \"nope\""),
            "nope",
        );
    }

    #[test]
    fn rejects_priority_out_of_range() {
        reject(&base().replace("default = 5 ", "default = 10 "), "0..=9");
    }

    #[test]
    fn rejects_slug_cap() {
        reject(
            &base().replace("max_bytes = 200", "max_bytes = 241"),
            "1..=240",
        );
        reject(
            &base().replace("max_bytes = 200", "max_bytes = 0"),
            "1..=240",
        );
    }

    #[test]
    fn rejects_random_len() {
        reject(
            &base().replace("random_len = 4", "random_len = 1"),
            "2..=16",
        );
        reject(
            &base().replace("random_len = 4", "random_len = 17"),
            "2..=16",
        );
    }

    #[test]
    fn rejects_unknown_scheme() {
        reject(&base().replace("\"seq\"", "\"uuid\""), "");
    }

    #[test]
    fn rejects_garbage() {
        reject("this is not toml at all {", "");
    }

    /// A version 2 config with the roles and the terminal set spelled out.
    fn v2() -> String {
        "version = 2\n[ids]\nscheme = \"seq\"\n\
         [statuses]\n\
         list = [\"todo\", \"doing\", \"blocked\", \"done\", \"cancelled\"]\n\
         default = \"todo\"\n\
         start = \"doing\"\n\
         done = \"done\"\n\
         cancel = \"cancelled\"\n\
         terminal = [\"done\", \"cancelled\"]\n"
            .to_string()
    }

    #[test]
    fn version_two_names_its_roles() {
        let cfg = Config::parse(&v2()).unwrap();
        assert_eq!(cfg.start_status(), "doing");
        assert_eq!(cfg.done_status(), "done");
        assert_eq!(cfg.cancel_status(), Some("cancelled"));
        assert!(cfg.is_terminal("done"));
        assert!(cfg.is_terminal("cancelled"));
        assert!(!cfg.is_terminal("blocked"));
        assert!(!cfg.is_terminal("todo"));
    }

    /// Positional fallbacks, so an untouched file keeps its meaning.
    #[test]
    fn version_one_derives_the_roles_positionally() {
        let cfg = Config::parse(&base()).unwrap();
        assert_eq!(cfg.start_status(), "doing");
        assert_eq!(cfg.done_status(), "done");
        assert_eq!(cfg.cancel_status(), None);
        assert!(cfg.is_terminal("done"));
        assert!(!cfg.is_terminal("doing"));
    }

    #[test]
    fn version_two_round_trips() {
        let cfg = Config::parse(&v2()).unwrap();
        let again = Config::parse(&cfg.render()).unwrap();
        assert_eq!(again.statuses.start.as_deref(), Some("doing"));
        assert_eq!(again.statuses.done.as_deref(), Some("done"));
        assert_eq!(again.statuses.cancel.as_deref(), Some("cancelled"));
        assert_eq!(
            again.statuses.terminal.as_deref(),
            Some(["done".to_string(), "cancelled".to_string()].as_slice())
        );
    }

    #[test]
    fn rejects_new_keys_on_a_version_one_config() {
        for key in ["start = \"doing\"", "done = \"done\"", "cancel = \"done\""] {
            reject(
                &base().replace("default = \"todo\"", &format!("default = \"todo\"\n{key}")),
                "needs version = 2",
            );
        }
        reject(
            &base().replace(
                "default = \"todo\"",
                "default = \"todo\"\nterminal = [\"done\"]",
            ),
            "needs version = 2",
        );
    }

    /// `["todo", "done"]` derives start == done == terminal. It is legal today
    /// and must stay legal, which is why three of the rules are version-gated.
    #[test]
    fn a_two_status_version_one_list_stays_legal() {
        let text = base().replace(
            "list = [\"todo\", \"doing\", \"done\"]",
            "list = [\"todo\", \"done\"]",
        );
        let cfg = Config::parse(&text).unwrap();
        assert_eq!(cfg.start_status(), "done");
        assert_eq!(cfg.done_status(), "done");
    }

    #[test]
    fn rejects_roles_outside_the_list() {
        for (key, needle) in [
            ("start", "statuses.start"),
            ("done", "statuses.done"),
            ("cancel", "statuses.cancel"),
        ] {
            let text = v2().replace(&format!("{key} = "), &format!("{key} = \"nope\" # "));
            reject(&text, needle);
            reject(&text, "is not in statuses.list");
        }
    }

    #[test]
    fn rejects_a_terminal_entry_outside_the_list() {
        reject(
            &v2().replace(
                "terminal = [\"done\", \"cancelled\"]",
                "terminal = [\"nope\"]",
            ),
            "statuses.terminal entry \"nope\" is not in statuses.list",
        );
    }

    #[test]
    fn rejects_a_terminal_entry_that_is_not_a_directory_name() {
        let text = v2()
            .replace(
                "list = [\"todo\", \"doing\", \"blocked\", \"done\", \"cancelled\"]",
                "list = [\"todo\", \"doing\", \"done\", \"won't fix\"]",
            )
            .replace("cancel = \"cancelled\"\n", "")
            .replace(
                "terminal = [\"done\", \"cancelled\"]",
                "terminal = [\"done\", \"won't fix\"]",
            );
        reject(&text, "is not a usable directory name");
    }

    #[test]
    fn rejects_a_repeated_terminal_entry() {
        reject(
            &v2().replace("cancel = \"cancelled\"\n", "").replace(
                "terminal = [\"done\", \"cancelled\"]",
                "terminal = [\"done\", \"done\"]",
            ),
            "lists \"done\" twice",
        );
    }

    #[test]
    fn rejects_terminal_entries_differing_only_in_case() {
        reject(
            &v2()
                .replace(
                    "list = [\"todo\", \"doing\", \"blocked\", \"done\", \"cancelled\"]",
                    "list = [\"todo\", \"doing\", \"done\", \"DONE\"]",
                )
                .replace("cancel = \"cancelled\"\n", "")
                .replace(
                    "terminal = [\"done\", \"cancelled\"]",
                    "terminal = [\"done\", \"DONE\"]",
                ),
            "differ only in case",
        );
    }

    #[test]
    fn rejects_a_done_status_outside_the_terminal_set() {
        reject(
            &v2().replace("cancel = \"cancelled\"\n", "").replace(
                "terminal = [\"done\", \"cancelled\"]",
                "terminal = [\"cancelled\"]",
            ),
            "statuses.done \"done\" is not in statuses.terminal",
        );
    }

    #[test]
    fn rejects_a_cancel_status_outside_the_terminal_set() {
        reject(
            &v2().replace(
                "terminal = [\"done\", \"cancelled\"]",
                "terminal = [\"done\"]",
            ),
            "statuses.cancel \"cancelled\" is not in statuses.terminal",
        );
    }

    #[test]
    fn rejects_a_terminal_default_or_start() {
        reject(
            &v2().replace(
                "terminal = [\"done\", \"cancelled\"]",
                "terminal = [\"todo\", \"done\", \"cancelled\"]",
            ),
            "must not contain statuses.default \"todo\"",
        );
        reject(
            &v2().replace(
                "terminal = [\"done\", \"cancelled\"]",
                "terminal = [\"doing\", \"done\", \"cancelled\"]",
            ),
            "must not contain the start status \"doing\"",
        );
    }

    #[test]
    fn rejects_a_config_with_no_open_status() {
        reject(
            "version = 2\n[ids]\nscheme = \"seq\"\n\
             [statuses]\nlist = [\"todo\", \"done\"]\ndefault = \"todo\"\n\
             terminal = [\"todo\", \"done\"]\n",
            "must not contain statuses.default",
        );
    }

    /// The grammar is also the reserved-name list: everything yman keeps in
    /// `.yman` contains a `.`, and so does every task folder.
    #[test]
    fn status_dir_grammar_excludes_reserved_and_task_names() {
        for n in [
            "config.toml",
            ".git",
            ".gitignore",
            ".gitattributes",
            "t.md",
            "m.yml",
            "d.md",
            "5.1.slug",
            "in progress",
            "-done",
            "done/x",
            "",
        ] {
            assert!(!is_status_dir_name(n), "accepted {n}");
        }
        for n in ["done", "cancelled", "wont_fix", "on-hold", "Done2", "f"] {
            assert!(is_status_dir_name(n), "rejected {n}");
        }
    }
}
