//! The rootless throwaway root (Phase 11 plan, Task 5; spike, 2026-10-25): built with
//! `pacstrap -N`, entered with `arch-chroot -N`, never booted. Its files belong to this account's
//! subordinate IDs, so reading or removing all of them happens inside the same user namespace,
//! through `unshare` with `pacstrap`'s own mapping, where this program runs again as root.
//!
//! Two things `arch-chroot -N` does are put right here (arch-install-scripts 31, read
//! 2026-11-11):
//! - it binds the host's `resolv.conf` before bind-mounting the root on itself, which hides that
//!   bind, so the root gets the host's resolver in its own `/etc/resolv.conf` when it's built;
//! - it mounts `/proc` for its first PID namespace and runs the command in a second one, so a
//!   process's own PID isn't in `/proc`. The build user's commands first mount a `/proc` of
//!   their own (a test that checks `/proc/<pid>` failed in `check()` until then), and get
//!   `/run/systemd`, which every running system has and `systemd-analyze` needs.
//!
//! Every command gets `SYSTEMD_IN_CHROOT=1`, systemd's own override of its chroot check, so
//! pacman's hooks and the scriptlets act as in a chroot. A fresh `/proc` would otherwise hide it:
//! its PID 1 is inside the root. `sudo` keeps it for the build user's `makepkg -si`.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::snapshot::{self, Snapshot};

/// The build user inside the root, who runs `makepkg`, as a user does.
pub const BUILDER: &str = "builder";
pub const BUILDER_HOME: &str = "/home/builder";

/// `pacstrap`'s and `arch-chroot`'s own mapping: this account as root, its first block of
/// subordinate IDs from 1.
const UNSHARE: [&str; 5] = ["--map-auto", "--map-root-user", "--setuid", "0", "--setgid"];

/// A command's result, with both outputs as text.
pub struct Ran {
    pub ok: bool,
    pub code: String,
    pub stdout: String,
    pub stderr: String,
}

