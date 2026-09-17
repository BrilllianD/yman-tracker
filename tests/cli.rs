mod common;

use common::{Fx, stderr, stdout};

#[test]
fn init_creates_worktree_and_config() {
    let fx = Fx::new();
    let out = fx.yman(&fx.a).arg("init").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("initialized .yman"), "{text}");
    assert!(text.contains("scheme:   seq"), "{text}");
    assert!(text.contains("tasks:    0"), "{text}");

    // The worktree is a linked one, not a nested repo.
    assert!(fx.a.join(".yman/.git").is_file());
    assert!(fx.a.join(".yman/config.toml").is_file());
    assert!(fx.a.join(".yman/.gitattributes").is_file());

    // HEAD points at the ref, and the ref is not a branch.
    assert_eq!(
        fx.git(&fx.a.join(".yman"), &["symbolic-ref", "HEAD"]),
        "refs/yman/local"
    );
    let branches = fx.git(&fx.a, &["branch", "-a"]);
    assert!(!branches.contains("yman"), "{branches}");
    let refs = fx.git(&fx.a, &["show-ref"]);
    assert!(refs.contains("refs/yman/local"), "{refs}");

    // The main worktree is untouched and does not see .yman.
    assert_eq!(fx.git(&fx.a, &["status", "--porcelain"]), "");
    let exclude = fx.read(&fx.a.join(".git/info/exclude"));
    assert!(exclude.lines().any(|l| l.trim() == ".yman/"), "{exclude}");

    // The fetch refspec is in place; remote.origin.push is not.
    let fetch = fx.git(&fx.a, &["config", "--get-all", "remote.origin.fetch"]);
    assert!(
        fetch.lines().any(|l| l == "+refs/tasks/main:refs/yman/remote"),
        "{fetch}"
    );
    let (ok, _, _) = fx.git_try(&fx.a, &["config", "--get", "remote.origin.push"]);
    assert!(!ok, "remote.origin.push must stay unset");

    // A fresh init publishes the tasks ref and no branch.
    let remote_refs = fx.git(&fx.remote, &["for-each-ref", "--format=%(refname)"]);
    let mut names: Vec<&str> = remote_refs.lines().collect();
    names.sort();
    assert_eq!(names, ["refs/heads/main", "refs/tasks/main"]);
}

