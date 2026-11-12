//! The package test's steps, in order (Phase 11 plan, Task 5). A step that later ones need stops
//! the run when it fails; the root is removed however the run ends, unless `--keep`.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::Instant;

use odc_sensors_detect::record::{Board, RECORD, Record};

use crate::candidate::{self, Commit, REPO_INSIDE};
use crate::root::{self, BUILDER, BUILDER_HOME, Ran, Root};
use crate::snapshot::{self, Allowed, Difference, Snapshot};

/// The build user clones through `file://` URLs, as makepkg does and as a user clones the AUR's
/// over a URL: a plain path is opened as a local repository, which git refuses when another
/// account owns it ("dubious ownership").
///
/// The build user's home holds the builds, so the comparisons inside a run leave it out; the
/// last one, after the build user is gone, doesn't.
const SKIP: &[&str] = &[BUILDER_HOME];
const AUR_DIR: &str = "/home/builder/aur/opendeepcool";
const GITHUB_DIR: &str = "/home/builder/github/OpenDeepCool/packaging/arch";
const STAGE: &str = "/srv/stage";
const CANDIDATE: &str = "/srv/candidate";
const ID: &str = "io.github.dtmarangoni.OpenDeepCool";
const BOOT: &str = "/usr/lib/opendeepcool/opendeepcool-boot";
const DETECT_DIR: &str = "/var/lib/opendeepcool-detect";
const CONF_D: &str = "/etc/conf.d/lm_sensors";
const MODPROBE_D: &str = "/etc/modprobe.d/lm_sensors.conf";
const LM_SENSORS_LINK: &str = "/etc/systemd/system/multi-user.target.wants/lm_sensors.service";
/// The switch's two links (`opendeepcool-boot on`, FILES.md).
const SWITCH_LINKS: [(&str, &str); 2] = [
    (
        "/etc/systemd/system/multi-user.target.wants/opendeepcool.service",
        "/usr/lib/systemd/system/opendeepcool.service",
    ),
    (
        "/etc/systemd/user/default.target.wants/opendeepcool-audio.service",
        "/usr/lib/systemd/user/opendeepcool-audio.service",
    ),
];

/// One line a check, flushed at once, for `odc-e2e` to read as it goes.
#[derive(Default)]
struct Report {
    failed: bool,
}

impl Report {
    fn check(&mut self, name: &str, step: impl FnOnce() -> Result<String, String>) -> bool {
        let started = Instant::now();
        let result = step();
        let secs = started.elapsed().as_secs_f64();
        let one_line = |text: &str| {
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let mut out = std::io::stdout().lock();
        let ok = match &result {
            Ok(note) => {
                let _ = writeln!(out, "ok {name} {secs:.1} {}", one_line(note));
                true
            }
            Err(why) => {
                self.failed = true;
                let _ = writeln!(out, "FAILED {name} {secs:.1} {}", one_line(why));
                false
            }
        };
        let _ = out.flush();
        ok
    }
}

/// The run: whether every check passed.
pub fn run(repo: &Path, work: &Path, keep: bool) -> bool {
    let mut report = Report::default();
    let mut commit = None;
    report.check("candidate", || {
        let head = candidate::head(repo)?;
        let note = format!("HEAD {}", &head.hash[..12]);
        commit = Some(head);
        Ok(note)
    });
    let Some(commit) = commit else {
        return false;
    };
    let root = match Root::new(&work.join("root")) {
        Ok(root) => root,
        Err(e) => {
            report.check("root", || Err(e));
            return false;
        }
    };
    // A killed run's root goes first; its files belong to the namespace's IDs.
    if !report.check("leftovers", || {
        root.remove(work)
            .map(|()| "the work folder is empty".to_owned())
    }) {
        return false;
    }
    if !report.check("container", || {
        root::usable()
            .map(|()| "pacstrap, arch-chroot and a user namespace with subordinate IDs".into())
    }) {
        return false;
    }
    let counters = power_counters();
    let mut test = Test {
        repo: repo.to_owned(),
        work: work.to_owned(),
        root,
        commit,
        pristine: Snapshot::new(),
        baseline: Snapshot::new(),
        packages: Vec::new(),
        aur_packages: Vec::new(),
    };
    test.steps(&mut report);
    report.check("power_counters", || {
        let now = power_counters();
        if now == counters {
            Ok(format!(
                "the host's {} package energy counters are as they were",
                now.len()
            ))
        } else {
            Err(format!("before {counters:?}, after {now:?}"))
        }
    });
    if !keep {
        report.check("cleanup", || {
            test.root
                .remove(&test.work)
                .map(|()| "the root and its builds are removed".to_owned())
        });
    }
    !report.failed
}

struct Test {
    repo: PathBuf,
    work: PathBuf,
    root: Root,
    commit: Commit,
    /// Before the build user and the sources: the last comparison's.
    pristine: Snapshot,
    /// After them, the build user's home left out: every uninstall's.
    baseline: Snapshot,
    /// The GitHub way's packages, release 1, as paths inside the root.
    packages: Vec<String>,
    /// The AUR way's first build, for the comparison with the GitHub way's.
    aur_packages: Vec<String>,
}

/// The account files shadow-utils backs up as `<file>-` when an account changes.
const ACCOUNT_FILES: [&str; 6] = ["passwd", "group", "shadow", "gshadow", "subuid", "subgid"];

/// What every comparison accepts:
/// - pacman's log and package cache;
/// - `ldconfig`'s auxiliary cache, which pacman's `ldconfig` rewrites after every transaction:
///   each entry is a file's inode, change time, size and device (glibc, `elf/cache.c`, read
///   2026-11-11), so a reinstalled file changes it;
/// - the backups shadow-utils writes when an account changes, as long as each then holds what
///   its file held before, at the snapshot or just before the change (`Test::held`).
fn allowed() -> Allowed {
    Allowed {
        anything: vec![
            "/var/log/pacman.log".into(),
            "/var/cache/pacman/pkg".into(),
            "/var/cache/ldconfig/aux-cache".into(),
        ],
        backups: ACCOUNT_FILES
            .iter()
            .map(|f| (format!("/etc/{f}-"), format!("/etc/{f}")))
            .collect(),
        starts: Vec::new(),
        held: Vec::new(),
    }
}

/// What a case made that must go however the case ends: folders it created, files it put, and
/// whether it enabled `lm_sensors.service`; and what the comparison accepts after its uninstall.
#[derive(Default)]
struct Made {
    dirs: Vec<String>,
    files: Vec<String>,
    lm_sensors: bool,
    allowed: Option<Allowed>,
}

fn equal(before: &Snapshot, after: &Snapshot, allowed: &Allowed) -> Result<(), String> {
    let differences = snapshot::compare(before, after, allowed);
    if differences.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} differences: {}",
            differences.len(),
            snapshot::lines(&differences, 40)
        ))
    }
}

