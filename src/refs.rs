//! Reading refs off disk instead of spawning `git rev-parse`.
//!
//! `yman ls` prints from the filesystem alone, yet every command used to pay
//! for a handful of `git` processes before it started: two to discover the
//! repository, one to check the worktree's HEAD, and two to resolve
//! `refs/yman/{local,remote}`. Resolving a ref is a file read, so this module
//! does the file read.
//!
//! The contract with the callers is the interesting part. `Ok(None)` means the
//! ref genuinely does not exist. `Err` means "this is not a layout I read by
//! hand" — a reftable repository, an unreadable file, a shape nobody wrote —
//! and the caller falls back to spawning git, which is never wrong. Being
//! conservative here is what makes the optimisation safe: an unfamiliar
//! repository gets slower, never a wrong answer.

use anyhow::{Result, bail};
use std::path::Path;

/// Symbolic refs pointing at symbolic refs are legal but vanishingly rare;
/// this is a loop guard, not a limit anyone will hit.
const MAX_SYMREF_HOPS: usize = 5;

/// Resolve `name` (a full ref name, e.g. `refs/yman/local`) to a full object
/// id. `Ok(None)` when the ref does not exist; `Err` when the caller must
/// spawn git instead.
pub fn resolve(common: &Path, name: &str) -> Result<Option<String>> {
    check_backend(common)?;
    let mut name = name.to_string();
    for _ in 0..MAX_SYMREF_HOPS {
        match read_loose(common, &name)? {
            Some(Target::Oid(oid)) => return Ok(Some(oid)),
            Some(Target::Ref(next)) => {
                name = next;
                continue;
            }
            // Not loose: packed-refs holds oids only, never symrefs, so this
            // is the end of the chain either way.
            None => return read_packed(common, &name),
        }
    }
    bail!("symbolic ref chain from {name} is too deep")
}

/// Target of a symbolic `HEAD` in `gitdir`, e.g. `refs/yman/local`.
/// `Ok(None)` when HEAD is detached; `Err` when the caller must spawn git.
pub fn head_symref(gitdir: &Path) -> Result<Option<String>> {
    check_backend(gitdir)?;
    match read_loose(gitdir, "HEAD")? {
        Some(Target::Ref(r)) => Ok(Some(r)),
        Some(Target::Oid(_)) => Ok(None),
        // A worktree always has a HEAD. Missing means we are looking at the
        // wrong directory, which is exactly when git should answer instead.
        None => bail!("no HEAD in {}", gitdir.display()),
    }
}

enum Target {
    Oid(String),
    Ref(String),
}

/// git 2.45 added the `reftable` backend, which keeps no `refs/` files at all.
/// Reading them there would report every ref as missing, so refuse the whole
/// directory the moment it looks like one.
fn check_backend(dir: &Path) -> Result<()> {
    if dir.join("reftable").is_dir() {
        bail!("reftable backend");
    }
    Ok(())
}

/// Read one loose ref file. `Ok(None)` when it does not exist.
fn read_loose(dir: &Path, name: &str) -> Result<Option<Target>> {
    let text = match std::fs::read_to_string(dir.join(name)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        // A directory in the way (`refs/yman/local/` when `refs/yman/local` is
        // wanted), a permission problem: let git have it.
        Err(e) => bail!("reading {name}: {e}"),
    };
    let text = text.trim();
    if let Some(target) = text.strip_prefix("ref:") {
        let target = target.trim();
        if target.is_empty() {
            bail!("empty symbolic ref in {name}");
        }
        return Ok(Some(Target::Ref(target.to_string())));
    }
    Ok(Some(Target::Oid(checked_oid(text, name)?)))
}

/// Scan `packed-refs` for `name`. `Ok(None)` when the file is absent or the
/// ref is not in it.
fn read_packed(dir: &Path, name: &str) -> Result<Option<String>> {
    let text = match std::fs::read_to_string(dir.join("packed-refs")) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => bail!("reading packed-refs: {e}"),
    };
    for line in text.lines() {
        // `#` is the header, `^` a peeled tag target belonging to the line
        // above; neither can be the ref we want.
        if line.starts_with('#') || line.starts_with('^') || line.is_empty() {
            continue;
        }
        let Some((oid, r)) = line.split_once(' ') else {
            bail!("unparsable packed-refs line");
        };
        if r.trim() == name {
            return Ok(Some(checked_oid(oid, name)?));
        }
    }
    Ok(None)
}