#[test]
fn init_idempotent() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    let head1 = fx.git(&fx.a, &["rev-parse", "refs/yman/local"]);

    let out = fx.yman(&fx.a).arg("init").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("already initialized .yman"));
    assert_eq!(fx.git(&fx.a, &["rev-parse", "refs/yman/local"]), head1);

    // The refspec is added once, not once per run.
    let fetch = fx.git(&fx.a, &["config", "--get-all", "remote.origin.fetch"]);
    assert_eq!(
        fetch
            .lines()
            .filter(|l| *l == "+refs/tasks/main:refs/yman/remote")
            .count(),
        1,
        "{fetch}"
    );
    let exclude = fx.read(&fx.a.join(".git/info/exclude"));
    assert_eq!(exclude.lines().filter(|l| l.trim() == ".yman/").count(), 1);

    // A different --id-scheme is refused, loudly but harmlessly.
    let out = fx
        .yman(&fx.a)
        .args(["init", "--id-scheme", "random"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(stderr(&out).contains("--id-scheme ignored"), "{}", stderr(&out));
}

#[test]
fn init_requires_a_repo() {
    let fx = Fx::new();
    let plain = fx.tmp.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let out = fx.yman(&plain).arg("init").output().unwrap();
    assert!(!out.status.success());
    assert_eq!(stderr(&out).trim(), "error: not inside a git repository");
}

#[test]
fn add_ls_show() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();

    let out = fx
        .yman(&fx.a)
        .args(["add", "Fix login", "-p", "2", "-t", "auth", "-t", "bug", "-m", "body text"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).starts_with("added 1  2.1.fix-login"), "{}", stdout(&out));
    assert!(fx.a.join(".yman/2.1.fix-login/t.md").is_file());
    assert_eq!(
        fx.read(&fx.a.join(".yman/2.1.fix-login/t.md")),
        "# Fix login\n\nbody text\n"
    );

    // Second task takes the next sequential id and the default priority.
    fx.yman(&fx.a).args(["add", "Write docs"]).assert().success();
    assert!(fx.a.join(".yman/5.2.write-docs").is_dir());

    let out = fx.yman(&fx.a).arg("ls").output().unwrap();
    let text = stdout(&out);
    assert!(text.contains("2  1  todo  Fix login   auth,bug"), "{text}");
    assert!(text.contains("5  2  todo  Write docs"), "{text}");
    // No header when stdout is not a tty.
    assert!(!text.contains("STATUS"), "{text}");

    let out = fx.yman(&fx.a).args(["ls", "-t", "auth"]).output().unwrap();
    let text = stdout(&out);
    assert!(text.contains("Fix login"), "{text}");
    assert!(!text.contains("Write docs"), "{text}");

    let out = fx.yman(&fx.a).args(["show", "1"]).output().unwrap();
    let text = stdout(&out);
    assert!(text.starts_with("1  Fix login\n"), "{text}");
    assert!(text.contains("priority: 2   status: todo   assignee: -   tags: auth, bug"), "{text}");
    assert!(text.contains("folder:   .yman/2.1.fix-login"), "{text}");
    assert!(text.contains("\nbody text\n"), "{text}");
    assert!(!text.contains("attachments:"), "{text}");
    assert!(!text.contains("discussion:"), "{text}");

    // path prints exactly one absolute line.
    let out = fx.yman(&fx.a).args(["path", "1"]).output().unwrap();
    assert_eq!(
        stdout(&out).trim_end(),
        fx.a.join(".yman/2.1.fix-login").to_string_lossy()
    );

    // Unknown ids and statuses fail with the documented wording.
    let out = fx.yman(&fx.a).args(["show", "99"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(stderr(&out).trim(), "error: task 99 not found");

    let out = fx.yman(&fx.a).args(["add", "X", "-s", "nope"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: unknown status \"nope\"; allowed: todo, doing, done"
    );

    // Every task commit lands on refs/yman/local and nowhere else.
    let log = fx.git(&fx.a.join(".yman"), &["log", "--oneline"]);
    assert!(log.contains("task(1): add \"Fix login\""), "{log}");
    assert_eq!(fx.git(&fx.a, &["status", "--porcelain"]), "");
    let branches = fx.git(&fx.a, &["branch", "-a"]);
    assert!(!branches.contains("yman"), "{branches}");
}

#[test]
fn ls_json_and_unicode_slug() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a)
        .args(["add", "Первая задача", "-p", "1", "-t", "demo"])
        .assert()
        .success();
    assert!(fx.a.join(".yman/1.1.первая-задача").is_dir());

    let out = fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap();
    let text = stdout(&out);
    assert!(text.starts_with('['), "{text}");
    assert!(text.contains("\"id\":\"1\""), "{text}");
    assert!(text.contains("\"priority\":1"), "{text}");
    assert!(text.contains("\"title\":\"Первая задача\""), "{text}");
    assert!(text.contains("\"tags\":[\"demo\"]"), "{text}");
    assert!(text.contains("\"assignee\":null"), "{text}");
    assert!(text.contains("\"attachments\":0"), "{text}");
    assert!(text.contains("\"dir\":\"1.1.первая-задача\""), "{text}");
}

#[test]
fn init_existing_remote() {
    let fx = Fx::new();
    fx.yman(&fx.a)
        .args(["init", "--id-scheme", "seq"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();

    let out = fx.yman(&fx.b).arg("init").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("tasks:    1"), "{}", stdout(&out));
    assert_eq!(fx.title(&fx.b, "1"), "Fix login");
    assert_eq!(
        fx.git(&fx.b, &["rev-parse", "refs/yman/local"]),
        fx.git(&fx.b, &["rev-parse", "refs/yman/remote"])
    );

    // B's next task continues A's numbering.
    fx.yman(&fx.b).args(["add", "Write docs"]).assert().success();
    assert!(fx.has_task(&fx.b, "2"));
}

#[test]
fn commands_refuse_before_init() {
    let fx = Fx::new();
    let out = fx.yman(&fx.a).arg("ls").output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: .yman is not initialized; run: yman init"
    );
}

// ---------------------------------------------------------------- mutations

#[test]
fn set_priority_renames_folder() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    assert!(fx.a.join(".yman/5.1.fix-login").is_dir());

    let out = fx.yman(&fx.a).args(["prio", "1", "0"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim_end(), "1: priority 5 -> 0");
    assert!(fx.a.join(".yman/0.1.fix-login").is_dir());
    assert!(!fx.a.join(".yman/5.1.fix-login").exists());

    let subject = fx.git(&fx.a.join(".yman"), &["log", "--oneline", "-1"]);
    assert!(subject.contains("set priority=5->0"), "{subject}");

    // The rename is recorded as a rename, and `log <id>` still sees the add.
    let log = stdout(&fx.yman(&fx.a).args(["log", "1"]).output().unwrap());
    assert!(log.contains("task(1): add \"Fix login\""), "{log}");
    assert!(log.contains("set priority=5->0"), "{log}");

    // Working tree of the main repo is still untouched.
    assert_eq!(fx.git(&fx.a, &["status", "--porcelain"]), "");
}

#[test]
fn set_title_renames_folder() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--title", "Fix logout", "--status", "doing", "--tag", "auth"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("1: status todo -> doing"), "{text}");
    assert!(text.contains("1: title Fix login -> Fix logout"), "{text}");
    assert!(text.contains("1: tags +auth"), "{text}");

    assert!(fx.a.join(".yman/5.1.fix-logout").is_dir());
    assert_eq!(fx.title(&fx.a, "1"), "Fix logout");
    assert_eq!(fx.status(&fx.a, "1"), "doing");

    let subject = fx.git(&fx.a.join(".yman"), &["log", "--oneline", "-1"]);
    assert!(subject.contains("status=todo->doing"), "{subject}");
    assert!(subject.contains("title"), "{subject}");
    assert!(subject.contains("tags=+auth"), "{subject}");
}

