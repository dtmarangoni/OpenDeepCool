//! Settings v1 (Phase 7 plan, Task 5): defaults, a round trip of every field, invalid and future
//! files moved aside, unknown fields kept, atomic writes, and at most three invalid copies.

use std::fs;
use std::path::PathBuf;

use odc_service::settings::{
    FILE, Fall, Found, SavedInput, Settings, SettingsStore, TemperatureUnit, check_device_path,
};

/// An empty directory of its own under `CARGO_TARGET_TMPDIR`.
fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("settings")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a test directory");
    dir
}

fn invalid_copies(dir: &PathBuf) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("readable")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .filter(|n| n.starts_with(&format!("{FILE}.invalid-")))
        .collect();
    names.sort();
    names
}

/// Every field set, with made-up devices (AGENTS.md rule 10).
fn every_field() -> Settings {
    Settings {
        mode: 5,
        rotation: 2,
        temperature_unit: TemperatureUnit::Fahrenheit,
        gpu_choice: Some("0000:0a:00.0".to_owned()),
        cooler_fan_input: Some(SavedInput {
            chip: "fakeboardchip".to_owned(),
            device_path: "devices/platform/fakeboardchip.2592".to_owned(),
            input: 1,
        }),
        pump_input: Some(SavedInput {
            chip: "fakeboardchip".to_owned(),
            device_path: "devices/platform/fakeboardchip.2592".to_owned(),
            input: 2,
        }),
        music_peaks: false,
        music_bar_fall: Fall::Faster,
        music_peak_fall: Fall::Slower,
        ..Settings::default()
    }
}

#[test]
fn a_fresh_directory_gets_the_defaults_written() {
    let d = dir("fresh");
    let (settings, found) = SettingsStore::new(&d).load().expect("loads");
    assert_eq!(found, Found::Created);
    // settings.md: mode 1, rotation 0, Celsius, no fan inputs.
    let defaults = Settings::default();
    assert_eq!(settings, defaults);
    assert_eq!(
        (settings.mode, settings.rotation, settings.temperature_unit),
        (1, 0, TemperatureUnit::Celsius)
    );
    assert!(settings.gpu_choice.is_none() && settings.cooler_fan_input.is_none());
    let text = fs::read_to_string(d.join(FILE)).expect("written");
    assert!(text.contains("version = 1"), "{text}");
    let (again, found) = SettingsStore::new(&d).load().expect("loads");
    assert_eq!((again, found), (defaults, Found::Read));
}

#[test]
fn every_field_round_trips() {
    let d = dir("round-trip");
    let store = SettingsStore::new(&d);
    store.store(&every_field()).expect("stores");
    let (settings, found) = store.load().expect("loads");
    assert_eq!((settings, found), (every_field(), Found::Read));
}

/// Writes `text` as the file and loads it; returns what was found and where the file went.
fn load_text(name: &str, text: &str) -> (Settings, Found, PathBuf) {
    let d = dir(name);
    fs::write(d.join(FILE), text).expect("a file");
    let (settings, found) = SettingsStore::new(&d).load().expect("loads");
    (settings, found, d)
}

#[test]
fn a_file_out_of_form_is_moved_aside_and_the_defaults_used() {
    let base = "version = 1\nmode = 4\nrotation = 1\ntemperature_unit = \"celsius\"\n";
    let input = |path: &str| {
        format!(
            "{base}[pump_input]\nchip = \"fakeboardchip\"\ndevice_path = \"{path}\"\ninput = 2\n"
        )
    };
    let cases = [
        ("garbage", "this isn't = = toml".to_owned()),
        ("mode", base.replace("mode = 4", "mode = 24")),
        ("rotation", base.replace("rotation = 1", "rotation = 4")),
        ("unit", base.replace("celsius", "kelvin")),
        ("type", base.replace("mode = 4", "mode = \"four\"")),
        ("missing", base.replace("rotation = 1\n", "")),
        ("gpu-form", format!("{base}gpu_choice = \"0000:0A:00.0\"\n")),
        (
            "gpu-junk",
            format!("{base}gpu_choice = \"../../etc/shadow\"\n"),
        ),
        ("dotdot", input("devices/../../etc")),
        ("absolute", input("/etc")),
        ("sysfs-absolute", input("/sys/devices/platform/x")),
        ("dot", input("devices/./platform")),
        ("long", input(&format!("devices/{}", "a".repeat(1100)))),
        ("future", base.replace("version = 1", "version = 2")),
        ("stop", format!("{base}music_bar_fall = \"fastest\"\n")),
        ("stop-case", format!("{base}music_peak_fall = \"Slow\"\n")),
        ("peaks-type", format!("{base}music_peaks = \"yes\"\n")),
    ];
    for (name, text) in cases {
        let (settings, found, d) = load_text(name, &text);
        assert_eq!(settings, Settings::default(), "{name}");
        let Found::Replaced { moved_to, why } = found else {
            panic!("{name}: {found:?}");
        };
        assert!(!why.is_empty(), "{name}");
        assert_eq!(
            fs::read_to_string(&moved_to).expect("kept"),
            text,
            "{name}: kept as it was"
        );
        assert_eq!(invalid_copies(&d).len(), 1, "{name}");
        let written = fs::read_to_string(d.join(FILE)).expect("the defaults");
        assert!(written.contains("mode = 1"), "{name}: {written}");
    }
}

#[test]
fn a_future_version_is_never_downgraded_silently() {
    let text = "version = 2\nmode = 4\nrotation = 1\ntemperature_unit = \"celsius\"\nnew = true\n";
    let (_, found, _) = load_text("future-version", text);
    let Found::Replaced { why, .. } = found else {
        panic!("{found:?}");
    };
    assert!(why.contains("version 2"), "{why}");
}

