//! Error types and the process exit-code mapping.
//!
//! Everything in the crate returns `anyhow::Result`. Two failures carry a
//! code of their own so scripts can branch without parsing stderr: a command
//! that cannot finish because a sync merge is unresolved returns a
//! [`MergePending`] (exit 3), and a task id that matches nothing returns a
//! [`NotFound`] (exit 4). `main` recognises both by downcasting; returning
//! the same text as a plain `anyhow` error silently degrades it to 1.

use std::fmt;

/// Failure exit codes. Success is 0 and usage errors (2) come from clap.
pub mod exit {
    pub const ERROR: i32 = 1;
    pub const MERGE_PENDING: i32 = 3;
    pub const NOT_FOUND: i32 = 4;
}

/// Marker error for "a merge is in progress and must be resolved first".
#[derive(Debug)]
pub struct MergePending(pub String);

impl MergePending {
    pub fn new(msg: impl Into<String>) -> Self {
        MergePending(msg.into())
    }
}

impl fmt::Display for MergePending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MergePending {}

/// Marker error for "no task has this id". A broken or duplicated folder is
/// not this: the id exists, the repository needs repair, exit 1.
#[derive(Debug)]
pub struct NotFound(pub String);

impl NotFound {
    pub fn new(msg: impl Into<String>) -> Self {
        NotFound(msg.into())
    }
}

impl fmt::Display for NotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotFound {}

/// The exit code an error should produce.
pub fn exit_code_for(err: &anyhow::Error) -> i32 {
    if err.downcast_ref::<MergePending>().is_some() {
        exit::MERGE_PENDING
    } else if err.downcast_ref::<NotFound>().is_some() {
        exit::NOT_FOUND
    } else {
        exit::ERROR
    }
}
