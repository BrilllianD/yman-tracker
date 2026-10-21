//! Task id generation.
//!
//! An id must be unique against everything that ever existed in the history,
//! not just what is on disk right now — otherwise deleting a task would let a
//! later `add` reuse its id and confuse anyone reading the log.

use crate::config::{Config, Scheme};
use crate::repo::{Context, LOCAL, REMOTE};
use crate::task::{task_dirs, task_path_of};
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
    assigned_in(ctx, &present)
}

/// Start of the commit subject a sync collision renumber writes.
pub const RENUMBER_PREFIX: &str = "yman: renumber ";

/// `[("2","3"), ("7","8")]` -> `yman: renumber 2->3, 7->8 (sync collision)`
pub fn renumber_subject(moves: &[(String, String)]) -> String {
    let pairs: Vec<String> = moves.iter().map(|(o, n)| format!("{o}->{n}")).collect();
    format!("{RENUMBER_PREFIX}{} (sync collision)", pairs.join(", "))
}

/// The inverse of `renumber_subject`; any subject that does not start with
/// `RENUMBER_PREFIX` gives no pairs.
pub fn renumber_pairs(subject: &str) -> Vec<(String, String)> {
    let Some(rest) = subject.strip_prefix(RENUMBER_PREFIX) else {
        return Vec::new();
    };
    let rest = rest.strip_suffix(" (sync collision)").unwrap_or(rest);
    rest.split(", ")
        .filter_map(|p| p.split_once("->"))
        .map(|(old, new)| (old.to_string(), new.to_string()))
        .collect()
}

