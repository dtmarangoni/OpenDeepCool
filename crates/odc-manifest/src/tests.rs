use super::*;

/// What the development setup writes in Phase 5.
fn setup_manifest() -> Manifest {
    let mut m = Manifest::default();
    for f in [
        "/etc/sysusers.d/opendeepcool.conf",
        "/etc/udev/rules.d/70-opendeepcool.rules",
        "/etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules",
    ] {
        m.push(Entry::file(f).expect("a dev file")).expect("new");
    }
    m.push(Entry::Group).expect("new");
    m.push(Entry::member("dev_user").expect("a user"))
        .expect("new");
    m
}

#[test]
fn what_the_setup_writes_is_accepted() {
    let m = setup_manifest();
    assert_eq!(
        m.render(),
        "file /etc/sysusers.d/opendeepcool.conf\n\
             file /etc/udev/rules.d/70-opendeepcool.rules\n\
             file /etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules\n\
             group opendeepcool\n\
             member opendeepcool dev_user\n"
    );
    assert_eq!(Manifest::parse(&m.render()), Ok(m));
    assert_eq!(Manifest::parse(""), Ok(Manifest::default()));
}

/// What the setup adds in Phase 7, after Phase 5's entries: the dev copy of the service.
fn service_entries() -> Vec<Entry> {
    let mut entries: Vec<Entry> = [
        "/usr/local/libexec",
        "/usr/local/libexec/opendeepcool-dev",
        "/etc/systemd/system/opendeepcool.service.d",
        "/etc/systemd/system/opendeepcool-gpu-rank.service.d",
        "/etc/dbus-1",
        "/etc/dbus-1/system.d",
        "/etc/polkit-1/actions",
    ]
    .into_iter()
    .map(|d| Entry::dir(d).expect("a created folder"))
    .collect();
    for f in [
        "/usr/local/libexec/opendeepcool-dev/opendeepcoold",
        "/usr/local/libexec/opendeepcool-dev/odc-gpu-rank",
        "/etc/systemd/system/opendeepcool.service",
        "/etc/systemd/system/opendeepcool-gpu-rank.service",
        "/etc/systemd/system/opendeepcool.service.d/dev.conf",
        "/etc/systemd/system/opendeepcool-gpu-rank.service.d/dev.conf",
        "/etc/dbus-1/system.d/io.github.dtmarangoni.OpenDeepCool1.conf",
        "/etc/polkit-1/actions/io.github.dtmarangoni.OpenDeepCool1.policy",
    ] {
        entries.push(Entry::file(f).expect("a dev file"));
    }
    entries.extend([Entry::User, Entry::State, Entry::Enabled]);
    entries
}

#[test]
fn what_the_setup_writes_for_the_service_is_accepted() {
    let mut m = setup_manifest();
    for entry in service_entries() {
        m.push(entry).expect("new");
    }
    let text = m.render();
    assert!(text.ends_with(
        "user opendeepcool\nstate /var/lib/opendeepcool\nenabled opendeepcool.service\n"
    ));
    assert_eq!(Manifest::parse(&text), Ok(m));
}

// Phase 9 plan, Task 4: the sound helper's binary, its user unit and dev drop-in, the
// drop-in's folder, and the unit enabled for every user, last.
#[test]
fn what_the_setup_writes_for_the_sound_helper_is_accepted() {
    let mut m = setup_manifest();
    for entry in service_entries() {
        m.push(entry).expect("new");
    }
    m.push(Entry::dir("/etc/systemd/user/opendeepcool-audio.service.d").expect("created"))
        .expect("new");
    for f in [
        "/usr/local/libexec/opendeepcool-dev/opendeepcool-audio",
        "/etc/systemd/user/opendeepcool-audio.service",
        "/etc/systemd/user/opendeepcool-audio.service.d/dev.conf",
    ] {
        m.push(Entry::file(f).expect("a dev file")).expect("new");
    }
    m.push(Entry::EnabledForEveryUser).expect("new");
    let text = m.render();
    assert!(text.ends_with("enabled-for-every-user opendeepcool-audio.service\n"));
    assert_eq!(Manifest::parse(&text), Ok(m));
}

