# `yman-tracker` — implementation spec

Self-contained. An implementer needs only this file, Rust toolchain, and `git` ≥ 2.42.

## 1. Context

Project task tracker that lives next to the code, syncs through the project's own git remote, yet never touches the main git tree, branches, or CI. A CLI `yman` wraps every git operation.

Fixed decisions (do not revisit):
- Language: Rust. Git access: shell out to the `git` binary via `std::process::Command`. No `git2`, no `gix`.
- `.yman/` in the project root is a **linked git worktree of the main repo**, whose HEAD is the symbolic ref `refs/yman/local` (outside `refs/heads/` → invisible to `git branch` and IDE branch pickers; visible only in `git log --all`, `git show-ref`, `git worktree list`).
- Remote side: single ref `refs/tasks/main`. Never a branch. Fetched into main as `refs/yman/remote` via an extra fetch refspec that `yman init` adds to the main repo config.
- One task = one folder `.yman/{priority}.{id}.{slug}/` containing `t.md`, `m.yml`, optional `d.md`, optional `f/`.
- ID scheme (`random` | `seq` | `author`) chosen at `init`, stored in committed `config.toml`.
- Refresh policy: `yman.refresh = lazy` (default) | `manual`; optional git hooks for eager refresh.
- Slug max 200 bytes by default.

Crate location: `/home/bronnikov/Projects/yman-tracker/` (already `cargo init`-ed: `Cargo.toml`, `src/main.rs`, `.gitignore`). Package name `yman-tracker`, binary `yman`.

Environment: Linux, git 2.55, rustc 1.98. Tests must not need network.

## 2. Vocabulary and paths

| Term | Meaning |
|---|---|
| `ROOT` | main repo toplevel: `git rev-parse --show-toplevel` |
| `COMMON` | main repo common git dir: `git rev-parse --git-common-dir` (absolute; handles worktrees and `.git`-file layouts) |
| `YDIR` | `ROOT/.yman` |
| `WT_GITDIR` | `COMMON/worktrees/yman` — private git dir of the `.yman` worktree (HEAD, index, MERGE_HEAD live here) |
| `LOCAL` | `refs/yman/local` — task history head |
| `REMOTE` | `refs/yman/remote` — remote-tracking ref |
| `REMOTE_REF` | `refs/tasks/main` — ref name on the origin server |
| `<dir>` | task folder name `{p}.{id}.{slug}` |

Git invocation targets:
- "in main": `git -C ROOT …` — ref operations, fetch, push, config, worktree management.
- "in wt": `git -C YDIR …` — everything touching task files: add, mv, rm, commit, merge, status, diff, log.
Both see the same object store and refs. `rev-parse`/`update-ref` on `LOCAL`/`REMOTE` work from either.

## 3. On-disk formats

### 3.1 Layout

```
ROOT/
  .git/
    config                      # + remote.origin.fetch refspec, yman.refresh, yman.author
    info/exclude                # + ".yman/"
    refs/yman/local
    refs/yman/remote
    worktrees/yman/             # git-managed
    hooks/post-merge            # optional (--hooks)
    hooks/post-checkout         # optional
  .yman/
    .git                        # file: "gitdir: <COMMON>/worktrees/yman"
    .gitignore
    .gitattributes
    config.toml
    2.14.fix-login/
      t.md
      m.yml
      d.md                      # only after first comment
      f/                        # only after first attachment
        screenshot.png
```

### 3.2 Task folder name

Regex (anchored): `^([0-9])\.([^./\\]+)\.(.+)$` → `priority: u8`, `id: String`, `slug: String`.

- Priority `0`–`9`, `0` = highest. Plain `ls .yman/` therefore lists highest priority first.
- `id` never contains `.`; all schemes satisfy this (`14`, `t-7f3a`, `ab-12`).
- Slug = `slugify(title)`:
  1. Unicode lowercase (`to_lowercase()`).
  2. Each char that is alphanumeric (`char::is_alphanumeric`) is kept; every other char becomes `-`.
  3. Collapse runs of `-`, trim leading/trailing `-`.
  4. Truncate to `config.slug.max_bytes` bytes at a char boundary (`s.is_char_boundary`), then trim trailing `-` again.
  5. Empty result → `task`.
- Folder name is the **source of truth** for priority and id. `m.yml` does not repeat them.
- Entries in `.yman/` that are not directories, or are directories not matching the regex, are ignored by `list()` (`.git` file, `config.toml`, etc.). A directory that matches the regex but fails to load (bad `m.yml`, `t.md` without H1) is a **broken task**: `ls` prints it with `!` marker and the error; other commands error out when targeting it.
- Lookup by id: scan `.yman/*`, match on the id segment. Two folders with the same id → error `duplicate task id <id>: <dirA>, <dirB>` (can only happen after a bad manual merge).

### 3.3 `t.md`

```markdown
# Fix login

Free body. May be empty.
```
- Title = text of the first line matching `^#\s+(.*\S)\s*$` scanning from the top, skipping blank lines. Anything before it that is not blank → parse error `t.md must start with "# Title"`.
- Body = everything after the title line, with one leading blank line stripped. Written back verbatim.
- `add` writes `# {title}\n\n{body}\n` (body may be empty → file is `# {title}\n`).

### 3.4 `m.yml`

```yaml
status: todo
tags: [auth, bug]
assignee: null
created: 2026-09-16T10:00:00Z
updated: 2026-09-16T10:12:30Z
attachments:
  - path: f/screenshot.png
    name: screenshot.png
    added: 2026-09-16T10:05:00Z
    by: Ivan
links:
  - https://github.com/org/repo/issues/12
related: ["5", "t-7f3a"]
```

