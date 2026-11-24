use super::*;

/// The removal undoes every change made to OpenDeepCool's units and files under its names
/// (owner, 2026-11-11): first the files an administrator made (copies, masks and drop-ins, and
/// same-named overrides), since `systemctl disable` skips a masked unit, then every link to the
/// units. In a chroot nothing in `/run` is touched, nor is the service stopped; and the service
/// is stopped only where systemd runs.
#[test]
fn the_removal_undoes_every_change_under_opendeepcools_names() {
    let names = |folder: &str| -> String {
        installed()
            .into_iter()
            .filter_map(|(path, ..)| {
                path.strip_prefix(folder)
                    .filter(|name| !name.contains('/'))
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let (system, user) = (
        names("/usr/lib/systemd/system/"),
        names("/usr/lib/systemd/user/"),
    );
    for (chroot, booted) in [(false, true), (false, false), (true, false), (true, true)] {
        let runs = pre_remove_runs(chroot, booted);
        let removed: BTreeSet<String> = runs
            .iter()
            .filter_map(|line| line.strip_prefix("rm -rfv "))
            .flat_map(|paths| paths.split(' ').map(str::to_owned))
            .collect();
        assert_eq!(removed, overrides(chroot), "chroot {chroot}");
        let mut wanted = Vec::new();
        if !chroot && booted {
            wanted.push("systemctl stop opendeepcool.service".to_owned());
        }
        wanted.push(format!("systemctl -q --global disable {user}"));
        wanted.push(format!("systemctl -q disable {system}"));
        if !chroot {
            wanted.push(format!("systemctl -q --global --runtime disable {user}"));
            wanted.push(format!("systemctl -q --runtime disable {system}"));
        }
        let rest: Vec<String> = runs
            .iter()
            .filter(|line| !line.starts_with("rm -rfv "))
            .cloned()
            .collect();
        assert_eq!(rest, wanted, "chroot {chroot}, booted {booted}");
        // The stop comes before anything is removed, and every removal before the links.
        let first_link = runs.iter().position(|l| l.contains(" disable ")).unwrap();
        let last_rm = runs.iter().rposition(|l| l.starts_with("rm ")).unwrap();
        assert!(last_rm < first_link, "{runs:#?}");
        if !chroot && booted {
            assert!(runs[0].starts_with("systemctl stop"), "{runs:#?}");
        }
    }
}

/// The restart hook: after an upgrade of this package, systemd's own script restarts the service
/// if it runs; its file name sorts after systemd's numbered hooks and `dbus-reload.hook`, and,
/// on an Arch system, after every systemd and D-Bus hook installed (alpm-hooks(5): hooks run in
/// the alphabetical order of their file names).
#[test]
fn the_restart_hook_runs_after_systemds_and_dbus_s() {
    let hooks: Vec<String> = fs::read_dir(packaging("arch/hooks"))
        .expect("packaging/arch/hooks")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(hooks, ["opendeepcool-restart.hook"]);
    let name = hooks[0].as_str();
    let text = arch(&format!("hooks/{name}"));
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        lines,
        [
            "[Trigger]",
            "Operation = Upgrade",
            "Type = Package",
            "Target = opendeepcool",
            "[Action]",
            "Description = Restart a running opendeepcool.service",
            "When = PostTransaction",
            "Exec = /usr/share/libalpm/scripts/systemd-hook restart opendeepcool.service",
        ]
    );
    // systemd's hooks are numbered; D-Bus's is `dbus-reload.hook`.
    for before in ["99-", "dbus-reload.hook"] {
        assert!(name > before, "{name} runs before {before}…");
    }
    if let Ok(hooks) = fs::read_dir("/usr/share/libalpm/hooks") {
        for hook in hooks.flatten() {
            let other = hook.file_name().to_string_lossy().into_owned();
            if other.contains("systemd") || other.contains("dbus") {
                assert!(name > other.as_str(), "{other} runs after {name}");
            }
        }
    }
}

/// The package sources' own license is the 0BSD text Arch publishes for them, word for word
/// (Arch package guidelines, Package sources licenses; AUR submission guidelines).
#[test]
fn the_package_sources_license_is_arch_s_0bsd() {
    assert_eq!(
        arch("LICENSE"),
        "Copyright Arch Linux Contributors\n\
         \n\
         Permission to use, copy, modify, and/or distribute this software for\n\
         any purpose with or without fee is hereby granted.\n\
         \n\
         THE SOFTWARE IS PROVIDED “AS IS” AND THE AUTHOR DISCLAIMS ALL\n\
         WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES\n\
         OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE\n\
         FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY\n\
         DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN\n\
         AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT\n\
         OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.\n"
    );
}
