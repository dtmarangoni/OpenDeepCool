//! The guard in throwaway repositories (Phase 5 plan, Task 10; solution review P2-01 to P2-04):
//! hook files that start the built guard, a local bare remote, and a made-up sensitive list. Git
//! runs with a cleared environment and a home of its own, so neither the developer's git
//! settings nor an outer hook's variables reach these repositories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const NO_REPLY: &str = "1+dev@users.noreply.github.com";
const LISTED: &str = "SecretSerial123";
const LISTED_NAME: &str = "Dev Realname";

/// A repository with the guard's hooks, the made-up list, and a bare remote `origin`.
struct Repo {
    dir: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Repo {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("guard")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        let (dir, home, hooks) = (root.join("repo"), root.join("home"), root.join("hooks"));
        for d in [&dir, &home, &hooks] {
            fs::create_dir_all(d).expect("a folder");
        }
        for hook in ["pre-commit", "commit-msg", "pre-push"] {
            let path = hooks.join(hook);
            let starter = format!(
                "#!/bin/sh\nexec '{}' {hook} \"$@\"\n",
                env!("CARGO_BIN_EXE_odc-guard")
            );
            fs::write(&path, starter).expect("a hook file");
            make_executable(&path);
        }
        let repo = Repo { dir, home };
        repo.ok(&["init", "-q", "-b", "main"]);
        repo.ok(&["config", "user.name", "Dev"]);
        repo.ok(&["config", "user.email", NO_REPLY]);
        repo.ok(&["config", "core.hooksPath", hooks.to_str().expect("UTF-8")]);
        let remote = root.join("remote.git");
        let remote = remote.to_str().expect("UTF-8");
        repo.ok(&["init", "-q", "--bare", remote]);
        repo.ok(&["remote", "add", "origin", remote]);
        repo.write(
            "docs/local/machine.md",
            &format!(
                "<!-- guard:sensitive-begin -->\n- {LISTED}\n- {LISTED_NAME}\n<!-- guard:sensitive-end -->\n"
            ),
        );
        repo
    }

    fn write(&self, path: &str, content: &str) {
        self.write_bytes(path, content.as_bytes());
    }

    fn write_bytes(&self, path: &str, content: &[u8]) {
        let path = self.dir.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
        fs::write(path, content).expect("a file");
    }

    fn git(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = Command::new("git");
        command
            .args(args)
            .current_dir(&self.dir)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &self.home)
            .env("GIT_CONFIG_NOSYSTEM", "1");
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().expect("git runs")
    }

    fn ok(&self, args: &[&str]) {
        let out = self.git(args, &[]);
        assert!(out.status.success(), "git {args:?}: {}", said(&out));
    }

    /// Commits a new file and returns git's output.
    fn commit(&self, file: &str, content: &str, args: &[&str], env: &[(&str, &str)]) -> Output {
        self.write(file, content);
        self.ok(&["add", file]);
        let mut all = vec!["commit", "-q", "-m", "a change"];
        all.extend_from_slice(args);
        self.git(&all, env)
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("an executable hook");
}

