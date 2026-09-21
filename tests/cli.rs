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
        fetch
            .lines()
            .any(|l| l == "+refs/tasks/main:refs/yman/remote"),
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
    assert!(
        stderr(&out).contains("--id-scheme ignored"),
        "{}",
        stderr(&out)
    );
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
        .args([
            "add",
            "Fix login",
            "-p",
            "2",
            "-t",
            "auth",
            "-t",
            "bug",
            "-m",
            "body text",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).starts_with("added 1  2.1.fix-login"),
        "{}",
        stdout(&out)
    );
    assert!(fx.a.join(".yman/2.1.fix-login/t.md").is_file());
    assert_eq!(
        fx.read(&fx.a.join(".yman/2.1.fix-login/t.md")),
        "# Fix login\n\nbody text\n"
    );

    // Second task takes the next sequential id and the default priority.
    fx.yman(&fx.a)
        .args(["add", "Write docs"])
        .assert()
        .success();
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
    assert!(
        text.contains("priority: 2   status: todo   assignee: -   tags: auth, bug"),
        "{text}"
    );
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
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(stderr(&out).trim(), "error: task 99 not found");

    let out = fx
        .yman(&fx.a)
        .args(["add", "X", "-s", "nope"])
        .output()
        .unwrap();
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
    fx.yman(&fx.b)
        .args(["add", "Write docs"])
        .assert()
        .success();
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
        .args([
            "set",
            "1",
            "--title",
            "Fix logout",
            "--status",
            "doing",
            "--tag",
            "auth",
        ])
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
    assert!(
        stdout(&out).contains("attached screenshot.png"),
        "{}",
        stdout(&out)
    );
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

    let out = fx
        .yman(&fx.a)
        .args(["detach", "1", "nope.png"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: no attachment \"nope.png\" on task 1"
    );
}