// Phase 10 plan, Task 9: the settings app's binary, its launcher entry and icon, and the
// folders under /usr/local/share they need; nothing else of /usr/local/share.
#[test]
fn what_the_setup_writes_for_the_settings_app_is_accepted() {
    let mut m = setup_manifest();
    for dir in [
        "/usr/local/share/applications",
        "/usr/local/share/icons",
        "/usr/local/share/icons/hicolor",
        "/usr/local/share/icons/hicolor/scalable",
        "/usr/local/share/icons/hicolor/scalable/apps",
    ] {
        m.push(Entry::dir(dir).expect("created")).expect("new");
    }
    for f in [
        "/usr/local/libexec/opendeepcool-dev/opendeepcool",
        "/usr/local/share/applications/io.github.dtmarangoni.OpenDeepCool.desktop",
        "/usr/local/share/icons/hicolor/scalable/apps/io.github.dtmarangoni.OpenDeepCool.svg",
    ] {
        m.push(Entry::file(f).expect("a dev file")).expect("new");
    }
    assert_eq!(Manifest::parse(&m.render()), Ok(m));
    for refused in [
        "/usr/local/share/applications/firefox.desktop",
        "/usr/local/share/icons/hicolor/scalable/apps/other.svg",
        "/usr/local/share/icons/hicolor/index.theme",
    ] {
        assert!(Entry::file(refused).is_err(), "{refused}");
    }
    for refused in ["/usr/local/share", "/usr/local/share/man"] {
        assert!(Entry::dir(refused).is_err(), "{refused}");
    }
}

#[test]
fn only_the_files_the_service_and_its_helper_write_are_state_files() {
    for name in [
        "settings.toml",
        "settings.toml.new",
        "settings.toml.invalid-1793232821",
        "settings.toml.invalid-1793232821-2",
        "gpu-rank.json",
        ".gpu-rank.json.new",
        "dev-frames.log",
        "dev-frames.log.1",
    ] {
        assert!(is_state_file(name), "{name}");
    }
    for name in [
        "settings.toml.invalid-",
        "settings.toml.invalid-x",
        "settings.toml.old",
        "dev-frames.log.2",
        "notes.txt",
        "..",
    ] {
        assert!(!is_state_file(name), "{name}");
    }
}

#[test]
fn a_malformed_manifest_is_refused_whole() {
    let good = "file /etc/udev/rules.d/70-opendeepcool.rules\n";
    let refused = [
        ("\n", "isn't `<kind> <value>`"),
        ("file /", "isn't a plain path"),
        ("file /etc/udev/rules.d/../shadow", "isn't a plain path"),
        (
            "file /etc/udev/rules.d/./70-opendeepcool.rules",
            "isn't a plain path",
        ),
        (
            "file /etc/udev//rules.d/70-opendeepcool.rules",
            "isn't a plain path",
        ),
        (
            "file etc/udev/rules.d/70-opendeepcool.rules",
            "isn't an absolute path",
        ),
        ("file /etc/passwd", "isn't an OpenDeepCool file"),
        (
            "file /etc/udev/rules.d/99-other.rules",
            "isn't an OpenDeepCool file",
        ),
        (
            "file /etc/udev/rules.d/x/opendeepcool.rules",
            "isn't an OpenDeepCool file",
        ),
        ("group wheel", "isn't opendeepcool"),
        (
            "member wheel dev_user",
            "isn't `member opendeepcool <user>`",
        ),
        ("member opendeepcool -a", "isn't a user name"),
        ("member opendeepcool Dev", "isn't a user name"),
        ("dir /etc/udev/rules.d", "isn't a folder the setup creates"),
        ("dir /etc", "isn't a folder the setup creates"),
        (
            "dir /usr/local/libexec/opendeepcool-dev/..",
            "isn't a folder the setup creates",
        ),
        (
            "file /usr/local/libexec/opendeepcool-dev/x/opendeepcoold",
            "isn't an OpenDeepCool file",
        ),
        ("file /usr/local/libexec/sh", "isn't an OpenDeepCool file"),
        (
            "file /etc/systemd/system/sshd.service",
            "isn't an OpenDeepCool file",
        ),
        (
            "file /etc/systemd/system/opendeepcool.service.d/../../shadow",
            "isn't a plain path",
        ),
        ("user root", "isn't opendeepcool"),
        ("state /var/lib", "isn't /var/lib/opendeepcool"),
        (
            "state /var/lib/opendeepcool/..",
            "isn't /var/lib/opendeepcool",
        ),
        ("enabled sshd.service", "isn't opendeepcool.service"),
        (
            "enabled-for-every-user pipewire.service",
            "isn't opendeepcool-audio.service",
        ),
        (
            "file /etc/systemd/user/pipewire.service",
            "isn't an OpenDeepCool file",
        ),
        ("dir /etc/systemd/user", "isn't a folder the setup creates"),
        ("tmpfiles /etc/tmpfiles.d", "isn't a kind of entry"),
        (good.trim_end(), "is there twice"),
    ];
    for (bad, reason) in refused {
        let text = format!("{good}{bad}\n");
        let refusal = Manifest::parse(&text).expect_err(bad);
        assert_eq!(refusal.line, 2, "{bad:?}");
        assert!(
            refusal.reason.contains(reason),
            "{bad:?}: {}",
            refusal.reason
        );
    }
}
