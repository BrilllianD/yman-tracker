use crate::config::Config;
use crate::task::{self, Task};
use std::cmp::Ordering;

/// The order every task listing uses: priority, then the status's place in
/// `statuses.list`, then id.
pub(crate) fn cmp_tasks(cfg: &Config, x: &Task, y: &Task) -> Ordering {
    x.priority()
        .cmp(&y.priority())
        .then_with(|| {
            cfg.status_index(&x.meta.status)
                .cmp(&cfg.status_index(&y.meta.status))
        })
        .then_with(|| task::cmp_id(x.id(), y.id()))
}

/// A body under its row, indented so every row still starts in column 0 and
/// a script can tell the two apart; blank body lines stay blank, not four
/// spaces.
pub(crate) fn print_body(body: &str) {
    for line in body.trim_matches('\n').lines() {
        if line.is_empty() {
            println!();
        } else {
            println!("    {line}");
        }
    }
}

/// Columns are padded by character count; good enough for the Latin and
/// Cyrillic titles this is meant for, and never worse than raw bytes.
pub(crate) fn display_width(s: &str) -> usize {
    s.chars().count()
}

pub(crate) fn join_row<const N: usize>(row: &[String; N], width: &[usize; N]) -> String {
    let mut out = String::new();
    // Trailing empty columns are dropped so short rows stay short.
    let last = row.iter().rposition(|c| !c.is_empty()).unwrap_or(0);
    for (i, cell) in row.iter().enumerate().take(last + 1) {
        if i > 0 {
            out.push_str("  ");
        }
        out.push_str(cell);
        if i < last {
            for _ in display_width(cell)..width[i] {
                out.push(' ');
            }
        }
    }
    out
}