/// `init` writes `*.swp`, `*~`, `.#*` and `*.orig` to `.yman/.gitignore`, and a
/// plain `git add <dir>` skips an ignored path in silence. Attaching a merge
/// leftover therefore recorded the entry in `m.yml`, never committed the file,
/// and left every other clone with a dangling attachment after `sync`.
#[test]
fn attach_stages_a_gitignored_file() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    // One of each shape `init` ignores: a merge leftover and an editor backup.
    let orig = fx.tmp.path().join("notes.orig");
    fx.write(&orig, "the other side of the merge");
    let backup = fx.tmp.path().join("draft~");
    fx.write(&backup, "an editor left this");

    fx.yman(&fx.a)
        .args([
            "attach",
            "1",
            orig.to_str().unwrap(),
            backup.to_str().unwrap(),
        ])
        .assert()
        .success();

    let tracked = fx.git(&fx.a, &["-C", ".yman", "ls-files"]);
    assert!(
        tracked.contains("5.1.fix-login/f/notes.orig"),
        "attachment was not committed: {tracked}"
    );
    assert!(
        tracked.contains("5.1.fix-login/f/draft~"),
        "attachment was not committed: {tracked}"
    );
    // Nothing left behind: the copy is committed, not sitting untracked.
    assert_eq!(fx.git(&fx.a, &["-C", ".yman", "status", "--porcelain"]), "");

    // The point of all this: the other clone actually receives the bytes.
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();
    assert_eq!(
        fx.read(&fx.b.join(".yman/5.1.fix-login/f/notes.orig")),
        "the other side of the merge"
    );
    let tracked = fx.git(&fx.b, &["-C", ".yman", "ls-files"]);
    assert!(
        tracked.contains("5.1.fix-login/f/notes.orig"),
        "attachment did not reach the second clone: {tracked}"
    );

    // And detach still removes what force-add put in.
    fx.yman(&fx.a)
        .args(["detach", "1", "notes.orig"])
        .assert()
        .success();
    assert!(
        !fx.git(&fx.a, &["-C", ".yman", "ls-files"])
            .contains("notes.orig"),
        "detached file is still tracked"
    );
    assert_eq!(fx.git(&fx.a, &["-C", ".yman", "status", "--porcelain"]), "");
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

    let out = fx
        .yman(&fx.a)
        .args(["comment", "1", "-m", "   "])
        .output()
        .unwrap();
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
    assert!(
        subject.contains("task(1): remove \"Fix login\""),
        "{subject}"
    );

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
    fx.yman(&fx.a)
        .args(["add", "Other task"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["prio", "1", "2"]).assert().success();
    fx.yman(&fx.a)
        .args(["set", "1", "--title", "Fix logout"])
        .assert()
        .success();
    fx.yman(&fx.a)
        .args(["comment", "1", "-m", "note"])
        .assert()
        .success();

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
    fx.yman(&fx.a)
        .args(["add", "Fix login", "-p", "2"])
        .assert()
        .success();

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
    fx.yman(&fx.b)
        .args(["add", "Write docs"])
        .assert()
        .success();
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
fn collision_renumber_rewrites_related() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    // Both mint id 2 offline; B also has a task 3 pointing at its own 2.
    fx.yman(&fx.a).args(["add", "A two"]).assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.b).args(["add", "B three"]).assert().success();
    fx.yman(&fx.b)
        .args(["set", "3", "--relate", "2"])
        .assert()
        .success();
    fx.yman(&fx.b)
        .args(["set", "2", "--relate", "3"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("renumbered 2 -> 4"),
        "{}",
        stdout(&out)
    );
    let err = stderr(&out);
    assert!(err.contains("rewrote 1 reference(s)"), "{err}");
    assert!(!err.contains("manually"), "{err}");

    // The reference followed the move; the moved task keeps its own.
    assert_eq!(fx.title(&fx.b, "4"), "B two");
    let three = stdout(&fx.yman(&fx.b).args(["show", "3"]).output().unwrap());
    assert!(three.contains("related:  4"), "{three}");
    let four = stdout(&fx.yman(&fx.b).args(["show", "4"]).output().unwrap());
    assert!(four.contains("related:  3"), "{four}");

    // Nothing was left uncommitted by the rewrite.
    assert_eq!(fx.git(&fx.b, &["-C", ".yman", "status", "--porcelain"]), "");
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
    fx.yman(&fx.a)
        .args(["set", "1", "--status", "doing"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b)
        .args(["set", "1", "--status", "done"])
        .assert()
        .success();

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("5.1.fix-login/m.yml"), "{err}");
    assert!(err.contains("yman sync --continue"), "{err}");

    // Any mutating command is refused until the merge is settled.
    let out = fx.yman(&fx.b).args(["add", "Nope"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(
        stderr(&out).contains("sync merge in progress"),
        "{}",
        stderr(&out)
    );

    // --continue refuses while markers are still there.
    let out = fx
        .yman(&fx.b)
        .args(["sync", "--continue"])
        .output()
        .unwrap();
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

    let out = fx
        .yman(&fx.b)
        .args(["sync", "--continue"])
        .output()
        .unwrap();
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

    fx.yman(&fx.a)
        .args(["set", "1", "--status", "doing"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b)
        .args(["set", "1", "--status", "done"])
        .assert()
        .success();
    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert_eq!(out.status.code(), Some(3));

    let out = fx.yman(&fx.b).args(["sync", "--abort"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim_end(), "merge aborted");
    assert_eq!(fx.status(&fx.b, "1"), "done");
    assert!(
        !fx.read(&fx.b.join(".yman/5.1.fix-login/m.yml"))
            .contains("<<<<<<<")
    );

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
    fx.yman(&fx.a)
        .args(["comment", "1", "-m", "from A first"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    fx.yman(&fx.a)
        .args(["comment", "1", "-m", "only A"])
        .assert()
        .success();
    fx.yman(&fx.b)
        .args(["comment", "1", "-m", "only B"])
        .assert()
        .success();
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
    assert!(
        stdout(&out).contains("snapshotted 1 local change(s)"),
        "{}",
        stdout(&out)
    );
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
    fx.yman(&fx.a)
        .args(["init", "--refresh", "manual"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();
    fx.yman(&fx.b).args(["add", "B two"]).assert().success();
    fx.yman(&fx.b).arg("sync").assert().success();

    fx.git(&fx.a, &["fetch", "origin"]);
    let text = stdout(&fx.yman(&fx.a).arg("ls").output().unwrap());
    assert!(
        !text.contains("B two"),
        "manual policy must not refresh: {text}"
    );

    // An explicit refresh still works, and says what it did.
    let out = fx.yman(&fx.a).arg("refresh").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("refreshed: 1 new commit(s)"),
        "{}",
        stderr(&out)
    );
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
    assert!(
        fx.read(&fx.a.join(".yman/5.1.a-one/t.md"))
            .contains("mid-edit")
    );
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
    assert!(
        stdout(&out).contains("already installed"),
        "{}",
        stdout(&out)
    );

    fx.yman(&fx.a).args(["hooks", "remove"]).assert().success();
    assert!(!hook.exists());

    // Someone else's hook is reported, never overwritten.
    fx.write(&hook, "#!/bin/sh\necho mine\n");
    let out = fx.yman(&fx.a).args(["hooks", "install"]).output().unwrap();
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("add this line to it"),
        "{}",
        stderr(&out)
    );
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
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

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
    assert!(
        text.contains("(refresh: lazy, hooks: not installed)"),
        "{text}"
    );
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
    fx.write(
        &fx.a.join(".yman/5.1.fix-login/t.md"),
        "# Fix login\n\nedit\n",
    );
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
    fx.yman(&fx.a)
        .args(["set", "1", "--status", "doing"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b)
        .args(["set", "1", "--status", "done"])
        .assert()
        .success();
    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert_eq!(out.status.code(), Some(3));

    let text = stdout(&fx.yman(&fx.b).arg("status").output().unwrap());
    assert!(
        text.contains("merge:   in progress, 1 unmerged file(s)"),
        "{text}"
    );
    assert!(text.contains("5.1.fix-login/m.yml"), "{text}");

    let json = stdout(&fx.yman(&fx.b).args(["status", "--json"]).output().unwrap());
    assert!(json.contains("\"in_progress\":true"), "{json}");
    assert!(
        json.contains("\"unmerged\":[\"5.1.fix-login/m.yml\"]"),
        "{json}"
    );
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
    let out = fx
        .yman(&fx.a)
        .args(["git", "--", "cat-file", "-e", "deadbeef"])
        .output()
        .unwrap();
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
    assert!(
        stderr(&out).contains("task 2 is broken"),
        "{}",
        stderr(&out)
    );

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
    assert!(
        stdout(&out).contains("edited 1  5.1.fix-logout"),
        "{}",
        stdout(&out)
    );

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
    assert!(
        fx.read(&fx.a.join(".yman/5.1.fix-login/t.md"))
            .contains("just a new body")
    );
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
    assert!(
        stdout(&out).contains("snapshotted 1 local change(s)"),
        "{}",
        stdout(&out)
    );
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
    assert!(
        stdout(&out).contains("added 1  1.1.real-title"),
        "{}",
        stdout(&out)
    );

    assert!(fx.a.join(".yman/1.1.real-title").is_dir());
    assert!(!fx.a.join(".yman/1.1.placeholder").exists());
    assert!(
        fx.read(&fx.a.join(".yman/1.1.real-title/t.md"))
            .contains("written in the editor")
    );

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

// -------------------------------------------- init variants and id schemes

#[test]
fn init_offline_touches_no_remote() {
    let fx = Fx::new();
    // An origin that could not be reached even if we tried.
    fx.git(
        &fx.a,
        &["remote", "set-url", "origin", "/nonexistent/remote.git"],
    );

    let out = fx.yman(&fx.a).args(["init", "--offline"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("initialized .yman"),
        "{}",
        stdout(&out)
    );

    // Local history exists; nothing was fetched or pushed.
    assert!(fx.a.join(".yman/config.toml").is_file());
    let (ok, _, _) = fx.git_try(&fx.a, &["rev-parse", "--verify", "refs/yman/remote"]);
    assert!(!ok, "refs/yman/remote must not exist after an offline init");
    fx.yman(&fx.a)
        .args(["add", "Offline task"])
        .assert()
        .success();
    assert_eq!(fx.title(&fx.a, "1"), "Offline task");

    let text = stdout(&fx.yman(&fx.a).arg("status").output().unwrap());
    assert!(
        text.contains("remote: refs/tasks/main not fetched yet"),
        "{text}"
    );

    // Once a reachable origin is back, a plain sync publishes everything.
    fx.git(
        &fx.a,
        &["remote", "set-url", "origin", fx.remote.to_str().unwrap()],
    );
    let out = fx.yman(&fx.a).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        fx.git(&fx.remote, &["rev-parse", "refs/tasks/main"]),
        fx.git(&fx.a, &["rev-parse", "refs/yman/local"])
    );
}

#[test]
fn init_without_an_origin_needs_a_remote_url() {
    let fx = Fx::new();
    fx.git(&fx.a, &["remote", "remove", "origin"]);

    let out = fx.yman(&fx.a).arg("init").output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: main repo has no \"origin\" remote; pass --remote <url>"
    );

    // With a URL, init creates the remote it is going to push to.
    let out = fx
        .yman(&fx.a)
        .args(["init", "--remote", fx.remote.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        fx.git(&fx.a, &["config", "--get", "remote.origin.url"]),
        fx.remote.to_string_lossy()
    );
    fx.yman(&fx.a).args(["add", "A task"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    assert!(
        !fx.git(&fx.remote, &["rev-parse", "refs/tasks/main"])
            .is_empty()
    );
}

#[test]
fn init_refuses_a_foreign_yman_directory() {
    let fx = Fx::new();
    fx.write(&fx.a.join(".yman/notes.txt"), "someone else's directory\n");
    let out = fx.yman(&fx.a).arg("init").output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: .yman exists and is not a yman worktree; move it aside and rerun"
    );

    // A standalone repo there is called out separately, and left alone.
    std::fs::remove_dir_all(fx.a.join(".yman")).unwrap();
    fx.git(&fx.a, &["init", "-q", ".yman"]);
    let out = fx.yman(&fx.a).arg("init").output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: .yman is a standalone git repository, not a worktree; move it aside and rerun"
    );
    assert!(fx.a.join(".yman/.git").is_dir());
}

#[test]
fn random_scheme_round_trip() {
    let fx = Fx::new();
    fx.yman(&fx.a)
        .args(["init", "--id-scheme", "random"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    fx.yman(&fx.a).args(["add", "A two"]).assert().success();

    let ids = ids_in(&fx.a);
    assert_eq!(ids.len(), 2, "{ids:?}");
    for id in &ids {
        assert!(id.starts_with("t-"), "{id}");
        assert_eq!(id.len(), 6, "{id}");
        assert!(id[2..].bytes().all(|b| b.is_ascii_hexdigit()), "{id}");
    }
    assert_ne!(ids[0], ids[1], "random ids must not repeat");

    // Random ids do not collide across clones, so a sync never renumbers.
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();
    fx.yman(&fx.b).args(["add", "B three"]).assert().success();
    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("renumbered 0"), "{}", stdout(&out));
    assert_eq!(ids_in(&fx.b).len(), 3);

    // The scheme travels in config.toml, so B mints random ids too.
    for id in ids_in(&fx.b) {
        assert!(id.starts_with("t-"), "{id}");
    }
}

#[test]
fn author_scheme_derives_a_prefix_from_the_committer() {
    let fx = Fx::new();
    // No --author: the prefix comes from the initials of user.name ("Test A").
    fx.yman(&fx.a)
        .args(["init", "--id-scheme", "author"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["add", "A one"]).assert().success();
    assert_eq!(ids_in(&fx.a), vec!["ta-1".to_string()]);

    // An explicit prefix wins over the derived one.
    fx.yman(&fx.a)
        .args(["init", "--author", "iv"])
        .assert()
        .success();
    fx.yman(&fx.a).args(["add", "A two"]).assert().success();
    assert!(fx.has_task(&fx.a, "iv-1"));

    // A blank setting means "unset", not "invalid": it falls through to the
    // next source, which here is the initials again.
    fx.git(&fx.a, &["config", "yman.author", "   "]);
    fx.yman(&fx.a).args(["add", "A three"]).assert().success();
    assert!(fx.has_task(&fx.a, "ta-2"));
}

/// A prefix that would break the folder-name grammar is refused at `add`,
/// before anything is written. `init` stores it without complaint, so this is
/// where the user finds out.
#[test]
fn a_dotted_author_prefix_is_rejected() {
    let fx = Fx::new();
    fx.yman(&fx.a)
        .args(["init", "--id-scheme", "author", "--author", "v.i"])
        .assert()
        .success();

    let out = fx.yman(&fx.a).args(["add", "Fix login"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        stderr(&out).trim(),
        "error: author prefix \"v.i\" from yman.author must be letters, digits, \"_\" or \"-\""
    );
    // The id is minted before the folder, so nothing was created.
    assert!(fx.task_dirs(&fx.a).is_empty());
}

#[test]
fn a_bad_author_prefix_names_its_source() {
    let fx = Fx::new();
    fx.yman(&fx.a)
        .args(["init", "--id-scheme", "author"])
        .assert()
        .success();

    // The fixture removes YMAN_AUTHOR; a later `.env` puts it back.
    let out = fx
        .yman(&fx.a)
        .args(["add", "Fix login"])
        .env("YMAN_AUTHOR", "ivan p")
        .output()
        .unwrap();
    assert_eq!(
        stderr(&out).trim(),
        "error: author prefix \"ivan p\" from $YMAN_AUTHOR must be letters, digits, \"_\" or \"-\""
    );

    // Config wins over the environment, and reports itself as the source.
    fx.git(&fx.a, &["config", "yman.author", "a/b"]);
    let out = fx
        .yman(&fx.a)
        .args(["add", "Fix login"])
        .env("YMAN_AUTHOR", "ivan p")
        .output()
        .unwrap();
    assert_eq!(
        stderr(&out).trim(),
        "error: author prefix \"a/b\" from yman.author must be letters, digits, \"_\" or \"-\""
    );
}

#[test]
fn hooks_honour_core_hooks_path() {
    let fx = Fx::new();
    fx.git(&fx.a, &["config", "core.hooksPath", "githooks"]);
    fx.yman(&fx.a).arg("init").assert().success();

    let out = fx.yman(&fx.a).args(["hooks", "install"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("installing into core.hooksPath="),
        "{}",
        stderr(&out)
    );
    assert!(fx.a.join("githooks/post-merge").is_file());
    assert!(!fx.a.join(".git/hooks/post-merge").exists());

    let text = stdout(&fx.yman(&fx.a).args(["hooks", "status"]).output().unwrap());
    assert!(text.contains("hook post-merge: installed"), "{text}");

    fx.yman(&fx.a).args(["hooks", "remove"]).assert().success();
    assert!(!fx.a.join("githooks/post-merge").exists());
}

/// Task ids present in a clone, in folder-name order.
fn ids_in(clone: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(clone.join(".yman"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.split('.').count() >= 3)
        .collect();
    names.sort();
    names
        .iter()
        .filter_map(|n| n.split('.').nth(1).map(String::from))
        .collect()
}

// ------------------------------------------------------- the rest of `set`

#[test]
fn set_assignee_links_and_related() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).args(["add", "Other"]).assert().success();

    let out = fx
        .yman(&fx.a)
        .args([
            "set",
            "1",
            "--assignee",
            "Ivan",
            "--link",
            "https://example.invalid/issues/12",
            "--relate",
            "2",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("1: assignee - -> Ivan"), "{text}");
    assert!(
        text.contains("1: links +https://example.invalid/issues/12"),
        "{text}"
    );
    assert!(text.contains("1: related +2"), "{text}");

    let shown = stdout(&fx.yman(&fx.a).args(["show", "1"]).output().unwrap());
    assert!(shown.contains("assignee: Ivan"), "{shown}");
    assert!(
        shown.contains("links:    https://example.invalid/issues/12"),
        "{shown}"
    );
    assert!(shown.contains("related:  2"), "{shown}");

    // Removing works, and removing something absent is quietly fine.
    let out = fx
        .yman(&fx.a)
        .args([
            "set",
            "1",
            "--no-assignee",
            "--unlink",
            "https://example.invalid/issues/12",
            "--unrelate",
            "2",
            "--unrelate",
            "999",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("1: assignee Ivan -> -"), "{text}");
    assert!(text.contains("1: related -2"), "{text}");
    assert!(
        !text.contains("999"),
        "removing an absent value is not a change: {text}"
    );

    let shown = stdout(&fx.yman(&fx.a).args(["show", "1"]).output().unwrap());
    assert!(shown.contains("assignee: -"), "{shown}");
    assert!(!shown.contains("links:"), "{shown}");
    assert!(!shown.contains("related:"), "{shown}");
}

#[test]
fn set_tags_keep_their_order_and_dedupe() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a)
        .args(["add", "Fix login", "-t", "auth", "-t", "auth", "-t", "bug"])
        .assert()
        .success();
    let text = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(text.contains("\"tags\":[\"auth\",\"bug\"]"), "{text}");

    // Adding a tag that is already there changes nothing at all.
    let before = fx.git(&fx.a.join(".yman"), &["rev-parse", "HEAD"]);
    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--tag", "auth"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(stdout(&out).trim_end(), "no changes");
    assert_eq!(fx.git(&fx.a.join(".yman"), &["rev-parse", "HEAD"]), before);

    fx.yman(&fx.a)
        .args(["set", "1", "--tag", "ui", "--untag", "auth"])
        .assert()
        .success();
    let text = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(text.contains("\"tags\":[\"bug\",\"ui\"]"), "{text}");

    let subject = fx.git(&fx.a.join(".yman"), &["log", "--oneline", "-1"]);
    assert!(subject.contains("tags=+ui,-auth"), "{subject}");
}

#[test]
fn set_rejects_an_unknown_status_and_an_empty_title() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--status", "wip"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: unknown status \"wip\"; allowed: todo, doing, done"
    );

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--title", "   "])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(stderr(&out).trim(), "error: title must not be empty");

    // A priority outside 0..=9 is a usage error, caught by the parser.
    let out = fx.yman(&fx.a).args(["prio", "1", "10"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));

    // Nothing above touched the task.
    assert_eq!(fx.status(&fx.a, "1"), "todo");
    assert_eq!(fx.title(&fx.a, "1"), "Fix login");
    assert!(fx.a.join(".yman/5.1.fix-login").is_dir());
}

#[test]
fn assignee_flags_conflict() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--assignee", "Ivan", "--no-assignee"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn awkward_field_values_survive_a_rewrite() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    // Values that a naive YAML writer would lose: a colon, a leading `#`, a
    // quote, something that reads as a number, and a trailing space.
    fx.yman(&fx.a)
        .args([
            "set",
            "1",
            "--assignee",
            "O'Brien: lead ",
            "--tag",
            "5",
            "--tag",
            "#urgent",
            "--link",
            "https://example.com/a#b",
            "--relate",
            "0x1f",
        ])
        .assert()
        .success();

    let json = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(json.contains("O'Brien: lead "), "{json}");

    // Reading the folder back is what proves the file round-tripped: every
    // command loads `m.yml` before it does anything else.
    let out = fx.yman(&fx.a).args(["show", "1"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    for expected in [
        "O'Brien: lead",
        "5",
        "#urgent",
        "https://example.com/a#b",
        "0x1f",
    ] {
        assert!(text.contains(expected), "{expected} missing from\n{text}");
    }

    // A second write must not drift: the file is identical apart from
    // `updated`, which `set` touches on purpose.
    let before = fx.read(&fx.a.join(".yman/5.1.fix-login/m.yml"));
    fx.yman(&fx.a)
        .args(["set", "1", "--title", "Fix login"])
        .assert()
        .success();
    let after = fx.read(&fx.a.join(".yman/5.1.fix-login/m.yml"));
    assert_eq!(before, after, "an unchanged task was rewritten");
}

/// A key this yman does not know — a newer version's field, or one someone
/// added by hand — is carried through a rewrite instead of being eaten.
#[test]
fn hand_added_m_yml_keys_survive_a_rewrite() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    // Spliced in the middle, where an older yman used to drop it.
    let path = fx.a.join(".yman/5.1.fix-login/m.yml");
    let text = fx.read(&path);
    let (head, tail) = text.split_once("links: []\n").unwrap();
    fx.write(&path, &format!("{head}due: 2026-12-01\nlinks: []\n{tail}"));

    fx.yman(&fx.a)
        .args(["set", "1", "--tag", "x"])
        .assert()
        .success();

    let text = fx.read(&path);
    assert!(text.contains("due: 2026-12-01"), "{text}");
    assert!(text.ends_with("due: 2026-12-01\n"), "{text}");

    // And the task is still a task, not a broken folder.
    let out = fx.yman(&fx.a).arg("ls").output().unwrap();
    let listing = stdout(&out);
    assert!(listing.contains("Fix login"), "{listing}");
    assert!(!listing.contains('!'), "{listing}");

    // A second rewrite neither drops it nor emits it twice.
    fx.yman(&fx.a)
        .args(["set", "1", "--tag", "y"])
        .assert()
        .success();
    let text = fx.read(&path);
    assert_eq!(text.matches("due: 2026-12-01").count(), 1, "{text}");
    assert!(text.ends_with("due: 2026-12-01\n"), "{text}");
}

/// Every closed status is hidden, not just the last one in the list.
#[test]
fn ls_hides_the_whole_terminal_set() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.v2_config(&fx.a);
    for title in ["open one", "closed one", "dropped one"] {
        fx.yman(&fx.a).args(["add", title]).assert().success();
    }
    fx.yman(&fx.a)
        .args(["set", "2", "--status", "done"])
        .assert()
        .success();
    fx.yman(&fx.a)
        .args(["set", "3", "--status", "cancelled"])
        .assert()
        .success();

    let out = fx.yman(&fx.a).arg("ls").output().unwrap();
    let text = stdout(&out);
    assert!(text.contains("open one"), "{text}");
    assert!(!text.contains("closed one"), "{text}");
    assert!(!text.contains("dropped one"), "{text}");

    let text = stdout(&fx.yman(&fx.a).args(["ls", "-a"]).output().unwrap());
    assert!(
        text.contains("closed one") && text.contains("dropped one"),
        "{text}"
    );

    // Naming a terminal status still includes it, and only it.
    let text = stdout(
        &fx.yman(&fx.a)
            .args(["ls", "-s", "cancelled"])
            .output()
            .unwrap(),
    );
    assert!(text.contains("dropped one"), "{text}");
    assert!(
        !text.contains("closed one") && !text.contains("open one"),
        "{text}"
    );
}

/// A folder one level down is a task like any other, and a broken one is
/// reported with the path that tells you where to look.
#[test]
fn a_broken_folder_in_a_status_dir_is_listed_with_its_path() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.v2_config(&fx.a);
    fx.yman(&fx.a).args(["add", "real one"]).assert().success();

    let ydir = fx.a.join(".yman");
    std::fs::create_dir_all(ydir.join("done").join("5.9.hand-made")).unwrap();
    std::fs::write(
        ydir.join("done").join("5.9.hand-made").join("m.yml"),
        "status: done\n",
    )
    .unwrap();

    let out = fx.yman(&fx.a).arg("ls").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("done/5.9.hand-made"), "{text}");
    assert!(text.contains("cannot read t.md"), "{text}");

    let text = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(text.contains("\"dir\":\"done/5.9.hand-made\""), "{text}");

    // And it is findable by id, not invisible.
    let out = fx.yman(&fx.a).args(["show", "9"]).output().unwrap();
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("task 9 is broken: done/5.9.hand-made"),
        "{}",
        stderr(&out)
    );
}

/// A task closed to a different status on each clone is a rename/rename
/// conflict: git keeps both folders, both load, and neither carries a marker.
/// Without the duplicate-id gate `--continue` commits and pushes a task that
/// no later command can touch.
#[test]
fn a_split_close_cannot_be_continued_into_a_duplicate_id() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    // Each side files the same task under a different status directory.
    let archive = |clone: &std::path::Path, status: &str| {
        let ydir = clone.join(".yman");
        std::fs::create_dir_all(ydir.join(status)).unwrap();
        fx.git(
            &ydir,
            &["mv", "5.1.fix-login", &format!("{status}/5.1.fix-login")],
        );
        fx.git(&ydir, &["commit", "-q", "--no-verify", "-m", "archive"]);
    };
    archive(&fx.a, "done");
    fx.yman(&fx.a).arg("sync").assert().success();
    archive(&fx.b, "cancelled");

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("was closed to two different statuses"),
        "{err}"
    );
    assert!(err.contains("done/5.1.fix-login"), "{err}");
    assert!(err.contains("cancelled/5.1.fix-login"), "{err}");

    // Both folders are on disk, both load, no file has a marker — every other
    // check passes, so only the gate stands between here and a corrupt push.
    assert!(fx.b.join(".yman/done/5.1.fix-login/t.md").exists());
    assert!(fx.b.join(".yman/cancelled/5.1.fix-login/t.md").exists());

    let out = fx
        .yman(&fx.b)
        .args(["sync", "--continue"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("duplicate task id 1: cancelled/5.1.fix-login, done/5.1.fix-login"),
        "{err}"
    );
    assert!(err.contains("delete one folder"), "{err}");

    // Resolving as instructed finishes the sync.
    std::fs::remove_dir_all(fx.b.join(".yman/cancelled/5.1.fix-login")).unwrap();
    fx.yman(&fx.b)
        .args(["sync", "--continue"])
        .assert()
        .success();
    assert_eq!(fx.task_rel(&fx.b, "1"), "done/5.1.fix-login");
    assert_eq!(fx.git(&fx.b, &["-C", ".yman", "status", "--porcelain"]), "");
}

/// Commenting on an existing task is not creating one. Both sides adding a
/// `d.md` to the same folder used to look like two people minting the same id,
/// so syncing second renumbered a task that had been published for weeks.
#[test]
fn commenting_on_both_sides_does_not_renumber() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    fx.yman(&fx.a)
        .args(["comment", "1", "-m", "from A"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b)
        .args(["comment", "1", "-m", "from B"])
        .assert()
        .success();

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    // The summary always reports a count; it must be zero.
    assert!(stdout(&out).contains("renumbered 0"), "{}", stdout(&out));
    assert!(
        !stdout(&out).contains("(id taken on origin)"),
        "{}",
        stdout(&out)
    );
    assert!(fx.has_task(&fx.b, "1"), "task 1 was renumbered away");
    assert!(!fx.has_task(&fx.b, "2"));

    // And with no renumber in the way, the union merge does its job.
    let shown = stdout(&fx.yman(&fx.b).args(["show", "1"]).output().unwrap());
    assert!(
        shown.contains("from A") && shown.contains("from B"),
        "{shown}"
    );
}
/// Retitling is not creating. A retitle is a `git mv` plus a rewritten first
/// line of a short file, and git scored that rename below its 50% threshold,
/// so the pair read as a delete plus an add: the task looked newly created,
/// its own published id counted as "taken on origin", and sync renumbered it.
#[test]
fn retitling_does_not_renumber() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    // B retitles offline; the folder moves and `t.md`'s first line changes.
    fx.yman(&fx.b)
        .args(["set", "1", "--title", "Repair the login flow"])
        .assert()
        .success();
    // Meanwhile origin moves, so B's sync has to merge rather than fast-forward.
    fx.yman(&fx.a)
        .args(["add", "Ship the release"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("renumbered 0"), "{}", stdout(&out));
    assert!(
        !stdout(&out).contains("(id taken on origin)"),
        "{}",
        stdout(&out)
    );
    assert!(fx.has_task(&fx.b, "1"), "task 1 was renumbered away");
    assert_eq!(fx.title(&fx.b, "1"), "Repair the login flow");
    assert_eq!(fx.task_rel(&fx.b, "1"), "5.1.repair-the-login-flow");
    // The task A added in the meantime keeps the id it was published under.
    assert_eq!(fx.title(&fx.b, "2"), "Ship the release");

    // And the retitle survives the round trip back to A.
    fx.yman(&fx.a).arg("sync").assert().success();
    assert_eq!(fx.title(&fx.a, "1"), "Repair the login flow");
}

/// Closing a task moves its folder into the status directory, in the same
/// commit as the status change.
#[test]
fn closing_a_task_archives_its_folder() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.v2_config(&fx.a);
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let before = fx.git(&fx.a, &["-C", ".yman", "rev-list", "--count", "HEAD"]);
    let out = fx.yman(&fx.a).args(["done", "1"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("1: status todo -> done"),
        "{}",
        stdout(&out)
    );
    assert!(
        stderr(&out).contains("note: task folder is now done/5.1.fix-login"),
        "{}",
        stderr(&out)
    );

    assert_eq!(fx.task_rel(&fx.a, "1"), "done/5.1.fix-login");
    assert_eq!(fx.status(&fx.a, "1"), "done");
    assert!(!fx.a.join(".yman/5.1.fix-login").exists());
    assert_eq!(fx.git(&fx.a, &["-C", ".yman", "status", "--porcelain"]), "");

    // One commit for the move and the field change together, and the subject
    // is the one the format already pins.
    let after = fx.git(&fx.a, &["-C", ".yman", "rev-list", "--count", "HEAD"]);
    assert_eq!(
        before.parse::<u32>().unwrap() + 1,
        after.parse::<u32>().unwrap()
    );
    let subject = fx.git(&fx.a, &["-C", ".yman", "log", "-1", "--format=%s"]);
    assert_eq!(subject, "task(1): set status=todo->done");

    // `yman path` follows the task.
    let path = stdout(&fx.yman(&fx.a).args(["path", "1"]).output().unwrap());
    assert!(path.trim().ends_with("/.yman/done/5.1.fix-login"), "{path}");

    // And the history survives the rename.
    let log = stdout(&fx.yman(&fx.a).args(["log", "1"]).output().unwrap());
    assert!(log.contains("add \"Fix login\""), "{log}");
}

/// Reopening moves the folder back out and takes the emptied directory away.
#[test]
fn reopening_moves_the_folder_back_and_prunes_the_directory() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.v2_config(&fx.a);
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).args(["add", "Other"]).assert().success();

    fx.yman(&fx.a).args(["done", "1"]).assert().success();
    fx.yman(&fx.a).args(["done", "2"]).assert().success();
    assert!(fx.a.join(".yman/done").is_dir());

    fx.yman(&fx.a)
        .args(["set", "1", "--status", "todo"])
        .assert()
        .success();
    assert_eq!(fx.task_rel(&fx.a, "1"), "5.1.fix-login");
    // Task 2 is still in there, so the directory stays.
    assert!(fx.a.join(".yman/done").is_dir());

    fx.yman(&fx.a)
        .args(["set", "2", "--status", "todo"])
        .assert()
        .success();
    assert!(
        !fx.a.join(".yman/done").exists(),
        "empty status dir left behind"
    );
    assert_eq!(fx.git(&fx.a, &["-C", ".yman", "status", "--porcelain"]), "");
}

/// A task added straight into a closed status is born in the right place.
#[test]
fn adding_into_a_closed_status_lands_in_the_archive() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.v2_config(&fx.a);
    let out = fx
        .yman(&fx.a)
        .args(["add", "Already done", "-s", "cancelled"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("cancelled/5.1.already-done"),
        "{}",
        stdout(&out)
    );
    assert_eq!(fx.task_rel(&fx.a, "1"), "cancelled/5.1.already-done");
    assert_eq!(fx.git(&fx.a, &["-C", ".yman", "status", "--porcelain"]), "");
}

/// A version 1 repository never archives, whatever the new binary knows.
#[test]
fn version_one_keeps_every_task_flat() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let out = fx.yman(&fx.a).args(["done", "1"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!stderr(&out).contains("note:"), "{}", stderr(&out));
    assert_eq!(fx.task_rel(&fx.a, "1"), "5.1.fix-login");
}

/// Discussions on an archived task still union-merge. `*/d.md` does not match
/// a path one level deeper, so the pattern had to become `**/d.md`.
#[test]
fn comments_on_an_archived_task_merge_without_conflict() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.v2_config(&fx.a);
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.yman(&fx.a).args(["done", "1"]).assert().success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b).arg("init").assert().success();

    fx.yman(&fx.a)
        .args(["comment", "1", "-m", "from A"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();
    fx.yman(&fx.b)
        .args(["comment", "1", "-m", "from B"])
        .assert()
        .success();

    let out = fx.yman(&fx.b).arg("sync").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let shown = stdout(&fx.yman(&fx.b).args(["show", "1"]).output().unwrap());
    assert!(
        shown.contains("from A") && shown.contains("from B"),
        "{shown}"
    );
}

/// A task in the wrong place — a hand edit, a resolved merge, or a repository
/// that opted into version 2 with closed tasks already on disk — is put right
/// by setting its status to what it already is.
#[test]
fn setting_a_status_relocates_a_misplaced_task() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    // Closed under version 1, so it stayed flat.
    fx.yman(&fx.a).args(["done", "1"]).assert().success();
    assert_eq!(fx.task_rel(&fx.a, "1"), "5.1.fix-login");

    fx.v2_config(&fx.a);
    // Still listed correctly: m.yml is the source of truth, not the path.
    let text = stdout(&fx.yman(&fx.a).args(["ls", "-a"]).output().unwrap());
    assert!(text.contains("done"), "{text}");

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--status", "done"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("1: folder 5.1.fix-login -> done/5.1.fix-login"),
        "{}",
        stdout(&out)
    );
    assert_eq!(fx.task_rel(&fx.a, "1"), "done/5.1.fix-login");
    assert_eq!(fx.git(&fx.a, &["-C", ".yman", "status", "--porcelain"]), "");

    // Nothing left to do the second time.
    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--status", "done"])
        .output()
        .unwrap();
    assert_eq!(stdout(&out).trim(), "no changes");
}

