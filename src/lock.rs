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
#[allow(dead_code)]
pub struct Lock(File);

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
        Ok(()) => Ok(Some(Lock(file))),
        Err(TryLockError::WouldBlock) => {
            eprintln!("note: waiting for another yman command in this clone to finish");
            match file.lock() {
                Ok(()) => Ok(Some(Lock(file))),
                Err(e) => bail!("cannot lock {}: {e}", path.display()),
            }
        }
        Err(TryLockError::Error(e)) if e.kind() == ErrorKind::Unsupported => {
            eprintln!(
                "warning: cannot lock {}: {e}; not serialising with other yman commands",
                path.display()
            );
            Ok(None)
        }
        Err(TryLockError::Error(e)) => bail!("cannot lock {}: {e}", path.display()),
    }
}
