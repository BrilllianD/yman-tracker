use crate::cli::InitArgs;
use crate::config::{Config, Scheme};
use crate::hooks;
use crate::repo::{Context, FETCH_REFSPEC, LOCAL, REMOTE, REMOTE_REF, YDIR_NAME, YdirState};
use crate::task;
use anyhow::{Result, bail};

use super::sync::{PushResult, push};

const GITIGNORE: &str = "*.swp\n*~\n.#*\n*.orig\n";
const GITATTRIBUTES: &str = "*/d.md merge=union\n";

pub fn run(ctx: &mut Context, a: InitArgs) -> Result<()> {
    let url = resolve_remote(ctx, a.remote.as_deref())?;

    match ctx.ydir_state() {
        YdirState::Worktree => return repair(ctx, &a, &url),
        YdirState::StandaloneRepo => {
            bail!(
                "{YDIR_NAME} is a standalone git repository, not a worktree; move it aside and rerun"
            )
        }
        YdirState::PlainDir => {
            bail!("{YDIR_NAME} exists and is not a yman worktree; move it aside and rerun")
        }
        YdirState::Absent => {}
    }

    ctx.exclude_add()?;
    if !ctx.fetch_refspec_present()? {
        ctx.fetch_refspec_add()?;
    }
    let policy = a.refresh.map(|r| r.as_str()).unwrap_or("lazy");
    ctx.set_cfg("yman.refresh", policy)?;
    if let Some(author) = &a.author {
        ctx.set_cfg("yman.author", author)?;
    }

    // A previous `rm -rf .yman` leaves a stale worktree registration behind.
    ctx.main.run(&["worktree", "prune"])?;

    if !a.offline {
        fetch_tasks(ctx, "fetch failed (see above); use --offline to skip")?;
    }

    // Where does the new worktree start?
    let fresh = if ctx.main.rev_parse(LOCAL)?.is_some() {
        false
    } else if ctx.main.rev_parse(REMOTE)?.is_some() {
        ctx.main.ok(&["update-ref", LOCAL, REMOTE])?;
        false
    } else {
        let tree = ctx
            .main
            .with_stdin(&["mktree"], "")?
            .stdout
            .trim()
            .to_string();
        let commit = ctx.main.out(&["commit-tree", &tree, "-m", "yman: init"])?;
        ctx.main.ok(&["update-ref", LOCAL, commit.trim()])?;
        true
    };

    add_worktree(ctx)?;

    let scheme = a.id_scheme.map(Scheme::from).unwrap_or(Scheme::Seq);
    if fresh {
        let cfg = Config::new(scheme);
        cfg.save(&ctx.ydir)?;
        std::fs::write(ctx.ydir.join(".gitignore"), GITIGNORE)?;
        std::fs::write(ctx.ydir.join(".gitattributes"), GITATTRIBUTES)?;
        ctx.wt.ok(&["add", "-A"])?;
        ctx.wt.commit(&format!("yman: init ({scheme})"))?;
        ctx.config = Some(cfg);
        if !a.offline && push(ctx)? == PushResult::Rejected {
            // Someone initialized the tracker between our fetch and our push.
            fetch_tasks(ctx, "fetch failed (see above); use --offline to skip")?;
            ctx.wt.ok(&["reset", "-q", "--hard", REMOTE])?;
            ctx.main.ok(&["update-ref", LOCAL, REMOTE])?;
            eprintln!("warning: remote already had tasks; adopted remote state");
            ctx.config = Some(load_history_config(ctx)?);
        }
    } else {
        ctx.config = Some(load_history_config(ctx)?);
        if a.id_scheme.is_some() {
            warn_scheme_ignored(ctx);
        }
    }

    if a.hooks {
        hooks::install(ctx)?;
    }

    summary(ctx, &url, "initialized")
}

