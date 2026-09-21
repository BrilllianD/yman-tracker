//! Task id generation.
//!
//! An id must be unique against everything that ever existed in the history,
//! not just what is on disk right now — otherwise deleting a task would let a
//! later `add` reuse its id and confuse anyone reading the log.

use crate::config::{Config, Scheme};
use crate::repo::{Context, LOCAL, REMOTE};
use crate::task::{FolderName, list, task_path_of};
use anyhow::{Result, bail};
use rand::Rng;
use std::collections::HashSet;
use std::path::Path;

/// The prefix lands in a folder name (`5.{prefix}-1.slug`), so it has to
/// survive `FolderName::parse` and stay typeable: ASCII letters, digits, `_`
/// and `-`, nothing else. A `.` is the dangerous one — `v.i` would mint
/// `5.v.i-1.slug`, which parses as id `v` with slug `i-1.slug`, so the task
/// exists under an id nobody asked for.
fn checked_prefix(p: &str, source: &str) -> Result<String> {
    let ok = !p.is_empty()
        && p.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !ok {
        bail!("author prefix \"{p}\" from {source} must be letters, digits, \"_\" or \"-\"");
    }
    Ok(p.to_string())
}

/// Prefix for the `author` scheme: explicit config, then env, then the
/// initials of the committer's name. Whichever source wins is checked, so a
/// bad prefix fails here rather than on disk.
pub fn author_prefix(ctx: &Context) -> Result<String> {
    if let Some(p) = ctx.get_cfg("yman.author")?
        && !p.trim().is_empty()
    {
        return checked_prefix(p.trim(), "yman.author");
    }
    if let Ok(p) = std::env::var("YMAN_AUTHOR")
        && !p.trim().is_empty()
    {
        return checked_prefix(p.trim(), "$YMAN_AUTHOR");
    }
    if let Some(name) = ctx.get_cfg("user.name")? {
        let initials: String = name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .flat_map(|c| c.to_lowercase())
            .filter(|c| c.is_ascii_alphabetic())
            .collect();
        if !initials.is_empty() {
            return checked_prefix(&initials, "user.name initials");
        }
    }
    bail!("author prefix unknown; run: git config yman.author <prefix>  (or set YMAN_AUTHOR)")
}

/// Every id that ever had a file added under it, across the given refs.
pub fn ever_assigned(ctx: &Context, refs: &[&str]) -> Result<HashSet<String>> {
    let mut present: Vec<&str> = Vec::new();
    for r in refs {
        if ctx.resolve_ref(r)?.is_some() {
            present.push(r);
        }
    }
    if present.is_empty() {
        return Ok(HashSet::new());
    }
    let mut args: Vec<&str> = vec!["log", "--diff-filter=A", "--name-only", "--format="];
    args.extend_from_slice(&present);
    args.extend_from_slice(&["--", "."]);
    let out = ctx.wt.out(&args)?;
    let mut ids = HashSet::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Both `5.1.x/m.yml` and `done/5.1.x/m.yml` count: an id archived
        // under a status directory was still assigned, and must never be
        // handed out again.
        if let Some((_, f)) = task_path_of(line) {
            ids.insert(f.id);
        }
    }
    Ok(ids)
}

/// Ids of the folders currently on disk, including broken ones.
pub fn fs_ids(ydir: &Path) -> Result<HashSet<String>> {
    Ok(list(ydir)?
        .iter()
        .filter_map(|e| FolderName::parse(&e.dir_name()).map(|f| f.id))
        .collect())
}

/// First id of `scheme` that is not in `taken`.
pub fn next_free(
    scheme: Scheme,
    cfg: &Config,
    prefix: Option<&str>,
    taken: &HashSet<String>,
) -> String {
    match scheme {
        Scheme::Random => {
            let len = cfg.ids.random_len as usize;
            let mut rng = rand::rng();
            loop {
                let mut id = String::with_capacity(len + 2);
                id.push_str("t-");
                for _ in 0..len {
                    id.push(char::from_digit(rng.random_range(0..16), 16).expect("0..16 is hex"));
                }
                if !taken.contains(&id) {
                    return id;
                }
            }
        }
        Scheme::Seq => {
            let max = taken
                .iter()
                .filter_map(|id| parse_all_digits(id))
                .max()
                .unwrap_or(0);
            (max + 1).to_string()
        }
        Scheme::Author => {
            let prefix = prefix.unwrap_or("x");
            let head = format!("{prefix}-");
            let max = taken
                .iter()
                .filter_map(|id| id.strip_prefix(&head))
                .filter_map(parse_all_digits)
                .max()
                .unwrap_or(0);
            format!("{prefix}-{}", max + 1)
        }
    }
}