Rust:
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Meta {
    pub status: String,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub assignee: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    #[serde(default)] pub attachments: Vec<Attachment>,
    #[serde(default)] pub links: Vec<String>,
    #[serde(default)] pub related: Vec<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Attachment {
    pub path: String,      // relative to task folder, always "f/<name>"
    pub name: String,
    pub added: DateTime<Utc>,
    pub by: String,
}
```
- Timestamps: RFC 3339, UTC, second precision (`Utc::now().with_nanosecond(0)`), serialized as `2026-09-16T10:05:00Z` (chrono serde default is fine; keep `Z`).
- Unknown keys: preserved? No — v1 uses plain serde; unknown keys are dropped on rewrite. Acceptable, documented.
- `status` validated against `config.statuses.list` on load only with a warning, never a hard error (so a config change does not brick tasks).

### 3.5 `d.md`

Append-only. Entry format, exactly:
```markdown
## 2026-09-16T10:10:00Z — Ivan

comment text (may span lines)

```
i.e. header line `## {rfc3339} — {author}`, blank line, text, blank line. `append_entry` opens with `OpenOptions::append().create(true)` and writes `format!("## {} — {}\n\n{}\n\n", ts, author, text.trim_end())`. Parser splits on lines starting with `## ` and parses the header with `^## (\S+) — (.*)$`; unparsable chunks are kept as raw text so `show` never fails on hand-edited files.

`.gitattributes` contains `*/d.md merge=union` so concurrent comments merge without conflict.

### 3.6 `config.toml` (committed in `.yman/`)