#[test]
fn start_done_and_ls_hiding() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "One"]).assert().success();
    fx.yman(&fx.a).args(["add", "Two"]).assert().success();

    fx.yman(&fx.a).args(["start", "1"]).assert().success();
    assert_eq!(fx.status(&fx.a, "1"), "doing");
    fx.yman(&fx.a).args(["done", "2"]).assert().success();
    assert_eq!(fx.status(&fx.a, "2"), "done");

    // Done tasks are hidden by default, shown with -a or an explicit -s.
    let text = stdout(&fx.yman(&fx.a).arg("ls").output().unwrap());
    assert!(text.contains("One"), "{text}");
    assert!(!text.contains("Two"), "{text}");
    let text = stdout(&fx.yman(&fx.a).args(["ls", "-a"]).output().unwrap());
    assert!(text.contains("Two"), "{text}");
    let text = stdout(&fx.yman(&fx.a).args(["ls", "-s", "done"]).output().unwrap());
    assert!(text.contains("Two"), "{text}");
    assert!(!text.contains("One"), "{text}");

    // A no-op set says so and commits nothing.
    let before = fx.git(&fx.a.join(".yman"), &["rev-parse", "HEAD"]);
    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--status", "doing"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(stdout(&out).trim_end(), "no changes");
    assert_eq!(fx.git(&fx.a.join(".yman"), &["rev-parse", "HEAD"]), before);

    // `set` with no flags at all is a usage error.
    let out = fx.yman(&fx.a).args(["set", "1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn attach_detach() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let src = fx.tmp.path().join("screenshot.png");
    fx.write(&src, "not really a png");

    let out = fx
        .yman(&fx.a)
        .args(["attach", "1", src.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("attached screenshot.png"), "{}", stdout(&out));
    assert!(fx.a.join(".yman/5.1.fix-login/f/screenshot.png").is_file());

    let shown = stdout(&fx.yman(&fx.a).args(["show", "1"]).output().unwrap());
    assert!(shown.contains("attachments:"), "{shown}");
    assert!(shown.contains("screenshot.png   (added "), "{shown}");
    assert!(shown.contains("by Test A)"), "{shown}");

    // A second attach under the same name needs --force.
    let out = fx
        .yman(&fx.a)
        .args(["attach", "1", src.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("already exists"), "{}", stderr(&out));
    fx.yman(&fx.a)
        .args(["attach", "1", src.to_str().unwrap(), "--force"])
        .assert()
        .success();

    // ls counts attachments in the F column.
    let text = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(text.contains("\"attachments\":1"), "{text}");

    let out = fx
        .yman(&fx.a)
        .args(["detach", "1", "screenshot.png"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!fx.a.join(".yman/5.1.fix-login/f/screenshot.png").exists());
    let shown = stdout(&fx.yman(&fx.a).args(["show", "1"]).output().unwrap());
    assert!(!shown.contains("attachments:"), "{shown}");

    let out = fx.yman(&fx.a).args(["detach", "1", "nope.png"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: no attachment \"nope.png\" on task 1"
    );
}

#[test]
fn comment_appends() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    fx.yman(&fx.a)
        .args(["comment", "1", "-m", "first note"])
        .assert()
        .success();
    fx.yman(&fx.a)
        .args(["comment", "1", "-m", "second note"])
        .assert()
        .success();

    let d = fx.read(&fx.a.join(".yman/5.1.fix-login/d.md"));
    assert_eq!(d.matches("— Test A").count(), 2, "{d}");
    assert!(d.contains("first note"), "{d}");
    assert!(d.contains("second note"), "{d}");
    // Entries are appended, never rewritten.
    assert!(d.find("first note").unwrap() < d.find("second note").unwrap());

    let shown = stdout(&fx.yman(&fx.a).args(["show", "1"]).output().unwrap());
    assert!(shown.contains("discussion:"), "{shown}");
    assert!(shown.contains("    first note"), "{shown}");

    let text = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(text.contains("\"comments\":2"), "{text}");

    let out = fx.yman(&fx.a).args(["comment", "1", "-m", "   "]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(stderr(&out).trim(), "error: empty comment");
}

#[test]
fn rm_requires_force_without_a_tty() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let out = fx.yman(&fx.a).args(["rm", "1"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(stderr(&out).trim(), "error: refusing to remove without -f");

    let out = fx.yman(&fx.a).args(["rm", "1", "-f"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim_end(), "removed 1");
    assert!(!fx.a.join(".yman/5.1.fix-login").exists());
    let subject = fx.git(&fx.a.join(".yman"), &["log", "--oneline", "-1"]);
    assert!(subject.contains("task(1): remove \"Fix login\""), "{subject}");

    // The id is never handed out again.
    fx.yman(&fx.a).args(["add", "Another"]).assert().success();
    assert!(fx.has_task(&fx.a, "2"));
    assert!(!fx.has_task(&fx.a, "1"));
}

#[test]
fn log_follows_renames() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).args(["add", "Other task"]).assert().success();
    fx.yman(&fx.a).args(["prio", "1", "2"]).assert().success();
    fx.yman(&fx.a)
        .args(["set", "1", "--title", "Fix logout"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["comment", "1", "-m", "note"]).assert().success();

    let log = stdout(&fx.yman(&fx.a).args(["log", "1"]).output().unwrap());
    assert!(log.contains("task(1): add \"Fix login\""), "{log}");
    assert!(log.contains("task(1): set priority=5->2"), "{log}");
    assert!(log.contains("task(1): set title"), "{log}");
    assert!(log.contains("task(1): comment"), "{log}");
    assert!(!log.contains("Other task"), "{log}");

    // The unfiltered log shows everything, newest first.
    let log = stdout(&fx.yman(&fx.a).args(["log", "-n", "2"]).output().unwrap());
    assert_eq!(log.lines().count(), 2, "{log}");
}

// --------------------------------------------------------------------- sync

#[test]
fn round_trip() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login", "-p", "2"]).assert().success();

    let out = fx.yman(&fx.a).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("synced  pulled 0, pushed 1"), "{text}");

    // The remote grew exactly one extra ref, and it is not a branch.
    let remote_refs = fx.git(&fx.remote, &["for-each-ref", "--format=%(refname)"]);
    let mut names: Vec<&str> = remote_refs.lines().collect();
    names.sort();
    assert_eq!(names, ["refs/heads/main", "refs/tasks/main"]);

    // B picks the task up through a plain init.
    fx.yman(&fx.b).arg("init").assert().success();
    assert_eq!(fx.title(&fx.b, "1"), "Fix login");

    // Nothing about any of this is visible to ordinary git use in A.
    let branches = fx.git(&fx.a, &["branch", "-a"]);
    assert!(!branches.contains("yman"), "{branches}");
    let refs = fx.git(&fx.a, &["show-ref"]);
    assert!(refs.contains("refs/yman/local"), "{refs}");
    assert!(refs.contains("refs/yman/remote"), "{refs}");
    assert_eq!(fx.git(&fx.a, &["status", "--porcelain"]), "");
    assert!(fx.a.join(".yman/.git").is_file());

    // B's change comes back to A.
    fx.yman(&fx.b).args(["add", "Write docs"]).assert().success();
    fx.yman(&fx.b).arg("sync").assert().success();
    let out = fx.yman(&fx.a).arg("sync").output().unwrap();
    let text = stdout(&out);
    assert!(text.contains("pulled 1"), "{text}");
    assert_eq!(fx.title(&fx.a, "2"), "Write docs");
    assert_eq!(
        fx.git(&fx.a, &["rev-parse", "refs/yman/local"]),
        fx.git(&fx.a, &["rev-parse", "refs/yman/remote"])
    );
}

#[test]
fn seq_collision_renumber() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    // Both add offline; both mint id 2.
    fx.yman(&fx.a).args(["add", "A two"]).assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("renumbered 2 -> 3"), "{text}");
    assert!(text.contains("renumbered 1"), "{text}");

    // B keeps its task under the new id; A's task 2 arrives intact.
    assert_eq!(fx.title(&fx.b, "3"), "B two");
    assert_eq!(fx.title(&fx.b, "2"), "A two");

    // A ends up with all three.
    fx.yman(&fx.a).arg("sync").assert().success();
    assert_eq!(fx.title(&fx.a, "1"), "A one");
    assert_eq!(fx.title(&fx.a, "2"), "A two");
    assert_eq!(fx.title(&fx.a, "3"), "B two");
}

#[test]
fn author_collision_renumber() {
    let fx = Fx::new();
    fx.yman(&fx.a)
        .args(["init", "--id-scheme", "author", "--author", "iv"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    assert!(fx.has_task(&fx.a, "iv-1"));
    fx.yman(&fx.a).arg("sync").assert().success();

    // B adopts the scheme from config.toml but uses its own prefix.
    fx.yman(&fx.b)
        .args(["init", "--author", "iv"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["add", "A two"]).assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    assert!(fx.has_task(&fx.a, "iv-2"));
    assert!(fx.has_task(&fx.b, "iv-2"));

    fx.yman(&fx.a).arg("sync").assert().success();
    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("renumbered iv-2 -> iv-3"), "{text}");
    assert_eq!(fx.title(&fx.b, "iv-3"), "B two");
    assert_eq!(fx.title(&fx.b, "iv-2"), "A two");
}

#[test]
fn conflict_and_continue() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    // Same task, different status on each side.
    fx.yman(&fx.a).args(["set", "1", "--status", "doing"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).args(["set", "1", "--status", "done"]).assert().success();

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("5.1.fix-login/m.yml"), "{err}");
    assert!(err.contains("yman sync --continue"), "{err}");

    // Any mutating command is refused until the merge is settled.
    let out = fx.yman(&fx.b).args(["add", "Nope"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(stderr(&out).contains("sync merge in progress"), "{}", stderr(&out));

    // --continue refuses while markers are still there.
    let out = fx.yman(&fx.b).args(["sync", "--continue"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(stderr(&out).contains("still unmerged"), "{}", stderr(&out));

    // Resolve by hand, the way the docs describe.
    let mpath = fx.b.join(".yman/5.1.fix-login/m.yml");
    let conflicted = fx.read(&mpath);
    assert!(conflicted.contains("<<<<<<<"), "{conflicted}");
    let resolved: String = conflicted
        .lines()
        .filter(|l| {
            !l.starts_with("<<<<<<<") && !l.starts_with("=======") && !l.starts_with(">>>>>>>")
        })
        .filter(|l| !l.starts_with("status: doing"))
        .map(|l| format!("{l}\n"))
        .collect();
    fx.write(&mpath, &resolved);

    let out = fx.yman(&fx.b).args(["sync", "--continue"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fx.status(&fx.b, "1"), "done");

    // A sees the resolution.
    fx.yman(&fx.a).arg("sync").assert().success();
    assert_eq!(fx.status(&fx.a, "1"), "done");
}

#[test]
fn conflict_abort() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    fx.yman(&fx.a).args(["set", "1", "--status", "doing"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).args(["set", "1", "--status", "done"]).assert().success();
    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert_eq!(out.status.code(), Some(3));

    let out = fx.yman(&fx.b).args(["sync", "--abort"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim_end(), "merge aborted");
    assert_eq!(fx.status(&fx.b, "1"), "done");
    assert!(!fx.read(&fx.b.join(".yman/5.1.fix-login/m.yml")).contains("<<<<<<<"));

    // With no merge outstanding, both flags say so plainly.
    let out = fx.yman(&fx.b).args(["sync", "--abort"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(stderr(&out).trim(), "error: no merge in progress");
}

#[test]
fn concurrent_comments_union() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).args(["comment", "1", "-m", "from A first"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    fx.yman(&fx.a).args(["comment", "1", "-m", "only A"]).assert().success();
    fx.yman(&fx.b).args(["comment", "1", "-m", "only B"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    fx.yman(&fx.a).arg("sync").assert().success();

    for clone in [&fx.a, &fx.b] {
        let d = fx.read(&clone.join(".yman/5.1.fix-login/d.md"));
        assert!(d.contains("only A"), "{d}");
        assert!(d.contains("only B"), "{d}");
        assert!(!d.contains("<<<<<<<"), "{d}");
    }
}

#[test]
fn remote_deleted_ref() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();

    // Someone wipes refs/tasks/main on the server.
    fx.git(&fx.remote, &["update-ref", "-d", "refs/tasks/main"]);

    let out = fx.yman(&fx.a).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("refs/tasks/main disappeared from origin; will recreate it"),
        "{}",
        stderr(&out)
    );
    assert_eq!(
        fx.git(&fx.remote, &["rev-parse", "refs/tasks/main"]),
        fx.git(&fx.a, &["rev-parse", "refs/yman/local"])
    );
}

#[test]
fn push_rejected_retry() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    // A merges but holds its push back; B publishes in the meantime.
    fx.yman(&fx.a).args(["add", "A two"]).assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    let out = fx.yman(&fx.a).args(["sync", "--no-push"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("pushed 0"), "{}", stdout(&out));
    fx.yman(&fx.b).arg("sync").assert().success();

    // A's push is now stale; sync refetches, renumbers and lands it.
    let out = fx.yman(&fx.a).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("renumbered 2 -> 3"), "{text}");
    assert_eq!(fx.title(&fx.a, "3"), "A two");
    assert_eq!(fx.title(&fx.a, "2"), "B two");

    fx.yman(&fx.b).arg("sync").assert().success();
    assert_eq!(fx.title(&fx.b, "3"), "A two");
}

#[test]
fn sync_snapshots_hand_edits() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.write(
        &fx.a.join(".yman/5.1.fix-login/t.md"),
        "# Fix login\n\nedited by hand\n",
    );

    let out = fx.yman(&fx.a).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("snapshotted 1 local change(s)"), "{}", stdout(&out));
    assert_eq!(fx.git(&fx.a.join(".yman"), &["status", "--porcelain"]), "");
}

// ------------------------------------------------- refresh, hooks and status

#[test]
fn fresh_on_fetch() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.b).arg("sync").assert().success();

    // A does nothing but a plain git fetch; the task shows up anyway.
    fx.git(&fx.a, &["fetch", "origin"]);
    let text = stdout(&fx.yman(&fx.a).arg("ls").output().unwrap());
    assert!(text.contains("B two"), "{text}");
    assert_eq!(
        fx.git(&fx.a, &["rev-parse", "refs/yman/local"]),
        fx.git(&fx.a, &["rev-parse", "refs/yman/remote"])
    );
}

#[test]
fn refresh_manual() {
    let fx = Fx::new();
    fx.yman(&fx.a).args(["init", "--refresh", "manual"]).assert().success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.b).arg("sync").assert().success();

    fx.git(&fx.a, &["fetch", "origin"]);
    let text = stdout(&fx.yman(&fx.a).arg("ls").output().unwrap());
    assert!(!text.contains("B two"), "manual policy must not refresh: {text}");

    // An explicit refresh still works, and says what it did.
    let out = fx.yman(&fx.a).arg("refresh").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("refreshed: 1 new commit(s)"), "{}", stderr(&out));
    let text = stdout(&fx.yman(&fx.a).arg("ls").output().unwrap());
    assert!(text.contains("B two"), "{text}");

    let out = fx.yman(&fx.a).arg("refresh").output().unwrap();
    assert_eq!(stdout(&out).trim_end(), "up to date");
}

#[test]
fn refresh_skips_dirty() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.b).arg("sync").assert().success();

    fx.git(&fx.a, &["fetch", "origin"]);
    fx.write(&fx.a.join(".yman/5.1.a-one/t.md"), "# A one\n\nmid-edit\n");

    let out = fx.yman(&fx.a).arg("refresh").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains(".yman has uncommitted changes, refresh skipped"),
        "{}",
        stderr(&out)
    );
    assert!(!fx.has_task(&fx.a, "2"));
    // The hand edit survives untouched.
    assert!(fx.read(&fx.a.join(".yman/5.1.a-one/t.md")).contains("mid-edit"));
}

#[test]
fn refresh_skips_diverged() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.b).arg("sync").assert().success();

    // A has its own unpushed commit, so a fast-forward is not available.
    fx.yman(&fx.a).args(["add", "A three"]).assert().success();
    fx.git(&fx.a, &["fetch", "origin"]);

    let out = fx.yman(&fx.a).arg("refresh").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("local has unpushed commits; run: yman sync"),
        "{}",
        stderr(&out)
    );
    // A's own offline task kept id 2; B's task is still only on the remote.
    assert_eq!(fx.title(&fx.a, "2"), "A three");
    let text = stdout(&fx.yman(&fx.a).arg("ls").output().unwrap());
    assert!(!text.contains("B two"), "{text}");

    // A lazy refresh on an ordinary command stays quiet about it.
    let out = fx.yman(&fx.a).arg("ls").output().unwrap();
    assert!(out.status.success());
    assert_eq!(stderr(&out), "");
}