fn said(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

fn refused(out: &Output, reason: &str) {
    assert!(
        !out.status.success(),
        "accepted, expected {reason:?}: {}",
        said(out)
    );
    assert!(
        said(out).contains(reason),
        "expected {reason:?}: {}",
        said(out)
    );
}

#[test]
fn a_clean_commit_and_its_push_are_accepted() {
    let repo = Repo::new("clean");
    let out = repo.commit("README.txt", "hello\n", &[], &[]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("no Rust checks"), "{}", said(&out));
    let out = repo.git(&["push", "-q", "origin", "main"], &[]);
    assert!(out.status.success(), "{}", said(&out));
}

/// A way to set the identity: its name, the commit's arguments, and its environment.
type IdentityCase<'a> = (&'a str, Vec<&'a str>, Vec<(&'a str, &'a str)>);

#[test]
fn another_identity_is_refused_at_commit_however_it_is_set() {
    let wrong = "x@example.com";
    let repo = Repo::new("identity");
    let author = format!("X <{wrong}>");
    let cases: [IdentityCase<'_>; 6] = [
        ("--author", vec!["--author", &author], vec![]),
        (
            "GIT_AUTHOR_EMAIL",
            vec![],
            vec![("GIT_AUTHOR_EMAIL", wrong)],
        ),
        (
            "GIT_COMMITTER_EMAIL",
            vec![],
            vec![("GIT_COMMITTER_EMAIL", wrong)],
        ),
        (
            "GIT_AUTHOR_NAME",
            vec![],
            vec![("GIT_AUTHOR_NAME", LISTED_NAME)],
        ),
        (
            "GIT_COMMITTER_NAME",
            vec![],
            vec![("GIT_COMMITTER_NAME", LISTED_NAME)],
        ),
        ("-c author.email", vec![], vec![]),
    ];
    for (i, (case, args, env)) in cases.into_iter().enumerate() {
        let file = format!("file{i}.txt");
        let out = if case == "-c author.email" {
            repo.write(&file, "x\n");
            repo.ok(&["add", &file]);
            repo.git(
                &[
                    "-c",
                    &format!("author.email={wrong}"),
                    "commit",
                    "-q",
                    "-m",
                    "m",
                ],
                &[],
            )
        } else {
            repo.commit(&file, "x\n", &args, &env)
        };
        let reason = if case.ends_with("NAME") {
            "contains a sensitive value"
        } else {
            "isn't the GitHub no-reply address"
        };
        assert!(!out.status.success(), "{case}: accepted: {}", said(&out));
        assert!(said(&out).contains(reason), "{case}: {}", said(&out));
        repo.ok(&["reset", "-q"]);
    }
}

#[test]
fn a_binary_a_listed_value_or_a_local_file_is_refused_at_commit() {
    let repo = Repo::new("files");
    repo.write_bytes("img.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
    repo.ok(&["add", "img.png"]);
    refused(
        &repo.git(&["commit", "-q", "-m", "m"], &[]),
        "img.png: a binary file",
    );
    repo.ok(&["reset", "-q"]);

    let out = repo.commit(
        "notes.txt",
        &format!("serial {}\n", LISTED.to_uppercase()),
        &[],
        &[],
    );
    refused(&out, "notes.txt contains a sensitive value");
    repo.ok(&["reset", "-q"]);

    repo.ok(&["add", "-f", "docs/local/machine.md"]);
    refused(
        &repo.git(&["commit", "-q", "-m", "m"], &[]),
        "a local-only folder",
    );
}

#[test]
fn a_listed_value_in_the_message_is_refused_at_commit() {
    let repo = Repo::new("message");
    repo.write("a.txt", "x\n");
    repo.ok(&["add", "a.txt"]);
    let out = repo.git(&["commit", "-q", "-m", &format!("fix for {LISTED}")], &[]);
    refused(&out, "the commit message contains a sensitive value");
}

#[test]
fn what_skipped_pre_commit_is_refused_at_push() {
    let repo = Repo::new("push-identity");
    assert!(repo.commit("a.txt", "x\n", &[], &[]).status.success());
    let out = repo.commit(
        "b.txt",
        "y\n",
        &["--no-verify"],
        &[("GIT_AUTHOR_EMAIL", "x@example.com")],
    );
    assert!(
        out.status.success(),
        "--no-verify skips the hook: {}",
        said(&out)
    );
    refused(
        &repo.git(&["push", "-q", "origin", "main"], &[]),
        "isn't the GitHub no-reply address",
    );

    let repo = Repo::new("push-value");
    assert!(repo.commit("a.txt", "x\n", &[], &[]).status.success());
    repo.ok(&["push", "-q", "origin", "main"]);
    let out = repo.commit("c.txt", &format!("{LISTED}\n"), &["--no-verify"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    refused(
        &repo.git(&["push", "-q", "origin", "main"], &[]),
        "c.txt contains a sensitive value",
    );
}

#[test]
fn a_tag_whose_message_holds_a_listed_value_is_refused_at_push() {
    let repo = Repo::new("push-tag");
    assert!(repo.commit("a.txt", "x\n", &[], &[]).status.success());
    repo.ok(&["push", "-q", "origin", "main"]);
    repo.ok(&["tag", "-a", "v0.0.1", "-m", &format!("release {LISTED}")]);
    refused(
        &repo.git(&["push", "-q", "origin", "v0.0.1"], &[]),
        "refs/tags/v0.0.1's message contains a sensitive value",
    );
}