/// `move` is `set --status` without the flag, and rejects the same values.
#[test]
fn move_is_a_positional_set_status() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.v2_config(&fx.a);
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let out = fx
        .yman(&fx.a)
        .args(["move", "1", "blocked"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("1: status todo -> blocked"),
        "{}",
        stdout(&out)
    );
    assert_eq!(fx.status(&fx.a, "1"), "blocked");
    // Not a closed status, so it stays at the top level.
    assert_eq!(fx.task_rel(&fx.a, "1"), "5.1.fix-login");

    let out = fx.yman(&fx.a).args(["move", "1", "nope"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: unknown status \"nope\"; allowed: todo, doing, blocked, done, cancelled"
    );
}

/// `cancel` needs the config to say which status it means.
#[test]
fn cancel_uses_the_configured_status() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    // Version 1 has no cancel status at all.
    let out = fx.yman(&fx.a).args(["cancel", "1"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: no cancel status configured; set statuses.cancel in .yman/config.toml"
    );

    fx.v2_config(&fx.a);
    let out = fx.yman(&fx.a).args(["cancel", "1"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fx.status(&fx.a, "1"), "cancelled");
    assert_eq!(fx.task_rel(&fx.a, "1"), "cancelled/5.1.fix-login");
}

/// `reopen` only applies to a closed task, and says so when it does not.
#[test]
fn reopen_returns_a_closed_task_to_the_default_status() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.v2_config(&fx.a);
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    let out = fx.yman(&fx.a).args(["reopen", "1"]).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        stderr(&out).trim(),
        "error: task 1 is not closed (status \"todo\"); closed statuses: done, cancelled"
    );

    fx.yman(&fx.a).args(["cancel", "1"]).assert().success();
    let out = fx.yman(&fx.a).args(["reopen", "1"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("1: status cancelled -> todo"),
        "{}",
        stdout(&out)
    );
    assert_eq!(fx.task_rel(&fx.a, "1"), "5.1.fix-login");
    assert!(!fx.a.join(".yman/cancelled").exists());
}

/// One `add` can carry everything `set` would otherwise add in a second
/// commit; the values are deduplicated like tags.
#[test]
fn add_sets_assignee_links_and_related_in_one_call() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Other"]).assert().success();
    let before = fx.git(&fx.a, &["rev-list", "--count", "refs/yman/local"]);

    let out = fx
        .yman(&fx.a)
        .args([
            "add",
            "Fix login",
            "-a",
            "claude",
            "--link",
            "https://example.invalid/issues/12",
            "--link",
            "https://example.invalid/issues/12",
            "--relate",
            "1",
            "--relate",
            "1",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));

    let meta = fx.read(&fx.task_dir(&fx.a, "2").join("m.yml"));
    assert!(meta.contains("assignee: claude"), "{meta}");
    assert_eq!(
        meta.matches("https://example.invalid/issues/12").count(),
        1,
        "{meta}"
    );
    let related: Vec<&str> = meta
        .split("related:\n")
        .nth(1)
        .unwrap_or("")
        .lines()
        .take_while(|l| l.starts_with("- "))
        .collect();
    assert_eq!(related.len(), 1, "{meta}");
    assert!(related[0].contains('1'), "{meta}");

    let shown = stdout(&fx.yman(&fx.a).args(["show", "2"]).output().unwrap());
    assert!(shown.contains("assignee: claude"), "{shown}");
    assert!(
        shown.contains("links:    https://example.invalid/issues/12"),
        "{shown}"
    );
    assert!(shown.contains("related:  1"), "{shown}");

    let json = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(json.contains("\"assignee\":\"claude\""), "{json}");

    // Only one commit was made for the whole thing.
    let after = fx.git(&fx.a, &["rev-list", "--count", "refs/yman/local"]);
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() + 1
    );
}

