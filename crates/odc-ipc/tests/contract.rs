//! The D-Bus contract (Phase 7 plan, Task 4): the names are valid D-Bus names, the errors carry
//! the prefix, the state round-trips through `a{sv}` and the D-Bus encoding, and
//! `docs/spec/dbus.md` lists exactly the crate's state keys, each with its type, and its errors.
//! The service's introspection is checked against the document's XML in Task 7.

use std::collections::HashMap;
use std::fs;

use odc_ipc::{
    BUS_NAME, CHANGE_SETTINGS_ACTION, CoolerState, DEV_INTERFACE, DEV_PATH, ERROR_PREFIX, Error,
    FEED_AUDIO_ACTION, FallStop, FanInput, FanSpeed, Gpu, GpuKind, INTERFACE, Input, OBJECT_PATH,
    STATE_KEYS, State, StateError, TemperatureUnit, USE_ACTION,
};
use zbus::DBusError;
use zbus::names::{ErrorName, InterfaceName, WellKnownName};
use zbus::wire::serialized::Context;
use zbus::wire::{LE, ObjectPath, OwnedVariant, Variant, to_bytes};

const ERRORS: [&str; 8] = [
    "NotAuthorized",
    "ModeUnavailable",
    "NotYetAvailable",
    "InvalidArgument",
    "TooManyWatchers",
    "Busy",
    "AudioNotWanted",
    "Failed",
];

#[test]
fn the_names_are_valid_dbus_names() {
    WellKnownName::try_from(BUS_NAME).expect("a well-known name");
    InterfaceName::try_from(INTERFACE).expect("an interface name");
    InterfaceName::try_from(DEV_INTERFACE).expect("an interface name");
    ObjectPath::try_from(OBJECT_PATH).expect("an object path");
    ObjectPath::try_from(DEV_PATH).expect("an object path");
    for name in ERRORS {
        ErrorName::try_from(format!("{ERROR_PREFIX}.{name}")).expect("an error name");
    }
    // polkit(8): an action id is made of [A-Z][a-z][0-9].-
    for action in [USE_ACTION, CHANGE_SETTINGS_ACTION, FEED_AUDIO_ACTION] {
        assert!(
            action
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
            "{action}"
        );
        assert!(action.starts_with(&format!("{BUS_NAME}.")), "{action}");
    }
}

#[test]
fn each_error_is_named_with_the_prefix() {
    let errors = [
        Error::NotAuthorized("x".into()),
        Error::ModeUnavailable("x".into()),
        Error::NotYetAvailable("x".into()),
        Error::InvalidArgument("x".into()),
        Error::TooManyWatchers("x".into()),
        Error::Busy("x".into()),
        Error::AudioNotWanted("x".into()),
        Error::Failed("x".into()),
    ];
    assert_eq!(errors.len(), ERRORS.len());
    for (error, name) in errors.iter().zip(ERRORS) {
        assert_eq!(error.name().as_str(), format!("{ERROR_PREFIX}.{name}"));
        assert_eq!(error.description(), Some("x"));
    }
}

/// Every key present, with made-up devices (AGENTS.md rule 10).
fn full() -> State {
    State {
        mode: Some(4),
        rotation: Some(3),
        temperature_unit: Some(TemperatureUnit::Fahrenheit),
        cooler_connected: Some(false),
        cooler_state: Some(CoolerState::NoAccess),
        available_modes: Some(vec![1, 3, 5]),
        unavailable_reasons: Some(HashMap::from([
            (2, "no device found".to_owned()),
            (6, "no permission to read it".to_owned()),
        ])),
        gpus: Some(vec![
            Gpu {
                pci: "0000:0a:00.0".to_owned(),
                driver: "amdgpu".to_owned(),
                kind: GpuKind::Dedicated,
                vendor: 0x0abc,
                device: 0x0def,
                has_rule: true,
            },
            Gpu {
                pci: "0000:00:02.0".to_owned(),
                driver: "othergpu".to_owned(),
                kind: GpuKind::QueryFailed,
                vendor: 0,
                device: 0,
                has_rule: false,
            },
        ]),
        chosen_gpu: Some("0000:0a:00.0".to_owned()),
        gpu_in_use: Some("0000:0a:00.0".to_owned()),
        fan_inputs: Some(vec![FanInput {
            chip: "fakeboardchip".to_owned(),
            device: "devices/platform/fakeboardchip.2592".to_owned(),
            input: 2,
            label: String::new(),
            rpm: FanSpeed::Rpm(2345),
        }]),
        cooler_fan_input: Some(Some(Input {
            chip: "fakeboardchip".to_owned(),
            device: "devices/platform/fakeboardchip.2592".to_owned(),
            input: 1,
        })),
        pump_input: Some(None),
        cooler_fan_input_present: Some(false),
        pump_input_present: Some(true),
        cooler_fan_rpm: Some(FanSpeed::Rpm(1234)),
        pump_rpm: Some(FanSpeed::Unavailable),
        music_peaks: Some(false),
        music_bar_fall: Some(FallStop::Faster),
        music_peak_fall: Some(FallStop::Slower),
        audio_connected: Some(true),
        audio_wanted: Some(true),
    }
}