/// `.yman` is already a worktree: make sure the main-repo side is intact.
fn repair(ctx: &mut Context, a: &InitArgs, url: &str) -> Result<()> {
    ctx.check_worktree_head()?;
    ctx.exclude_add()?;
    if !ctx.fetch_refspec_present()? {
        ctx.fetch_refspec_add()?;
    }
    if let Some(r) = a.refresh {
        ctx.set_cfg("yman.refresh", r.as_str())?;
    } else if ctx.get_cfg("yman.refresh")?.is_none() {
        ctx.set_cfg("yman.refresh", "lazy")?;
    }
    if let Some(author) = &a.author {
        ctx.set_cfg("yman.author", author)?;
    }
    ctx.config = Some(load_history_config(ctx)?);
    if a.id_scheme.is_some() {
        warn_scheme_ignored(ctx);
    }
    if a.hooks {
        hooks::install(ctx)?;
    }
    summary(ctx, url, "already initialized")
}

fn warn_scheme_ignored(ctx: &Context) {
    eprintln!(
        "warning: id scheme is \"{}\" (from config.toml); --id-scheme ignored",
        ctx.config().ids.scheme
    );
}

/// The URL we report, creating `origin` when the user supplied one and the
/// repo has none (push and fetch both address `origin` by name).
fn resolve_remote(ctx: &Context, requested: Option<&str>) -> Result<String> {
    match (ctx.origin_url()?, requested) {
        (Some(existing), Some(given)) if existing != given => {
            eprintln!("warning: origin already points at {existing}; --remote ignored");
            Ok(existing)
        }
        (Some(existing), _) => Ok(existing),
        (None, Some(given)) => {
            ctx.main.ok(&["remote", "add", "origin", given])?;
            eprintln!("note: added remote \"origin\" -> {given}");
            Ok(given.to_string())
        }
        (None, None) => bail!("main repo has no \"origin\" remote; pass --remote <url>"),
    }
}

/// Fetch `refs/tasks/main`. A remote that simply has no tasks yet is not an
/// error; anything else is.
fn fetch_tasks(ctx: &Context, failure_msg: &str) -> Result<()> {
    let out = ctx.main.run(&["fetch", "origin", FETCH_REFSPEC])?;
    if out.ok() {
        return Ok(());
    }
    if out.stderr.contains("couldn't find remote ref") {
        return Ok(());
    }
    eprint!("{}", out.stderr);
    bail!("{failure_msg}")
}

fn add_worktree(ctx: &mut Context) -> Result<()> {
    let ydir = ctx.ydir.to_string_lossy().into_owned();
    let out = ctx
        .main
        .run(&["worktree", "add", "--detach", &ydir, LOCAL])?;
    if !out.ok() {
        if out.stderr.contains("already checked out")
            || out.stderr.contains("is already used by worktree")
        {
            bail!(
                "{LOCAL} is already checked out in another worktree of this repo; only one {YDIR_NAME} per clone is supported"
            );
        }
        eprint!("{}", out.stderr);
        bail!("git worktree add failed");
    }
    // Point HEAD at the ref itself, so commits made here move refs/yman/local
    // while staying invisible to `git branch`.
    ctx.wt.ok(&["symbolic-ref", "HEAD", LOCAL])?;
    ctx.relearn_wt_gitdir();
    Ok(())
}

fn load_history_config(ctx: &Context) -> Result<Config> {
    Config::load(&ctx.ydir).map_err(|e| {
        anyhow::anyhow!(
            "{REMOTE_REF} on origin is not a yman history (missing or invalid config.toml): {e:#}"
        )
    })
}

fn summary(ctx: &Context, url: &str, verb: &str) -> Result<()> {
    let tasks = task::list(&ctx.ydir)?.len();
    let refresh = ctx
        .get_cfg("yman.refresh")?
        .unwrap_or_else(|| "lazy".to_string());
    let hooks_state = if hooks::all_installed(ctx)? {
        "installed"
    } else {
        "not installed"
    };
    println!("{verb} {YDIR_NAME}");
    println!("  scheme:   {}", ctx.config().ids.scheme);
    println!("  remote:   {url}  ({REMOTE_REF})");
    println!("  refresh:  {refresh}");
    println!("  hooks:    {hooks_state}");
    println!("  tasks:    {tasks}");
    Ok(())
}
