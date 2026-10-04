//! Plans: a task and the tasks whose `related` carries its id.
//!
//! `related` is one-way and untyped, so it carries two readings: a step
//! relates to its epic, and a waiting task relates to its blocker. The tag
//! `epic` tells them apart; nothing in storage does.

use crate::config::Config;
use crate::repo::Context;
use crate::tags;
use crate::task::{self, Entry, Task};
use anyhow::{Result, bail};
use std::path::Path;

/// The status `--waits-on` sets and `plan` reads as "waiting". Not a config
/// role: the docs have always named it, and a project without it in
/// `statuses.list` simply does not get the feature.
pub const BLOCKED: &str = "blocked";

/// `--waits-on` sets a status the project may not have; refuse before any id
/// is minted or anything is written, the way `cancel` refuses without a
/// cancel status.
pub fn require_blocked(cfg: &Config) -> Result<()> {
    if !cfg.has_status(BLOCKED) {
        bail!(
            "--waits-on needs a \"{BLOCKED}\" status; add it to statuses.list in .yman/config.toml"
        );
    }
    Ok(())
}

/// Every task that loads. Broken folders cannot relate to anything and are
/// skipped, as `show` and `rm` skip them.
pub fn tasks(ydir: &Path) -> Result<Vec<Task>> {
    Ok(task::list(ydir)?
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Task(t) => Some(t),
            Entry::Broken { .. } => None,
        })
        .collect())
}

/// A container rather than a blocker: relating to it does not mean waiting.
pub fn is_epic(t: &Task) -> bool {
    t.meta.tags.iter().any(|tag| tags::fold(tag) == "epic")
}

/// The tasks whose `related` carries `id`: the same rule as `ls --related`
/// and `show`'s `related by:`.
pub fn steps_of<'a>(tasks: &'a [Task], id: &str) -> Vec<&'a Task> {
    tasks
        .iter()
        .filter(|t| t.id() != id && t.meta.related.iter().any(|r| r == id))
        .collect()
}

/// What `t` is still waiting on: the tasks it relates to that exist here,
/// are open, and are not epics. Sorted in id order.
pub fn waits_on(t: &Task, tasks: &[Task], cfg: &Config) -> Vec<String> {
    let mut ids: Vec<String> = t
        .meta
        .related
        .iter()
        .filter(|r| r.as_str() != t.id())
        .filter(|r| {
            tasks
                .iter()
                .any(|o| o.id() == r.as_str() && !cfg.is_terminal(&o.meta.status) && !is_epic(o))
        })
        .cloned()
        .collect();
    ids.sort_by(|x, y| task::cmp_id(x, y));
    ids.dedup();
    ids
}

/// What closing `closed` changed for everyone else, on stderr: a blocked
/// task that now waits on nothing open gets a note naming the command that
/// moves it on. Nothing is moved — whether the work can really start is the
/// reader's call. Closing an epic unblocks nothing: its steps relate to it as
/// a container. Called once after a whole multi-id verb, so `done 32 33`
/// judges a task waiting on both after both are closed.
pub fn report_closed(ctx: &Context, closed: &[String]) -> Result<()> {
    if closed.is_empty() {
        return Ok(());
    }
    let tasks = tasks(&ctx.ydir)?;
    let cfg = ctx.config();
    let mut free: Vec<&str> = Vec::new();
    for id in closed {
        let Some(x) = tasks.iter().find(|t| t.id() == id) else {
            continue;
        };
        if is_epic(x) {
            continue;
        }
        for t in steps_of(&tasks, id) {
            if t.meta.status == BLOCKED
                && !free.contains(&t.id())
                && waits_on(t, &tasks, cfg).is_empty()
            {
                free.push(t.id());
            }
        }
    }
    free.sort_by(|x, y| task::cmp_id(x, y));
    for id in free {
        eprintln!(
            "note: {id} no longer waits on anything open: yman move {id} {}",
            cfg.statuses.default
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Scheme;
    use crate::task::{FolderName, Meta};
    use std::path::PathBuf;

    fn cfg() -> Config {
        let mut cfg = Config::new(Scheme::Seq);
        cfg.statuses.list = ["todo", "doing", "blocked", "done"]
            .map(String::from)
            .to_vec();
        cfg
    }

    fn mk(id: &str, status: &str, tags: &[&str], related: &[&str]) -> Task {
        let name = format!("5.{id}.t{id}");
        let mut meta = Meta::new(status, tags.iter().map(|s| s.to_string()).collect());
        meta.related = related.iter().map(|s| s.to_string()).collect();
        Task {
            folder: FolderName::parse(&name).unwrap(),
            parent: None,
            dir: PathBuf::from("/tmp/.yman").join(&name),
            title: format!("Task {id}"),
            body: String::new(),
            meta,
        }
    }

    #[test]
    fn epic_is_a_case_folded_tag() {
        assert!(is_epic(&mk("1", "todo", &["Epic"], &[])));
        assert!(!is_epic(&mk("1", "todo", &["epics"], &[])));
    }

    #[test]
    fn steps_are_the_tasks_relating_to_the_id() {
        let tasks = vec![
            mk("1", "todo", &["epic"], &["1"]),
            mk("2", "todo", &[], &["1"]),
            mk("3", "done", &[], &["2", "1"]),
            mk("4", "todo", &[], &["2"]),
        ];
        let ids: Vec<&str> = steps_of(&tasks, "1").iter().map(|t| t.id()).collect();
        assert_eq!(ids, ["2", "3"], "self-relation is not a step");
    }

    #[test]
    fn waits_on_skips_closed_missing_self_and_epics() {
        let tasks = vec![
            mk("1", "doing", &["epic"], &[]),
            mk("2", "todo", &[], &["1"]),
            mk("3", "done", &[], &[]),
            mk("10", "doing", &[], &[]),
            mk("5", "blocked", &[], &["1", "10", "3", "99", "5", "2"]),
        ];
        assert_eq!(waits_on(&tasks[4], &tasks, &cfg()), ["2", "10"]);
        assert!(waits_on(&tasks[1], &tasks, &cfg()).is_empty());
    }
}