/// `YMAN_ACTOR` names who a comment or attachment came from without touching
/// who git says committed it.
#[test]
fn comment_and_attach_record_the_actor() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let file = fx.a.join("notes.txt");
    fx.write(&file, "hello\n");

    fx.yman(&fx.a)
        .env("YMAN_ACTOR", "  claude  ")
        .args(["comment", "1", "-m", "on it"])
        .assert()
        .success();
    fx.yman(&fx.a)
        .env("YMAN_ACTOR", "claude")
        .args(["attach", "1", file.to_str().unwrap()])
        .assert()
        .success();

    let d = fx.read(&fx.task_dir(&fx.a, "1").join("d.md"));
    assert!(d.contains(" — claude\n"), "{d}");
    let m = fx.read(&fx.task_dir(&fx.a, "1").join("m.yml"));
    assert!(m.contains("by: claude"), "{m}");
    let committer = fx.git(&fx.a, &["log", "-1", "--format=%an", "refs/yman/local"]);
    assert_eq!(committer.trim(), "Test A");

    // Blank is the same as unset: back to user.name.
    fx.yman(&fx.a)
        .env("YMAN_ACTOR", "   ")
        .args(["comment", "1", "-m", "again"])
        .assert()
        .success();
    let d = fx.read(&fx.task_dir(&fx.a, "1").join("d.md"));
    assert!(d.contains(" — Test A\n"), "{d}");
}

