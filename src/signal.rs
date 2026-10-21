//! The few signal dispositions yman sets by hand.
//!
//! Declared without a crate: the numbers are the same on Linux, macOS and the
//! BSDs (SIGINT 2, SIGQUIT 3, SIGPIPE 13, SIG_DFL 0, SIG_IGN 1, SIG_ERR -1).

/// Put SIGPIPE back to its default action, so `yman ls | head -1` dies
/// silently from the signal once `head` has gone, like git and coreutils do.
///
/// The Rust runtime sets SIGPIPE to "ignore" before `main`, which turns a
/// write to a closed pipe into an `EPIPE` error, and `println!` panics on
/// that. Children are unaffected: `std::process::Command` already resets
/// SIGPIPE to the default in every child it spawns. The one place yman itself
/// writes into a child, `Git::with_stdin`, only feeds `mktree` an empty tree
/// and `update-ref` a few lines; a caller that streams real data there would
/// now be killed, not handed an error, if git exited before reading it all.
#[cfg(unix)]
pub fn restore_sigpipe() {
    // SAFETY: `signal` is async-signal-safe and called once, before any other
    // thread exists; SIG_DFL is a valid disposition for SIGPIPE.
    unsafe {
        sys::signal(sys::SIGPIPE, sys::SIG_DFL);
    }
}

#[cfg(not(unix))]
pub fn restore_sigpipe() {}

#[cfg(unix)]
mod sys {
    pub const SIGINT: i32 = 2;
    pub const SIGQUIT: i32 = 3;
    pub const SIGPIPE: i32 = 13;
    pub const SIG_DFL: usize = 0;
    pub const SIG_IGN: usize = 1;
    pub const SIG_ERR: usize = usize::MAX;

    unsafe extern "C" {
        pub fn signal(signum: i32, handler: usize) -> usize;
    }
}

/// While alive, yman ignores SIGINT and SIGQUIT: a Ctrl-C meant for the
/// editor in the foreground then reaches only the editor, and the failure
/// cleanup of `add -e` and `comment -e` still runs. Dropping it restores the
/// handlers that were in place.
pub struct IgnoreInterrupts {
    #[cfg(unix)]
    previous: [(i32, usize); 2],
}

impl IgnoreInterrupts {
    #[cfg(unix)]
    pub fn new() -> IgnoreInterrupts {
        let mut previous = [(sys::SIGINT, sys::SIG_ERR), (sys::SIGQUIT, sys::SIG_ERR)];
        for (signum, old) in previous.iter_mut() {
            // SAFETY: `signal` is async-signal-safe and SIG_IGN is a valid
            // disposition for SIGINT and SIGQUIT.
            *old = unsafe { sys::signal(*signum, sys::SIG_IGN) };
        }
        IgnoreInterrupts { previous }
    }

    #[cfg(not(unix))]
    pub fn new() -> IgnoreInterrupts {
        IgnoreInterrupts {}
    }
}

#[cfg(unix)]
impl Drop for IgnoreInterrupts {
    fn drop(&mut self) {
        for (signum, old) in self.previous {
            if old != sys::SIG_ERR {
                // SAFETY: `old` is a disposition `signal` itself returned for
                // this signal.
                unsafe {
                    sys::signal(signum, old);
                }
            }
        }
    }
}