/// `ever_assigned` for revisions known to exist. A pinned sha must come
/// through here: `Context::resolve_ref` reads a bare sha as a missing ref.
pub fn assigned_in(ctx: &Context, revs: &[&str]) -> Result<HashSet<String>> {
    if revs.is_empty() {
        return Ok(HashSet::new());
    }
    // `--no-renames`: the `git mv` of a sync renumber would otherwise read as
    // a rename, and the id it lands on would never be seen as added. `-z`:
    // NUL-separated paths are never quoted, whatever characters they hold.
    let mut args: Vec<&str> = vec![
        "log",
        "--no-renames",
        "--diff-filter=A",
        "--name-only",
        "--format=",
        "-z",
    ];
    args.extend_from_slice(revs);
    args.extend_from_slice(&["--", "."]);
    let out = ctx.wt.out(&args)?;
    let mut ids = HashSet::new();
    for line in out.split('\0') {
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

/// Ids of the folders currently on disk, including broken ones. The id is in
/// the folder name, so this never opens a task.
pub fn fs_ids(ydir: &Path) -> Result<HashSet<String>> {
    Ok(task_dirs(ydir)?
        .into_iter()
        .map(|(_, _, folder)| folder.id)
        .collect())
}

/// First id of `scheme` that is not in `taken`, or an error when the scheme
/// has none left.
pub fn next_free(
    scheme: Scheme,
    cfg: &Config,
    prefix: Option<&str>,
    taken: &HashSet<String>,
) -> Result<String> {
    Ok(match scheme {
        Scheme::Random => {
            let len = cfg.ids.random_len as usize;
            // Drawing until a free id turns up never ends once there is none:
            // `random_len = 2` is legal and holds only 256 ids.
            let in_space = taken
                .iter()
                .filter(|id| random_tail(id).is_some_and(|h| h.len() == len))
                .count() as u128;
            if in_space >= 16u128.pow(len as u32) {
                bail!("random id space exhausted (random_len = {len}); raise ids.random_len");
            }
            let mut rng = rand::rng();
            loop {
                let mut id = String::with_capacity(len + 2);
                id.push_str("t-");
                for _ in 0..len {
                    id.push(char::from_digit(rng.random_range(0..16), 16).expect("0..16 is hex"));
                }
                if !taken.contains(&id) {
                    break id;
                }
            }
        }
        Scheme::Seq => {
            let max = taken
                .iter()
                .filter_map(|id| match IdShape::of(id) {
                    IdShape::Seq(n) => Some(n),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            successor(max)?.to_string()
        }
        Scheme::Author => {
            let prefix = prefix.unwrap_or("x");
            let max = taken
                .iter()
                .filter_map(|id| match IdShape::of(id) {
                    IdShape::Author { prefix: p, n } if p == prefix => Some(n),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            format!("{prefix}-{}", successor(max)?)
        }
    })
}

/// `max + 1`, refused rather than wrapped: a hand-made `18446744073709551615`
/// is all it takes to get there.
fn successor(max: u64) -> Result<u64> {
    max.checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("id space exhausted: {max} is the largest id there can be"))
}

/// What an id looks like, by the scheme that could have minted it. The
/// variant order is the sort order (`task::cmp_id`).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdShape<'a> {
    /// All digits: the sequential scheme.
    Seq(u64),
    /// `prefix-N`, `N` all digits: the author scheme.
    Author {
        prefix: &'a str,
        n: u64,
    },
    /// `t-` and hex digits: the random scheme. Only when it is not also what
    /// an author whose prefix is `t` mints (`t-1234`), which stays `Author`:
    /// counting it out would make the author scheme re-mint taken numbers.
    Random(&'a str),
    Other,
}

impl<'a> IdShape<'a> {
    pub fn of(id: &'a str) -> IdShape<'a> {
        if let Some(n) = all_digits(id) {
            return IdShape::Seq(n);
        }
        if let Some(h) = random_tail(id)
            && (h.starts_with('0') || h.bytes().any(|c| c.is_ascii_lowercase()))
        {
            return IdShape::Random(h);
        }
        match id.rfind('-') {
            Some(i) => match all_digits(&id[i + 1..]) {
                Some(n) => IdShape::Author {
                    prefix: &id[..i],
                    n,
                },
                None => IdShape::Other,
            },
            None => IdShape::Other,
        }
    }
}

fn all_digits(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// The hex part of a `t-` id with 2 to 16 lowercase hex digits, whether or
/// not it also reads as an author id.
fn random_tail(id: &str) -> Option<&str> {
    let h = id.strip_prefix("t-")?;
    let hex = h.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'));
    (hex && (2..=16).contains(&h.len())).then_some(h)
}

/// The `prefix-` part of an id, for keeping a renumbered task with its author.
pub fn prefix_of(id: &str) -> Option<&str> {
    match IdShape::of(id) {
        IdShape::Author { prefix, .. } => Some(prefix),
        _ => None,
    }
}

/// Everything an id must avoid colliding with.
pub fn taken_ids(ctx: &Context) -> Result<HashSet<String>> {
    let mut taken = fs_ids(&ctx.ydir)?;
    taken.extend(ever_assigned(ctx, &[LOCAL, REMOTE])?);
    Ok(taken)
}

/// A fresh id, avoiding `taken` (from `taken_ids`). The caller owns the set
/// so a run that mints several ids walks the history once and adds each new
/// id to it.
pub fn new_id(ctx: &Context, taken: &HashSet<String>) -> Result<String> {
    let cfg = ctx.config();
    let prefix = match cfg.ids.scheme {
        Scheme::Author => Some(author_prefix(ctx)?),
        _ => None,
    };
    next_free(cfg.ids.scheme, cfg, prefix.as_deref(), taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::FolderName;

    fn cfg() -> Config {
        Config::new(Scheme::Seq)
    }

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn seq_starts_at_one() {
        assert_eq!(
            next_free(Scheme::Seq, &cfg(), None, &set(&[])).unwrap(),
            "1"
        );
    }

    #[test]
    fn seq_takes_max_plus_one() {
        assert_eq!(
            next_free(Scheme::Seq, &cfg(), None, &set(&["1", "2", "9"])).unwrap(),
            "10"
        );
        // Gaps are never reused: 3 is free but 9 is the high-water mark.
        assert_eq!(
            next_free(Scheme::Seq, &cfg(), None, &set(&["1", "9"])).unwrap(),
            "10"
        );
    }

    #[test]
    fn seq_ignores_foreign_ids() {
        assert_eq!(
            next_free(Scheme::Seq, &cfg(), None, &set(&["t-ab12", "iv-7", "3"])).unwrap(),
            "4"
        );
    }

    #[test]
    fn author_scheme_counts_its_own_prefix() {
        let taken = set(&["iv-1", "iv-4", "an-9", "7"]);
        assert_eq!(
            next_free(Scheme::Author, &cfg(), Some("iv"), &taken).unwrap(),
            "iv-5"
        );
        assert_eq!(
            next_free(Scheme::Author, &cfg(), Some("an"), &taken).unwrap(),
            "an-10"
        );
        assert_eq!(
            next_free(Scheme::Author, &cfg(), Some("zz"), &taken).unwrap(),
            "zz-1"
        );
    }

    #[test]
    fn random_scheme_shape_and_uniqueness() {
        let mut c = cfg();
        c.ids.random_len = 4;
        let id = next_free(Scheme::Random, &c, None, &set(&[])).unwrap();
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
        assert_eq!(next_free(Scheme::Random, &c, None, &taken).unwrap(), "t-ff");
        taken.insert("t-ff".into());
        let err = next_free(Scheme::Random, &c, None, &taken).unwrap_err();
        assert_eq!(
            err.to_string(),
            "random id space exhausted (random_len = 2); raise ids.random_len"
        );
    }

    #[test]
    fn counting_schemes_refuse_to_wrap() {
        let max = u64::MAX.to_string();
        let err = next_free(Scheme::Seq, &cfg(), None, &set(&[&max])).unwrap_err();
        assert!(err.to_string().starts_with("id space exhausted"), "{err}");
        let author = format!("iv-{max}");
        let err = next_free(Scheme::Author, &cfg(), Some("iv"), &set(&[&author])).unwrap_err();
        assert!(err.to_string().starts_with("id space exhausted"), "{err}");
    }

    #[test]
    fn id_shapes() {
        let cases = [
            ("14", IdShape::Seq(14)),
            ("007", IdShape::Seq(7)),
            (
                "iv-12",
                IdShape::Author {
                    prefix: "iv",
                    n: 12,
                },
            ),
            (
                "a-b-3",
                IdShape::Author {
                    prefix: "a-b",
                    n: 3,
                },
            ),
            // `t-` plus 2 to 16 hex digits is random, unless it is also what
            // an author whose prefix is `t` mints.
            (
                "t-1234",
                IdShape::Author {
                    prefix: "t",
                    n: 1234,
                },
            ),
            ("t-0042", IdShape::Random("0042")),
            ("t-7f3a", IdShape::Random("7f3a")),
            ("t-12ab", IdShape::Random("12ab")),
            ("t-7F3A", IdShape::Other),
            ("t-a", IdShape::Other),
            ("t-0000000000000000a", IdShape::Other),
            ("t-xyz", IdShape::Other),
            ("5-a", IdShape::Other),
            ("", IdShape::Other),
            ("99999999999999999999", IdShape::Other),
        ];
        for (id, shape) in cases {
            assert_eq!(IdShape::of(id), shape, "{id}");
        }
    }

    #[test]
    fn prefix_extraction() {
        assert_eq!(prefix_of("iv-12"), Some("iv"));
        assert_eq!(prefix_of("a-b-3"), Some("a-b"));
        assert_eq!(prefix_of("t-7f3a"), None);
        assert_eq!(prefix_of("t-0042"), None);
        assert_eq!(prefix_of("t-12"), Some("t"));
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

    #[test]
    fn renumber_subject_round_trips() {
        let moves = vec![
            ("2".to_string(), "3".to_string()),
            ("iv-7".to_string(), "iv-8".to_string()),
        ];
        assert_eq!(
            renumber_subject(&moves),
            "yman: renumber 2->3, iv-7->iv-8 (sync collision)"
        );
        assert_eq!(renumber_pairs(&renumber_subject(&moves)), moves);
        let one = vec![("2".to_string(), "3".to_string())];
        assert_eq!(renumber_pairs(&renumber_subject(&one)), one);
    }

    #[test]
    fn parses_renumber_subjects() {
        assert_eq!(
            renumber_pairs("yman: renumber 2->3, iv-7->iv-8 (sync collision)"),
            vec![
                ("2".to_string(), "3".to_string()),
                ("iv-7".to_string(), "iv-8".to_string())
            ]
        );
        assert!(renumber_pairs("task(2): set title").is_empty());
    }
}
