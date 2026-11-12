//! The release candidate and what the repository says about the package (Phase 11 plan,
//! Recommendation): the committed HEAD of a tree with no change in it, the PKGBUILD's temporary
//! copy whose source is that commit, the files the AUR repository holds, the package's
//! dependencies, and `FILES.md`'s list of what it installs.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Where the root holds a bare clone of the repository; the candidate's source.
pub const REPO_INSIDE: &str = "/srv/opendeepcool.git";

/// The committed HEAD.
pub struct Commit {
    pub hash: String,
    /// The commit's time, the builds' `SOURCE_DATE_EPOCH`.
    pub time: String,
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("git can't start: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// HEAD, or an error when the tree has a change or a file git doesn't know: the build would
/// leave it out, so the test wouldn't test what's in the tree.
pub fn head(repo: &Path) -> Result<Commit, String> {
    let status = git(repo, &["status", "--porcelain"])?;
    if !status.is_empty() {
        let first: Vec<&str> = status.lines().take(5).collect();
        return Err(format!(
            "the tree has changes the candidate wouldn't hold; commit them first: {}",
            first.join(", ")
        ));
    }
    Ok(Commit {
        hash: git(repo, &["rev-parse", "HEAD"])?,
        time: git(repo, &["log", "-1", "--format=%ct"])?,
    })
}

/// The PKGBUILD's temporary copy for `commit`: its one `source=` line built from the root's
/// clone at that commit instead of the release's signed tag. A signed tag doesn't exist before a
/// release; the commit pins the source as firmly.
pub fn pkgbuild(repo: &Path, commit: &Commit) -> Result<String, String> {
    let path = repo.join("packaging/arch/PKGBUILD");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let sources = text.lines().filter(|l| l.starts_with("source=(")).count();
    if sources != 1 {
        return Err(format!("the PKGBUILD has {sources} source= lines, not one"));
    }
    Ok(text
        .lines()
        .map(|line| {
            if line.starts_with("source=(") {
                format!(
                    "source=(\"$pkgname::git+file://{REPO_INSIDE}#commit={}\")",
                    commit.hash
                )
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n")
}

/// The rows of `FILES.md`'s table under `heading`, each row's cells without backticks.
fn table(repo: &Path, heading: &str) -> Result<Vec<Vec<String>>, String> {
    let path = repo.join("packaging/FILES.md");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let section = text
        .split(&format!("## {heading}"))
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .ok_or_else(|| format!("FILES.md has no section {heading:?}"))?;
    Ok(section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().trim_matches('`').to_owned())
                .collect()
        })
        .collect())
}

/// One file the package installs, as `FILES.md` lists it.
pub struct Installed {
    pub path: String,
    /// `program`, `file` or `folder`.
    pub kind: String,
    /// `root:root`.
    pub owner: String,
    /// `0755`.
    pub mode: String,
}

/// `FILES.md`'s "Installed by the package".
pub fn installed(repo: &Path) -> Result<Vec<Installed>, String> {
    table(repo, "Installed by the package")?
        .into_iter()
        .map(|cells| match &cells[..] {
            [path, kind, owner, mode, _] => Ok(Installed {
                path: path.clone(),
                kind: kind.clone(),
                owner: owner.clone(),
                mode: mode.clone(),
            }),
            _ => Err(format!("FILES.md: a row of {} cells", cells.len())),
        })
        .collect()
}

/// `FILES.md`'s "The PKGBUILD's own files", which the AUR repository holds.
pub fn aur_files(repo: &Path) -> Result<Vec<String>, String> {
    Ok(table(repo, "The PKGBUILD's own files")?
        .into_iter()
        .filter_map(|cells| cells.first().cloned())
        .collect())
}

/// `.SRCINFO`'s `depends` and `makedepends`, which the root gets before its first snapshot, so
/// the comparison sees only OpenDeepCool's own changes (solution review P11-10).
pub fn dependencies(repo: &Path) -> Result<Vec<String>, String> {
    let path = repo.join("packaging/arch/.SRCINFO");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(text
        .lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once(" = ")?;
            matches!(key, "depends" | "makedepends").then(|| value.to_owned())
        })
        .collect())
}
