//! Error types and the process exit-code mapping.
//!
//! Everything in the crate returns `anyhow::Result`. A command that cannot
//! finish because a sync merge is unresolved returns a [`MergePending`], which
//! `main` recognises by downcasting and turns into exit code 3.

use std::fmt;

/// Failure exit codes. Success is 0 and usage errors (2) come from clap.
pub mod exit {
    pub const ERROR: i32 = 1;
    pub const MERGE_PENDING: i32 = 3;
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

/// The exit code an error should produce.
pub fn exit_code_for(err: &anyhow::Error) -> i32 {
    if err.downcast_ref::<MergePending>().is_some() {
        exit::MERGE_PENDING
    } else {
        exit::ERROR
    }
}
