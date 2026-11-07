//! The GTK window (Phase 10 plan, Tasks 4–7): it shows the view model's [`crate::model::View`]
//! and sends the service what the user asks, through a [`Request`] sink, the client or a test's
//! fake.

pub mod banners;
pub mod display_page;
pub mod dots;
pub mod names;
pub mod preview;
pub mod tiles;
pub mod window;

use odc_ipc::TemperatureUnit;

/// What the window asks of the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// `SetMode(u)`: a tile was activated.
    SetMode(u32),
    /// `SetTemperatureUnit(s)`.
    SetTemperatureUnit(TemperatureUnit),
    /// `Refresh()`.
    Refresh,
    /// `WatchFrames()` or `UnwatchFrames()`: while the window is mapped.
    WatchFrames(bool),
    /// `Watch()` and `GetState()` again: the window became active (Talking to the service).
    Reconnect,
}

/// What the service refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Change,
    Refresh,
}

/// Why it refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    /// `NotAuthorized`: the app's own words.
    NotAuthorized,
    /// The service's message, a lost reading's metric named as the display does.
    Message(String),
}

/// The words the feedback label shows for a refusal (Refused changes).
pub fn refusal_text(what: Refused, why: &Reason) -> String {
    let head = match what {
        Refused::Change => "Not changed.",
        Refused::Refresh => "Not refreshed.",
    };
    let reason = match why {
        Reason::NotAuthorized => "This session may not change settings.",
        Reason::Message(message) => message.as_str(),
    };
    format!("{head} {reason}")
}

/// How long the feedback label shows "Refreshed", and a refusal.
pub const REFRESHED_FOR: std::time::Duration = std::time::Duration::from_millis(1500);
pub const REFUSED_FOR: std::time::Duration = std::time::Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_read_as_the_design_writes_them() {
        assert_eq!(
            refusal_text(Refused::Change, &Reason::NotAuthorized),
            "Not changed. This session may not change settings."
        );
        assert_eq!(
            refusal_text(
                Refused::Change,
                &Reason::Message("GPU usage: no device found".to_owned())
            ),
            "Not changed. GPU usage: no device found"
        );
        assert_eq!(
            refusal_text(Refused::Refresh, &Reason::NotAuthorized),
            "Not refreshed. This session may not change settings."
        );
    }
}