/// `done -m` closes and explains in one commit; a bare `set -m` is a valid
/// change on its own; an empty message is refused like `comment`.
#[test]
fn done_with_message_comments_in_one_commit() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let before = fx.git(&fx.a, &["rev-list", "--count", "refs/yman/local"]);

    let out = fx
        .yman(&fx.a)
        .args(["done", "1", "-m", "fixed in 3f2a"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("1: status todo -> done"), "{text}");
    assert!(text.contains("1: commented"), "{text}");

    let after = fx.git(&fx.a, &["rev-list", "--count", "refs/yman/local"]);
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() + 1
    );
    let subject = fx.git(&fx.a, &["log", "-1", "--format=%s", "refs/yman/local"]);
    assert_eq!(subject.trim(), "task(1): set status=todo->done comment");
    let d = fx.read(&fx.task_dir(&fx.a, "1").join("d.md"));
    assert!(d.contains("fixed in 3f2a"), "{d}");

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "-m", "second note"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "1: commented");
    let subject = fx.git(&fx.a, &["log", "-1", "--format=%s", "refs/yman/local"]);
    assert_eq!(subject.trim(), "task(1): set comment");

    let out = fx
        .yman(&fx.a)
        .args(["prio", "1", "3", "-m", "  "])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(stderr(&out).trim(), "error: empty comment");
    // The priority was not applied either: the bail happens before any write.
    assert_eq!(fx.task_rel(&fx.a, "1"), "5.1.fix-login");
}

