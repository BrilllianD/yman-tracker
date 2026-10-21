//! One advisory lock per clone, so two yman processes never write the task
//! history at once.
//!
//! - Writers take it (`Cmd::takes_lock`) in `main`, after discovery and
//!   before preflight, and hold it until the process exits, autosync
//!   included. Readers never take it, so a read never waits.
//! - The kernel drops the lock when the process ends, crash included, so
//!   there is no stale-lock handling and no PID bookkeeping. The file is
//!   never removed.
//! - A command holding the lock must never spawn a `yman` that takes it.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::ErrorKind;

use anyhow::{Result, bail};

use crate::repo::Context;

pub const LOCK_FILE: &str = "yman.lock";

/// Held for the life of the process; dropping it (or the process dying)
/// releases the lock.
pub struct Lock {
    _file: File,
}

/// `ENOLCK`: Linux reports it for a filesystem that cannot lock (NFS without
/// lockd) instead of `Unsupported`. std has no `ErrorKind` for it and the
/// crate takes no libc, so the number is spelled out.
#[cfg(target_os = "linux")]
const ENOLCK: i32 = 37;

/// Does this error mean the filesystem cannot lock at all, as opposed to a
/// real failure?
fn cannot_lock_here(e: &std::io::Error) -> bool {
    #[cfg(target_os = "linux")]
    if e.raw_os_error() == Some(ENOLCK) {
        return true;
    }
    e.kind() == ErrorKind::Unsupported
}

/// Take the lock, waiting for another yman to finish when it is held.
///
/// `Ok(None)` when `ctx.wt_gitdir` does not exist (preflight then reports the
/// real problem) or when the filesystem cannot lock (after a warning).
pub fn acquire(ctx: &Context) -> Result<Option<Lock>> {
    let path = ctx.wt_gitdir.join(LOCK_FILE);
    let file = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => bail!("cannot lock {}: {e}", path.display()),
    };
    match file.try_lock() {
        Ok(()) => Ok(Some(Lock { _file: file })),
        Err(TryLockError::WouldBlock) => {
            eprintln!("note: waiting for another yman command in this clone to finish");
            match file.lock() {
                Ok(()) => Ok(Some(Lock { _file: file })),
                Err(e) => bail!("cannot lock {}: {e}", path.display()),
            }
        }
        Err(TryLockError::Error(e)) if cannot_lock_here(&e) => {
            eprintln!(
                "warning: cannot lock {}: {e}; not serialising with other yman commands",
                path.display()
            );
            Ok(None)
        }
        Err(TryLockError::Error(e)) => bail!("cannot lock {}: {e}", path.display()),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn enolck_and_unsupported_mean_the_filesystem_cannot_lock() {
        assert!(cannot_lock_here(&std::io::Error::from_raw_os_error(ENOLCK)));
        assert!(cannot_lock_here(&ErrorKind::Unsupported.into()));
        assert!(!cannot_lock_here(&ErrorKind::PermissionDenied.into()));
    }
}
