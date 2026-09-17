use crate::cli::LogArgs;
use crate::repo::{Context, LOCAL};
use crate::task::{self, FolderName};
use anyhow::Result;
use std::collections::HashSet;

pub fn run(ctx: &mut Context, a: LogArgs) -> Result<()> {
    let n = a.number.to_string();
    let mut args: Vec<String> = vec![
        "log".into(),
        "--oneline".into(),
        "-n".into(),
        n,
        LOCAL.into(),
    ];

    if let Some(id) = &a.id {
        let t = task::find(&ctx.ydir, id)?;
        let names = historical_names(ctx, &t.rel())?;
        args.push("--".into());
        args.extend(names);
    }

    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = ctx.wt.out(&refs)?;
    if !out.is_empty() {
        println!("{out}");
    }
    Ok(())
}

/// Every folder name this task ever had, walking the rename graph backwards
/// from the current one — a task that changed priority or title would
/// otherwise lose its history.
fn historical_names(ctx: &Context, current: &str) -> Result<Vec<String>> {
    let out = ctx.wt.out(&[
        "log",
        "--format=",
        "--name-status",
        "--diff-filter=R",
        "-M",
        LOCAL,
        "--",
        ".",
    ])?;

    // new folder name -> old folder name
    let mut origin: Vec<(String, String)> = Vec::new();
    for line in out.lines() {
        let mut parts = line.split('\t');
        let Some(tag) = parts.next() else { continue };
        if !tag.starts_with('R') {
            continue;
        }
        let (Some(old), Some(new)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Some(old), Some(new)) = (first_segment(old), first_segment(new)) else {
            continue;
        };
        if old != new {
            origin.push((new.to_string(), old.to_string()));
        }
    }

    let mut names: Vec<String> = vec![current.to_string()];
    let mut seen: HashSet<String> = names.iter().cloned().collect();
    let mut frontier = vec![current.to_string()];
    while let Some(name) = frontier.pop() {
        for (new, old) in &origin {
            if *new == name && seen.insert(old.clone()) {
                names.push(old.clone());
                frontier.push(old.clone());
            }
        }
    }
    Ok(names)
}

/// First path segment, when it names a task folder.
fn first_segment(path: &str) -> Option<&str> {
    let mut segs = path.split('/');
    let first = segs.next()?;
    segs.next()?;
    FolderName::parse(first)?;
    Some(first)
}
