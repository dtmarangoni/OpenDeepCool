//! The Arch package's files through Arch's own tools (Phase 11 plan, Task 4): `.SRCINFO` is what
//! `makepkg --printsrcinfo` writes, shellcheck finds nothing in the scriptlets nor in the
//! PKGBUILD with makepkg's variables declared, namcap finds no error in the PKGBUILD, and the
//! workspace builds with Arch's `rust` package. Here, not in `odc-service`'s packaging tests,
//! because the PKGBUILD's own `check()` leaves this crate out: these tools aren't the package's
//! build dependencies. A missing tool fails with the package that brings it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn arch() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/arch")
}

/// `program`'s output, run with `args` in `dir`; a missing program fails with its package.
fn run(program: &str, package: &str, args: &[&str], dir: &Path) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("{program}: {e}; fix: pacman -S {package}"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Whether this test runs as root: CI's container does, and makepkg refuses root.
fn root() -> bool {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("Uid:"))
                .and_then(|ids| ids.split_whitespace().nth(1).map(|euid| euid == "0"))
        })
        .unwrap_or(false)
}

/// `.SRCINFO` is current: what makepkg writes from the PKGBUILD. makepkg refuses root, so as root
/// it runs as `nobody` (util-linux's setpriv), with its build, package and source folders in
/// `/tmp`: it checks they're writable even to print, and `nobody` can't write the checkout.
#[test]
fn srcinfo_is_what_makepkg_writes() {
    let dir = arch();
    let out = if root() {
        run(
            "setpriv",
            "util-linux",
            &[
                "--reuid=65534",
                "--regid=65534",
                "--clear-groups",
                "env",
                "-i",
                "PATH=/usr/bin",
                "HOME=/",
                "BUILDDIR=/tmp",
                "PKGDEST=/tmp",
                "SRCDEST=/tmp",
                "makepkg",
                "--printsrcinfo",
            ],
            &dir,
        )
    } else {
        run("makepkg", "pacman", &["--printsrcinfo"], &dir)
    };
    assert!(out.status.success(), "makepkg: {}", text(&out.stderr));
    let committed = fs::read_to_string(dir.join(".SRCINFO")).expect(".SRCINFO");
    assert_eq!(
        committed,
        text(&out.stdout),
        "fix: cd packaging/arch && makepkg --printsrcinfo > .SRCINFO"
    );
}

/// shellcheck finds nothing in the scriptlets (POSIX sh, as their first line says), nor in the
/// PKGBUILD read as makepkg reads it: with `pkgdir` and `srcdir` set, the variables makepkg
/// reads used, and SC2164 (a `cd` without `|| exit`) left out, since makepkg runs each function
/// with errexit, so a failed `cd` stops the build.
#[test]
fn shellcheck_finds_nothing() {
    let dir = arch();
    let out = run("shellcheck", "shellcheck", &["opendeepcool.install"], &dir);
    assert!(
        out.status.success(),
        "opendeepcool.install: {}",
        text(&out.stdout)
    );
    let pkgbuild = fs::read_to_string(dir.join("PKGBUILD")).expect("the PKGBUILD");
    let framed = format!(
        "# shellcheck shell=bash\npkgdir=/pkgdir srcdir=/srcdir\n{pkgbuild}\n: \"$pkgver \
         $pkgrel $pkgdesc ${{arch[*]}} $url ${{license[*]}} ${{depends[*]}} ${{makedepends[*]}} \
         ${{optdepends[*]}} $install ${{source[*]}} ${{validpgpkeys[*]}} ${{sha256sums[*]}} \
         $srcdir\"\n"
    );
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("arch-tools");
    fs::create_dir_all(&scratch).expect("a scratch folder");
    let file = scratch.join("PKGBUILD.sh");
    fs::write(&file, framed).expect("the framed PKGBUILD");
    let out = run(
        "shellcheck",
        "shellcheck",
        &["--exclude=SC2164", "PKGBUILD.sh"],
        &scratch,
    );
    assert!(out.status.success(), "PKGBUILD: {}", text(&out.stdout));
}

/// namcap finds no error in the PKGBUILD; any warning is printed for the report.
#[test]
fn namcap_finds_no_error_in_the_pkgbuild() {
    let out = run("namcap", "namcap", &["PKGBUILD"], &arch());
    let said = text(&out.stdout);
    if !said.is_empty() {
        println!("namcap: {said}");
    }
    assert!(out.status.success(), "namcap: {}", text(&out.stderr));
    assert!(
        !said.lines().any(|line| line.contains(" E: ")),
        "namcap: {said}"
    );
}

/// `major.minor` of a version such as `1.99`, `1.99.1` or pacman's `1:1.99.1-1`.
fn major_minor(version: &str) -> Option<(u32, u32)> {
    let version = version.rsplit_once(':').map_or(version, |(_, v)| v);
    let version = version.split_once('-').map_or(version, |(v, _)| v);
    let mut parts = version.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// The workspace's `rust-version` is at or below Arch's `rust` package, so an AUR user builds it
/// with the distribution's compiler (solution review P11-07). pacman reads its synced database.
#[test]
fn the_workspace_builds_with_arch_s_rust() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml");
    let wanted = manifest
        .lines()
        .find_map(|line| line.strip_prefix("rust-version = "))
        .map(|v| v.trim_matches('"'))
        .and_then(major_minor)
        .expect("the workspace's rust-version");
    let out = run("pacman", "pacman", &["-Si", "rust"], &root);
    assert!(
        out.status.success(),
        "pacman -Si rust: {}; fix: pacman -Sy",
        text(&out.stderr)
    );
    let arch = text(&out.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Version"))
        .and_then(|rest| rest.split_once(':').map(|(_, v)| v.trim().to_owned()))
        .and_then(|v| major_minor(&v))
        .expect("rust's version");
    assert!(
        wanted <= arch,
        "rust-version {wanted:?} is newer than Arch's rust {arch:?}"
    );
    assert_eq!(major_minor("1:1.99.1-1"), Some((1, 99)));
    assert_eq!(major_minor("1.100"), Some((1, 100)));
}