/// Each package zone's `energy_uj` on the host, with its owner and mode: the uninstall in the
/// root must never reach them (`post_remove` skips them in a chroot).
fn power_counters() -> Vec<String> {
    let mut found = Vec::new();
    for entry in fs::read_dir("/sys/class/powercap")
        .into_iter()
        .flatten()
        .flatten()
    {
        let zone = entry.path();
        let name = fs::read_to_string(zone.join("name")).unwrap_or_default();
        if !name.starts_with("package-") {
            continue;
        }
        let counter = zone.join("energy_uj");
        if let Ok(meta) = fs::metadata(&counter) {
            found.push(format!(
                "{} {}:{} {:o}",
                counter.display(),
                meta.uid(),
                meta.gid(),
                meta.mode() & 0o7777
            ));
        }
    }
    found.sort();
    found
}

impl Test {
    fn steps(&mut self, report: &mut Report) {
        if !report.check("root", || self.build_root()) {
            return;
        }
        if !report.check("setup", || self.setup()) {
            return;
        }
        if !self.aur_way(report) {
            return;
        }
        if !self.github_way(report) {
            return;
        }
        report.check("upgrade", || self.upgrade());
        for case in Case::ALL {
            report.check(case.name(), || self.lm_sensors(case));
        }
        report.check("signed_tags", || self.signed_tags());
        report.check("teardown", || self.teardown());
    }

    fn builder<S: AsRef<std::ffi::OsStr>>(&self, dir: &str, argv: &[S]) -> Ran {
        self.root
            .builder(dir, argv, &[("SOURCE_DATE_EPOCH", &self.commit.time)])
    }

    fn snapshot(&self) -> Result<Snapshot, String> {
        self.root.snapshot(SKIP)
    }

