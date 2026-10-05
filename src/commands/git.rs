use crate::cli::GitArgs;
use crate::git::LEAKY_ENV;
use crate::repo::Context;
use anyhow::{Result, anyhow};
use std::process::Command;

/// Raw git passthrough inside `.yman`, exit code and all.
///
/// Raw except for the environment: the same `LEAKY_ENV` the `Git` wrapper
/// strips goes here too. Hooks run `yman refresh` with `GIT_DIR` exported,
/// and a script calling `yman git -- status` from one would otherwise act on
/// the main repository, not `.yman`.
pub fn run(ctx: &mut Context, a: GitArgs) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(&ctx.ydir).args(&a.args);
    for key in LEAKY_ENV {
        cmd.env_remove(key);
    }
    let status = cmd.status().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow!("git not found in PATH")
        } else {
            anyhow::Error::new(e).context("failed to run git")
        }
    })?;
    if !status.success() {
        std::process::exit(exit_code(&status));
    }
    Ok(())
}

/// git's own exit code, or `128 + signal` when it was killed, the way a
/// shell reports it.
fn exit_code(status: &std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    status.code().unwrap_or(1)
}