/// The verbs take several ids: one commit per task, the first failure stops
/// the run with the earlier tasks already committed.
#[test]
fn done_accepts_several_ids() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    for title in ["One", "Two", "Three"] {
        fx.yman(&fx.a).args(["add", title]).assert().success();
    }
    let before = fx.git(&fx.a, &["rev-list", "--count", "refs/yman/local"]);

    let out = fx
        .yman(&fx.a)
        .args(["done", "1", "2", "3", "-m", "batch"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    for id in ["1", "2", "3"] {
        assert!(
            text.contains(&format!("{id}: status todo -> done")),
            "{text}"
        );
        assert!(text.contains(&format!("{id}: commented")), "{text}");
    }
    let after = fx.git(&fx.a, &["rev-list", "--count", "refs/yman/local"]);
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() + 3
    );
    let listed = stdout(&fx.yman(&fx.a).arg("ls").output().unwrap());
    assert_eq!(listed.trim(), "", "closed tasks are hidden: {listed}");

    // A missing id in the middle: the ones before it are done, the ones
    // after are untouched.
    fx.yman(&fx.a).args(["reopen", "1", "2"]).assert().success();
    let out = fx
        .yman(&fx.a)
        .args(["done", "1", "99", "2"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(stderr(&out).trim(), "error: task 99 not found");
    assert_eq!(stdout(&out).trim(), "1: status todo -> done");
    assert_eq!(fx.status(&fx.a, "1"), "done");
    assert_eq!(fx.status(&fx.a, "2"), "todo");
}

/// `move` and `prio` keep their value last, so the single-id form reads as
/// it always did and the list form is unambiguous to the parser.
#[test]
fn move_and_prio_take_ids_then_value() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    for title in ["One", "Two"] {
        fx.yman(&fx.a).args(["add", title]).assert().success();
    }

    let out = fx
        .yman(&fx.a)
        .args(["move", "1", "2", "doing"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fx.status(&fx.a, "1"), "doing");
    assert_eq!(fx.status(&fx.a, "2"), "doing");

    let out = fx
        .yman(&fx.a)
        .args(["prio", "1", "2", "3"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("1: priority 5 -> 3"), "{text}");
    assert!(text.contains("2: priority 5 -> 3"), "{text}");

    // Single-id spelling unchanged.
    let out = fx.yman(&fx.a).args(["move", "1", "todo"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "1: status doing -> todo");
}

/// The `ls` filters compose, `--assignee -` is "nobody", and the limit
/// applies after sorting so `-n 1` is the top of the backlog.
#[test]
fn ls_filters_by_text_assignee_priority_and_limit() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a)
        .args([
            "add",
            "Fix login",
            "-p",
            "2",
            "-a",
            "claude",
            "-m",
            "The OAuth flow breaks",
        ])
        .assert()
        .success();
    fx.yman(&fx.a)
        .args(["add", "Write docs", "-p", "5"])
        .assert()
        .success();
    fx.yman(&fx.a)
        .args(["add", "Login page copy", "-p", "5", "-a", "ivan"])
        .assert()
        .success();

    let ls = |args: &[&str]| -> Vec<String> {
        let mut full = vec!["ls"];
        full.extend_from_slice(args);
        stdout(&fx.yman(&fx.a).args(&full).output().unwrap())
            .lines()
            .map(|l| l.split_whitespace().nth(1).unwrap().to_string())
            .collect()
    };

    assert_eq!(ls(&["-q", "LOGIN"]), ["1", "3"], "title match, any case");
    assert_eq!(ls(&["-q", "oauth"]), ["1"], "body match");
    assert_eq!(ls(&["--assignee", "claude"]), ["1"]);
    assert_eq!(ls(&["--assignee", "-"]), ["2"]);
    assert_eq!(ls(&["-p", "5"]), ["2", "3"]);
    assert_eq!(ls(&["-n", "1"]), ["1"], "after sorting: priority 2 first");
    assert_eq!(ls(&["-n", "0"]), Vec::<String>::new());
    assert_eq!(ls(&["-q", "login", "-p", "5", "--assignee", "ivan"]), ["3"]);
    assert_eq!(ls(&["-q", "nothing here"]), Vec::<String>::new());
}

/// `show -n` keeps the newest entries and says how many it dropped; the
/// default output does not change.
#[test]
fn show_limits_the_discussion() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    for n in 1..=3 {
        fx.yman(&fx.a)
            .args(["comment", "1", "-m", &format!("note {n}")])
            .assert()
            .success();
    }

    let full = stdout(&fx.yman(&fx.a).args(["show", "1"]).output().unwrap());
    assert!(full.contains("\ndiscussion:\n"), "{full}");
    assert!(full.contains("note 1") && full.contains("note 3"), "{full}");

    let last2 = stdout(
        &fx.yman(&fx.a)
            .args(["show", "1", "-n", "2"])
            .output()
            .unwrap(),
    );
    assert!(last2.contains("\ndiscussion (last 2 of 3):\n"), "{last2}");
    assert!(!last2.contains("note 1"), "{last2}");
    assert!(
        last2.contains("note 2") && last2.contains("note 3"),
        "{last2}"
    );

    let none = stdout(
        &fx.yman(&fx.a)
            .args(["show", "1", "-n", "0"])
            .output()
            .unwrap(),
    );
    assert!(!none.contains("discussion"), "{none}");

    let big = stdout(
        &fx.yman(&fx.a)
            .args(["show", "1", "-n", "9"])
            .output()
            .unwrap(),
    );
    assert_eq!(big, full, "a limit above the count changes nothing");
}

/// A body can come from a file or stdin at creation time.
#[test]
fn add_body_from_file_and_stdin() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    let src = fx.a.join("body.md");
    fx.write(&src, "From a file.\n\nSecond paragraph.\n");

    fx.yman(&fx.a)
        .args(["add", "Filed", "--body-file", src.to_str().unwrap()])
        .assert()
        .success();
    let md = fx.read(&fx.task_dir(&fx.a, "1").join("t.md"));
    assert_eq!(md, "# Filed\n\nFrom a file.\n\nSecond paragraph.\n");

    fx.yman(&fx.a)
        .args(["add", "Piped", "--body-file", "-"])
        .write_stdin("from stdin\n")
        .assert()
        .success();
    let md = fx.read(&fx.task_dir(&fx.a, "2").join("t.md"));
    assert_eq!(md, "# Piped\n\nfrom stdin\n");

    // `-m` and `--body-file` exclude each other at the parser.
    let out = fx
        .yman(&fx.a)
        .args(["add", "Both", "-m", "x", "--body-file", "-"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

/// `set --body` rewrites t.md in place: no rename, `no changes` on a repeat,
/// and an empty string clears the body.
#[test]
fn set_body_updates_t_md_without_renaming() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a)
        .args(["add", "Fix login", "-m", "old body"])
        .assert()
        .success();

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--body", "new body\n"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "1: body updated");
    let subject = fx.git(&fx.a, &["log", "-1", "--format=%s", "refs/yman/local"]);
    assert_eq!(subject.trim(), "task(1): set body");
    assert_eq!(fx.task_rel(&fx.a, "1"), "5.1.fix-login");
    let md = fx.read(&fx.task_dir(&fx.a, "1").join("t.md"));
    assert_eq!(md, "# Fix login\n\nnew body\n");

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--body", "new body"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "no changes");

    let src = fx.a.join("body.md");
    fx.write(&src, "filed body\n");
    let out = fx
        .yman(&fx.a)
        .args([
            "set",
            "1",
            "--body-file",
            src.to_str().unwrap(),
            "-m",
            "rewrote",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("1: body updated") && text.contains("1: commented"),
        "{text}"
    );
    let subject = fx.git(&fx.a, &["log", "-1", "--format=%s", "refs/yman/local"]);
    assert_eq!(subject.trim(), "task(1): set body comment");

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--body", ""])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "1: body updated");
    assert_eq!(
        fx.read(&fx.task_dir(&fx.a, "1").join("t.md")),
        "# Fix login\n"
    );
}

/// An unreadable `--body-file` fails before anything is written.
#[test]
fn set_body_file_missing_is_an_error() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let before = fx.git(&fx.a, &["rev-list", "--count", "refs/yman/local"]);

    let out = fx
        .yman(&fx.a)
        .args(["set", "1", "--body-file", "nope.md", "--priority", "1"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.starts_with("error: cannot read nope.md: "), "{err}");
    let after = fx.git(&fx.a, &["rev-list", "--count", "refs/yman/local"]);
    assert_eq!(before, after);
    assert_eq!(fx.task_rel(&fx.a, "1"), "5.1.fix-login");
}

/// With no editor configured and no terminal, `edit` refuses instead of
/// leaving `vi` waiting on a pipe. An explicit `$EDITOR` is still honoured.
#[test]
fn edit_without_editor_or_tty_fails_fast() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    for args in [
        vec!["edit", "1"],
        vec!["comment", "1", "-e"],
        vec!["add", "Another", "-e"],
    ] {
        let out = fx
            .yman(&fx.a)
            .env_remove("EDITOR")
            .env_remove("VISUAL")
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert_eq!(
            stderr(&out).trim(),
            "error: no terminal for vi; set $EDITOR, or use -m / --body-file",
            "{args:?}"
        );
    }
    assert!(!fx.has_task(&fx.a, "2"), "add -e must not create anything");

    // The fixture's EDITOR=true still works on a pipe.
    let out = fx.yman(&fx.a).args(["edit", "1"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "no changes");
}

/// Every command that looks an id up exits 4 on a missing one; a folder that
/// exists but is broken is a repository problem and stays at 1.
#[test]
fn missing_id_exits_4_broken_folder_exits_1() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();

    for args in [
        vec!["set", "99", "--priority", "1"],
        vec!["path", "99"],
        vec!["comment", "99", "-m", "x"],
        vec!["rm", "99", "-f"],
    ] {
        let out = fx.yman(&fx.a).args(&args).output().unwrap();
        assert_eq!(out.status.code(), Some(4), "{args:?}");
        assert_eq!(stderr(&out).trim(), "error: task 99 not found", "{args:?}");
    }

    fx.write(&fx.task_dir(&fx.a, "1").join("m.yml"), "status: [\n");
    let out = fx.yman(&fx.a).args(["show", "1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).starts_with("error: task 1 is broken: "),
        "{}",
        stderr(&out)
    );
}

/// `--help` describes every flag and ends by pointing at the guide.
#[test]
fn help_documents_flags_and_points_at_guide() {
    let fx = Fx::new();
    let top = stdout(&fx.yman(&fx.a).arg("--help").output().unwrap());
    assert!(top.contains("Scripts and agents:  yman guide"), "{top}");
    assert!(top.contains("4 no such task"), "{top}");

    let set = stdout(&fx.yman(&fx.a).args(["set", "--help"]).output().unwrap());
    for (flag, doc) in [
        ("--status <STATUS>", "New status"),
        ("--title <TITLE>", "New title"),
        ("--unrelate <ID>", "Remove a related task id"),
        ("--body-file <PATH>", "Replace the body with a file"),
    ] {
        let line = set.lines().find(|l| l.trim_start().starts_with(flag));
        assert!(
            line.is_some_and(|l| l.contains(doc)),
            "{flag}: {line:?}\n{set}"
        );
    }
    let ls = stdout(&fx.yman(&fx.a).args(["ls", "--help"]).output().unwrap());
    assert!(ls.contains("Print JSON instead of the table"), "{ls}");
}

/// `guide` prints docs/agents.md byte for byte, from a directory that is
/// not a git repository at all.
#[test]
fn guide_needs_no_repository() {
    let fx = Fx::new();
    let nowhere = tempfile::tempdir().unwrap();
    let out = fx.yman(nowhere.path()).arg("guide").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let expected =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/agents.md")).unwrap();
    assert_eq!(stdout(&out), expected);
    assert_eq!(stderr(&out), "");
    assert!(expected.lines().count() <= 60, "agents.md must stay short");
}

/// `show --json` is one object: the header fields, the body, the attachments
/// and the discussion, with `-n` trimming the discussion but not the count.
#[test]
fn show_json_carries_the_whole_task() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Other"]).assert().success();
    fx.yman(&fx.a)
        .args([
            "add",
            "Fix login",
            "-m",
            "Body text",
            "-a",
            "claude",
            "-t",
            "ui",
            "--link",
            "https://example.test/1",
            "--relate",
            "1",
        ])
        .assert()
        .success();
    let src = fx.tmp.path().join("screenshot.png");
    fx.write(&src, "not really a png");
    fx.yman(&fx.a)
        .args(["attach", "2", src.to_str().unwrap()])
        .assert()
        .success();
    for n in 1..=2 {
        fx.yman(&fx.a)
            .args(["comment", "2", "-m", &format!("note {n}")])
            .assert()
            .success();
    }

    let text = stdout(
        &fx.yman(&fx.a)
            .args(["show", "2", "--json"])
            .output()
            .unwrap(),
    );
    assert!(
        text.starts_with('{') && text.trim_end().ends_with('}'),
        "{text}"
    );
    assert!(text.contains("\"id\":\"2\""), "{text}");
    assert!(text.contains("\"title\":\"Fix login\""), "{text}");
    assert!(text.contains("\"assignee\":\"claude\""), "{text}");
    assert!(text.contains("\"tags\":[\"ui\"]"), "{text}");
    assert!(
        text.contains("\"links\":[\"https://example.test/1\"]"),
        "{text}"
    );
    assert!(text.contains("\"related\":[\"1\"]"), "{text}");
    assert!(text.contains("\"body\":\"Body text\""), "{text}");
    assert!(text.contains("\"dir\":\"5.2.fix-login\""), "{text}");
    assert!(text.contains("\"name\":\"screenshot.png\""), "{text}");
    assert!(text.contains("\"text\":\"note 1\""), "{text}");
    assert!(text.contains("\"discussion_total\":2"), "{text}");

    // A task with nothing on it still has every key, as an empty array.
    let bare = stdout(
        &fx.yman(&fx.a)
            .args(["show", "1", "--json"])
            .output()
            .unwrap(),
    );
    assert!(bare.contains("\"assignee\":null"), "{bare}");
    assert!(bare.contains("\"tags\":[]"), "{bare}");
    assert!(bare.contains("\"links\":[]"), "{bare}");
    assert!(bare.contains("\"attachments\":[]"), "{bare}");
    assert!(bare.contains("\"discussion\":[]"), "{bare}");
    assert!(bare.contains("\"discussion_total\":0"), "{bare}");

    // `-n` trims the entries; the total still says how many there were.
    let last = stdout(
        &fx.yman(&fx.a)
            .args(["show", "2", "--json", "-n", "1"])
            .output()
            .unwrap(),
    );
    assert!(!last.contains("note 1"), "{last}");
    assert!(last.contains("\"text\":\"note 2\""), "{last}");
    assert!(last.contains("\"discussion_total\":2"), "{last}");

    // An unknown id is still exit 4, and says nothing on stdout.
    let out = fx
        .yman(&fx.a)
        .args(["show", "404", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(stdout(&out), "");
}

/// `ls --json` carries the links and the related ids, so a script does not
/// need a `show` per row.
#[test]
fn ls_json_carries_links_and_related() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a).args(["add", "Other"]).assert().success();
    fx.yman(&fx.a)
        .args([
            "add",
            "Fix login",
            "--link",
            "https://example.test/1",
            "--relate",
            "1",
        ])
        .assert()
        .success();

    let text = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(
        text.contains("\"assignee\":null,\"links\":[\"https://example.test/1\"],\"related\":[\"1\"],\"created\":"),
        "{text}"
    );
    assert!(text.contains("\"links\":[],\"related\":[],"), "{text}");
}

/// `status --json` reports the same facts as the text form, before and after
/// the remote ref exists.
#[test]
fn status_json_reports_refs_and_counts() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();

    // `init` publishes the ref but does not fetch it back.
    fx.git(&fx.a, &["update-ref", "-d", "refs/yman/remote"]);
    let text = stdout(&fx.yman(&fx.a).args(["status", "--json"]).output().unwrap());
    assert!(
        text.contains("\"remote\":{\"ref\":\"refs/tasks/main\",\"fetched\":false,\"head\":null,\"ahead\":0,\"behind\":0}"),
        "{text}"
    );
    assert!(text.contains("\"refresh\":\"lazy\""), "{text}");
    assert!(text.contains("\"hooks\":false"), "{text}");
    assert!(text.contains("\"worktree\":{\"dirty\":0}"), "{text}");
    assert!(
        text.contains("\"merge\":{\"in_progress\":false,\"unmerged\":[]}"),
        "{text}"
    );
    assert!(
        text.contains(
            "\"tasks\":{\"by_status\":{\"todo\":0,\"doing\":0,\"done\":0},\"other\":0,\"broken\":0}"
        ),
        "{text}"
    );

    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    fx.git(&fx.a, &["fetch", "origin"]);
    let text = stdout(&fx.yman(&fx.a).args(["status", "--json"]).output().unwrap());
    assert!(text.contains("\"fetched\":true"), "{text}");
    assert!(text.contains("\"ahead\":1,\"behind\":0"), "{text}");
    assert!(text.contains("\"todo\":1"), "{text}");

    // A dirty worktree is counted, not described.
    fx.write(
        &fx.a.join(".yman/5.1.fix-login/t.md"),
        "# Fix login\n\nedit\n",
    );
    let text = stdout(&fx.yman(&fx.a).args(["status", "--json"]).output().unwrap());
    assert!(text.contains("\"worktree\":{\"dirty\":1}"), "{text}");
}