```toml
version = 1

[ids]
scheme = "seq"        # "random" | "seq" | "author"
random_len = 4        # hex chars, random scheme only

[statuses]
list = ["todo", "doing", "done"]
default = "todo"

[priorities]
default = 5           # 0..=9

[slug]
max_bytes = 200       # hard cap 240
```
Rust:
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub version: u32,
    pub ids: IdsCfg,
    pub statuses: StatusesCfg,
    #[serde(default)] pub priorities: PrioCfg,
    #[serde(default)] pub slug: SlugCfg,
}
pub struct IdsCfg { pub scheme: Scheme, #[serde(default = "four")] pub random_len: u8 }
pub enum Scheme { Random, Seq, Author }          // serde rename_all = "lowercase"
pub struct StatusesCfg { pub list: Vec<String>, pub default: String }
pub struct PrioCfg { pub default: u8 }            // Default → 5
pub struct SlugCfg { pub max_bytes: usize }       // Default → 200
```
Validation on load: `version == 1`; `statuses.list.len() >= 2` and contains `default`; `priorities.default <= 9`; `slug.max_bytes` in `1..=240`; `random_len` in `2..=16`. Failure → error `invalid .yman/config.toml: <why>`.

Derived: `start` status = `list[1]`, `done` status = `list.last()`.

### 3.7 Other root files written at init

`.gitignore`:
```
*.swp
*~
.#*
*.orig
```
`.gitattributes`:
```
*/d.md merge=union
```

### 3.8 Main repo config keys

| Key | Value | Set by |
|---|---|---|
| `remote.origin.fetch` (multi-valued, appended) | `+refs/tasks/main:refs/yman/remote` | init, once (check with `git config --get-all remote.origin.fetch`) |
| `yman.refresh` | `lazy` \| `manual` | init (`--refresh`), default `lazy` |
| `yman.author` | author prefix string | init `--author`, only meaningful for `author` scheme |

Push is always explicit `git push origin refs/yman/local:refs/tasks/main`. Never set `remote.origin.push` (it would hijack the user's plain `git push`).

## 4. Crate structure

```
Cargo.toml
src/
  main.rs         entry: parse Cli, build Context, run preflight+refresh per command, map errors → exit code
  cli.rs          clap derive types only
  errors.rs       YmanError enum + exit code mapping
  git.rs          Git runner
  repo.rs         Context discovery, main-config helpers, exclude file, worktree state, preflight
  refresh.rs      lazy refresh
  config.rs       Config + load/save/validate + defaults
  task.rs         folder name parse/format, slugify, Task load/save/list/find, rename
  discussion.rs   d.md append + parse
  ids.rs          id generation, taken-set computation, next_free, author prefix
  hooks.rs        install/remove/status
  commands/
    mod.rs        pub fn dispatch(ctx, cmd) -> Result<()>
    init.rs add.rs ls.rs show.rs edit.rs set.rs rm.rs attach.rs comment.rs path.rs log.rs status.rs
    sync.rs refresh.rs hooks.rs git.rs
tests/
  common/mod.rs
  cli.rs          integration tests (all scenarios in §9)
```

### 4.1 `Cargo.toml`

```toml
[package]
name = "yman-tracker"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "yman"
path = "src/main.rs"

[dependencies]
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
rand = "0.9"
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
toml = "0.8"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```
`serde_yaml` is archived but stable; acceptable for v1.

### 4.2 `errors.rs`

```rust
pub enum Exit { Ok = 0, Error = 1, Usage = 2, MergePending = 3 }

#[derive(thiserror-free, Debug)]
pub struct YmanError { pub msg: String, pub exit: Exit }
```
Use `anyhow::Error` everywhere; attach `Exit` by downcasting a `MergePending` marker type: `struct MergePending(String)` implementing `Display`+`Error`. `main` does: if `err.downcast_ref::<MergePending>()` → exit 3; else exit 1. clap handles usage errors → exit 2 automatically. All errors print `error: {msg}` to stderr (anyhow chain with `{:#}`).

### 4.3 `git.rs`

```rust
pub struct Git { pub dir: PathBuf }

pub struct Output { pub status: i32, pub stdout: String, pub stderr: String }

impl Git {
    pub fn new(dir: impl Into<PathBuf>) -> Self;
    /// Run, capture both streams. Never fails on non-zero exit; caller inspects.
    pub fn run(&self, args: &[&str]) -> Result<Output>;
    /// Run, capture, error if exit != 0 (error text = trimmed stderr).
    pub fn ok(&self, args: &[&str]) -> Result<Output>;
    /// Same as ok(), returns trimmed stdout.
    pub fn out(&self, args: &[&str]) -> Result<String>;
    /// Run with inherited stdio (fetch/push/editor-like), returns exit status.
    pub fn interactive(&self, args: &[&str]) -> Result<i32>;
    /// Run with stdin = data (update-ref --stdin, mktree).
    pub fn with_stdin(&self, args: &[&str], stdin: &str) -> Result<Output>;

    // helpers used everywhere
    pub fn rev_parse(&self, r: &str) -> Result<Option<String>>;       // -q --verify, None if missing
    pub fn is_ancestor(&self, a: &str, b: &str) -> Result<bool>;       // merge-base --is-ancestor
    pub fn merge_base(&self, a: &str, b: &str) -> Result<Option<String>>;
    pub fn is_dirty(&self) -> Result<bool>;                            // status --porcelain non-empty
    pub fn commit(&self, msg: &str) -> Result<()>;                     // see §4.3.1
}
```
Every invocation: `Command::new("git").arg("-C").arg(&self.dir).args(args)`, and `.env_remove()` for `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR` so running `yman` from inside a git hook or alias does not leak. Add `-c core.quotePath=false` to every call so non-ASCII folder names come back unescaped in `status`/`diff`/`ls-tree` output. Add `-c color.ui=never`.

Missing `git` binary → error `git not found in PATH`.

#### 4.3.1 Commit and the symref question

Inside the worktree, HEAD is a symbolic ref to `refs/yman/local`. `git commit` and `git merge` update the ref HEAD points at, whatever its namespace, so normally nothing special is needed. **Spike first (§8 phase 0)**: in a temp repo, create the worktree as in init, run `git commit`, and check `git rev-parse refs/yman/local` moved. If it did not, set `Git::commit` to run `git commit -q -m msg` followed by `git update-ref refs/yman/local HEAD` and make preflight accept detached HEAD equal to `LOCAL`. Keep this logic in exactly one place (`Git::commit` + `repo::check_worktree_head`).

Commit always: `git commit -q --no-verify -m <msg>` (`--no-verify` so main-repo hooks under `core.hooksPath` cannot interfere; `--no-gpg-sign` is NOT added — respect user's signing config).

Commit error containing `Please tell me who you are` → rewrap: `git identity missing; run: git config --global user.name "…" && git config --global user.email "…"`.

### 4.4 `repo.rs`

```rust
pub struct Context {
    pub root: PathBuf,        // ROOT
    pub common: PathBuf,      // COMMON (absolute)
    pub ydir: PathBuf,        // ROOT/.yman
    pub wt_gitdir: PathBuf,   // COMMON/worktrees/yman
    pub main: Git,            // Git::new(root)
    pub wt: Git,              // Git::new(ydir)
    pub config: Option<Config>,   // loaded lazily after preflight
}

pub fn discover() -> Result<Context>;           // errors: "not inside a git repository"
pub fn origin_url(&self) -> Result<Option<String>>;
pub fn exclude_add(&self) -> Result<bool>;      // appends ".yman/" if absent; returns whether added
pub fn fetch_refspec_present(&self) -> Result<bool>;
pub fn fetch_refspec_add(&self) -> Result<()>;
pub fn get_cfg(&self, key: &str) -> Result<Option<String>>;   // git config --get in main
pub fn set_cfg(&self, key: &str, val: &str) -> Result<()>;
pub enum YdirState { Absent, Worktree, StandaloneRepo, PlainDir }
pub fn ydir_state(&self) -> YdirState;   // .git file whose content starts with "gitdir:" and resolves under COMMON → Worktree
pub fn merge_in_progress(&self) -> bool; // wt_gitdir/MERGE_HEAD exists
pub fn preflight(&mut self, mutating: bool) -> Result<()>;
```

`preflight(mutating)`:
1. `ydir_state() == Worktree` else error `.yman is not initialized; run: yman init`.
2. `wt.out(["symbolic-ref","-q","HEAD"])` must equal `refs/yman/local` (or, in the fallback mode of §4.3.1, `rev-parse HEAD == rev-parse LOCAL`) else error `.yman worktree is not on refs/yman/local; run: yman init`.
3. Load `config.toml` into `ctx.config`.
4. If `mutating && merge_in_progress()` → `MergePending("sync merge in progress; resolve conflicts then run: yman sync --continue  (or: yman sync --abort)")`.

### 4.5 `task.rs`

```rust
pub struct FolderName { pub priority: u8, pub id: String, pub slug: String }
impl FolderName {
    pub fn parse(name: &str) -> Option<FolderName>;
    pub fn to_string(&self) -> String;          // "{p}.{id}.{slug}"
}
pub fn slugify(title: &str, max_bytes: usize) -> String;
pub fn extract_title(md: &str) -> Result<(String, String)>;   // (title, body)
pub fn render_md(title: &str, body: &str) -> String;

pub struct Task {
    pub folder: FolderName,
    pub dir: PathBuf,           // absolute
    pub title: String,
    pub body: String,
    pub meta: Meta,
}
pub enum Entry { Task(Task), Broken { dir: PathBuf, error: String } }

pub fn list(ydir: &Path) -> Result<Vec<Entry>>;        // sorted by (priority, status index?, id) — sort is done by ls, list() returns folder-name order
pub fn find(ydir: &Path, id: &str) -> Result<Task>;    // errors: "task <id> not found", duplicate id
pub fn load(dir: &Path) -> Result<Task>;
impl Task {
    pub fn write_md(&self) -> Result<()>;
    pub fn write_meta(&self) -> Result<()>;
    pub fn touch(&mut self);                            // meta.updated = now
    pub fn rel(&self) -> String;                        // folder name (path relative to ydir)
    pub fn comment_count(&self) -> usize;               // parse d.md if present
}
```
ID comparison for sorting: if both ids are all digits → numeric; else if same `prefix-` and numeric tails → by tail; else lexical.

### 4.6 `ids.rs`

```rust
pub fn author_prefix(ctx: &Context) -> Result<String>;
// order: main git config yman.author → env YMAN_AUTHOR → initials of `git config user.name`
// (first letter of each whitespace-separated word, lowercased, ASCII letters only) → error if empty:
// "author prefix unknown; run: git config yman.author <prefix>  (or set YMAN_AUTHOR)"

pub fn ever_assigned(ctx: &Context, refs: &[&str]) -> Result<HashSet<String>>;
// git log <refs...> --diff-filter=A --name-only --format= -- .
// take each path, split on '/', if it has ≥2 segments parse segment[0] as FolderName → id.
// Ignores root files. Run in wt or main (same store). Missing refs are skipped.

pub fn fs_ids(ydir: &Path) -> Result<HashSet<String>>;   // from list(), including Broken entries whose folder name parses

pub fn next_free(scheme: Scheme, cfg: &Config, prefix: Option<&str>, taken: &HashSet<String>) -> String;
// random: loop { "t-" + random_len hex chars from rand::rng() } until not taken
// seq:    max over taken ids matching ^\d+$ (parse u64) + 1, or 1
// author: max over taken ids matching ^{prefix}-(\d+)$ + 1, or 1 → "{prefix}-{n}"

pub fn new_id(ctx: &Context) -> Result<String>;          // taken = fs_ids ∪ ever_assigned([LOCAL, REMOTE])
```

### 4.7 `refresh.rs`

```rust
pub struct RefreshReport { pub applied: usize, pub skipped: Option<&'static str> }
pub fn refresh(ctx: &Context, quiet: bool) -> Result<RefreshReport>;
```
Steps:
1. `main.run(["rev-parse","-q","--verify","refs/yman/local","refs/yman/remote"])`: exit ≠ 0 or only one line → `REMOTE` missing → return applied 0. If the two hashes are equal → applied 0.
2. `!is_ancestor(LOCAL, REMOTE)` → skipped `"local has unpushed commits; run: yman sync"` (printed only by `yman status`/`yman refresh`, not on every lazy call).
3. `wt.is_dirty()` → skipped `"worktree has uncommitted changes"`; print `note: .yman has uncommitted changes, refresh skipped` on stderr unless quiet.
4. `n = rev-list --count LOCAL..REMOTE`; `wt.ok(["merge","-q","--ff-only","refs/yman/remote"])`; print `refreshed: {n} new commit(s)` on stderr unless quiet. Return applied n.

`main.rs` calls `refresh(ctx, true)` before every command except `init`, `sync`, `refresh`, `status`, `hooks`, `git`, when `yman.refresh` is unset or `lazy`. Failures inside lazy refresh are downgraded to a stderr `warning:` and never abort the actual command.

### 4.8 `hooks.rs`

Marker line: `# yman-hook v1`. Script body:
```sh
#!/bin/sh
# yman-hook v1
command -v yman >/dev/null 2>&1 && yman refresh --quiet
exit 0
```
- Target dir: `git config core.hooksPath` if set (resolve relative to ROOT; print `warning: installing into core.hooksPath=<p>`), else `COMMON/hooks`.
- `install`: for `post-merge`, `post-checkout`: if absent → write + `chmod 755`; if present with marker → skip (`already installed`); if present without marker → do not touch, print `hook <name> exists; add this line to it:\n    yman refresh --quiet` and return error (exit 1) after processing both.
- `remove`: delete only files containing the marker.
- `status`: print per hook: `installed` / `foreign` / `absent`.

## 5. CLI surface (`cli.rs`)

```
yman init     [--id-scheme random|seq|author] [--author <pfx>] [--remote <url>] [--offline] [--hooks] [--refresh lazy|manual]
yman add      <title> [-p <0-9>] [-s <status>] [-t <tag>]... [-m <body>] [-e]
yman ls       [-s <status>]... [-t <tag>]... [-a] [--json]
yman show     <id>
yman edit     <id>
yman set      <id> [--status S] [--priority 0-9] [--title T] [--assignee A|--no-assignee]
                   [--tag X]... [--untag X]... [--link URL]... [--unlink URL]... [--relate ID]... [--unrelate ID]...
yman start    <id>          = set --status <list[1]>
yman done     <id>          = set --status <list.last()>
yman prio     <id> <0-9>    = set --priority
yman rm       <id> [-f]
yman attach   <id> <file>... [--name <n>] [--force]
yman detach   <id> <name>
yman comment  <id> [-m <text>] [-e]
yman path     <id>
yman log      [<id>] [-n <N>]
yman status
yman refresh  [--quiet]
yman hooks    install|remove|status
yman sync     [--continue] [--abort] [--no-push]
yman git      [--] <args>...
```
Global: `--version`, `-h`. All output to stdout except notes/warnings/errors to stderr. No color in v1.

## 6. Command semantics

Unless stated, each command runs `preflight(mutating)` then lazy refresh, then its body. Mutating = add, edit, set/start/done/prio, rm, attach, detach, comment, sync. Commit messages are exact strings below; `{title}` is the title with `"` replaced by `'`.

### 6.1 `init`

No preflight. Steps:
1. `discover()`. Failure → error `not inside a git repository`.
2. URL = `--remote` or `origin_url()`; none → error `main repo has no "origin" remote; pass --remote <url>`.
3. `ydir_state()`:
   - `Worktree` → already initialized. Ensure exclude line, fetch refspec, `yman.refresh` are present (idempotent repairs). Load config; if `--id-scheme` given and differs → `warning: id scheme is "<cfg>" (from config.toml); --id-scheme ignored`. If `--hooks` → install hooks. Print summary. Exit 0.
   - `StandaloneRepo` → error `.yman is a standalone git repository, not a worktree; move it aside and rerun`.
   - `PlainDir` → error `.yman exists and is not a yman worktree; move it aside and rerun`.
   - `Absent` → continue.
4. `exclude_add()`.
5. `fetch_refspec_add()` if absent. `set_cfg("yman.refresh", --refresh or "lazy")`. If `--author` → `set_cfg("yman.author", …)`.
6. `main.run(["worktree","prune"])`.
7. Unless `--offline`: `main.interactive(["fetch","origin","+refs/tasks/main:refs/yman/remote"])`. Exit ≠ 0 → check whether stderr mentions `couldn't find remote ref` (git prints `fatal: couldn't find remote ref refs/tasks/main`) → treat as "remote has no tasks yet"; other failures → error `fetch failed (see above); use --offline to skip`. Implementation: run with captured stderr first (`run`), and if it fails for a reason other than missing ref, re-run interactively? Simpler: capture (`run`) and print stderr through on unexpected failure. Auth prompts on captured stderr still work because git talks to the tty directly for ssh; for https credential helpers also use the tty. Use `run`, then print stderr verbatim on failure.
8. Determine start commit:
   - `rev_parse(LOCAL)` is Some → `fresh = false` (worktree was deleted but history survived).
   - else `rev_parse(REMOTE)` is Some → `update-ref refs/yman/local refs/yman/remote`; `fresh = false`.
   - else → `T = with_stdin(["mktree"], "")`, `C = out(["commit-tree", T, "-m", "yman: init"])`, `update-ref refs/yman/local C`; `fresh = true`.
9. `main.ok(["worktree","add","--detach", ydir, "refs/yman/local"])`; then `wt.ok(["symbolic-ref","HEAD","refs/yman/local"])` (or fallback per §4.3.1). Error containing `already checked out` / `is already used by worktree` → error `refs/yman/local is already checked out in another worktree of this repo; only one .yman per clone is supported`.
10. If `fresh`: write `config.toml` (scheme from `--id-scheme`, default `seq`), `.gitignore`, `.gitattributes`; `wt.ok(["add","-A"])`; `wt.commit("yman: init (<scheme>)")`; unless `--offline`: push (§6.19 push helper). Push rejected → someone initialized concurrently: fetch again, `wt.ok(["reset","-q","--hard","refs/yman/remote"])`, `warning: remote already had tasks; adopted remote state`.
11. Else: load and validate `config.toml`; failure → error `refs/tasks/main on origin is not a yman history (missing or invalid config.toml)`.
12. `--hooks` → `hooks::install`.
13. Print:
```
initialized .yman
  scheme:   seq
  remote:   git@github.com:org/repo.git  (refs/tasks/main)
  refresh:  lazy
  hooks:    installed | not installed
  tasks:    3
```

### 6.2 `add`

1. `id = ids::new_id(ctx)`; `priority = -p or config.priorities.default`; `status = -s or config.statuses.default` (validate ∈ list → error `unknown status "<s>"; allowed: todo, doing, done`); tags from `-t` (dedupe, keep order).
2. `folder = FolderName{priority,id,slug: slugify(title, max)}`; dir must not exist → error `folder already exists: <dir>`.
3. Create dir; write `t.md` via `render_md(title, body_from_-m_or_empty)`; write `m.yml` with `created = updated = now`.
4. If `-e`: open editor on `t.md` (§6.5 editor rules); re-parse; title may have changed → recompute slug and rename dir before commit.
5. `wt.ok(["add","--", dir])`; `wt.commit(&format!("task({id}): add \"{title}\""))`.
6. Print `added {id}  {dir}`.

### 6.3 `ls`

No git calls. `list()` → filter: default hides tasks whose status == `done` status (last in list) unless `-a`; `-s` (repeatable) restricts to given statuses (implies showing done if named); `-t` requires all given tags. Sort by `(priority, status index, id)`.

Text format (aligned columns, header only when stdout is a tty):
```
P  ID     STATUS  TITLE                          TAGS       F  C
2  14     doing   Fix login                      auth,bug   1  3
5  15     todo    Write docs
!  <dir>  broken  <error text>
```
`F` = attachment count, `C` = comment count; blank when 0. `--json`: array of objects `{id, priority, status, title, tags, assignee, created, updated, attachments: n, comments: n, dir}`; broken entries as `{dir, error}`.

### 6.4 `show`

```
14  Fix login
priority: 2   status: doing   assignee: -   tags: auth, bug
created: 2026-09-16T10:00:00Z   updated: 2026-09-16T10:12:30Z
links:    https://github.com/org/repo/issues/12
related:  5, t-7f3a
folder:   .yman/2.14.fix-login

<body verbatim>

attachments:
  screenshot.png   (added 2026-09-16T10:05:00Z by Ivan)

discussion:
  2026-09-16T10:10:00Z — Ivan
    comment text
```
Sections `links`, `related`, `attachments`, `discussion` are omitted when empty. Body omitted when empty.

### 6.5 `edit`

1. Editor = `$VISUAL`, else `$EDITOR`, else `vi`. Split on whitespace (allows `code --wait`). Spawn with inherited stdio on `<dir>/t.md`. Non-zero exit → error `editor exited with status N; file left as is`.
2. Re-load task. Parse failure → error `t.md invalid after edit: <why>; fix the file then run: yman edit <id>` (file left modified; the next mutating command will snapshot-commit it — document; do not revert).
3. `wt.run(["diff","--quiet","--","<dir>/t.md"])` exit 0 → print `no changes`, done.
4. If title changed → slug changed → `wt.ok(["mv", old_dir, new_dir])` (only if the name actually differs).
5. `touch()`; write `m.yml`; `add -- <dir>`; commit `task({id}): edit`.

### 6.6 `set` (and `start`, `done`, `prio`)

Collect changes; at least one flag required (clap `required = true` on the arg group). Apply in memory:
- `--status`: validate ∈ list.
- `--priority`: 0..=9 (clap `value_parser!(u8).range(0..=9)`).
- `--title`: non-empty after trim; rewrites H1; slug recomputed.
- `--assignee` / `--no-assignee` (conflicts).
- `--tag`/`--untag`, `--link`/`--unlink`, `--relate`/`--unrelate`: set semantics, order preserved, no error when removing something absent.
Then:
1. If priority or slug changed → `wt.ok(["mv", old_dir, new_dir])`; update `task.dir`.
2. `touch()`; write `m.yml`; write `t.md` if title changed.
3. `add -- <new_dir>`; commit message: `task({id}): set ` + space-joined `key=old->new` pairs, e.g. `task(14): set status=todo->doing priority=5->2 title tags=+bug,-old`. (For title: just `title`; for list fields: `+x` / `-x` entries.)
4. Print one line per change: `14: status todo -> doing`.

### 6.7 `rm`

If stdin is a tty and not `-f`: prompt `remove task 14 "Fix login"? [y/N] `; anything but `y`/`Y` → `aborted`, exit 1. Non-tty without `-f` → error `refusing to remove without -f`. Then `wt.ok(["rm","-rq","--", dir])`; commit `task({id}): remove "{title}"`; print `removed {id}`.

### 6.8 `attach`

For each file: must exist and be a regular file; `name = --name` (only allowed with a single file) or the file's basename. Reject if `<dir>/f/<name>` exists unless `--force`. Size > 5 MiB → `warning: <name> is N MiB; git is not great at large binaries`. Copy into `<dir>/f/` (create dir). Push `Attachment{path: "f/<name>", name, added: now, by: git config user.name (fallback "unknown")}`; replace existing entry on `--force`. `touch()`; write meta; `add -- <dir>`; commit `task({id}): attach <name>[, <name2>…]`. Print `attached <name> -> <dir>/f/<name>`.

### 6.9 `detach`

Find entry by name → error `no attachment "<name>" on task <id>`. `wt.ok(["rm","-q","--","<dir>/f/<name>"])` (if the file is missing on disk but listed, just drop the entry). Remove from meta; `touch()`; commit `task({id}): detach <name>`.

### 6.10 `comment`

Text = `-m`, or `-e` opens editor on a temp file (`$TMPDIR/yman-comment-<id>.md`, pre-filled with nothing), or, if neither and stdin is not a tty, read stdin. Empty after trim → error `empty comment`. Author = `git config user.name` (fallback `unknown`). `discussion::append_entry(<dir>/d.md, now, author, text)`. `touch()`; write meta; `add -- <dir>`; commit `task({id}): comment`. Print `commented on {id}`.

### 6.11 `path`

Print absolute path of the task folder. Exit 1 if not found. Nothing else on stdout (so `cd $(yman path 14)` works). Lazy refresh still runs but stays silent (quiet).

### 6.12 `log`

Default `-n 20`. Without id: `wt.out(["log","--oneline","-n",N,"refs/yman/local"])`. With id: collect every historical folder name of the task:
1. current dir name;
2. `wt.out(["log","--format=","--name-status","--diff-filter=R","-M","refs/yman/local","--", "."])`, parse lines `R<score>\t<old>\t<new>`; take first path segments; build rename graph old→new; walk backwards from the current name collecting all names.
Then `git log --oneline -n N refs/yman/local -- <name1> <name2> …`. Print verbatim.

### 6.13 `status`

No lazy refresh (it reports instead). Output:
```
.yman  refs/yman/local @ 3f2a1c9   (refresh: lazy, hooks: installed)
remote: refs/tasks/main @ 9b8c7d6   ahead 2, behind 1   → run: yman sync
worktree: clean | 2 uncommitted change(s) (will be snapshotted by next sync)
merge:   in progress, 1 unmerged file(s): 2.14.fix-login/m.yml   → yman sync --continue | --abort
tasks:   todo 4, doing 1, done 7   (1 broken)
```
Lines `merge:` only when applicable; `remote:` says `not fetched yet` if `REMOTE` missing. Ahead/behind from `rev-list --left-right --count refs/yman/local...refs/yman/remote` (prints `ahead\tbehind`). Exit 0 always (informational), except preflight failures.

### 6.14 `refresh`

`refresh(ctx, quiet)`; print `up to date` when applied 0 and no skip; print the skip reason when skipped (stderr, unless quiet). Exit 0.

### 6.15 `hooks`

See §4.8. No preflight beyond `discover()`.

### 6.16 `git`

`discover()` only. `Command::new("git").arg("-C").arg(ydir).args(rest)` with inherited stdio; propagate exit code. Prints nothing itself.

### 6.17 `sync`

Flags mutually exclusive (`--continue`, `--abort`). No lazy refresh (sync supersedes). Preflight(mutating=false) — sync itself handles MERGE_HEAD.

**`--abort`**: require `merge_in_progress()` else error `no merge in progress`. `wt.ok(["merge","--abort"])`. Print `merge aborted`. Exit 0.

**`--continue`**:
1. require `merge_in_progress()` else error `no merge in progress`.
2. `wt.out(["diff","--name-only","--diff-filter=U"])` non-empty → `MergePending("still unmerged: <files>")`.
3. For each top-level dir changed between `HEAD` and `MERGE_HEAD` (`git diff --name-only HEAD MERGE_HEAD` → first segments): `task::load` must succeed and no file in it may contain a line starting with `<<<<<<< `, `=======`, or `>>>>>>> ` → else `MergePending("conflict markers or invalid task in <dir>: <why>")`.
4. `wt.ok(["add","-A"])`; `wt.ok(["commit","-q","--no-verify","--no-edit"])`.
5. → Push (§6.19). Print summary.

**normal**:
1. `merge_in_progress()` → `MergePending("merge in progress; resolve then: yman sync --continue  (or --abort)")`.
2. `wt.is_dirty()` → `add -A`; commit `yman: snapshot local changes`; print `snapshotted N local change(s)`.
3. `had_remote = rev_parse(REMOTE).is_some()`.
4. `main.run(["fetch","origin","+refs/tasks/main:refs/yman/remote"])`:
   - exit 0 → continue.
   - stderr contains `couldn't find remote ref` → remote has no tasks ref: if `had_remote` → `main.ok(["update-ref","-d","refs/yman/remote"])`, print `warning: refs/tasks/main disappeared from origin; will recreate it`; → step 9.
   - else → print stderr, error `fetch failed`.
5. `remote = rev_parse(REMOTE)`; None → step 9.
6. `base = merge_base(LOCAL, REMOTE)`; None → error `task history unrelated to origin refs/tasks/main; re-init from remote:  rm -rf .yman && git update-ref -d refs/yman/local && yman init`. `base == remote` → nothing to merge → step 9. `base == local` (pure fast-forward) → `wt.ok(["merge","-q","--ff-only","refs/yman/remote"])` → step 9.
7. **Collision renumber**:
   - `local_added = wt.out(["diff","--name-only","--diff-filter=A", base, "refs/yman/local"])` → paths with `/` → first segment → `FolderName::parse` → ids (set).
   - `remote_dirs = wt.out(["ls-tree","-d","--name-only","refs/yman/remote"])` → parse → map id → folder name.
   - `colliding = local_added ∩ remote_ids`, sorted with the id comparator.
   - If empty → step 8.
   - `taken = fs_ids ∪ remote_ids ∪ ever_assigned([LOCAL, REMOTE])`.
   - For each `old` in `colliding`: `t = find(old)`; `new = next_free(scheme, cfg, prefix_of(old), &taken)`; insert `new` into taken; `new_dir = FolderName{priority: t.priority, id: new, slug: t.slug}`; `wt.ok(["mv", old_dir, new_dir])`; `t.touch()`; write meta; `add -- new_dir`; print `renumbered {old} -> {new}  (id taken on origin)`.
   - `prefix_of(old)` for author scheme = part before the last `-`; for others `None`.
   - `wt.commit("yman: renumber 2->3, 7->8 (sync collision)")` (comma-separated pairs).
   - Do NOT rewrite `related` references elsewhere (documented limitation; print `note: update references to <old> manually if any`).
8. `wt.run(["merge","--no-edit","--no-verify","-m","yman: merge origin refs/tasks/main","refs/yman/remote"])`:
   - exit 0 → continue.
   - exit ≠ 0 and `merge_in_progress()` → print unmerged file list (`diff --name-only --diff-filter=U`) and `MergePending("conflicts in N file(s); edit them, remove markers, then: yman sync --continue  (or: yman sync --abort)")`.
   - exit ≠ 0 otherwise → error with stderr.
9. **Push** unless `--no-push` (§6.19). On `rejected` result: attempt counter; if < 3 → go to step 3; else error `origin keeps moving; retry yman sync`.
10. Summary:
```
synced  pulled 3, pushed 2, renumbered 1   refs/yman/local @ 3f2a1c9
```
`pulled` = `rev-list --count base..remote_before_merge`; `pushed` = `rev-list --count remote_before..local_after` (0 when `--no-push`).

### 6.19 Push helper

```rust
pub enum PushResult { Ok, Rejected, UpToDate }
fn push(ctx) -> Result<PushResult>
```
`main.run(["push","--no-verify","origin","refs/yman/local:refs/tasks/main"])`. Exit 0 → `Ok` (or `UpToDate` when stderr contains `Everything up-to-date`). Non-zero with stderr matching `rejected` or `non-fast-forward` or `fetch first` → `Rejected`. Otherwise print stderr and error `push failed`. After a successful push: `main.ok(["update-ref","refs/yman/remote","refs/yman/local"])` so `REMOTE` reflects what origin now has.

## 7. Error message catalogue (exact strings)

| Situation | Message |
|---|---|
| not in repo | `not inside a git repository` |
| no origin | `main repo has no "origin" remote; pass --remote <url>` |
| not initialized | `.yman is not initialized; run: yman init` |
| wrong HEAD | `.yman worktree is not on refs/yman/local; run: yman init` |
| task missing | `task <id> not found` |
| bad status | `unknown status "<s>"; allowed: <list joined by ", ">` |
| merge pending (exit 3) | `sync merge in progress; resolve conflicts then run: yman sync --continue  (or: yman sync --abort)` |
| identity | `git identity missing; run: git config --global user.name "…" && git config --global user.email "…"` |
| git missing | `git not found in PATH` |
| git failure (generic) | `git <subcommand> failed: <trimmed stderr>` |

All errors: stderr, prefixed `error: `. Warnings: `warning: `. Notes: `note: `.

## 8. Implementation order

Each phase ends with `cargo build`, `cargo clippy -- -D warnings`, and the listed tests green.

**Phase 0 — spike (30 min, throwaway shell script, keep it in `scripts/spike-symref.sh`)**
In a temp dir: `git init main`, add a commit, `git update-ref refs/yman/local $(git commit-tree $(git mktree </dev/null) -m init)`, `git worktree add --detach .yman refs/yman/local`, `git -C .yman symbolic-ref HEAD refs/yman/local`, create a file, `git -C .yman add -A && git -C .yman commit -m x`, then assert `git rev-parse refs/yman/local` == `git -C .yman rev-parse HEAD`. Also assert `git branch -a` prints nothing new, and `git -C .yman merge --ff-only` works against a second commit made via `commit-tree`. Record the result in a comment at the top of `git.rs`. Choose §4.3.1 primary or fallback path accordingly.

**Phase 1 — foundation**: `Cargo.toml`, `errors.rs`, `git.rs`, `repo.rs` (discover, config helpers, exclude, ydir_state, preflight), `config.rs`, `cli.rs` skeleton with all subcommands stubbed (`todo!()` bodies replaced by `bail!("not implemented")`). Unit tests: config validation.

**Phase 2 — data layer**: `task.rs` (FolderName, slugify, extract_title, render_md, list/find/load/write), `discussion.rs`, `ids.rs` (pure parts: next_free). Unit tests: folder name parse/format round-trip incl. Unicode slug; slugify cases (`"Fix login!"→"fix-login"`, Cyrillic preserved, 300-byte title truncated at boundary ≤ max, empty → `task`); extract_title (leading blank lines ok, missing H1 error, body with `---` and `#` inside preserved); discussion append+parse round-trip; next_free for each scheme.

**Phase 3 — init + add + ls + show + path**: `commands/init.rs` full; `add`, `ls`, `show`, `path`. Test fixture (§9.1). Integration tests: `init_creates_worktree_and_config`, `init_idempotent`, `add_ls_show`, `init_existing_remote` (uses a second clone).

**Phase 4 — mutations**: `edit`, `set`/`start`/`done`/`prio` (with folder rename), `rm`, `attach`/`detach`, `comment`, `log`. Tests: `set_priority_renames_folder`, `set_title_renames_folder`, `attach_detach`, `comment_appends`, `log_follows_renames`.

**Phase 5 — sync**: push helper, `sync` normal path, `--continue`, `--abort`, collision renumber. Tests: `round_trip`, `seq_collision_renumber`, `author_collision_renumber`, `conflict_and_continue`, `conflict_abort`, `concurrent_comments_union`, `remote_deleted_ref`, `push_rejected_retry` (simulate by pushing from B between A's fetch and push — do it by running A with `--no-push`, then B sync, then A sync).

**Phase 6 — refresh + hooks + status**: `refresh.rs`, lazy call in `main.rs`, `commands/refresh.rs`, `hooks.rs`, `status`. Tests: `fresh_on_fetch`, `refresh_manual`, `refresh_skips_dirty`, `refresh_skips_diverged`, `hooks_install_remove_status`, `hooks_refresh_on_pull`, `survives_clean`, `status_reports_ahead_behind`.

**Phase 7 — polish**: `--json` for ls, tty-aware headers, README.md with the storage model, the `git clean -fdx` warning, Windows path note, and the hooks/refresh matrix from this spec. Final full `cargo test`.

## 9. Tests

### 9.1 Fixture (`tests/common/mod.rs`)

```rust
pub struct Fx { pub tmp: TempDir, pub remote: PathBuf, pub a: PathBuf, pub b: PathBuf, home: PathBuf }
impl Fx {
    pub fn new() -> Fx;          // creates remote.git (bare), clones a/ and b/ each with one commit pushed to main
    pub fn yman(&self, dir: &Path) -> assert_cmd::Command;   // cargo_bin("yman"), current_dir(dir), env below
    pub fn git(&self, dir: &Path, args: &[&str]) -> String;   // runs git, panics on failure, returns trimmed stdout
    pub fn task_dir(&self, clone: &Path, id: &str) -> PathBuf; // finds ".yman/*.{id}.*"
    pub fn write(&self, path: &Path, content: &str);
}
```
Env applied to every yman and git call: `HOME=<tmp>/home`, `GIT_CONFIG_GLOBAL=<tmp>/home/.gitconfig` (contains `user.name=Test A|B` per clone via `GIT_AUTHOR_NAME`/`GIT_COMMITTER_NAME` and email, `init.defaultBranch=main`), `GIT_CONFIG_NOSYSTEM=1`, `EDITOR=true`, `VISUAL=` (empty), `YMAN_AUTHOR` unset, `TERM=dumb`. Remote created with `git init --bare -b main remote.git`. Clones via `git clone <remote> a` after an initial commit is pushed from a temp clone.

### 9.2 Scenarios (all in `tests/cli.rs`)

Names match Phase lists in §8. Key assertions worth spelling out:
- `round_trip`: after A sync and B init, `git --git-dir remote.git for-each-ref` lists exactly `refs/heads/main` and `refs/tasks/main`; `git -C a branch -a` output contains no `yman`; `git -C a show-ref` contains `refs/yman/local` and `refs/yman/remote`; `git -C a status --porcelain` is empty; `a/.git/info/exclude` contains `.yman/`; `a/.yman/.git` is a file.
- `seq_collision_renumber`: B's `sync` stdout contains `renumbered 2 -> 3`; `task_dir(b,"3")` exists with B's title; `task_dir(b,"2")` has A's title; after A sync, A has ids 1,2,3.
- `conflict_and_continue`: B `sync` exits 3, stderr lists `<dir>/m.yml`; test writes a valid `m.yml`; `sync --continue` exits 0; A sync then shows the chosen status.
- `concurrent_comments_union`: both `d.md` files contain both authors' entries; no `<<<<<<<` anywhere.
- `survives_clean`: after `fs::remove_dir_all(a/.yman)`, `yman init` exit 0, `yman ls` shows the unsynced task, `yman status` stdout contains `ahead 1`.
- `fresh_on_fetch`: A `git fetch` then `yman ls` shows B's task; `rev-parse refs/yman/local` == `rev-parse refs/yman/remote`.
- `hooks_refresh_on_pull`: A `init --hooks`; B commits a file on `main` and pushes; B `yman add` + `sync`; A `git pull` (post-merge fires) → `task_dir(a, id)` exists without any further yman call.
- `set_priority_renames_folder`: `yman prio 1 0` → folder starts with `0.1.`; `git -C a/.yman log --oneline -1` mentions `set priority=5->0`; `yman log 1` still lists the `add` commit (rename-following).

## 10. Verification (manual, after all tests pass)

In any real repo with an origin you can push to:
1. `yman init --id-scheme seq` → `git status` clean, `git branch -a` unchanged, `cat .git/info/exclude`, `git config --get-all remote.origin.fetch` shows the tasks refspec.
2. `yman add "Первая задача" -p 1 -t demo`; `ls .yman` shows `1.1.первая-задача`; `yman ls`; `yman start 1`; `yman attach 1 ./some.png`; `yman comment 1 -m "note"`; `yman show 1`; `yman prio 1 3` → folder renamed; `yman done 1`; `yman ls -a`; `yman log 1` shows all commits despite renames.
3. `yman sync`; `git ls-remote origin 'refs/tasks/*'` shows one ref; `git ls-remote --heads origin` unchanged; hosting UI shows no new branch and no CI run.
4. Second clone: `yman init` pulls tasks. Add offline on both, sync both → renumber printed on the second. On the first clone: plain `git fetch`, then `yman ls` shows the new task without sync.
5. Edit `m.yml` status differently on both, sync → exit 3, fix, `yman sync --continue`.
6. `rm -rf .yman && yman init` → history intact, `yman status` shows ahead if unpushed.
7. `yman git -- log --oneline --graph`, `cd $(yman path 1)`.

## 11. Out of scope (v1) — record in README as future work

- Field-wise merge driver for `m.yml` (auto-resolve status/tag conflicts).
- Rewriting `related` ids after renumber.
- git-lfs for attachments.
- Reading refs without spawning git (micro-optimization of lazy refresh).
- Windows support beyond "should work with `core.longpaths`"; not tested.
- Multiple `.yman` worktrees per clone.
- Colors, TUI, web UI, GitHub Issues bridge.
