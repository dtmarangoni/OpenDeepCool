//! Back from standby (Phase 7 plan, Task 7; solution reviews P3-04, P5-05, OV-19): logind sends
//! `PrepareForSleep(false)` right after a suspend or hibernation (org.freedesktop.login1(5)), and
//! the service then starts a new conversation with the cooler and its readings over.

use std::pin::Pin;

use futures_core::Stream;
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::names::BusName;
use zbus::{Connection, MatchRule, MessageStream};

/// logind's bus name.
pub const LOGIND: &str = "org.freedesktop.login1";

/// The next item of a stream, with no async library of the service's own.
pub(crate) async fn next<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    std::future::poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await
}

/// logind's `PrepareForSleep` signals, subscribed before the service takes its name, so none is
/// missed.
pub struct SleepWatch {
    stream: MessageStream,
    dbus: DBusProxy<'static>,
}

impl SleepWatch {
    /// The match rule names logind as the sender, so the bus delivers only logind's broadcasts.
    ///
    /// # Errors
    ///
    /// When the bus refuses the match rule.
    pub async fn subscribe(conn: &Connection) -> zbus::Result<SleepWatch> {
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(LOGIND)
            .interface("org.freedesktop.login1.Manager")
            .member("PrepareForSleep")
            .build()?;
        Ok(SleepWatch {
            stream: MessageStream::for_match_rule(rule, conn, None).await?,
            dbus: DBusProxy::new(conn).await?,
        })
    }

    /// Calls `woke` after each sleep, until the connection ends. A signal addressed to the service
    /// reaches it whatever its match rules say (D-Bus specification, Message Bus Message
    /// Routing), and zbus's own filtering lets any sender through a rule that names a well-known
    /// one (`MatchRule::matches`), so each signal's sender is also compared with the connection
    /// that owns logind's name now: no other process can fake a wake-up.
    pub async fn run(mut self, woke: impl Fn()) {
        while let Some(message) = next(&mut self.stream).await {
            let Ok(message) = message else {
                continue;
            };
            if !matches!(message.body().deserialize::<bool>(), Ok(false)) {
                continue;
            }
            let Ok(logind) = BusName::try_from(LOGIND) else {
                continue;
            };
            let owner = self.dbus.get_name_owner(logind).await.ok();
            let header = message.header();
            let sender = header.sender().map(|s| s.as_str());
            if owner.is_some() && owner.as_ref().map(|o| o.as_str()) == sender {
                woke();
            }
        }
    }
}