fn parse_all_digits(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// The `prefix-` part of an id, for keeping a renumbered task with its author.
pub fn prefix_of(id: &str) -> Option<&str> {
    let i = id.rfind('-')?;
    let tail = &id[i + 1..];
    if tail.is_empty() || !tail.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(&id[..i])
}

/// Everything an id must avoid colliding with.
pub fn taken_ids(ctx: &Context) -> Result<HashSet<String>> {
    let mut taken = fs_ids(&ctx.ydir)?;
    taken.extend(ever_assigned(ctx, &[LOCAL, REMOTE])?);
    Ok(taken)
}

pub fn new_id(ctx: &Context) -> Result<String> {
    let cfg = ctx.config();
    let prefix = match cfg.ids.scheme {
        Scheme::Author => Some(author_prefix(ctx)?),
        _ => None,
    };
    let taken = taken_ids(ctx)?;
    Ok(next_free(cfg.ids.scheme, cfg, prefix.as_deref(), &taken))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::new(Scheme::Seq)
    }

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn seq_starts_at_one() {
        assert_eq!(next_free(Scheme::Seq, &cfg(), None, &set(&[])), "1");
    }

    #[test]
    fn seq_takes_max_plus_one() {
        assert_eq!(
            next_free(Scheme::Seq, &cfg(), None, &set(&["1", "2", "9"])),
            "10"
        );
        // Gaps are never reused: 3 is free but 9 is the high-water mark.
        assert_eq!(
            next_free(Scheme::Seq, &cfg(), None, &set(&["1", "9"])),
            "10"
        );
    }

    #[test]
    fn seq_ignores_foreign_ids() {
        assert_eq!(
            next_free(Scheme::Seq, &cfg(), None, &set(&["t-ab12", "iv-7", "3"])),
            "4"
        );
    }

    #[test]
    fn author_scheme_counts_its_own_prefix() {
        let taken = set(&["iv-1", "iv-4", "an-9", "7"]);
        assert_eq!(
            next_free(Scheme::Author, &cfg(), Some("iv"), &taken),
            "iv-5"
        );
        assert_eq!(
            next_free(Scheme::Author, &cfg(), Some("an"), &taken),
            "an-10"
        );
        assert_eq!(
            next_free(Scheme::Author, &cfg(), Some("zz"), &taken),
            "zz-1"
        );
    }

    #[test]
    fn random_scheme_shape_and_uniqueness() {
        let mut c = cfg();
        c.ids.random_len = 4;
        let id = next_free(Scheme::Random, &c, None, &set(&[]));
        assert!(id.starts_with("t-"), "{id}");
        assert_eq!(id.len(), 6);
        assert!(id[2..].bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(FolderName::parse(&format!("5.{id}.slug")).is_some());
    }

    #[test]
    fn random_scheme_avoids_taken() {
        let mut c = cfg();
        c.ids.random_len = 2;
        // Exhaust all but one of the 256 possible ids.
        let mut taken: HashSet<String> = HashSet::new();
        for i in 0..256u32 {
            taken.insert(format!("t-{i:02x}"));
        }
        taken.remove("t-ff");
        assert_eq!(next_free(Scheme::Random, &c, None, &taken), "t-ff");
    }

    #[test]
    fn prefix_extraction() {
        assert_eq!(prefix_of("iv-12"), Some("iv"));
        assert_eq!(prefix_of("a-b-3"), Some("a-b"));
        assert_eq!(prefix_of("t-7f3a"), None);
        assert_eq!(prefix_of("14"), None);
    }

    #[test]
    fn prefix_grammar_accepts_and_rejects() {
        for p in ["iv", "a-b", "iv_2", "x9", "ta", "A"] {
            assert!(checked_prefix(p, "test").is_ok(), "{p} should be accepted");
            // The point of the grammar: the minted id survives the folder name.
            let folder = FolderName::parse(&format!("5.{p}-1.slug")).expect("{p}");
            assert_eq!(folder.id, format!("{p}-1"));
            assert_eq!(folder.slug, "slug");
        }
        for p in ["", "v.i", "a/b", "a\\b", "ivan p", "ив", "a\tb", "a:b"] {
            assert!(checked_prefix(p, "test").is_err(), "{p} should be rejected");
        }
    }

    #[test]
    fn the_prefix_error_names_the_value_and_its_source() {
        let err = checked_prefix("v.i", "yman.author")
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "author prefix \"v.i\" from yman.author must be letters, digits, \"_\" or \"-\""
        );
    }
}