    /// Writes `content` at `inside`, creating its missing folders, each recorded in `dirs`.
    fn put(&self, inside: &str, content: &[u8], dirs: &mut Vec<String>) -> Result<(), String> {
        self.folders(inside, dirs)?;
        let path = self.root.path(inside);
        fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    fn link(&self, inside: &str, target: &str, dirs: &mut Vec<String>) -> Result<(), String> {
        self.folders(inside, dirs)?;
        let path = self.root.path(inside);
        symlink(target, &path).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn folders(&self, inside: &str, dirs: &mut Vec<String>) -> Result<(), String> {
        let mut missing = Vec::new();
        let mut parent = Path::new(inside).parent();
        while let Some(dir) = parent {
            if self.root.path(&dir.to_string_lossy()).exists() {
                break;
            }
            missing.push(dir.to_string_lossy().into_owned());
            parent = dir.parent();
        }
        for dir in missing.iter().rev() {
            let path = self.root.path(dir);
            fs::create_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("{}: {e}", path.display()))?;
            dirs.push(dir.clone());
        }
        Ok(())
    }

    /// Removes the folders `dirs` lists, last first, that are still there: each must be empty,
    /// so what the uninstall should have removed from it is gone.
    fn unfold(&self, dirs: &[String]) -> Result<(), String> {
        for dir in dirs.iter().rev() {
            let path = self.root.path(dir);
            if fs::symlink_metadata(&path).is_ok() {
                fs::remove_dir(&path).map_err(|e| format!("{dir} isn't empty: {e}"))?;
            }
        }
        Ok(())
    }

    fn copy(&self, from: &str, to: &str, dirs: &mut Vec<String>) -> Result<(), String> {
        let path = self.root.path(from);
        let content = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.put(to, &content, dirs)
    }

    fn build_root(&mut self) -> Result<String, String> {
        let mut packages: Vec<String> = ["base", "base-devel", "git"].map(String::from).to_vec();
        packages.extend(candidate::dependencies(&self.repo)?);
        let ran = self.root.build(&packages)?;
        let installed = ran
            .stdout
            .lines()
            .filter(|l| l.starts_with("installing "))
            .count();
        self.pristine = self.root.snapshot(&[])?;
        Ok(format!(
            "pacstrap -N: base, base-devel, git and {} dependencies of the package; {installed} \
             packages, {} paths",
            packages.len() - 3,
            self.pristine.len()
        ))
    }

    /// The build user, allowed `sudo pacman` only, as makepkg -si needs; the repository's bare
    /// clone; the AUR repository with exactly the AUR's files; the PKGBUILD's temporary copy.
    fn setup(&mut self) -> Result<String, String> {
        self.root
            .root(&["useradd", "-m", BUILDER], &[])
            .check("useradd")?;
        let mut none = Vec::new();
        let sudoers = format!(
            "{BUILDER} ALL=(root) NOPASSWD: /usr/bin/pacman\n\
             Defaults:{BUILDER} env_keep += \"SYSTEMD_IN_CHROOT\"\n"
        );
        self.put("/etc/sudoers.d/builder", sudoers.as_bytes(), &mut none)?;
        fs::set_permissions(
            self.root.path("/etc/sudoers.d/builder"),
            fs::Permissions::from_mode(0o440),
        )
        .map_err(|e| format!("the sudoers file: {e}"))?;
        let host_git = |args: &[&std::ffi::OsStr]| -> Result<(), String> {
            let out = std::process::Command::new("git")
                .args(args)
                .output()
                .map_err(|e| format!("git: {e}"))?;
            if out.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "git {args:?}: {}",
                    String::from_utf8_lossy(&out.stderr)
                ))
            }
        };
        let os = std::ffi::OsStr::new;
        host_git(&[
            os("clone"),
            os("--quiet"),
            os("--bare"),
            os("--no-local"),
            self.repo.as_os_str(),
            self.root.path(REPO_INSIDE).as_os_str(),
        ])?;
        let pkgbuild = candidate::pkgbuild(&self.repo, &self.commit)?;
        self.put(
            &format!("{CANDIDATE}/PKGBUILD"),
            pkgbuild.as_bytes(),
            &mut none,
        )?;
        fs::create_dir_all(self.root.path(STAGE)).map_err(|e| format!("{STAGE}: {e}"))?;
        // The AUR repository: the files FILES.md names, the PKGBUILD as the candidate's copy and
        // .SRCINFO written from it.
        let aur = self.work.join("aur");
        fs::create_dir_all(&aur).map_err(|e| format!("{}: {e}", aur.display()))?;
        let mut names = Vec::new();
        for file in candidate::aur_files(&self.repo)? {
            let name = Path::new(&file)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or_else(|| format!("{file}: no file name"))?;
            let content = match name.as_str() {
                "PKGBUILD" => pkgbuild.clone().into_bytes(),
                ".SRCINFO" => Vec::new(),
                _ => fs::read(self.repo.join(&file)).map_err(|e| format!("{file}: {e}"))?,
            };
            fs::write(aur.join(&name), content).map_err(|e| format!("{name}: {e}"))?;
            names.push(name);
        }
        let srcinfo = std::process::Command::new("makepkg")
            .arg("--printsrcinfo")
            .current_dir(&aur)
            .output()
            .map_err(|e| format!("makepkg: {e}"))?;
        if !srcinfo.status.success() {
            return Err(format!(
                "makepkg --printsrcinfo: {}",
                String::from_utf8_lossy(&srcinfo.stderr)
            ));
        }
        fs::write(aur.join(".SRCINFO"), &srcinfo.stdout).map_err(|e| format!(".SRCINFO: {e}"))?;
        let in_aur = |args: &[&str]| -> Result<(), String> {
            let mut full = vec![os("-C"), aur.as_os_str()];
            full.extend(args.iter().map(|a| os(a)));
            host_git(&full)
        };
        in_aur(&["init", "--quiet"])?;
        in_aur(&["add", "--all"])?;
        in_aur(&[
            "-c",
            "user.name=odc-pkg-test",
            "-c",
            "user.email=odc-pkg-test@invalid",
            "commit",
            "--quiet",
            "--message",
            "The release candidate",
        ])?;
        host_git(&[
            os("clone"),
            os("--quiet"),
            os("--bare"),
            aur.as_os_str(),
            self.root.path("/srv/aur.git").as_os_str(),
        ])?;
        self.baseline = self.snapshot()?;
        names.sort();
        Ok(format!(
            "the build user, the repository at {}, the AUR repository with {}",
            &self.commit.hash[..12],
            names.join(", ")
        ))
    }

    /// The built packages in `dir`, by file name.
    fn built(&self, dir: &str) -> Result<Vec<String>, String> {
        let ran = self
            .builder(
                dir,
                &[
                    "find",
                    ".",
                    "-maxdepth",
                    "1",
                    "-name",
                    "*.pkg.tar.zst",
                    "-printf",
                    "%f\n",
                ],
            )
            .check("listing the packages")?;
        let mut names: Vec<String> = ran.stdout.lines().map(str::to_owned).collect();
        names.sort();
        if names.is_empty() {
            return Err(format!("{dir}: no package"));
        }
        Ok(names)
    }

    /// OpenDeepCool's packages installed now: the package and, with makepkg's default `debug`
    /// option, its debug package.
    fn installed(&self) -> Result<Vec<String>, String> {
        let ran = self
            .root
            .root(&["pacman", "-Qq"], &[])
            .check("pacman -Qq")?;
        Ok(ran
            .stdout
            .lines()
            .filter(|name| name.starts_with("opendeepcool"))
            .map(str::to_owned)
            .collect())
    }

    /// What the comparison accepts after an account change about to happen: the backups may
    /// also hold what the account files hold now (shadow-utils saves the file as it was before
    /// each change, so after `userdel` the backup still has the removed account's line).
    fn held(&self) -> Allowed {
        let mut allowed = allowed();
        for file in ACCOUNT_FILES {
            if let Ok(sha) = snapshot::sha256(&self.root.path(&format!("/etc/{file}"))) {
                allowed.held.push((format!("/etc/{file}-"), sha));
            }
        }
        allowed
    }

    /// `pacman -R` of OpenDeepCool's installed packages, and what the comparison afterwards
    /// accepts.
    fn uninstall(&self) -> Result<(Ran, Allowed), String> {
        let mut argv = vec!["pacman".to_owned(), "-R".into(), "--noconfirm".into()];
        let installed = self.installed()?;
        if installed.is_empty() {
            return Err("nothing to uninstall".into());
        }
        argv.extend(installed);
        // GTK's icon cache tool keeps a cache it thinks is up to date, by whole seconds, so an
        // uninstall in the same second as its install would leave the icon theme's cache naming
        // OpenDeepCool's icon (checked in the root, 2026-11-11). A person never does that; the
        // test waits a second.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let allowed = self.held();
        Ok((self.root.root(&argv, &[]).check("pacman -R")?, allowed))
    }

    /// After a case, however it ended: the package uninstalled if the case didn't get to it,
    /// what the case made removed, and, when the case passed, the root compared with the
    /// baseline. The case's own error comes first.
    fn settle(&self, result: Result<String, String>, mut made: Made) -> Result<String, String> {
        let mut problems = Vec::new();
        match self.installed() {
            Ok(left) if !left.is_empty() => match self.uninstall() {
                Ok((_, allowed)) => made.allowed = Some(allowed),
                Err(e) => problems.push(e),
            },
            Ok(_) => {}
            Err(e) => problems.push(e),
        }
        for file in made.files.iter().rev() {
            let path = self.root.path(file);
            if fs::symlink_metadata(&path).is_ok()
                && let Err(e) = fs::remove_file(&path)
            {
                problems.push(format!("{file}: {e}"));
            }
        }
        if made.lm_sensors
            && fs::symlink_metadata(self.root.path(LM_SENSORS_LINK)).is_ok()
            && let Err(e) = self
                .root
                .root(&["systemctl", "disable", "lm_sensors.service"], &[])
                .check("disabling lm_sensors")
        {
            problems.push(e);
        }
        if let Err(e) = self.unfold(&made.dirs) {
            problems.push(e);
        }
        let note = result?;
        if !problems.is_empty() {
            return Err(format!("cleaning up: {}", problems.join("; ")));
        }
        let allowed = made.allowed.unwrap_or_else(allowed);
        equal(&self.baseline, &self.snapshot()?, &allowed)?;
        Ok(format!("{note}; the root is as before the install"))
    }

    fn aur_way(&mut self, report: &mut Report) -> bool {
        let mut output = String::new();
        if !report.check("aur_build", || {
            self.builder(
                BUILDER_HOME,
                &["git", "clone", "-q", "file:///srv/aur.git", AUR_DIR],
            )
            .check("git clone of the AUR repository")?;
            let ran = self
                .builder(AUR_DIR, &["makepkg", "-si", "--noconfirm"])
                .check("makepkg -si")?;
            output = ran.said();
            let names = self.built(AUR_DIR)?;
            self.aur_packages = names.clone();
            Ok(format!(
                "git clone, makepkg -si with check(): {}",
                names.join(", ")
            ))
        }) {
            return false;
        }
        self.after_install(report, "aur", &output);
        report.check("switch", || self.switch());
        report.check("aur_uninstall", || self.uninstall_with_changes());
        // After the uninstall: .BUILDINFO lists the packages installed during the build, so the
        // second build needs the first one's root (seen 2026-11-11: the only difference was its
        // two `installed = opendeepcool…` lines).
        report.check("reproducible", || self.reproducible());
        true
    }

    fn github_way(&mut self, report: &mut Report) -> bool {
        let mut output = String::new();
        if !report.check("github_build", || {
            self.builder(
                BUILDER_HOME,
                &[
                    "git",
                    "clone",
                    "-q",
                    &format!("file://{REPO_INSIDE}"),
                    "github/OpenDeepCool",
                ],
            )
            .check("git clone of the repository")?;
            self.builder(
                &format!("{BUILDER_HOME}/github/OpenDeepCool"),
                &["git", "checkout", "-q", &self.commit.hash],
            )
            .check("checking out the candidate")?;
            self.builder(
                BUILDER_HOME,
                &[
                    "cp",
                    &format!("{CANDIDATE}/PKGBUILD"),
                    &format!("{GITHUB_DIR}/PKGBUILD"),
                ],
            )
            .check("the PKGBUILD's temporary copy")?;
            let ran = self
                .builder(GITHUB_DIR, &["makepkg", "-si", "--noconfirm"])
                .check("makepkg -si")?;
            output = ran.said();
            let names = self.built(GITHUB_DIR)?;
            self.packages = names.iter().map(|n| format!("{GITHUB_DIR}/{n}")).collect();
            // The same source and PKGBUILD as the AUR way's, built in another folder: alike, or
            // which of the packages' files differ.
            let mut compared = Vec::new();
            for name in &names {
                let alike = self.aur_packages.contains(name)
                    && self
                        .builder(
                            BUILDER_HOME,
                            &[
                                "cmp",
                                "-s",
                                &format!("{AUR_DIR}/{name}"),
                                &format!("{GITHUB_DIR}/{name}"),
                            ],
                        )
                        .ok;
                compared.push(if alike {
                    format!("{name} byte for byte the AUR way's")
                } else {
                    let a = format!("{AUR_DIR}/{name}");
                    let b = format!("{GITHUB_DIR}/{name}");
                    format!(
                        "{name} differs from the AUR way's in: {}",
                        self.how_they_differ(&a, &b)?
                    )
                });
            }
            Ok(format!(
                "git clone, the PKGBUILD's temporary copy, makepkg -si with check(); {}",
                compared.join("; ")
            ))
        }) {
            return false;
        }
        self.after_install(report, "github", &output);
        report.check("github_uninstall", || {
            let (_, allowed) = self.uninstall()?;
            let after = self.snapshot()?;
            equal(&self.baseline, &after, &allowed)?;
            Ok("the root is as before the install".into())
        });
        true
    }

    /// What every install must leave (Task 5, "After each install").
    fn after_install(&self, report: &mut Report, way: &str, output: &str) {
        let installed = match self.snapshot() {
            Ok(s) => s,
            Err(e) => {
                report.check(&format!("{way}_installed"), || Err(e));
                return;
            }
        };
        report.check(&format!("{way}_files"), || self.files(&installed));
        report.check(&format!("{way}_account"), || self.account());
        report.check(&format!("{way}_nothing_on"), || nothing_on(&installed));
        report.check(&format!("{way}_message"), || message(output));
        report.check(&format!("{way}_units"), || self.units());
    }

    /// Every path of `FILES.md`'s first table, with its kind, owner and mode.
    fn files(&self, installed: &Snapshot) -> Result<String, String> {
        let listed = candidate::installed(&self.repo)?;
        let mut wrong = Vec::new();
        for file in &listed {
            let path = match file.path.trim_end_matches('/') {
                "" => "/",
                path => path,
            };
            let Some(entry) = installed.get(path) else {
                wrong.push(format!("{} is missing", file.path));
                continue;
            };
            let kind = if file.kind == "folder" { 'd' } else { 'f' };
            let mode = u32::from_str_radix(&file.mode, 8).unwrap_or(u32::MAX);
            let owner = match file.owner.as_str() {
                "root:root" => (0, 0),
                other => {
                    wrong.push(format!("{}: owner {other} isn't checked", file.path));
                    continue;
                }
            };
            if entry.kind != kind || entry.mode != mode || (entry.uid, entry.gid) != owner {
                wrong.push(format!(
                    "{}: {} {:o} {}:{}, FILES.md says {} {} {}",
                    file.path,
                    entry.kind,
                    entry.mode,
                    entry.uid,
                    entry.gid,
                    file.kind,
                    file.mode,
                    file.owner
                ));
            }
        }
        if wrong.is_empty() {
            Ok(format!(
                "all {} paths of FILES.md, with their modes",
                listed.len()
            ))
        } else {
            Err(wrong.join("; "))
        }
    }

    /// `opendeepcool` is a locked system account that can't log in.
    fn account(&self) -> Result<String, String> {
        let read = |file: &str| {
            fs::read_to_string(self.root.path(file)).map_err(|e| format!("{file}: {e}"))
        };
        let passwd = read("/etc/passwd")?;
        let line = passwd
            .lines()
            .find(|l| l.starts_with("opendeepcool:"))
            .ok_or("no opendeepcool in /etc/passwd")?;
        let fields: Vec<&str> = line.split(':').collect();
        let uid: u32 = fields.get(2).and_then(|u| u.parse().ok()).ok_or(line)?;
        let shell = fields.last().copied().unwrap_or_default();
        let shadow = read("/etc/shadow")?;
        let password = shadow
            .lines()
            .find(|l| l.starts_with("opendeepcool:"))
            .and_then(|l| l.split(':').nth(1))
            .ok_or("no opendeepcool in /etc/shadow")?;
        let group = read("/etc/group")?;
        if !group.lines().any(|l| l.starts_with("opendeepcool:")) {
            return Err("no opendeepcool group".into());
        }
        if uid >= 1000 || !shell.ends_with("/nologin") || !password.starts_with('!') {
            return Err(format!("{line}, password field {password:?}"));
        }
        Ok(format!(
            "system account {uid}, shell {shell}, password locked ({password})"
        ))
    }

    /// The rank helper's unit has its state directory, and both helpers a start time limit
    /// (solution review P6-03, P6-14).
    fn units(&self) -> Result<String, String> {
        let read = |name: &str| {
            let file = format!("/usr/lib/systemd/system/{name}");
            fs::read_to_string(self.root.path(&file)).map_err(|e| format!("{file}: {e}"))
        };
        let rank = read("opendeepcool-gpu-rank.service")?;
        let detect = read("opendeepcool-sensors-detect.service")?;
        let has = |text: &str, line: &str| text.lines().any(|l| l.starts_with(line));
        if !has(&rank, "StateDirectory=opendeepcool") {
            return Err("the rank helper's unit has no StateDirectory=opendeepcool".into());
        }
        for (name, text) in [("rank", &rank), ("detection", &detect)] {
            if !has(text, "TimeoutStartSec=") {
                return Err(format!("the {name} helper's unit has no TimeoutStartSec="));
            }
        }
        Ok("the rank helper's StateDirectory=, both helpers' TimeoutStartSec=".into())
    }

    /// Two builds of the AUR way in the same folder, from a cleaned source folder, at the same
    /// SOURCE_DATE_EPOCH, with the same options and the same packages installed, are the same
    /// packages byte for byte.
    fn reproducible(&self) -> Result<String, String> {
        self.builder(BUILDER_HOME, &["mkdir", "-p", "first"])
            .check("mkdir")?;
        for name in &self.aur_packages {
            self.builder(
                BUILDER_HOME,
                &["cp", &format!("{AUR_DIR}/{name}"), "first/"],
            )
            .check("keeping the first build")?;
        }
        // The same options as the first build: .BUILDINFO records them, check() included.
        self.builder(AUR_DIR, &["makepkg", "-fC", "--noconfirm"])
            .check("the second build")?;
        let second = self.built(AUR_DIR)?;
        if second != self.aur_packages {
            return Err(format!("{:?}, then {second:?}", self.aur_packages));
        }
        let mut differ = Vec::new();
        for name in &second {
            let same = self
                .builder(
                    BUILDER_HOME,
                    &[
                        "cmp",
                        "-s",
                        &format!("first/{name}"),
                        &format!("{AUR_DIR}/{name}"),
                    ],
                )
                .ok;
            if !same {
                let first = format!("first/{name}");
                let second = format!("{AUR_DIR}/{name}");
                differ.push(format!(
                    "{name}: {}",
                    self.how_they_differ(&first, &second)?
                ));
            }
        }
        if differ.is_empty() {
            Ok(format!("{} alike byte for byte", second.join(", ")))
        } else {
            Err(differ.join("; "))
        }
    }

    /// Which files of two builds of `name` differ: both unpacked and compared.
    fn how_they_differ(&self, first: &str, second: &str) -> Result<String, String> {
        for (dir, from) in [("cmp/a", first), ("cmp/b", second)] {
            self.builder(BUILDER_HOME, &["rm", "-rf", dir])
                .check("rm")?;
            self.builder(BUILDER_HOME, &["mkdir", "-p", dir])
                .check("mkdir")?;
            self.builder(BUILDER_HOME, &["bsdtar", "-C", dir, "-xf", from])
                .check("bsdtar")?;
        }
        let ran = self.builder(BUILDER_HOME, &["diff", "-rq", "cmp/a", "cmp/b"]);
        let lines: Vec<String> = ran
            .stdout
            .lines()
            .take(20)
            .map(|line| line.replace("cmp/a/", "").replace(" and cmp/b/", " / "))
            .collect();
        Ok(lines.join(", "))
    }

    /// The Start at boot program inside the root: `on` makes exactly the two links and starts
    /// nothing (no systemd runs here), `off` removes them, anything else is refused and changes
    /// nothing; left on, so the uninstall must remove the links.
    fn switch(&self) -> Result<String, String> {
        let pkexec = [("PKEXEC_UID", "1000")];
        let before = self.snapshot()?;
        self.root
            .root(&[BOOT, "on"], &pkexec)
            .check("opendeepcool-boot on")?;
        let on = self.snapshot()?;
        let differences = snapshot::compare(&before, &on, &Allowed::default());
        let links: BTreeSet<&str> = SWITCH_LINKS.iter().map(|(l, _)| *l).collect();
        for d in &differences {
            let fine = match (&d.before, &d.after) {
                (None, Some(e)) if e.kind == 'l' => SWITCH_LINKS
                    .iter()
                    .any(|(l, t)| d.name == *l && e.data == *t),
                (None, Some(e)) if e.kind == 'd' => {
                    links.iter().any(|l| l.starts_with(&format!("{}/", d.name)))
                }
                _ => false,
            };
            if !fine {
                return Err(format!("on: {d}"));
            }
        }
        let made: Vec<&Difference> = differences
            .iter()
            .filter(|d| links.contains(d.name.as_str()))
            .collect();
        if made.len() != 2 {
            return Err(format!("on made {} of the two links", made.len()));
        }
        self.root
            .root(&[BOOT, "off"], &pkexec)
            .check("opendeepcool-boot off")?;
        equal(&before, &self.snapshot()?, &Allowed::default()).map_err(|e| format!("off: {e}"))?;
        let none: &[(&str, &str)] = &[];
        let refusals = [
            (vec![BOOT, "on"], none),
            (vec![BOOT, "start"], &pkexec[..]),
            (vec![BOOT], &pkexec[..]),
        ];
        for (argv, env) in refusals {
            if self.root.root(&argv, env).ok {
                return Err(format!("{argv:?} with {env:?} wasn't refused"));
            }
        }
        equal(&before, &self.snapshot()?, &Allowed::default())
            .map_err(|e| format!("a refusal changed something: {e}"))?;
        self.root
            .root(&[BOOT, "on"], &pkexec)
            .check("opendeepcool-boot on")?;
        Ok(
            "on made the two links, off removed them; no PKEXEC_UID, start and no argument refused"
                .into(),
        )
    }

    /// An administrator's changes under OpenDeepCool's names, then the uninstall: every one is
    /// gone (owner, 2026-11-11), a user's own change in their home stays, and the root is as
    /// before the install.
    fn uninstall_with_changes(&self) -> Result<String, String> {
        let mut dirs = Vec::new();
        let unit = |name: &str| format!("/usr/lib/systemd/system/{name}");
        let drop_in = b"[Service]\nNice=5\n";
        self.put(
            "/etc/systemd/system/opendeepcool.service.d/override.conf",
            drop_in,
            &mut dirs,
        )?;
        self.put(
            "/etc/systemd/system.control/opendeepcool.service.d/50-CPUQuota.conf",
            b"[Service]\nCPUQuota=50%\n",
            &mut dirs,
        )?;
        self.copy(
            &unit("opendeepcool-gpu-rank.service"),
            "/etc/systemd/system/opendeepcool-gpu-rank.service",
            &mut dirs,
        )?;
        self.root
            .root(
                &[
                    "systemctl",
                    "add-wants",
                    "multi-user.target",
                    "opendeepcool-gpu-rank.service",
                ],
                &[],
            )
            .check("systemctl add-wants")?;
        self.root
            .root(
                &["systemctl", "mask", "opendeepcool-sensors-detect.service"],
                &[],
            )
            .check("systemctl mask")?;
        self.put(
            "/etc/systemd/user/opendeepcool-audio.service.d/override.conf",
            drop_in,
            &mut dirs,
        )?;
        self.copy(
            "/usr/lib/sysusers.d/opendeepcool.conf",
            "/etc/sysusers.d/opendeepcool.conf",
            &mut dirs,
        )?;
        self.link(
            "/etc/udev/rules.d/70-opendeepcool.rules",
            "/dev/null",
            &mut dirs,
        )?;
        self.copy(
            &format!("/usr/share/polkit-1/actions/{ID}1.policy"),
            &format!("/etc/polkit-1/actions/{ID}1.policy"),
            &mut dirs,
        )?;
        self.copy(
            &format!("/usr/share/dbus-1/system.d/{ID}1.conf"),
            &format!("/etc/dbus-1/system.d/{ID}1.conf"),
            &mut dirs,
        )?;
        self.link(
            "/etc/pacman.d/hooks/opendeepcool-restart.hook",
            "/dev/null",
            &mut dirs,
        )?;
        let mine = ".config/systemd/user/opendeepcool-audio.service.d/override.conf";
        let mut none = Vec::new();
        self.put(&format!("{STAGE}/override.conf"), drop_in, &mut none)?;
        self.builder(
            BUILDER_HOME,
            &[
                "install",
                "-D",
                "-m",
                "0644",
                &format!("{STAGE}/override.conf"),
                mine,
            ],
        )
        .check("the user's own drop-in")?;
        let gone = [
            "/etc/systemd/system/opendeepcool.service.d".to_owned(),
            "/etc/systemd/system.control/opendeepcool.service.d".into(),
            "/etc/systemd/system/opendeepcool-gpu-rank.service".into(),
            "/etc/systemd/system/multi-user.target.wants/opendeepcool-gpu-rank.service".into(),
            "/etc/systemd/system/opendeepcool-sensors-detect.service".into(),
            "/etc/systemd/user/opendeepcool-audio.service.d".into(),
            "/etc/sysusers.d/opendeepcool.conf".into(),
            "/etc/udev/rules.d/70-opendeepcool.rules".into(),
            format!("/etc/polkit-1/actions/{ID}1.policy"),
            format!("/etc/dbus-1/system.d/{ID}1.conf"),
            "/etc/pacman.d/hooks/opendeepcool-restart.hook".into(),
            SWITCH_LINKS[0].0.into(),
            SWITCH_LINKS[1].0.into(),
        ];
        let (ran, allowed) = self.uninstall()?;
        let left: Vec<&String> = gone
            .iter()
            .filter(|p| fs::symlink_metadata(self.root.path(p)).is_ok())
            .collect();
        if !left.is_empty() {
            return Err(format!("left after the uninstall: {left:?}"));
        }
        let removed = ran
            .said()
            .lines()
            .filter(|l| l.starts_with("removed"))
            .count();
        self.builder(BUILDER_HOME, &["test", "-f", mine])
            .check("the user's own drop-in stays")?;
        let fs_dirs: Vec<String> = dirs.into_iter().filter(|d| !gone.contains(d)).collect();
        self.unfold(&fs_dirs)?;
        fs::remove_file(self.root.path(&format!("{STAGE}/override.conf")))
            .map_err(|e| format!("the staged drop-in: {e}"))?;
        equal(&self.baseline, &self.snapshot()?, &allowed)?;
        Ok(format!(
            "{} of an administrator's changes gone ({removed} removals printed), the user's own \
             stays, the root is as before the install",
            gone.len()
        ))
    }

    /// The candidate, then the same build at a raised pkgrel: the switch's links, the account,
    /// the detection record, the settings and an administrator's drop-in survive, the files are
    /// the same, and the restart hook skips its work in the chroot.
    fn upgrade(&self) -> Result<String, String> {
        let mut made = Made::default();
        let result = self.upgrade_steps(&mut made);
        self.settle(result, made)
    }

    fn upgrade_steps(&self, made: &mut Made) -> Result<String, String> {
        let mut argv = vec!["pacman".to_owned(), "-U".into(), "--noconfirm".into()];
        argv.extend(self.packages.iter().cloned());
        self.root.root(&argv, &[]).check("pacman -U release 1")?;
        self.root
            .root(&[BOOT, "on"], &[("PKEXEC_UID", "1000")])
            .check("opendeepcool-boot on")?;
        self.record(&record(false, false, false))?;
        let settings = format!("{STAGE}/settings.toml");
        made.files.push(settings.clone());
        self.put(&settings, b"mode = 4\n", &mut made.dirs)?;
        let as_service = ["-o", "opendeepcool", "-g", "opendeepcool"];
        let mut install_dir = vec!["install", "-d", "-m", "0700"];
        install_dir.extend(as_service);
        install_dir.push("/var/lib/opendeepcool");
        self.root
            .root(&install_dir, &[])
            .check("the state folder")?;
        let mut install_file = vec!["install", "-m", "0600"];
        install_file.extend(as_service);
        install_file.extend([settings.as_str(), "/var/lib/opendeepcool/settings.toml"]);
        self.root.root(&install_file, &[]).check("the settings")?;
        self.put(
            "/etc/systemd/system/opendeepcool.service.d/override.conf",
            b"[Service]\nNice=5\n",
            &mut made.dirs,
        )?;
        let before = self.snapshot()?;
        self.builder(
            GITHUB_DIR,
            &["sed", "-i", "s/^pkgrel=1$/pkgrel=2/", "PKGBUILD"],
        )
        .check("raising pkgrel")?;
        self.builder(GITHUB_DIR, &["makepkg", "-R", "--noconfirm"])
            .check("makepkg -R")?;
        let next: Vec<String> = self
            .built(GITHUB_DIR)?
            .into_iter()
            .filter(|n| n.ends_with("-2-x86_64.pkg.tar.zst"))
            .map(|n| format!("{GITHUB_DIR}/{n}"))
            .collect();
        if next.is_empty() {
            return Err("no release 2 package".into());
        }
        let mut argv = vec!["pacman".to_owned(), "-U".into(), "--noconfirm".into()];
        argv.extend(next.iter().cloned());
        let ran = self.root.root(&argv, &[]).check("pacman -U release 2")?;
        let said = ran.said();
        let hook = said
            .lines()
            .skip_while(|l| !l.contains("Restart a running opendeepcool.service"))
            .nth(1)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if !hook.starts_with("Skipped") {
            return Err(format!("the restart hook: {hook:?}"));
        }
        let after = self.snapshot()?;
        let mut upgraded = allowed();
        upgraded
            .starts
            .push("/var/lib/pacman/local/opendeepcool-".into());
        equal(&before, &after, &upgraded).map_err(|e| format!("the upgrade changed: {e}"))?;
        let (_, allowed) = self.uninstall()?;
        made.allowed = Some(allowed);
        if fs::symlink_metadata(self.root.path("/etc/systemd/system/opendeepcool.service.d"))
            .is_ok()
        {
            return Err("the administrator's drop-in stayed after the uninstall".into());
        }
        Ok(format!(
            "release 1, then 2: everything else alike; restart hook: {hook}"
        ))
    }

    /// Writes the detection's record where the detection keeps it: a root-only folder, root's
    /// file.
    fn record(&self, text: &str) -> Result<(), String> {
        let dir = self.root.path(DETECT_DIR);
        fs::create_dir_all(&dir).map_err(|e| format!("{DETECT_DIR}: {e}"))?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("{DETECT_DIR}: {e}"))?;
        let file = dir.join(RECORD);
        fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("{}: {e}", file.display()))
    }

    /// One lm-sensors case: install, the detection's files and record as the case says,
    /// uninstall, and what must be left.
    fn lm_sensors(&self, case: Case) -> Result<String, String> {
        let mut made = Made {
            files: vec![
                CONF_D.into(),
                MODPROBE_D.into(),
                format!("{STAGE}/record.json"),
            ],
            lm_sensors: true,
            ..Made::default()
        };
        let result = self.lm_sensors_steps(case, &mut made);
        self.settle(result, made)
    }

    fn lm_sensors_steps(&self, case: Case, made: &mut Made) -> Result<String, String> {
        let conf = b"HWMON_MODULES=\"coretemp\"\n";
        if case == Case::Before {
            self.put(CONF_D, conf, &mut made.dirs)?;
            self.root
                .root(&["systemctl", "enable", "lm_sensors.service"], &[])
                .check("enabling lm_sensors")?;
        }
        let mut argv = vec!["pacman".to_owned(), "-U".into(), "--noconfirm".into()];
        argv.extend(self.packages.iter().cloned());
        self.root.root(&argv, &[]).check("pacman -U")?;
        if case != Case::Before {
            self.put(CONF_D, conf, &mut made.dirs)?;
            self.put(
                MODPROBE_D,
                b"options coretemp tjmax=100000\n",
                &mut made.dirs,
            )?;
            self.root
                .root(&["systemctl", "enable", "lm_sensors.service"], &[])
                .check("enabling lm_sensors")?;
        }
        let real = record(
            case != Case::Before,
            case != Case::Before,
            case == Case::Before,
        );
        match case {
            Case::Created | Case::Before => self.record(&real)?,
            Case::Link => {
                let mut none = Vec::new();
                self.put(&format!("{STAGE}/record.json"), real.as_bytes(), &mut none)?;
                self.record("")?;
                let file = self.root.path(&format!("{DETECT_DIR}/{RECORD}"));
                fs::remove_file(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                symlink(format!("{STAGE}/record.json"), &file)
                    .map_err(|e| format!("{}: {e}", file.display()))?;
            }
            Case::Owner => {
                self.record(&real)?;
                self.root
                    .root(&["chown", BUILDER, &format!("{DETECT_DIR}/{RECORD}")], &[])
                    .check("chown")?;
            }
            Case::Path => self.record(&real.replace(
                "\"created_conf_d\": true,",
                "\"created_conf_d\": \"/etc/passwd\",",
            ))?,
            Case::Key => {
                self.record(&real.replace("\"created_modprobe_d\"", "\"created_other\""))?
            }
        }
        let (ran, allowed) = self.uninstall()?;
        made.allowed = Some(allowed);
        let there = |p: &str| fs::symlink_metadata(self.root.path(p)).is_ok();
        let state = format!(
            "conf.d {}, modprobe.d {}, lm_sensors.service {}",
            there(CONF_D),
            there(MODPROBE_D),
            there(LM_SENSORS_LINK)
        );
        let note = match case {
            Case::Created => {
                if there(CONF_D) || there(MODPROBE_D) || there(LM_SENSORS_LINK) {
                    return Err(format!("the detection's files stayed: {state}"));
                }
                "the detection's files and lm_sensors.service went".to_owned()
            }
            Case::Before => {
                let kept = fs::read(self.root.path(CONF_D)).ok();
                if kept.as_deref() != Some(&conf[..]) || !there(LM_SENSORS_LINK) {
                    return Err(format!("the PC's own lm-sensors setup changed: {state}"));
                }
                "the PC's own lm-sensors setup stayed as it was".to_owned()
            }
            _ => {
                if !(there(CONF_D) && there(MODPROBE_D) && there(LM_SENSORS_LINK)) {
                    return Err(format!("a forged record removed something: {state}"));
                }
                if !ran.said().contains("isn't as it wrote it") {
                    return Err("no word about the forged record".into());
                }
                "nothing it names was removed, and the uninstall said why".to_owned()
            }
        };
        Ok(note)
    }

    /// A copy of the PKGBUILD with `?signed` and a throwaway key's fingerprint verifies a tag
    /// that key signed, and refuses an unsigned tag and one another throwaway key signed.
    fn signed_tags(&self) -> Result<String, String> {
        let mut keys = Vec::new();
        for who in ["a", "b"] {
            self.builder(
                BUILDER_HOME,
                &[
                    "gpg",
                    "--batch",
                    "--pinentry-mode",
                    "loopback",
                    "--passphrase",
                    "",
                    "--quick-gen-key",
                    &format!("odc-pkg-test {who} <{who}@odc-pkg-test.invalid>"),
                    "ed25519",
                    "sign",
                    "never",
                ],
            )
            .check("gpg --quick-gen-key")?;
            let listed = self
                .builder(
                    BUILDER_HOME,
                    &[
                        "gpg",
                        "--batch",
                        "--with-colons",
                        "--list-secret-keys",
                        &format!("{who}@odc-pkg-test.invalid"),
                    ],
                )
                .check("gpg --list-secret-keys")?;
            let fpr = listed
                .stdout
                .lines()
                .find_map(|l| l.strip_prefix("fpr:"))
                .and_then(|rest| rest.split(':').find(|f| f.len() == 40))
                .ok_or("no fingerprint")?
                .to_owned();
            keys.push(fpr);
        }
        self.builder(
            BUILDER_HOME,
            &[
                "git",
                "clone",
                "-q",
                &format!("file://{REPO_INSIDE}"),
                "signed",
            ],
        )
        .check("git clone")?;
        let template = fs::read_to_string(self.repo.join("packaging/arch/PKGBUILD"))
            .map_err(|e| format!("the PKGBUILD: {e}"))?;
        let version = template
            .lines()
            .find_map(|l| l.strip_prefix("pkgver="))
            .ok_or("no pkgver")?
            .to_owned();
        let pkgbuild: String = template
            .lines()
            .map(|line| {
                if line.starts_with("source=(") {
                    "source=(\"$pkgname::git+file:///home/builder/signed?signed#tag=v$pkgver\")"
                        .to_owned()
                } else if line.starts_with("validpgpkeys=(") {
                    format!("validpgpkeys=('{}')", keys[0])
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut none = Vec::new();
        self.put(
            &format!("{STAGE}/signed/PKGBUILD"),
            pkgbuild.as_bytes(),
            &mut none,
        )?;
        let install = fs::read(self.repo.join("packaging/arch/opendeepcool.install"))
            .map_err(|e| format!("the install file: {e}"))?;
        self.put(
            &format!("{STAGE}/signed/opendeepcool.install"),
            &install,
            &mut none,
        )?;
        let tag = format!("v{version}");
        let cases = [
            ("signed by its key", Some(&keys[0]), true),
            ("unsigned", None, false),
            ("signed by another key", Some(&keys[1]), false),
        ];
        let mut notes = Vec::new();
        for (i, (what, key, accepted)) in cases.into_iter().enumerate() {
            let mut argv: Vec<String> = [
                "git",
                "-c",
                "user.name=odc-pkg-test",
                "-c",
                "user.email=odc-pkg-test@invalid",
            ]
            .map(String::from)
            .to_vec();
            if let Some(key) = key {
                argv.extend(["-c".into(), format!("user.signingkey={key}")]);
            }
            argv.extend([
                "tag".into(),
                "-f".into(),
                if key.is_some() {
                    "-s".into()
                } else {
                    "-a".into()
                },
                "-m".into(),
                tag.clone(),
                tag.clone(),
                self.commit.hash.clone(),
            ]);
            self.builder("/home/builder/signed", &argv)
                .check(&format!("the tag, {what}"))?;
            let dir = format!("signed-{i}");
            self.builder(
                BUILDER_HOME,
                &["cp", "-r", &format!("{STAGE}/signed"), &dir],
            )
            .check("the PKGBUILD's copy")?;
            let ran = self.builder(
                &format!("{BUILDER_HOME}/{dir}"),
                &["makepkg", "--verifysource", "--noconfirm"],
            );
            if ran.ok != accepted {
                return Err(format!(
                    "{what}: makepkg --verifysource {}: {}",
                    if ran.ok { "accepted it" } else { "refused it" },
                    ran.said()
                ));
            }
            notes.push(format!(
                "{what} {}",
                if accepted { "verified" } else { "refused" }
            ));
        }
        fs::remove_dir_all(self.root.path(&format!("{STAGE}/signed")))
            .map_err(|e| format!("the staged copy: {e}"))?;
        Ok(notes.join(", "))
    }

    /// The build user and the sources go, and the root must be as pacstrap left it.
    fn teardown(&self) -> Result<String, String> {
        let allowed = self.held();
        self.root
            .root(&["userdel", "-r", BUILDER], &[])
            .check("userdel -r")?;
        let sudoers = "/etc/sudoers.d/builder";
        fs::remove_file(self.root.path(sudoers)).map_err(|e| format!("{sudoers}: {e}"))?;
        for dir in [REPO_INSIDE, "/srv/aur.git", CANDIDATE, STAGE] {
            fs::remove_dir_all(self.root.path(dir)).map_err(|e| format!("{dir}: {e}"))?;
        }
        let after = self.root.snapshot(&[])?;
        equal(&self.pristine, &after, &allowed)?;
        Ok(format!(
            "every path as pacstrap left it, but for {}",
            accepted(&self.pristine, &after, &allowed)
        ))
    }
}

/// The differences `allowed` accepted between `before` and `after`, counted by kind: each
/// allowed path, or the account files' backups.
fn accepted(before: &Snapshot, after: &Snapshot, allowed: &Allowed) -> String {
    let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for d in snapshot::compare(before, after, &Allowed::default()) {
        let kind = allowed
            .anything
            .iter()
            .find(|p| d.name == **p || d.name.starts_with(&format!("{p}/")))
            .cloned()
            .or_else(|| {
                allowed
                    .backups
                    .iter()
                    .any(|(b, _)| *b == d.name)
                    .then(|| "the account files' backups".to_owned())
            })
            .unwrap_or(d.name);
        *kinds.entry(kind).or_default() += 1;
    }
    if kinds.is_empty() {
        return "nothing".into();
    }
    kinds
        .iter()
        .map(|(kind, n)| format!("{kind} ({n})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A record as the detection writes it, for a made-up board.
fn record(conf_d: bool, modprobe_d: bool, before: bool) -> String {
    Record {
        board: Board {
            vendor: "Test Vendor".into(),
            name: "TEST-BOARD".into(),
            version: "1.0".into(),
        },
        date: "2026-11-11".into(),
        created_conf_d: conf_d,
        created_modprobe_d: modprobe_d,
        lm_sensors_enabled_before: before,
    }
    .render()
}

/// No link under `/etc/systemd`, or in a `.wants` folder under `/usr/lib/systemd`, names an
/// OpenDeepCool unit.
fn nothing_on(installed: &Snapshot) -> Result<String, String> {
    let links: Vec<&String> = installed
        .iter()
        .filter(|(name, e)| {
            e.kind == 'l'
                && (name.starts_with("/etc/systemd/")
                    || (name.starts_with("/usr/lib/systemd/") && name.contains(".wants/")))
                && (name.contains("opendeepcool") || e.data.contains("opendeepcool"))
        })
        .map(|(name, _)| name)
        .collect();
    if links.is_empty() {
        Ok("no link names an OpenDeepCool unit".into())
    } else {
        Err(format!("enabled at install: {links:?}"))
    }
}

/// The install message, with both commands.
fn message(output: &str) -> Result<String, String> {
    for command in [
        "sudo systemctl enable --now opendeepcool.service",
        "sudo systemctl --global enable opendeepcool-audio.service",
    ] {
        if !output.contains(command) {
            return Err(format!("the install didn't print {command:?}"));
        }
    }
    Ok("printed, with both commands".into())
}

/// The lm-sensors cases (Task 5): the detection created the files; the PC had its own setup
/// before; and four forged records (solution review OV-01).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Created,
    Before,
    Link,
    Owner,
    Path,
    Key,
}

impl Case {
    const ALL: [Case; 6] = [
        Case::Created,
        Case::Before,
        Case::Link,
        Case::Owner,
        Case::Path,
        Case::Key,
    ];

    fn name(self) -> &'static str {
        match self {
            Case::Created => "lm_sensors_created",
            Case::Before => "lm_sensors_before",
            Case::Link => "forged_link",
            Case::Owner => "forged_owner",
            Case::Path => "forged_path",
            Case::Key => "forged_key",
        }
    }
}
