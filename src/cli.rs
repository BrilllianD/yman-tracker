//! clap derive types. No logic lives here.

use crate::config::Scheme;
use clap::{ArgAction, ArgGroup, Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "yman",
    version,
    about = "Task tracker that lives next to the code and syncs through the project's own git remote",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Set up .yman in this repository
    Init(InitArgs),
    /// Create a task
    Add(AddArgs),
    /// List tasks
    Ls(LsArgs),
    /// Show one task in full
    Show(IdArgs),
    /// Open a task's t.md in $EDITOR
    Edit(IdArgs),
    /// Change fields of a task
    Set(SetArgs),
    /// Move a task to the start status
    Start(VerbArgs),
    /// Move a task to the done status
    Done(VerbArgs),
    /// Move a task to any status
    Move(MoveArgs),
    /// Move a task to the cancel status
    Cancel(VerbArgs),
    /// Move a closed task back to the default status
    Reopen(VerbArgs),
    /// Change a task's priority
    Prio(PrioArgs),
    /// Delete a task
    Rm(RmArgs),
    /// Attach files to a task
    Attach(AttachArgs),
    /// Remove an attachment from a task
    Detach(DetachArgs),
    /// Append a comment to a task's discussion
    Comment(CommentArgs),
    /// Print the absolute path of a task folder
    Path(IdArgs),
    /// Show the task history log
    Log(LogArgs),
    /// Report the state of .yman and its remote
    Status,
    /// Fast-forward .yman onto the already-fetched remote state
    Refresh(RefreshArgs),
    /// Manage the git hooks that refresh .yman automatically
    Hooks(HooksArgs),
    /// Fetch, merge and push the task history
    Sync(SyncArgs),
    /// Run a raw git command inside .yman
    Git(GitArgs),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum SchemeArg {
    Random,
    Seq,
    Author,
}

impl From<SchemeArg> for Scheme {
    fn from(s: SchemeArg) -> Scheme {
        match s {
            SchemeArg::Random => Scheme::Random,
            SchemeArg::Seq => Scheme::Seq,
            SchemeArg::Author => Scheme::Author,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum RefreshPolicy {
    Lazy,
    Manual,
}

impl RefreshPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            RefreshPolicy::Lazy => "lazy",
            RefreshPolicy::Manual => "manual",
        }
    }
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// How task ids are generated (stored in config.toml)
    #[arg(long = "id-scheme", value_name = "SCHEME")]
    pub id_scheme: Option<SchemeArg>,
    /// Prefix for the "author" id scheme
    #[arg(long, value_name = "PFX")]
    pub author: Option<String>,
    /// Remote URL to use when the repo has no "origin"
    #[arg(long, value_name = "URL")]
    pub remote: Option<String>,
    /// Do not talk to the remote
    #[arg(long)]
    pub offline: bool,
    /// Also install the post-merge / post-checkout refresh hooks
    #[arg(long)]
    pub hooks: bool,
    /// When to pick up already-fetched remote task commits
    #[arg(long, value_name = "WHEN")]
    pub refresh: Option<RefreshPolicy>,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Task title
    pub title: String,
    /// Priority, 0 (highest) to 9
    #[arg(short = 'p', long, value_name = "N", value_parser = clap::value_parser!(u8).range(0..=9))]
    pub priority: Option<u8>,
    /// Initial status
    #[arg(short = 's', long, value_name = "STATUS")]
    pub status: Option<String>,
    /// Tag (repeatable)
    #[arg(short = 't', long = "tag", value_name = "TAG", action = ArgAction::Append)]
    pub tags: Vec<String>,
    /// Body text
    #[arg(short = 'm', long, value_name = "TEXT")]
    pub message: Option<String>,
    /// Open the new t.md in $EDITOR
    #[arg(short = 'e', long)]
    pub edit: bool,
    /// Assignee
    #[arg(short = 'a', long, value_name = "WHO")]
    pub assignee: Option<String>,
    /// Link (repeatable)
    #[arg(long = "link", value_name = "URL", action = ArgAction::Append)]
    pub links: Vec<String>,
    /// Related task id (repeatable)
    #[arg(long = "relate", value_name = "ID", action = ArgAction::Append)]
    pub related: Vec<String>,
}

#[derive(Args, Debug)]
pub struct LsArgs {
    /// Only these statuses (repeatable)
    #[arg(short = 's', long = "status", value_name = "STATUS", action = ArgAction::Append)]
    pub statuses: Vec<String>,
    /// Only tasks carrying all of these tags (repeatable)
    #[arg(short = 't', long = "tag", value_name = "TAG", action = ArgAction::Append)]
    pub tags: Vec<String>,
    /// Include tasks in the final status
    #[arg(short = 'a', long)]
    pub all: bool,
    /// Machine-readable output
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct IdArgs {
    /// Task id
    pub id: String,
}

/// `start`, `done`, `cancel`, `reopen`.
#[derive(Args, Debug)]
pub struct VerbArgs {
    /// Task id
    pub id: String,
    /// Append a comment in the same commit
    #[arg(short = 'm', long, value_name = "TEXT")]
    pub message: Option<String>,
}

#[derive(Args, Debug)]
pub struct MoveArgs {
    /// Task id
    pub id: String,
    /// Status to move it to
    pub status: String,
    /// Append a comment in the same commit
    #[arg(short = 'm', long, value_name = "TEXT")]
    pub message: Option<String>,
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("changes")
        .required(true)
        .multiple(true)
        .args([
            "status", "priority", "title", "assignee", "no_assignee",
            "tag", "untag", "link", "unlink", "relate", "unrelate", "message",
        ])
))]
pub struct SetArgs {
    /// Task id
    pub id: String,
    #[arg(long, value_name = "STATUS")]
    pub status: Option<String>,
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(0..=9))]
    pub priority: Option<u8>,
    #[arg(long, value_name = "TITLE")]
    pub title: Option<String>,
    #[arg(long, value_name = "WHO", conflicts_with = "no_assignee")]
    pub assignee: Option<String>,
    /// Clear the assignee
    #[arg(long = "no-assignee")]
    pub no_assignee: bool,
    #[arg(long = "tag", value_name = "TAG", action = ArgAction::Append)]
    pub tag: Vec<String>,
    #[arg(long = "untag", value_name = "TAG", action = ArgAction::Append)]
    pub untag: Vec<String>,
    #[arg(long = "link", value_name = "URL", action = ArgAction::Append)]
    pub link: Vec<String>,
    #[arg(long = "unlink", value_name = "URL", action = ArgAction::Append)]
    pub unlink: Vec<String>,
    #[arg(long = "relate", value_name = "ID", action = ArgAction::Append)]
    pub relate: Vec<String>,
    #[arg(long = "unrelate", value_name = "ID", action = ArgAction::Append)]
    pub unrelate: Vec<String>,
    /// Append a comment in the same commit
    #[arg(short = 'm', long, value_name = "TEXT")]
    pub message: Option<String>,
}

