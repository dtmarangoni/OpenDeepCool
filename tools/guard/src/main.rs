//! `odc-guard`: OpenDeepCool's commit guard (AGENTS.md rules 6, 10 and 13), started by git's hook
//! files in `.githooks/`, and CI's privacy check. It rejects what would publish vendor material,
//! personal or machine data, or binaries, and from Phase 5 it runs the Rust checks for commits
//! that touch code. It's a dev-only tool, never shipped.
//!
//! The Rust checks run on the working tree, not the staged snapshot, so unrelated unstaged
//! changes must not be present when committing.

mod checks;
mod git;

use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use checks::{Sensitive, check_file, check_identity, check_message, runs_rust_checks};
use git::{binary_paths, git, git_text, nul_fields};

const USAGE: &str = "usage: odc-guard <hook>

  pre-commit                 the staged files, the identity git will record, and the
                             Rust checks when code, frames, the contract, the guard or CI change
  commit-msg <file>          the commit message
  pre-push <remote> <url>    every commit and tag the push adds (refs on standard input)
  history                    CI: every commit's identity, and no binary file in the tree

git starts the first three through .githooks/ (git config core.hooksPath .githooks).
The Rust checks run on the working tree: commit with no unrelated unstaged changes.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["pre-commit"] => pre_commit(),
        ["commit-msg", file] => commit_msg(Path::new(file)),
        ["pre-push", remote, _url] => pre_push(remote),
        ["history"] => history(),
        ["-h" | "--help"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(problems) if problems.is_empty() => ExitCode::SUCCESS,
        Ok(problems) => {
            for problem in &problems {
                eprintln!("blocked: {problem}");
            }
            eprintln!(
                "\nRejected: vendor material, personal or machine-specific data and binaries never \
                 go into this public repository, and code goes in only with its checks passing. \
                 Keep machine facts in docs/local/machine.md (git-ignored)."
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("odc-guard: {e}");
            ExitCode::FAILURE
        }
    }
}

fn repo_root() -> Result<PathBuf, String> {
    Ok(PathBuf::from(
        git_text(&["rev-parse", "--show-toplevel"])?.trim_end(),
    ))
}

fn pre_commit() -> Result<Vec<String>, String> {
    let repo = repo_root()?;
    let sensitive = Sensitive::load(&repo)?;
    if sensitive.is_empty() {
        println!("odc-guard: no sensitive list in docs/local/machine.md: those checks are off");
    }
    let mut problems = Vec::new();
    for (who, var) in [
        ("the author", "GIT_AUTHOR_IDENT"),
        ("the committer", "GIT_COMMITTER_IDENT"),
    ] {
        let ident = git_text(&["var", var])?;
        problems.extend(check_identity(who, ident.trim_end(), &sensitive));
    }
    let staged = nul_fields(&git(&[
        "diff",
        "--cached",
        "--name-only",
        "--no-renames",
        "--diff-filter=ACMR",
        "-z",
    ])?);
    let binary = binary_paths(&git(&[
        "diff",
        "--cached",
        "--numstat",
        "--no-renames",
        "--diff-filter=ACMR",
        "-z",
    ])?);
    for path in &staged {
        let content = git(&["cat-file", "blob", &format!(":{path}")])?;
        problems.extend(check_file(
            path,
            &content,
            binary.contains(path),
            &sensitive,
        ));
    }
    if repo.join("Cargo.toml").is_file() && staged.iter().any(|p| runs_rust_checks(p)) {
        problems.extend(rust_checks(&repo));
    } else {
        println!(
            "odc-guard: no Rust checks: the commit touches no code, packaging, frames, contract, \
             guard or CI"
        );
    }
    Ok(problems)
}