/// What reading refs off disk buys: a read-only command in a repository that
/// needs no refresh costs one `git` process — the `rev-parse` in `discover`.
/// Everything else it used to ask git is a file it can read itself.
#[test]
#[cfg(unix)]
fn a_read_only_command_spawns_at_most_one_git() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a)
        .args(["add", "first task"])
        .assert()
        .success();
    fx.yman(&fx.a).arg("sync").assert().success();

    assert_eq!(fx.git_spawns(&fx.a, &["ls"]), 1);
    assert_eq!(fx.git_spawns(&fx.a, &["show", "1"]), 1);

    // Packed refs are the other on-disk shape, and no fixture reaches it
    // by default.
    fx.git(&fx.a, &["pack-refs", "--all"]);
    assert_eq!(fx.git_spawns(&fx.a, &["ls"]), 1);
    fx.yman(&fx.a)
        .args(["ls"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first task"));

    // Unpushed work is the other steady state. It costs the one ancestry
    // question that cannot be answered from a ref file, and nothing more:
    // the policy is not consulted for a repository that is not behind.
    fx.yman(&fx.a)
        .args(["add", "second task"])
        .assert()
        .success();
    assert_eq!(fx.git_spawns(&fx.a, &["ls"]), 2);
}

/// A tag is a token: trimmed, lowercased, and refused when it carries the
/// characters that would break the TAGS column or `-t`.
#[test]
fn tags_are_normalized_on_add_and_set() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a)
        .args(["add", "Fix login", "-t", "UI", "-t", "  ui  ", "-t", "Auth"])
        .assert()
        .success();
    let text = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(text.contains("\"tags\":[\"ui\",\"auth\"]"), "{text}");

    // `--untag` folds the same way, so removing `UI` removes the stored `ui`.
    fx.yman(&fx.a)
        .args(["set", "1", "--untag", "UI", "--tag", "BUG"])
        .assert()
        .success();
    let text = stdout(&fx.yman(&fx.a).args(["ls", "--json"]).output().unwrap());
    assert!(text.contains("\"tags\":[\"auth\",\"bug\"]"), "{text}");
}