#[derive(Args, Debug)]
pub struct PrioArgs {
    /// Task id
    pub id: String,
    /// New priority, 0 (highest) to 9
    #[arg(value_parser = clap::value_parser!(u8).range(0..=9))]
    pub priority: u8,
    /// Append a comment in the same commit
    #[arg(short = 'm', long, value_name = "TEXT")]
    pub message: Option<String>,
}

#[derive(Args, Debug)]
pub struct RmArgs {
    pub id: String,
    /// Do not ask for confirmation
    #[arg(short = 'f', long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct AttachArgs {
    pub id: String,
    /// Files to attach
    #[arg(required = true)]
    pub files: Vec<std::path::PathBuf>,
    /// Store the file under this name (single file only)
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// Overwrite an attachment with the same name
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct DetachArgs {
    pub id: String,
    /// Attachment name
    pub name: String,
}

#[derive(Args, Debug)]
pub struct CommentArgs {
    pub id: String,
    /// Comment text
    #[arg(short = 'm', long, value_name = "TEXT")]
    pub message: Option<String>,
    /// Write the comment in $EDITOR
    #[arg(short = 'e', long)]
    pub edit: bool,
}

#[derive(Args, Debug)]
pub struct LogArgs {
    /// Limit the log to one task, following its renames
    pub id: Option<String>,
    /// How many commits to show
    #[arg(short = 'n', long, value_name = "N", default_value_t = 20)]
    pub number: usize,
}

#[derive(Args, Debug)]
pub struct RefreshArgs {
    /// Say nothing on stderr
    #[arg(long)]
    pub quiet: bool,
}

#[derive(Args, Debug)]
pub struct HooksArgs {
    #[command(subcommand)]
    pub action: HooksAction,
}

#[derive(Subcommand, Debug)]
pub enum HooksAction {
    /// Write the refresh hooks
    Install,
    /// Delete hooks that yman installed
    Remove,
    /// Report whether the hooks are in place
    Status,
}

#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Finish a sync whose merge had conflicts
    #[arg(long = "continue", conflicts_with = "abort")]
    pub cont: bool,
    /// Throw away a conflicted merge
    #[arg(long)]
    pub abort: bool,
    /// Merge the remote state but do not push
    #[arg(long = "no-push")]
    pub no_push: bool,
}

#[derive(Args, Debug)]
pub struct GitArgs {
    /// Arguments passed straight to git, run inside .yman
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub args: Vec<String>,
}

impl Cmd {
    /// Commands that mutate the task history (preflight refuses while a sync
    /// merge is unresolved).
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            Cmd::Add(_)
                | Cmd::Edit(_)
                | Cmd::Set(_)
                | Cmd::Start(_)
                | Cmd::Done(_)
                | Cmd::Move(_)
                | Cmd::Cancel(_)
                | Cmd::Reopen(_)
                | Cmd::Prio(_)
                | Cmd::Rm(_)
                | Cmd::Attach(_)
                | Cmd::Detach(_)
                | Cmd::Comment(_)
        )
    }

    /// Commands that run their own refresh logic, or must not trigger one.
    pub fn skips_lazy_refresh(&self) -> bool {
        matches!(
            self,
            Cmd::Init(_)
                | Cmd::Sync(_)
                | Cmd::Refresh(_)
                | Cmd::Status
                | Cmd::Hooks(_)
                | Cmd::Git(_)
        )
    }

    /// Commands that do not need `.yman` to exist yet.
    pub fn skips_preflight(&self) -> bool {
        matches!(self, Cmd::Init(_) | Cmd::Hooks(_) | Cmd::Git(_))
    }
}
