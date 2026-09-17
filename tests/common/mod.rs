//! Two clones of one bare remote, on local disk. Nothing here touches the
//! network or the developer's real git configuration.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

pub struct Fx {
    pub tmp: TempDir,
    pub remote: PathBuf,
    pub a: PathBuf,
    pub b: PathBuf,
    home: PathBuf,
    gitconfig: PathBuf,
}

impl Fx {
    pub fn new() -> Fx {
        let tmp = TempDir::new().expect("tempdir");
        let base = tmp.path().to_path_buf();
        let home = base.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let gitconfig = home.join(".gitconfig");
        std::fs::write(
            &gitconfig,
            "[user]\n\tname = Test\n\temail = test@example.invalid\n\
             [init]\n\tdefaultBranch = main\n[advice]\n\tdetachedHead = false\n",
        )
        .unwrap();

        let fx = Fx {
            remote: base.join("remote.git"),
            a: base.join("a"),
            b: base.join("b"),
            tmp,
            home,
            gitconfig,
        };

        fx.git_at(&base, &["init", "--bare", "-b", "main", "remote.git"]);
        // Seed the default branch so cloning produces a normal checkout.
        let seed = base.join("seed");
        fx.git_at(&base, &["clone", fx.remote.to_str().unwrap(), "seed"]);
        std::fs::write(seed.join("README.md"), "# project\n").unwrap();
        fx.git_at(&seed, &["add", "-A"]);
        fx.git_at(&seed, &["commit", "-m", "initial"]);
        fx.git_at(&seed, &["push", "origin", "main"]);
        std::fs::remove_dir_all(&seed).unwrap();

        for (dir, who) in [(&fx.a, "A"), (&fx.b, "B")] {
            let name = dir.file_name().unwrap().to_str().unwrap().to_string();
            fx.git_at(&base, &["clone", fx.remote.to_str().unwrap(), &name]);
            fx.git_at(dir, &["config", "user.name", &format!("Test {who}")]);
            fx.git_at(
                dir,
                &[
                    "config",
                    "user.email",
                    &format!("{}@example.invalid", who.to_lowercase()),
                ],
            );
        }
        fx
    }

    fn env(&self, cmd: &mut Command) {
        cmd.env("HOME", &self.home)
            .env("GIT_CONFIG_GLOBAL", &self.gitconfig)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("EDITOR", "true")
            .env("VISUAL", "")
            .env("TERM", "dumb")
            .env_remove("YMAN_AUTHOR")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE");
    }