#[test]
fn the_state_round_trips_through_a_dictionary_and_the_wire() {
    let state = full();
    let dict = state.to_dict();
    assert_eq!(dict.len(), STATE_KEYS.len());
    assert_eq!(State::from_dict(&dict), Ok(state.clone()));
    // Through the D-Bus encoding, as GetState sends it.
    let bytes = to_bytes(Context::new(LE, 0), &dict).expect("encodes");
    let (back, _): (HashMap<String, OwnedVariant>, usize) = bytes.deserialize().expect("decodes");
    assert_eq!(State::from_dict(&back), Ok(state));
}

#[test]
fn each_key_has_the_contract_s_type() {
    let dict = full().to_dict();
    for (key, signature) in STATE_KEYS {
        let value = dict.get(key).unwrap_or_else(|| panic!("{key} is sent"));
        assert_eq!(value.value_signature().to_string(), signature, "{key}");
    }
}

fn variant<'a>(value: impl Into<Variant<'a>>) -> OwnedVariant {
    OwnedVariant::try_from(value.into()).expect("no file descriptor")
}

#[test]
fn an_unknown_key_is_ignored_and_a_missing_key_stays_absent() {
    let mut dict = State {
        mode: Some(4),
        ..State::default()
    }
    .to_dict();
    // A key a later phase adds, which an older client doesn't know.
    dict.insert("AKeyOfALaterPhase".to_owned(), variant(true));
    let state = State::from_dict(&dict).expect("still a state");
    assert_eq!(
        state,
        State {
            mode: Some(4),
            ..State::default()
        }
    );
}

#[test]
fn a_known_key_with_another_type_or_a_value_outside_the_contract_is_refused() {
    let one = |key: &str, value: OwnedVariant| HashMap::from([(key.to_owned(), value)]);
    assert_eq!(
        State::from_dict(&one("Mode", variant("four"))),
        Err(StateError::WrongType("Mode"))
    );
    assert_eq!(
        State::from_dict(&one("Rotation", variant(4_u8))),
        Err(StateError::BadValue("Rotation", "4".into()))
    );
    assert_eq!(
        State::from_dict(&one("TemperatureUnit", variant("kelvin"))),
        Err(StateError::BadValue("TemperatureUnit", "kelvin".into()))
    );
    assert_eq!(
        State::from_dict(&one("CoolerState", variant("on fire"))),
        Err(StateError::BadValue("CoolerState", "on fire".into()))
    );
    for key in ["MusicBarFall", "MusicPeakFall"] {
        assert!(
            matches!(
                State::from_dict(&one(key, variant("fastest"))),
                Err(StateError::BadValue(k, v)) if k == key && v == "fastest"
            ),
            "{key}"
        );
    }
    assert_eq!(
        State::from_dict(&one("AudioWanted", variant("yes"))),
        Err(StateError::WrongType("AudioWanted"))
    );
    let gpus = vec![(
        "0000:0a:00.0".to_owned(),
        "amdgpu".to_owned(),
        "big".to_owned(),
        0_u16,
        0_u16,
        true,
    )];
    assert_eq!(
        State::from_dict(&one("Gpus", variant(gpus))),
        Err(StateError::BadValue("Gpus", "big".into()))
    );
    // Phase 9's shapes of Gpus and FanInputs, without the IDs, the rule flag and the RPM.
    let old_gpus = vec![(
        "0000:0a:00.0".to_owned(),
        "amdgpu".to_owned(),
        "dedicated".to_owned(),
    )];
    assert_eq!(
        State::from_dict(&one("Gpus", variant(old_gpus))),
        Err(StateError::WrongType("Gpus"))
    );
    let old_inputs = vec![(
        "chip".to_owned(),
        "devices/x".to_owned(),
        1_u32,
        String::new(),
    )];
    assert_eq!(
        State::from_dict(&one("FanInputs", variant(old_inputs))),
        Err(StateError::WrongType("FanInputs"))
    );
    let inputs = vec![(
        "chip".to_owned(),
        "devices/x".to_owned(),
        1_u32,
        String::new(),
        12_000.0,
    )];
    assert!(matches!(
        State::from_dict(&one("FanInputs", variant(inputs))),
        Err(StateError::BadValue("FanInputs", _))
    ));
    for rpm in [10_000.0, -1.0, 1.5, f64::INFINITY] {
        assert!(
            matches!(
                State::from_dict(&one("PumpRpm", variant(rpm))),
                Err(StateError::BadValue("PumpRpm", _))
            ),
            "{rpm}"
        );
    }
}