#[test]
fn hooks_install_remove_status() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();

    let text = stdout(&fx.yman(&fx.a).args(["hooks", "status"]).output().unwrap());
    assert!(text.contains("hook post-merge: absent"), "{text}");

    let out = fx.yman(&fx.a).args(["hooks", "install"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let hook = fx.a.join(".git/hooks/post-merge");
    assert!(hook.is_file());
    assert!(fx.read(&hook).contains("yman refresh --quiet"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&hook).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "hook must be executable");
    }

    let text = stdout(&fx.yman(&fx.a).args(["hooks", "status"]).output().unwrap());
    assert!(text.contains("hook post-merge: installed"), "{text}");
    assert!(text.contains("hook post-checkout: installed"), "{text}");

    // Installing twice is a no-op.
    let out = fx.yman(&fx.a).args(["hooks", "install"]).output().unwrap();
    assert!(out.status.success());
    assert!(stdout(&out).contains("already installed"), "{}", stdout(&out));

    fx.yman(&fx.a).args(["hooks", "remove"]).assert().success();
    assert!(!hook.exists());

    // Someone else's hook is reported, never overwritten.
    fx.write(&hook, "#!/bin/sh\necho mine\n");
    let out = fx.yman(&fx.a).args(["hooks", "install"]).output().unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("add this line to it"), "{}", stderr(&out));
    assert_eq!(fx.read(&hook), "#!/bin/sh\necho mine\n");
    let text = stdout(&fx.yman(&fx.a).args(["hooks", "status"]).output().unwrap());
    assert!(text.contains("hook post-merge: foreign"), "{text}");
    // ...and `hooks remove` leaves it alone too.
    fx.yman(&fx.a).args(["hooks", "remove"]).assert().success();
    assert_eq!(fx.read(&hook), "#!/bin/sh\necho mine\n");
}