#[test]
fn a_field_a_later_build_added_is_kept_through_a_load_and_a_store() {
    let text = "version = 1\nmode = 3\nrotation = 0\ntemperature_unit = \"celsius\"\n\
                music_peak_dots = false\n\n[music]\nbar_fall = \"moderate\"\n";
    let (mut settings, found, d) = load_text("later-field", text);
    assert_eq!(found, Found::Read);
    assert_eq!(
        settings.later.get("music_peak_dots"),
        Some(&toml::Value::Boolean(false))
    );
    settings.rotation = 3;
    let store = SettingsStore::new(&d);
    store.store(&settings).expect("stores");
    let (back, _) = store.load().expect("loads");
    assert_eq!(back.rotation, 3);
    assert_eq!(back.later, settings.later, "the later fields, unchanged");
    let written = fs::read_to_string(d.join(FILE)).expect("written");
    assert!(written.contains("bar_fall = \"moderate\""), "{written}");
}

#[test]
fn an_interrupted_write_leaves_the_good_file() {
    let d = dir("interrupted");
    let store = SettingsStore::new(&d);
    store.store(&every_field()).expect("stores");
    // A write cut short: the temporary file left behind, half written.
    fs::write(d.join(format!("{FILE}.new")), "version = 1\nmode = ").expect("a temp file");
    let (settings, found) = store.load().expect("loads");
    assert_eq!((settings, found), (every_field(), Found::Read));
    // The next write replaces the leftover.
    store.store(&Settings::default()).expect("stores");
    assert!(!d.join(format!("{FILE}.new")).exists());
    assert_eq!(store.load().expect("loads").0, Settings::default());
}

#[test]
fn only_the_last_three_invalid_copies_are_kept() {
    let d = dir("three-copies");
    let store = SettingsStore::new(&d);
    for i in 0..4 {
        fs::write(d.join(FILE), format!("broken {i}")).expect("a file");
        store.load().expect("loads");
    }
    let copies = invalid_copies(&d);
    assert_eq!(copies.len(), 3, "{copies:?}");
    let kept: Vec<String> = copies
        .iter()
        .map(|n| fs::read_to_string(d.join(n)).expect("a copy"))
        .collect();
    assert!(!kept.contains(&"broken 0".to_owned()), "the oldest went");
    assert!(kept.contains(&"broken 3".to_owned()), "the newest stays");
}

#[test]
fn a_link_in_the_file_s_place_is_moved_aside_never_followed() {
    let d = dir("link");
    let target = d.join("elsewhere");
    fs::write(
        &target,
        "version = 1\nmode = 2\nrotation = 0\ntemperature_unit = \"celsius\"\n",
    )
    .expect("a target");
    std::os::unix::fs::symlink(&target, d.join(FILE)).expect("a link");
    let (settings, found) = SettingsStore::new(&d).load().expect("loads");
    assert_eq!(settings, Settings::default());
    assert!(matches!(found, Found::Replaced { .. }), "{found:?}");
    assert!(target.exists(), "the link's target is left alone");
}

#[test]
fn device_paths_are_relative_to_the_sysfs_root() {
    for good in [
        "devices/platform/fakeboardchip.2592",
        "devices/pci0000:00/0000:00:1f.3/hwmon",
    ] {
        assert_eq!(check_device_path(good), Ok(()), "{good}");
    }
    for bad in [
        "",
        "devices",
        "devices/",
        "/devices/x",
        "devices//x",
        "class/hwmon/hwmon3",
        "devices/x/..",
    ] {
        assert!(check_device_path(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn settings_out_of_form_are_never_written() {
    let d = dir("never-written");
    let store = SettingsStore::new(&d);
    let bad = Settings {
        rotation: 9,
        ..Settings::default()
    };
    assert!(store.store(&bad).is_err());
    assert!(!d.join(FILE).exists());
}

// Phase 9 plan, Task 3: a v1 file written before the Music settings loads with Winamp's defaults,
// peak dots on, bars moderate, peaks slow (music.md point 7), and the next store writes them.
#[test]
fn a_file_from_before_the_music_settings_gets_their_defaults() {
    let text = "version = 1\nmode = 4\nrotation = 1\ntemperature_unit = \"celsius\"\n";
    let (settings, found, d) = load_text("before-music", text);
    assert_eq!(found, Found::Read);
    assert_eq!(
        (
            settings.music_peaks,
            settings.music_bar_fall,
            settings.music_peak_fall
        ),
        (true, Fall::Moderate, Fall::Slow)
    );
    assert_eq!(settings.mode, 4, "the rest as it was");
    let store = SettingsStore::new(&d);
    store.store(&settings).expect("stores");
    let written = fs::read_to_string(d.join(FILE)).expect("written");
    for line in [
        "music_peaks = true",
        "music_bar_fall = \"moderate\"",
        "music_peak_fall = \"slow\"",
    ] {
        assert!(written.contains(line), "{line}: {written}");
    }
    assert_eq!(settings.music(), odc_render::MusicSettings::default());
}

// The five stops by their names, as the setters take them (music.md point 4).
#[test]
fn each_fall_stop_is_known_by_its_name() {
    for (name, fall) in [
        ("slower", Fall::Slower),
        ("slow", Fall::Slow),
        ("moderate", Fall::Moderate),
        ("fast", Fall::Fast),
        ("faster", Fall::Faster),
    ] {
        assert_eq!(Fall::from_name(name), Some(fall));
        assert_eq!(fall.name(), name);
    }
    assert_eq!(Fall::from_name("fastest"), None);
}