#[test]
fn merging_takes_only_the_keys_sent() {
    let mut state = full();
    state.merge(State {
        mode: Some(2),
        cooler_state: Some(CoolerState::Connected),
        ..State::default()
    });
    let want = State {
        mode: Some(2),
        cooler_state: Some(CoolerState::Connected),
        ..full()
    };
    assert_eq!(state, want);
}

#[test]
fn the_changes_since_an_older_state_hold_only_the_keys_that_differ() {
    let older = full();
    let newer = State {
        rotation: Some(1),
        pump_rpm: Some(FanSpeed::Rpm(2900)),
        ..full()
    };
    let changes = newer.changed_since(&older);
    assert_eq!(
        changes,
        State {
            rotation: Some(1),
            pump_rpm: Some(FanSpeed::Rpm(2900)),
            ..State::default()
        }
    );
    let mut merged = older.clone();
    merged.merge(changes);
    assert_eq!(merged, newer);
    assert!(newer.changed_since(&newer).is_empty());
    // From nothing, every key present is a change.
    assert_eq!(full().changed_since(&State::default()), full());
}

// docs/spec/dbus.md, State: the table the settings app's design reads lists exactly these keys,
// in this order, each with its type.
#[test]
fn dbus_md_lists_exactly_the_state_keys_and_their_types() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/spec/dbus.md");
    let text = fs::read_to_string(path).expect("docs/spec/dbus.md");
    let section = text
        .split("\n## State\n")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .expect("a State section");
    let rows: Vec<(String, String)> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            (
                cells[1].trim_matches('`').to_owned(),
                cells[2].trim_matches('`').to_owned(),
            )
        })
        .collect();
    let want: Vec<(String, String)> = STATE_KEYS
        .iter()
        .map(|(key, signature)| ((*key).to_owned(), (*signature).to_owned()))
        .collect();
    assert_eq!(rows, want);
}

// docs/spec/dbus.md, Errors: the table lists exactly the errors the service sends, in this order.
#[test]
fn dbus_md_lists_exactly_the_errors() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/spec/dbus.md");
    let text = fs::read_to_string(path).expect("docs/spec/dbus.md");
    let section = text
        .split("\n## Errors\n")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .expect("an Errors section");
    let rows: Vec<&str> = section
        .lines()
        .filter_map(|line| line.strip_prefix("| `…Error."))
        .filter_map(|rest| rest.split('`').next())
        .collect();
    assert_eq!(rows, ERRORS);
}

// music.md point 4: the five stops, slowest first, each by its name.
#[test]
fn each_fall_stop_reads_back_from_its_name() {
    let stops = [
        FallStop::Slower,
        FallStop::Slow,
        FallStop::Moderate,
        FallStop::Fast,
        FallStop::Faster,
    ];
    let names: Vec<&str> = stops.iter().map(|s| s.as_str()).collect();
    assert_eq!(names, ["slower", "slow", "moderate", "fast", "faster"]);
    for stop in stops {
        assert_eq!(FallStop::parse(stop.as_str()), Some(stop));
    }
    assert_eq!(FallStop::parse("Moderate"), None);
}
