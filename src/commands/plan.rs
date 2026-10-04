use crate::cli::PlanArgs;
use crate::config::Config;
use crate::json;
use crate::plan::{self, BLOCKED};
use crate::repo::Context;
use crate::task::{self, Task};
use anyhow::Result;

use super::ls::{cmp_tasks, display_width, join_row, print_body};

/// One step as printed: the task, and what it waits on if it is blocked.
struct Step<'a> {
    task: &'a Task,
    waits_on: Vec<String>,
}

pub fn run(ctx: &mut Context, a: PlanArgs) -> Result<()> {
    let head = task::find(&ctx.ydir, &a.id)?;
    let tasks = plan::tasks(&ctx.ydir)?;
    let cfg = ctx.config();

    let mut steps = plan::steps_of(&tasks, head.id());
    // Counted before the closed ones are hidden: progress is the point.
    let counts = counts(cfg, &steps);
    let total = steps.len();
    if !a.all {
        steps.retain(|t| !cfg.is_terminal(&t.meta.status));
    }
    steps.sort_by(|x, y| cmp_tasks(cfg, x, y));
    if let Some(n) = a.limit {
        steps.truncate(n);
    }
    let steps: Vec<Step> = steps
        .into_iter()
        .map(|t| Step {
            task: t,
            waits_on: if t.meta.status == BLOCKED {
                plan::waits_on(t, &tasks, cfg)
            } else {
                Vec::new()
            },
        })
        .collect();

    if a.json {
        print_json(&head, &counts, &steps, a.long);
    } else {
        print_text(&head, &counts, total, &steps, a.long);
    }
    Ok(())
}

/// Steps per status, in `statuses.list` order; a status the config does not
/// know (a hand edit, an older config) comes last, alphabetically. Zero
/// counts are left out.
fn counts(cfg: &Config, steps: &[&Task]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = cfg.statuses.list.iter().map(|s| (s.clone(), 0)).collect();
    for t in steps {
        match out.iter_mut().find(|(s, _)| *s == t.meta.status) {
            Some((_, n)) => *n += 1,
            None => out.push((t.meta.status.clone(), 1)),
        }
    }
    let known = cfg.statuses.list.len();
    out[known..].sort();
    out.retain(|(_, n)| *n > 0);
    out
}

fn print_text(head: &Task, counts: &[(String, usize)], total: usize, steps: &[Step], long: bool) {
    println!("{}  {}", head.id(), head.title);
    let mut line = format!("status: {}   steps: {total}", head.meta.status);
    if !counts.is_empty() {
        let parts: Vec<String> = counts.iter().map(|(s, n)| format!("{s} {n}")).collect();
        line.push_str("   ");
        line.push_str(&parts.join(", "));
    }
    println!("{line}");

    let rows: Vec<[String; 5]> = steps
        .iter()
        .map(|s| {
            [
                s.task.priority().to_string(),
                s.task.id().to_string(),
                s.task.meta.status.clone(),
                s.task.title.clone(),
                if s.waits_on.is_empty() {
                    String::new()
                } else {
                    format!("waits on {}", s.waits_on.join(", "))
                },
            ]
        })
        .collect();
    let mut width = [0usize; 5];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            width[i] = width[i].max(display_width(cell));
        }
    }
    for (row, s) in rows.iter().zip(steps) {
        println!("{}", join_row(row, &width));
        if long {
            print_body(&s.task.body);
        }
    }
}

/// One object; `steps` and every list in it are `[]` rather than missing,
/// as in `show --json`.
fn print_json(head: &Task, counts: &[(String, usize)], steps: &[Step], long: bool) {
    let counts = counts
        .iter()
        .fold(json::Object::new(), |o, (s, n)| o.raw(s, n.to_string()))
        .finish();
    let steps = json::array(steps.iter().map(|s| {
        let t = s.task;
        let mut obj = json::Object::new()
            .str("id", t.id())
            .raw("priority", t.priority().to_string())
            .str("status", &t.meta.status)
            .str("title", &t.title)
            .raw("tags", json::strings(&t.meta.tags))
            .opt("assignee", t.meta.assignee.as_deref())
            .raw("waits_on", json::strings(&s.waits_on))
            .str("dir", &t.rel());
        if long {
            obj = obj.str("body", t.body.trim_matches('\n'));
        }
        obj.finish()
    }));
    println!(
        "{}",
        json::Object::new()
            .str("id", head.id())
            .raw("priority", head.priority().to_string())
            .str("status", &head.meta.status)
            .str("title", &head.title)
            .raw("counts", counts)
            .raw("steps", steps)
            .finish()
    );
}
