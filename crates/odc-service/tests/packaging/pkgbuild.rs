use super::*;

/// `package()` installs exactly `FILES.md`'s first table, each file with its mode, from its
/// source; and nothing that enables a unit.
#[test]
fn package_installs_exactly_files_md() {
    let want: BTreeSet<(String, String, String)> = installed()
        .into_iter()
        .filter(|(_, kind, _, _)| kind != "folder")
        .map(|(path, _, mode, from)| (path, mode, from))
        .collect();
    let got = package_installs();
    assert_eq!(got, want);
    for (to, _, _) in &got {
        assert!(
            !to.contains(".wants/") && !to.contains(".requires/") && !to.starts_with("/etc/"),
            "{to}: the package enables nothing and ships nothing in /etc"
        );
    }
}

/// `build()` builds exactly the crates whose programs `package()` installs, so no test crate is
/// built on a user's PC; the functions follow Arch's Rust package guidelines, without
/// `--all-features`, which would start the app's weston tests.
#[test]
fn build_names_exactly_the_crates_whose_programs_ship() {
    let text = arch("PKGBUILD");
    let programs: BTreeSet<String> = installed()
        .into_iter()
        .filter(|(_, kind, _, _)| kind == "program")
        .map(|(path, _, _, _)| path.rsplit('/').next().expect("a name").to_owned())
        .collect();
    let build = commands(&function(&text, "build"));
    let cargo = build
        .iter()
        .find(|command| command.starts_with("cargo build "))
        .expect("build()'s cargo build");
    assert!(
        cargo.starts_with("cargo build --frozen --release -p "),
        "{cargo}"
    );
    let words: Vec<&str> = cargo.split_whitespace().collect();
    let built: BTreeSet<String> = words
        .windows(2)
        .filter(|pair| pair[0] == "-p")
        .map(|pair| pair[1].to_owned())
        .collect();
    assert_eq!(built.len(), words.iter().filter(|w| **w == "-p").count());
    assert_eq!(built, crates_of(&programs));
    assert_eq!(built.len(), 5, "{built:?}");

    let prepare = commands(&function(&text, "prepare"));
    assert!(prepare.contains(&"cargo fetch --locked --target host-tuple".to_owned()));
    let check = commands(&function(&text, "check"));
    assert!(
        check.contains(
            &"cargo test --frozen --workspace --exclude odc-e2e --exclude odc-ui-e2e --exclude \
          odc-pkg-test"
                .to_owned()
        )
    );
    for (name, body) in [("prepare", &prepare), ("build", &build), ("check", &check)] {
        assert!(
            body.contains(&"export RUSTUP_TOOLCHAIN=stable".to_owned()),
            "{name}()"
        );
        assert!(
            body.iter().all(|c| !c.contains("--all-features")),
            "{name}()"
        );
    }
    for (name, body) in [("build", &build), ("check", &check)] {
        assert!(
            body.contains(&"export CARGO_TARGET_DIR=target".to_owned()),
            "{name}()"
        );
    }
}

/// The PKGBUILD's fields: the release's signed tag through git, `v` and the version, a plain
/// `major.minor.patch` (0.1.z the alpha, 0.9.z the beta, 1.0.0 the first release; owner,
/// 2026-11-11), with no `pkgver()` (a release candidate is built from a temporary copy), and the
/// scriptlets' file; `.SRCINFO` holds the same version and dependencies.
#[test]
fn the_pkgbuild_builds_the_signed_tag() {
    let text = arch("PKGBUILD");
    assert_eq!(variable(&text, "pkgname"), "opendeepcool");
    assert_eq!(variable(&text, "arch"), "('x86_64')");
    assert_eq!(variable(&text, "license"), "('MIT')");
    assert_eq!(variable(&text, "install"), "opendeepcool.install");
    assert_eq!(variable(&text, "makedepends"), "('cargo' 'git')");
    // One package, so `pacman -R opendeepcool` removes everything: no split debug package
    // (owner, 2026-11-16), and no other option that changes what makepkg builds or strips.
    assert_eq!(variable(&text, "options"), "('!debug')");
    assert!(
        !text.contains("pkgname=("),
        "no split packages: pkgname is one name"
    );
    assert!(!text.contains("pkgver()"), "no pkgver()");
    assert_eq!(
        variable(&text, "source"),
        "(\"$pkgname::git+$url.git?signed#tag=v$pkgver\")"
    );
    let pkgver = variable(&text, "pkgver");
    let parts: Vec<&str> = pkgver.split('.').collect();
    assert!(
        parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
        "{pkgver}: major.minor.patch, digits only, so git, Cargo and pacman agree"
    );
    let srcinfo = arch(".SRCINFO");
    assert!(srcinfo.contains(&format!("\tpkgver = {pkgver}\n")));
    assert!(srcinfo.contains("\toptions = !debug\n"));
    assert_eq!(
        srcinfo.matches("pkgname = ").count(),
        1,
        ".SRCINFO names one package"
    );
    let depends: Vec<String> = text
        .split("depends=(\n")
        .nth(1)
        .and_then(|rest| rest.split("\n)").next())
        .expect("depends=( … )")
        .lines()
        .map(|line| line.trim().trim_matches('\'').to_owned())
        .collect();
    let listed: Vec<String> = srcinfo
        .lines()
        .filter_map(|line| line.strip_prefix("\tdepends = "))
        .map(str::to_owned)
        .collect();
    assert_eq!(listed, depends);
}

/// The scriptlets enable, start, restart and reload nothing. OpenDeepCool's units are stopped
/// and disabled only in `pre_remove`; `post_remove` runs no `systemctl` at all.
#[test]
fn the_scriptlets_enable_and_start_nothing() {
    let text = arch("opendeepcool.install");
    // The install message prints its commands as text, between `cat <<'EOF'` and `EOF`.
    let mut message = false;
    let run: Vec<&str> = text
        .lines()
        .filter(|line| {
            let inside = message;
            if line.trim_start().starts_with("cat <<'EOF'") {
                message = true;
            } else if *line == "EOF" {
                message = false;
            }
            !inside
        })
        .collect();
    for word in [
        "enable", "start", "restart", "reload", "--now", "preset", "link", "mask",
    ] {
        for line in run
            .iter()
            .filter(|l| l.trim_start().starts_with("systemctl"))
        {
            assert!(
                !line.split_whitespace().any(|w| w == word),
                "{line}: the scriptlets never {word}"
            );
        }
    }
    let code = run.join("\n");
    let systemctl = |name: &str| -> Vec<String> {
        commands(&function(&code, name))
            .into_iter()
            .filter(|line| line.starts_with("systemctl"))
            .collect()
    };
    for name in [
        "post_install",
        "post_remove",
        "_odc_power_counters",
        "_odc_overrides",
    ] {
        assert!(systemctl(name).is_empty(), "{name}()");
    }
    let message: String = function(&text, "post_install").join("\n");
    for command in [
        "sudo systemctl enable --now opendeepcool.service",
        "sudo systemctl --global enable opendeepcool-audio.service",
        "systemctl --user start opendeepcool-audio.service",
    ] {
        assert!(
            message.contains(command),
            "the install message prints {command}"
        );
    }
}
