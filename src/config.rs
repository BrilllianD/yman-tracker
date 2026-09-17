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
            },
            priorities: PrioCfg::default(),
            slug: SlugCfg::default(),
        }
    }

    /// Status that `yman start` moves a task to: the second one in the list.
    pub fn start_status(&self) -> &str {
        &self.statuses.list[1]
    }

    /// Status that `yman done` moves a task to, and that `ls` hides: the last.
    pub fn done_status(&self) -> &str {
        self.statuses
            .list
            .last()
            .expect("validated: list has >= 2 entries")
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
        if self.version != 1 {
            bail!(
                "invalid .yman/config.toml: unsupported version {} (this yman understands 1)",
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
             default = \"{}\"\n\n\
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
            self.priorities.default,
            self.slug.max_bytes,
            SLUG_MAX_BYTES_CAP,
        )
    }

    pub fn save(&self, ydir: &Path) -> Result<()> {
        std::fs::write(ydir.join(CONFIG_FILE), self.render())?;
        Ok(())
    }
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
        reject(&base().replace("version = 1", "version = 2"), "version 2");
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
}