impl Ran {
    fn from(output: std::io::Result<Output>, what: &str) -> Ran {
        match output {
            Ok(out) => Ran {
                ok: out.status.success(),
                code: out.status.to_string(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            },
            Err(e) => Ran {
                ok: false,
                code: format!("{what} can't start: {e}"),
                stdout: String::new(),
                stderr: String::new(),
            },
        }
    }

    /// Both outputs, without `arch-chroot`'s two lines about its own mounts: the root isn't a
    /// mount point until `-N` binds it, and the `resolv.conf` it couldn't unmount.
    pub fn said(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
            .lines()
            .filter(|line| !line.contains("is not a mountpoint") && !line.contains("resolv.conf"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `Ok` with the output, or an error naming `what`, its exit status and the output's end.
    pub fn check(self, what: &str) -> Result<Ran, String> {
        if self.ok {
            Ok(self)
        } else {
            let said = self.said();
            let tail: Vec<&str> = said.lines().rev().take(15).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            Err(format!("{what}: {}\n{}", self.code, tail.join("\n")))
        }
    }
}

pub struct Root {
    /// The root's folder, absolute.
    pub dir: PathBuf,
    /// This program, run again inside the user namespace.
    me: PathBuf,
}

impl Root {
    pub fn new(dir: &Path) -> Result<Root, String> {
        let me = std::env::current_exe().map_err(|e| format!("this program's path: {e}"))?;
        Ok(Root {
            dir: dir.to_owned(),
            me,
        })
    }

    /// A path inside the root, from the root's own view (`/etc/passwd`).
    pub fn path(&self, inside: &str) -> PathBuf {
        self.dir.join(inside.trim_start_matches('/'))
    }

    /// Builds the root with `pacstrap -N`, from the host's package cache where it can, with
    /// `packages`, then gives it the host's resolver.
    pub fn build(&self, packages: &[String]) -> Result<Ran, String> {
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let ran = Ran::from(
            Command::new("pacstrap")
                .args(["-N", "-c"])
                .arg(&self.dir)
                .args(packages)
                .output(),
            "pacstrap",
        )
        .check("pacstrap -N -c")?;
        let resolver =
            fs::read("/etc/resolv.conf").map_err(|e| format!("/etc/resolv.conf: {e}"))?;
        let inside = self.path("/etc/resolv.conf");
        // A link in the root would point into the root's own /run, empty here.
        let _ = fs::remove_file(&inside);
        fs::write(&inside, resolver).map_err(|e| format!("{}: {e}", inside.display()))?;
        Ok(ran)
    }

    fn chroot(&self, argv: &[&OsStr]) -> Ran {
        let mut command = Command::new("arch-chroot");
        command.arg("-N").arg(&self.dir).args(argv);
        Ran::from(command.output(), "arch-chroot")
    }

    /// `argv` as root inside the root, with an environment of `env`, `PATH` and the chroot
    /// override only.
    pub fn root<S: AsRef<OsStr>>(&self, argv: &[S], env: &[(&str, &str)]) -> Ran {
        let mut full: Vec<std::ffi::OsString> = vec!["/usr/bin/env".into(), "-i".into()];
        full.push("PATH=/usr/bin".into());
        full.push("SYSTEMD_IN_CHROOT=1".into());
        for (key, value) in env {
            full.push(format!("{key}={value}").into());
        }
        full.extend(argv.iter().map(|a| a.as_ref().to_owned()));
        let refs: Vec<&OsStr> = full.iter().map(|a| a.as_os_str()).collect();
        self.chroot(&refs)
    }

    /// `argv` as the build user, in folder `dir` of the root, with an environment of its home,
    /// `PATH`, `LANG` and `env`, as a login gives it.
    pub fn builder<S: AsRef<OsStr>>(&self, dir: &str, argv: &[S], env: &[(&str, &str)]) -> Ran {
        let mut full: Vec<std::ffi::OsString> = [
            "/usr/bin/sh",
            "-c",
            "mount -t proc -o nosuid,nodev,noexec proc /proc && mkdir -p /run/systemd && exec \"$@\"",
            "sh",
            "/usr/bin/setpriv",
            &format!("--reuid={BUILDER}"),
            &format!("--regid={BUILDER}"),
            "--init-groups",
            "--reset-env",
            "/usr/bin/env",
            "-i",
            "-C",
            dir,
            &format!("HOME={BUILDER_HOME}"),
            &format!("USER={BUILDER}"),
            &format!("LOGNAME={BUILDER}"),
            "PATH=/usr/bin",
            "LANG=C.UTF-8",
            "SYSTEMD_IN_CHROOT=1",
        ]
        .iter()
        .map(Into::into)
        .collect();
        for (key, value) in env {
            full.push(format!("{key}={value}").into());
        }
        full.extend(argv.iter().map(|a| a.as_ref().to_owned()));
        let refs: Vec<&OsStr> = full.iter().map(|a| a.as_os_str()).collect();
        self.chroot(&refs)
    }

    /// This program again, as root of the user namespace that owns the root's files.
    fn inside(&self, args: &[&OsStr]) -> Ran {
        Ran::from(
            Command::new("unshare")
                .args(UNSHARE)
                .arg("0")
                .arg("--")
                .arg(&self.me)
                .args(args)
                .output(),
            "unshare",
        )
    }

    /// Every path of the root, the subtrees in `skip` left out (`snapshot::take`).
    pub fn snapshot(&self, skip: &[&str]) -> Result<Snapshot, String> {
        let out = self.dir.parent().unwrap_or(&self.dir).join("snapshot.txt");
        let mut args: Vec<&OsStr> = vec![
            OsStr::new("--snapshot"),
            self.dir.as_os_str(),
            out.as_os_str(),
        ];
        for path in skip {
            args.push(OsStr::new("--skip"));
            args.push(OsStr::new(path));
        }
        self.inside(&args).check("the snapshot")?;
        let text = fs::read_to_string(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        let _ = fs::remove_file(&out);
        snapshot::parse(&text)
    }

    /// Removes `dir`, whatever its files' owners, from inside the user namespace.
    pub fn remove(&self, dir: &Path) -> Result<(), String> {
        if fs::symlink_metadata(dir).is_err() {
            return Ok(());
        }
        self.inside(&[OsStr::new("--remove"), dir.as_os_str()])
            .check(&format!("removing {}", dir.display()))
            .map(|_| ())
    }
}

/// Whether this account can build a rootless root here: the tools, and a user namespace with
/// `pacstrap`'s mapping. `Err` says what's missing.
pub fn usable() -> Result<(), String> {
    for tool in ["pacstrap", "arch-chroot", "unshare", "makepkg", "git"] {
        let found = std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(tool).is_file()));
        if !found {
            return Err(format!("{tool} is missing"));
        }
    }
    Ran::from(
        Command::new("unshare")
            .args(UNSHARE)
            .args(["0", "--", "true"])
            .output(),
        "unshare",
    )
    .check("a user namespace with subordinate IDs")
    .map(|_| ())
}