#[test]
fn hooks_refresh_on_pull() {
    let fx = Fx::new();
    fx.yman(&fx.a).args(["init", "--hooks"]).assert().success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    // B pushes both code and tasks.
    fx.write(&fx.b.join("NOTES.md"), "notes\n");
    fx.git(&fx.b, &["add", "-A"]);
    fx.git(&fx.b, &["commit", "-m", "notes"]);
    fx.git(&fx.b, &["push", "origin", "main"]);
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.b).arg("sync").assert().success();

    // A's plain `git pull` fires post-merge, which refreshes .yman.
    let yman_dir = std::path::Path::new(env!("CARGO_BIN_EXE_yman"))
        .parent()
        .unwrap()
        .to_path_buf();
    let path = format!(
        "{}:{}",
        yman_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = std::process::Command::new("git")
        .current_dir(&fx.a)
        // A plain pull, so the configured refs/tasks/main refspec applies.
        .args(["pull", "--no-rebase"])
        .env("PATH", path)
        .env("HOME", fx.tmp.path().join("home"))
        .env("GIT_CONFIG_GLOBAL", fx.tmp.path().join("home/.gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    assert!(
        fx.has_task(&fx.a, "2"),
        "post-merge hook should have refreshed .yman"
    );
}

#[test]
fn survives_clean() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.a).args(["add", "Unpushed"]).assert().success();

    // The equivalent of `git clean -fdx` eating .yman.
    std::fs::remove_dir_all(fx.a.join(".yman")).unwrap();

    let out = fx.yman(&fx.a).arg("init").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&fx.yman(&fx.a).arg("ls").output().unwrap());
    assert!(text.contains("Fix login"), "{text}");
    assert!(text.contains("Unpushed"), "history must survive: {text}");

    let text = stdout(&fx.yman(&fx.a).arg("status").output().unwrap());
    assert!(text.contains("ahead 1"), "{text}");
}