/// fmt, clippy, the tests and the udev rules, as in CI. git's own variables are kept from them,
/// so a test that runs git works on its own repository, not this one.
fn rust_checks(repo: &Path) -> Vec<String> {
    let checks: [(&str, &str, &[&str]); 4] = [
        ("fmt", "cargo", &["fmt", "--all", "--check"]),
        (
            "clippy",
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("test", "cargo", &["test", "--workspace"]),
        (
            "udev rules",
            "udevadm",
            &[
                "verify",
                "--resolve-names=never",
                "packaging/udev/70-opendeepcool.rules",
                "tools/dev/reconnect.rules",
            ],
        ),
    ];
    println!("odc-guard: Rust checks: fmt, clippy, test, udev rules");
    let mut problems = Vec::new();
    for (name, program, args) in checks {
        let mut command = Command::new(program);
        command.args(args).current_dir(repo);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        match command.status() {
            Ok(status) if status.success() => {}
            Ok(status) => problems.push(format!("{name}: {program} {}: {status}", args.join(" "))),
            Err(e) => problems.push(format!("{name}: {program}: {e}")),
        }
    }
    problems
}

fn commit_msg(file: &Path) -> Result<Vec<String>, String> {
    let sensitive = Sensitive::load(&repo_root()?)?;
    let message = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(check_message("the commit message", &message, &sensitive))
}

fn is_zero(sha: &str) -> bool {
    sha.bytes().all(|b| b == b'0')
}

/// Every commit and tag the push adds, checked like a commit, so what skipped `pre-commit`
/// (`--no-verify`, a rebase, a cherry-pick, a merge) can't leave the PC (P2-01, P2-04).
fn pre_push(remote: &str) -> Result<Vec<String>, String> {
    let sensitive = Sensitive::load(&repo_root()?)?;
    let mut problems = Vec::new();
    for line in io::stdin().lock().lines() {
        let line = line.map_err(|e| format!("the refs to push: {e}"))?;
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [local_ref, local_sha, _remote_ref, remote_sha] = fields.as_slice() else {
            continue;
        };
        if is_zero(local_sha) {
            continue;
        }
        let mut tip = local_sha.to_string();
        if local_ref.starts_with("refs/tags/")
            && git_text(&["cat-file", "-t", local_sha])?.trim() == "tag"
        {
            let tag = git_text(&["cat-file", "tag", local_sha])?;
            problems.extend(check_tag(local_ref, &tag, &sensitive));
            tip = git_text(&["rev-parse", &format!("{local_sha}^{{commit}}")])?
                .trim()
                .to_owned();
        }
        let range = if is_zero(remote_sha) || local_ref.starts_with("refs/tags/") {
            git_text(&["rev-list", &tip, "--not", &format!("--remotes={remote}")])?
        } else {
            git_text(&["rev-list", &format!("{remote_sha}..{tip}")])?
        };
        for commit in range.lines() {
            problems.extend(check_commit(commit, &sensitive)?);
        }
    }
    Ok(problems)
}

/// An annotated tag: its tagger and its message.
fn check_tag(name: &str, tag: &str, sensitive: &Sensitive) -> Vec<String> {
    let (header, message) = tag.split_once("\n\n").unwrap_or((tag, ""));
    let mut problems = Vec::new();
    if let Some(tagger) = header.lines().find_map(|l| l.strip_prefix("tagger ")) {
        problems.extend(check_identity(
            &format!("{name}'s tagger"),
            tagger,
            sensitive,
        ));
    }
    problems.extend(check_message(
        &format!("{name}'s message"),
        message,
        sensitive,
    ));
    problems
}

fn check_commit(commit: &str, sensitive: &Sensitive) -> Result<Vec<String>, String> {
    let short = &commit[..commit.len().min(10)];
    let mut problems = Vec::new();
    let people = git_text(&["log", "-1", "--format=%an <%ae>%x00%cn <%ce>", commit])?;
    let (author, committer) = people.trim_end().split_once('\0').unwrap_or((&people, ""));
    problems.extend(check_identity(
        &format!("{short}'s author"),
        author,
        sensitive,
    ));
    problems.extend(check_identity(
        &format!("{short}'s committer"),
        committer,
        sensitive,
    ));
    let message = git_text(&["log", "-1", "--format=%B", commit])?;
    problems.extend(check_message(
        &format!("{short}'s message"),
        &message,
        sensitive,
    ));
    // A merge is compared with its first parent: what it brings in.
    let diff = [
        "diff-tree",
        "-r",
        "--root",
        "-m",
        "--first-parent",
        "--no-commit-id",
    ];
    let filter = ["--no-renames", "--diff-filter=ACMR", "-z"];
    let names = nul_fields(&git(
        &[&diff[..], &filter[..], &["--name-only", commit]].concat()
    )?);
    let binary = binary_paths(&git(
        &[&diff[..], &filter[..], &["--numstat", commit]].concat()
    )?);
    for path in &names {
        let content = git(&["cat-file", "blob", &format!("{commit}:{path}")])?;
        for problem in check_file(path, &content, binary.contains(path), sensitive) {
            problems.push(format!("{short}: {problem}"));
        }
    }
    Ok(problems)
}

/// CI's privacy check (owner, 2026-10-27; solution review P2-02): every commit's author and
/// committer is the no-reply address, and the tree holds no binary file outside the allow-list.
/// The sensitive list stays on the PC, so CI checks only these.
fn history() -> Result<Vec<String>, String> {
    let none = Sensitive::default();
    let mut problems = Vec::new();
    let log = git_text(&["log", "--format=%H%x00%an <%ae>%x00%cn <%ce>", "HEAD"])?;
    let mut commits = 0;
    for line in log.lines() {
        let mut fields = line.split('\0');
        let (Some(sha), Some(author), Some(committer)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        commits += 1;
        let short = &sha[..sha.len().min(10)];
        problems.extend(check_identity(&format!("{short}'s author"), author, &none));
        problems.extend(check_identity(
            &format!("{short}'s committer"),
            committer,
            &none,
        ));
    }
    let empty = git_text(&["hash-object", "-t", "tree", "/dev/null"])?;
    let numstat = git(&["diff", "--numstat", "-z", empty.trim(), "HEAD"])?;
    let binaries = binary_paths(&numstat);
    for path in &binaries {
        problems.extend(check_file(path, &[], true, &none));
    }
    println!(
        "odc-guard: {commits} commits checked; {} binary files in the tree",
        binaries.len()
    );
    Ok(problems)
}
