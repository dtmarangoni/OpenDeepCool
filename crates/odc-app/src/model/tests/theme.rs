use super::*;

// The preview's description (Preview column, by state): the caption alone while the cooler has
// the picture, then the caption and why it hasn't; the column has no line of its own for it.
#[test]
fn the_previews_description_says_why_the_picture_isnt_on_the_cooler() {
    let caption = "Temperature · CPU + GPU";
    let described = |cooler: Option<CoolerState>, long: bool| {
        let mut state = base();
        state.cooler_state = cooler;
        let ui = Ui {
            sending_failed_long: long,
            ..Ui::default()
        };
        view_with(state, ui).column.description()
    };
    assert_eq!(described(Some(CoolerState::Connected), false), caption);
    assert_eq!(described(Some(CoolerState::WriteFailed), false), caption);
    assert_eq!(
        described(Some(CoolerState::NotFound), false),
        format!("{caption}. Not on the cooler: it isn't connected.")
    );
    assert_eq!(
        described(Some(CoolerState::Released), false),
        format!("{caption}. Not on the cooler: it isn't connected.")
    );
    assert_eq!(
        described(Some(CoolerState::NoAccess), false),
        format!("{caption}. Not on the cooler: the service can't open it.")
    );
    assert_eq!(
        described(Some(CoolerState::WriteFailed), true),
        format!("{caption}. Not on the cooler: sending it fails.")
    );
    // The service down: "No picture", and nothing about the cooler.
    let View::ServiceDown(down) = view(
        &Service::Down,
        Ui::default(),
        &HashMap::new(),
        &Boot::default(),
    ) else {
        panic!("the off screen");
    };
    assert_eq!(down.column.description(), "No picture");
}

// The 2 s on a fake clock: the wait starts when the state becomes `write-failed`, a timer is
// asked for once, and any other state ends it.
#[test]
fn the_sending_fails_wait_on_a_fake_clock() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut wait = SendingFails::default();
    assert_eq!(wait.follow(Some(CoolerState::Connected), at(0)), None);
    assert!(!wait.lasted(at(5000)));
    assert_eq!(
        wait.follow(Some(CoolerState::WriteFailed), at(100)),
        Some(SENDING_FAILS_AFTER)
    );
    // Still `write-failed` in a later StateChanged: the same wait.
    assert_eq!(wait.follow(Some(CoolerState::WriteFailed), at(900)), None);
    assert!(!wait.lasted(at(2099)));
    assert!(wait.lasted(at(2100)));
    // One bad write, then connected: the wait ends.
    assert_eq!(wait.follow(Some(CoolerState::Connected), at(2200)), None);
    assert!(!wait.lasted(at(9000)));
    // Unplugged: `write-failed` for a tick, then `not-found`; no banner of its own.
    assert!(
        wait.follow(Some(CoolerState::WriteFailed), at(10_000))
            .is_some()
    );
    assert_eq!(wait.follow(Some(CoolerState::NotFound), at(10_100)), None);
    assert!(!wait.lasted(at(12_100)));
    // A clock that seems to go back never reads as lasted.
    assert!(
        wait.follow(Some(CoolerState::WriteFailed), at(20_000))
            .is_some()
    );
    assert!(!wait.lasted(at(19_000)));
}