#[test]
fn status_reports_ahead_behind() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();

    let text = stdout(&fx.yman(&fx.a).arg("status").output().unwrap());
    assert!(text.contains(".yman  refs/yman/local @ "), "{text}");
    assert!(text.contains("(refresh: lazy, hooks: not installed)"), "{text}");
    assert!(text.contains("worktree: clean"), "{text}");
    assert!(text.contains("tasks:   todo 0, doing 0, done 0"), "{text}");
    assert!(!text.contains("merge:"), "{text}");

    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let text = stdout(&fx.yman(&fx.a).arg("status").output().unwrap());
    assert!(text.contains("ahead 1, behind 0"), "{text}");
    assert!(text.contains("→ run: yman sync"), "{text}");

    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.b).arg("sync").assert().success();
    fx.git(&fx.a, &["fetch", "origin"]);

    // Status reports, it does not refresh.
    let text = stdout(&fx.yman(&fx.a).arg("status").output().unwrap());
    assert!(text.contains("ahead 0, behind 1"), "{text}");
    assert!(!fx.has_task(&fx.a, "2"));

    // A dirty worktree is called out too.
    fx.write(&fx.a.join(".yman/5.1.fix-login/t.md"), "# Fix login\n\nedit\n");
    let text = stdout(&fx.yman(&fx.a).arg("status").output().unwrap());
    assert!(
        text.contains("worktree: 1 uncommitted change(s) (will be snapshotted by next sync)"),
        "{text}"
    );
}