    /// A `yman` invocation in `dir`, with the sandboxed environment applied.
    pub fn yman(&self, dir: &Path) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin("yman").expect("yman binary");
        cmd.current_dir(dir)
            .env("HOME", &self.home)
            .env("GIT_CONFIG_GLOBAL", &self.gitconfig)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("EDITOR", "true")
            .env("VISUAL", "")
            .env("TERM", "dumb")
            .env_remove("YMAN_AUTHOR")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE");
        cmd
    }

    /// Run git, panic on failure, return trimmed stdout.
    pub fn git(&self, dir: &Path, args: &[&str]) -> String {
        self.git_at(dir, args)
    }

    fn git_at(&self, dir: &Path, args: &[&str]) -> String {
        let mut cmd = Command::new("git");
        cmd.current_dir(dir).args(args);
        self.env(&mut cmd);
        let out = cmd.output().expect("git runs");
        if !out.status.success() {
            panic!(
                "git {:?} in {} failed:\n{}",
                args,
                dir.display(),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    }

    /// Same as `git`, but returns the whole output instead of panicking.
    pub fn git_try(&self, dir: &Path, args: &[&str]) -> (bool, String, String) {
        let mut cmd = Command::new("git");
        cmd.current_dir(dir).args(args);
        self.env(&mut cmd);
        let out = cmd.output().expect("git runs");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
            String::from_utf8_lossy(&out.stderr).trim_end().to_string(),
        )
    }

    /// Rewrite `.yman/config.toml` and commit it. There is no CLI for this:
    /// the status roles and the terminal set are a hand edit by design.
    pub fn set_config(&self, clone: &Path, text: &str) {
        let ydir = clone.join(".yman");
        std::fs::write(ydir.join("config.toml"), text).expect("write config.toml");
        self.git(&ydir, &["add", "--", "config.toml"]);
        self.git(
            &ydir,
            &["commit", "-q", "--no-verify", "-m", "yman: config"],
        );
    }

    /// A version 2 config with five statuses, two of them closed.
    pub fn v2_config(&self, clone: &Path) {
        self.set_config(
            clone,
            "version = 2\n\n\
             [ids]\nscheme = \"seq\"\nrandom_len = 4\n\n\
             [statuses]\n\
             list = [\"todo\", \"doing\", \"blocked\", \"done\", \"cancelled\"]\n\
             default = \"todo\"\n\
             start = \"doing\"\n\
             done = \"done\"\n\
             cancel = \"cancelled\"\n\
             terminal = [\"done\", \"cancelled\"]\n\n\
             [priorities]\ndefault = 5\n\n\
             [slug]\nmax_bytes = 200\n",
        );
    }

    /// Every task folder in a clone, top level and one directory down, as
    /// paths relative to `.yman`.
    pub fn task_dirs(&self, clone: &Path) -> Vec<PathBuf> {
        let ydir = clone.join(".yman");
        let mut out: Vec<PathBuf> = Vec::new();
        // `{priority}.{id}.{slug}`: a digit, then at least two more parts.
        let is_task =
            |n: &str| n.starts_with(|c: char| c.is_ascii_digit()) && n.split('.').count() >= 3;
        for e in std::fs::read_dir(&ydir).expect("read .yman") {
            let e = e.unwrap();
            if !e.file_type().unwrap().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if is_task(&name) {
                out.push(PathBuf::from(&name));
                continue;
            }
            if name.starts_with('.') {
                continue;
            }
            // A status directory: archived tasks live one level down.
            for nested in std::fs::read_dir(e.path()).expect("read status dir") {
                let nested = nested.unwrap();
                if !nested.file_type().unwrap().is_dir() {
                    continue;
                }
                let leaf = nested.file_name().to_string_lossy().into_owned();
                if is_task(&leaf) {
                    out.push(PathBuf::from(&name).join(leaf));
                }
            }
        }
        out.sort();
        out
    }

    /// The folder of a task, by id, wherever it currently sits.
    pub fn task_dir(&self, clone: &Path, id: &str) -> PathBuf {
        let ydir = clone.join(".yman");
        let mut hits: Vec<PathBuf> = self
            .task_dirs(clone)
            .into_iter()
            .filter(|rel| {
                let leaf = rel.file_name().unwrap().to_string_lossy().into_owned();
                leaf.split('.').nth(1) == Some(id)
            })
            .map(|rel| ydir.join(rel))
            .collect();
        match hits.len() {
            1 => hits.pop().unwrap(),
            0 => panic!("no task {id} in {}", ydir.display()),
            _ => panic!("several folders for task {id}: {hits:?}"),
        }
    }

    /// Path of a task relative to `.yman`, e.g. `done/5.1.fix-login`.
    pub fn task_rel(&self, clone: &Path, id: &str) -> String {
        self.task_dir(clone, id)
            .strip_prefix(clone.join(".yman"))
            .expect("inside .yman")
            .to_string_lossy()
            .replace('\\', "/")
    }

    pub fn has_task(&self, clone: &Path, id: &str) -> bool {
        self.task_dirs(clone).iter().any(|rel| {
            rel.file_name()
                .map(|n| n.to_string_lossy().split('.').nth(1) == Some(id))
                .unwrap_or(false)
        })
    }

    /// An `$EDITOR` that overwrites whatever file it is handed with
    /// `content`, so editor-driven commands can be driven from a test.
    pub fn editor_writing(&self, name: &str, content: &str) -> PathBuf {
        self.editor_script(
            name,
            &format!("cat > \"$1\" <<'YMAN_FIXTURE_EOF'\n{content}YMAN_FIXTURE_EOF\n"),
        )
    }

    /// An `$EDITOR` that leaves the file alone and exits with `code`.
    pub fn editor_failing(&self, name: &str, code: i32) -> PathBuf {
        self.editor_script(name, &format!("exit {code}\n"))
    }

    fn editor_script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.tmp.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    pub fn write(&self, path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    pub fn read(&self, path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    /// Title of a task, read straight off disk.
    pub fn title(&self, clone: &Path, id: &str) -> String {
        let md = self.read(&self.task_dir(clone, id).join("t.md"));
        md.lines()
            .find(|l| l.starts_with("# "))
            .map(|l| l[2..].to_string())
            .unwrap_or_default()
    }

    /// `status` field of a task's m.yml.
    pub fn status(&self, clone: &Path, id: &str) -> String {
        let yml = self.read(&self.task_dir(clone, id).join("m.yml"));
        yml.lines()
            .find_map(|l| l.strip_prefix("status:"))
            .map(|v| v.trim().trim_matches('"').to_string())
            .unwrap_or_default()
    }
}

/// stdout of a successful yman run.
pub fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}