#[test]
fn add_refuses_a_tag_that_is_empty_or_carries_a_separator() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();

    let out = fx
        .yman(&fx.a)
        .args(["add", "Fix login", "-t", "  "])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stderr(&out).trim_end(), "error: tag must not be empty");

    for bad in ["a,b", "needs review"] {
        let out = fx
            .yman(&fx.a)
            .args(["add", "Fix login", "-t", bad])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(
            stderr(&out).trim_end(),
            format!(
                "error: invalid tag \"{bad}\"; tags must not contain whitespace, \
                 commas or control characters"
            )
        );
    }

    // Nothing was minted and nothing was written: the check runs before the
    // id and the folder exist.
    assert!(fx.task_dirs(&fx.a).is_empty(), "{:?}", fx.task_dirs(&fx.a));
    fx.yman(&fx.a).args(["add", "Fix login"]).assert().success();
    let dirs = fx.task_dirs(&fx.a);
    assert_eq!(
        dirs,
        [std::path::PathBuf::from("5.1.fix-login")],
        "{dirs:?}"
    );
}

#[test]
fn set_refuses_an_invalid_tag_on_either_side() {
    let fx = Fx::new();
    fx.yman(&fx.a).arg("init").assert().success();
    fx.yman(&fx.a)
        .args(["add", "Fix login", "-t", "ui"])
        .assert()
        .success();
    let before = fx.git(&fx.a.join(".yman"), &["rev-parse", "HEAD"]);

    for args in [["set", "1", "--tag", "a,b"], ["set", "1", "--untag", "a,b"]] {
        let out = fx.yman(&fx.a).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert!(
            stderr(&out).contains("invalid tag \"a,b\""),
            "{}",
            stderr(&out)
        );
    }
    assert_eq!(fx.git(&fx.a.join(".yman"), &["rev-parse", "HEAD"]), before);
}