#[test]
fn status_reports_merge_in_progress() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();
    fx.yman(&fx.a).args(["set", "1", "--status", "doing"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).args(["set", "1", "--status", "done"]).assert().success();
    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert_eq!(out.status.code(), Some(3));

    let text = stdout(&fx.yman(&fx.b).arg("status").output().unwrap());
    assert!(text.contains("merge:   in progress, 1 unmerged file(s)"), "{text}");
    assert!(text.contains("5.1.fix-login/m.yml"), "{text}");
}

#[test]
fn yman_git_passthrough() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let out = fx
        .yman(&fx.a)
        .args(["git", "--", "log", "--oneline", "-1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("task(1): add"), "{}", stdout(&out));

    // Exit codes come straight from git.
    let out = fx.yman(&fx.a).args(["git", "--", "cat-file", "-e", "deadbeef"]).output().unwrap();
    assert!(!out.status.success());
}

#[test]
fn broken_task_is_listed_not_fatal() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.write(&fx.a.join(".yman/5.2.broken/m.yml"), "status: todo\n");
    fx.write(&fx.a.join(".yman/5.2.broken/t.md"), "no heading here\n");

    let out = fx.yman(&fx.a).arg("ls").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("Fix login"), "{text}");
    assert!(text.contains("!  5.2.broken  broken"), "{text}");
    assert!(text.contains("t.md must start with"), "{text}");

    // Commands that target it fail, and say why.
    let out = fx.yman(&fx.a).args(["show", "2"]).output().unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("task 2 is broken"), "{}", stderr(&out));

    let text = stdout(&fx.yman(&fx.a).arg("status").output().unwrap());
    assert!(text.contains("(1 broken)"), "{text}");
}

// ------------------------------------------------------------- editor paths