/// A full hex object id, sha-1 or sha-256. Anything else is a file we did not
/// understand rather than a ref value.
fn checked_oid(s: &str, name: &str) -> Result<String> {
    let s = s.trim();
    let sized = s.len() == 40 || s.len() == 64;
    if !sized || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("{name} does not hold an object id");
    }
    Ok(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const OID: &str = "0123456789abcdef0123456789abcdef01234567";
    const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";

    fn write(dir: &Path, rel: &str, text: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().expect("a ref file has a parent")).expect("mkdir");
        std::fs::write(p, text).expect("write");
    }

    fn tmp() -> (tempfile::TempDir, PathBuf) {
        let td = tempfile::tempdir().expect("tempdir");
        let p = td.path().to_path_buf();
        (td, p)
    }

    #[test]
    fn loose_ref_wins_over_packed() {
        let (_td, d) = tmp();
        write(&d, "refs/yman/local", &format!("{OID}\n"));
        write(&d, "packed-refs", &format!("{OTHER} refs/yman/local\n"));
        assert_eq!(
            resolve(&d, "refs/yman/local").unwrap().as_deref(),
            Some(OID)
        );
    }

    #[test]
    fn loose_symref_is_chased() {
        let (_td, d) = tmp();
        write(&d, "refs/yman/local", "ref: refs/yman/real\n");
        write(&d, "refs/yman/real", &format!("{OID}\n"));
        assert_eq!(
            resolve(&d, "refs/yman/local").unwrap().as_deref(),
            Some(OID)
        );
    }

    #[test]
    fn symref_loop_is_refused() {
        let (_td, d) = tmp();
        write(&d, "refs/a", "ref: refs/b\n");
        write(&d, "refs/b", "ref: refs/a\n");
        assert!(resolve(&d, "refs/a").is_err());
    }

    #[test]
    fn packed_refs_are_read() {
        let (_td, d) = tmp();
        write(
            &d,
            "packed-refs",
            &format!(
                "# pack-refs with: peeled fully-peeled sorted \n\
                 {OTHER} refs/heads/main\n\
                 {OID} refs/yman/local\n\
                 ^{OTHER}\n"
            ),
        );
        assert_eq!(
            resolve(&d, "refs/yman/local").unwrap().as_deref(),
            Some(OID)
        );
        assert_eq!(resolve(&d, "refs/yman/remote").unwrap(), None);
    }

    #[test]
    fn a_peel_line_is_never_mistaken_for_a_ref() {
        let (_td, d) = tmp();
        write(
            &d,
            "packed-refs",
            &format!("{OTHER} refs/tags/v1\n^{OID} refs/yman/local\n"),
        );
        assert_eq!(resolve(&d, "refs/yman/local").unwrap(), None);
    }

    #[test]
    fn a_missing_ref_is_not_an_error() {
        let (_td, d) = tmp();
        assert_eq!(resolve(&d, "refs/yman/local").unwrap(), None);
    }

    #[test]
    fn garbage_in_a_ref_file_falls_back() {
        let (_td, d) = tmp();
        write(&d, "refs/yman/local", "not a hash\n");
        assert!(resolve(&d, "refs/yman/local").is_err());
    }

    #[test]
    fn reftable_backend_falls_back() {
        let (_td, d) = tmp();
        std::fs::create_dir_all(d.join("reftable")).expect("mkdir");
        write(&d, "refs/yman/local", &format!("{OID}\n"));
        assert!(resolve(&d, "refs/yman/local").is_err());
        assert!(head_symref(&d).is_err());
    }

    #[test]
    fn head_symref_reads_the_target() {
        let (_td, d) = tmp();
        write(&d, "HEAD", "ref: refs/yman/local\n");
        assert_eq!(head_symref(&d).unwrap().as_deref(), Some("refs/yman/local"));
    }

    #[test]
    fn a_detached_head_has_no_symref() {
        let (_td, d) = tmp();
        write(&d, "HEAD", &format!("{OID}\n"));
        assert_eq!(head_symref(&d).unwrap(), None);
    }

    #[test]
    fn a_missing_head_falls_back() {
        let (_td, d) = tmp();
        assert!(head_symref(&d).is_err());
    }
}
