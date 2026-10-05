//! `add --sections`: one markdown file, one task per `# ` heading.
//!
//! Parsed by hand, like `t.md` titles: a line starting with `# ` opens a
//! section and is its title; everything up to the next one is the body. A
//! heading inside a fenced code block (``` or ~~~) is body text, so a shell
//! comment in an example does not split a task in two. Fences follow
//! CommonMark: three or more of one character open one, and only a run of the
//! same character, at least as long and with nothing after it, closes it. An
//! unclosed fence is refused rather than swallowing every later heading.
//! `##` and deeper are body text too — a step's body can have its own
//! structure.

use anyhow::{Result, bail};

/// `(title, body)` per section, in file order. `source` names the input in
/// errors. Every error is raised before the caller mints an id.
pub fn parse(text: &str, source: &str) -> Result<Vec<(String, String)>> {
    let source = if source == "-" { "stdin" } else { source };
    let mut out: Vec<(String, Vec<&str>)> = Vec::new();
    // The open fence: its character, its length, and the line it opened on.
    let mut fence: Option<(char, usize, usize)> = None;
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        match (fence, fence_of(line)) {
            (Some((c, len, _)), Some((m, mlen, info)))
                if m == c && mlen >= len && info.is_empty() =>
            {
                fence = None
            }
            (None, Some((m, mlen, _))) => fence = Some((m, mlen, n)),
            (None, None) if line == "#" || line.starts_with("# ") => {
                let title = line[1..].trim();
                if title.is_empty() {
                    bail!("{source}: line {n}: empty title");
                }
                out.push((title.to_string(), Vec::new()));
                continue;
            }
            _ => {}
        }
        match out.last_mut() {
            Some((_, body)) => body.push(line),
            None if line.trim().is_empty() => {}
            None => bail!("{source}: line {n}: text before the first \"# \" heading"),
        }
    }
    if let Some((_, _, n)) = fence {
        bail!("{source}: line {n}: unclosed code fence");
    }
    if out.is_empty() {
        bail!("{source}: no \"# \" heading, so no tasks");
    }
    Ok(out
        .into_iter()
        .map(|(title, body)| (title, trim_blank_lines(&body)))
        .collect())
}

/// A fence line: its character, run length, and what follows the run (the
/// info string on an opening fence; must be empty on a closing one).
fn fence_of(line: &str) -> Option<(char, usize, &str)> {
    let head = line.trim_start();
    let c = head.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = head.bytes().take_while(|&b| b == c as u8).count();
    (len >= 3).then(|| (c, len, head[len..].trim()))
}

/// The body without blank lines at either end, as `t.md` stores it.
fn trim_blank_lines(lines: &[&str]) -> String {
    let blank = |l: &&str| l.trim().is_empty();
    let start = lines.iter().position(|l| !blank(l)).unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !blank(l))
        .map_or(start, |i| i + 1);
    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(text: &str) -> Vec<(String, String)> {
        parse(text, "plan.md").unwrap()
    }

    #[test]
    fn one_task_per_heading_with_trimmed_bodies() {
        let got = titles("\n# One\n\nbody one\n\n## sub\nmore\n\n#  Two  \n# Three\n\nlast\n");
        assert_eq!(
            got,
            [
                ("One".into(), "body one\n\n## sub\nmore".into()),
                ("Two".into(), String::new()),
                ("Three".into(), "last".into()),
            ]
        );
    }

    #[test]
    fn fenced_headings_stay_in_the_body() {
        let got = titles("# One\n```sh\n# a comment\n~~~\n```\n#not a heading\n# Two\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].1, "```sh\n# a comment\n~~~\n```\n#not a heading");
        let got = titles("# One\n~~~\n# x\n```\n# y\n~~~\n# Two\n");
        assert_eq!(got.len(), 2, "a fence closes only on its own marker");
    }

    #[test]
    fn crlf_is_read_like_lf() {
        let got = titles("# One\r\nbody\r\n# Two\r\n");
        assert_eq!(got[0], ("One".into(), "body".into()));
        assert_eq!(got[1].0, "Two");
    }

    #[test]
    fn bad_input_is_refused_with_the_line() {
        let err = |t: &str| parse(t, "plan.md").unwrap_err().to_string();
        assert_eq!(
            err("\nintro\n# One\n"),
            "plan.md: line 2: text before the first \"# \" heading"
        );
        assert_eq!(err("# One\n#\n"), "plan.md: line 2: empty title");
        assert_eq!(err("# One\n#   \n"), "plan.md: line 2: empty title");
        assert_eq!(err("\n\n"), "plan.md: no \"# \" heading, so no tasks");
        assert_eq!(
            parse("", "-").unwrap_err().to_string(),
            "stdin: no \"# \" heading, so no tasks"
        );
    }

    #[test]
    fn an_unclosed_fence_is_refused() {
        let err = parse("# One\n\n```sh\n# not a heading\n# Two\n", "plan.md").unwrap_err();
        assert_eq!(err.to_string(), "plan.md: line 3: unclosed code fence");
    }

    #[test]
    fn a_fence_line_with_an_info_string_does_not_close() {
        let got = titles("# One\n```\n```sh\n# inside\n```\n# Two\n");
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got[0].1, "```\n```sh\n# inside\n```");
        assert_eq!(got[1].0, "Two");
    }

    #[test]
    fn a_longer_fence_holds_a_shorter_one() {
        let got = titles("# One\n````md\n```\n# inside\n```\n````\n# Two\n");
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got[1].0, "Two");
        // And a different character does not close it either.
        let got = titles("# One\n~~~\n```\n# inside\n~~~~\n# Two\n");
        assert_eq!(got.len(), 2, "{got:?}");
    }
}