#[test]
fn edit_rewrites_title_and_renames_folder() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let editor = fx.editor_writing("ed-title", "# Fix logout\n\nnow with a body\n");
    let out = fx
        .yman(&fx.a)
        .env("EDITOR", &editor)
        .args(["edit", "1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("edited 1  5.1.fix-logout"), "{}", stdout(&out));

    assert!(fx.a.join(".yman/5.1.fix-logout").is_dir());
    assert!(!fx.a.join(".yman/5.1.fix-login").exists());
    assert_eq!(fx.title(&fx.a, "1"), "Fix logout");

    let subject = fx.git(&fx.a.join(".yman"), &["log", "--oneline", "-1"]);
    assert!(subject.contains("task(1): edit"), "{subject}");
    // The rename is a git rename, so `log <id>` still reaches the add commit.
    let log = stdout(&fx.yman(&fx.a).args(["log", "1"]).output().unwrap());
    assert!(log.contains("task(1): add \"Fix login\""), "{log}");
    assert_eq!(fx.git(&fx.a.join(".yman"), &["status", "--porcelain"]), "");
}

#[test]
fn edit_body_only_keeps_the_folder() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let editor = fx.editor_writing("ed-body", "# Fix login\n\njust a new body\n");
    fx.yman(&fx.a)
        .env("EDITOR", &editor)
        .args(["edit", "1"])
        .assert()
        .success();

    assert!(fx.a.join(".yman/5.1.fix-login").is_dir());
    assert!(fx.read(&fx.a.join(".yman/5.1.fix-login/t.md")).contains("just a new body"));
}

#[test]
fn edit_without_changes_commits_nothing() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let before = fx.git(&fx.a.join(".yman"), &["rev-parse", "HEAD"]);

    // The fixture's default EDITOR is `true`: it touches nothing.
    let out = fx.yman(&fx.a).args(["edit", "1"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim_end(), "no changes");
    assert_eq!(fx.git(&fx.a.join(".yman"), &["rev-parse", "HEAD"]), before);
}

#[test]
fn edit_rejects_a_file_without_a_title() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let editor = fx.editor_writing("ed-broken", "no heading at all\n");
    let out = fx
        .yman(&fx.a)
        .env("EDITOR", &editor)
        .args(["edit", "1"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("t.md invalid after edit"), "{err}");
    assert!(err.contains("fix the file then run: yman edit 1"), "{err}");

    // The user's text is left exactly as they wrote it, not reverted.
    assert_eq!(
        fx.read(&fx.a.join(".yman/5.1.fix-login/t.md")),
        "no heading at all\n"
    );
    // ...and the next sync snapshots it rather than losing it.
    let out = fx.yman(&fx.a).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("snapshotted 1 local change(s)"), "{}", stdout(&out));
}

#[test]
fn edit_reports_an_editor_that_fails() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let editor = fx.editor_failing("ed-fail", 3);
    let out = fx
        .yman(&fx.a)
        .env("EDITOR", &editor)
        .args(["edit", "1"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: editor exited with status 3; file left as is"
    );
    assert_eq!(fx.title(&fx.a, "1"), "Fix login");
}

#[test]
fn visual_wins_over_editor() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let visual = fx.editor_writing("ed-visual", "# From VISUAL\n");
    let editor = fx.editor_writing("ed-editor", "# From EDITOR\n");
    fx.yman(&fx.a)
        .env("VISUAL", &visual)
        .env("EDITOR", &editor)
        .args(["edit", "1"])
        .assert()
        .success();
    assert_eq!(fx.title(&fx.a, "1"), "From VISUAL");
}

#[test]
fn add_with_editor_uses_the_edited_title() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();

    let editor = fx.editor_writing("ed-add", "# Real title\n\nwritten in the editor\n");
    let out = fx
        .yman(&fx.a)
        .env("EDITOR", &editor)
        .args(["add", "Placeholder", "-e", "-p", "1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("added 1  1.1.real-title"), "{}", stdout(&out));

    assert!(fx.a.join(".yman/1.1.real-title").is_dir());
    assert!(!fx.a.join(".yman/1.1.placeholder").exists());
    assert!(fx.read(&fx.a.join(".yman/1.1.real-title/t.md")).contains("written in the editor"));

    // One commit, under the final title; the placeholder never existed.
    let log = fx.git(&fx.a.join(".yman"), &["log", "--oneline"]);
    assert!(log.contains("task(1): add \"Real title\""), "{log}");
    assert!(!log.contains("Placeholder"), "{log}");
    assert_eq!(fx.git(&fx.a.join(".yman"), &["status", "--porcelain"]), "");
}

#[test]
fn comment_from_editor_and_from_stdin() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let editor = fx.editor_writing("ed-comment", "written in the editor\n");
    fx.yman(&fx.a)
        .env("EDITOR", &editor)
        .args(["comment", "1", "-e"])
        .assert()
        .success();

    fx.yman(&fx.a)
        .args(["comment", "1"])
        .write_stdin("piped from stdin\n")
        .assert()
        .success();

    let d = fx.read(&fx.a.join(".yman/5.1.fix-login/d.md"));
    assert!(d.contains("written in the editor"), "{d}");
    assert!(d.contains("piped from stdin"), "{d}");
    assert_eq!(d.matches("— Test A").count(), 2, "{d}");

    // An editor session the user left empty is not a comment.
    let empty = fx.editor_writing("ed-empty", "   \n");
    let out = fx
        .yman(&fx.a)
        .env("EDITOR", &empty)
        .args(["comment", "1", "-e"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(stderr(&out).trim(), "error: empty comment");
}
