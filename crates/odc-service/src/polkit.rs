//! Who may call (Phase 7 plan, Task 7; `docs/spec/dbus.md`, Who may call): polkit for every
//! call, by the caller's unique bus name, and for the dev interface also the caller's groups as
//! the bus reports them. Both sit behind traits, so the tests use fakes.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use serde::Deserialize;
use zbus::Connection;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::wire::{Type, Variant};

/// The polkit actions (`docs/spec/dbus.md`, Names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Every read.
    Use,
    /// Every change, `Refresh()` included, and every dev call.
    ChangeSettings,
    /// `AttachAudio()`, and its check every second while attached (Phase 9).
    FeedAudio,
}

impl Action {
    pub fn id(self) -> &'static str {
        match self {
            Action::Use => odc_ipc::USE_ACTION,
            Action::ChangeSettings => odc_ipc::CHANGE_SETTINGS_ACTION,
            Action::FeedAudio => odc_ipc::FEED_AUDIO_ACTION,
        }
    }
}

/// A check's answer, once it comes. An error is the check itself failing, which the service
/// treats as a refusal.
pub type Answer<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

/// Asks whether a caller may do an action.
pub trait Authorizer: Send + Sync {
    /// Whether the caller with this unique bus name may do `action` now, without being asked for
    /// a password.
    fn allowed<'a>(&'a self, sender: &'a str, action: Action) -> Answer<'a, bool>;
}

/// A caller's Unix identity, as the bus reports it; `None` where the bus says nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    pub user: Option<u32>,
    pub groups: Option<Vec<u32>>,
    /// The caller's process, which the journal names an attach by (Phase 9; solution review
    /// P9-11).
    pub pid: Option<u32>,
}

/// Asks the bus who a caller is.
pub trait Credentials: Send + Sync {
    fn of<'a>(&'a self, sender: &'a str) -> Answer<'a, Identity>;
}

/// `CheckAuthorization`'s answer, `(bba{ss})`: authorized, a challenge, details. Only the first
/// is used: with flags 0 polkit never asks the user.
#[derive(Deserialize, Type)]
struct AuthorizationResult {
    is_authorized: bool,
    _is_challenge: bool,
    _details: HashMap<String, String>,
}

/// polkit's authority, as its D-Bus interface declares it: `CheckAuthorization((sa{sv}) s a{ss}
/// u s) → (bba{ss})` (introspected from polkit 128, 2026-10-29).
#[zbus::proxy(
    interface = "org.freedesktop.PolicyKit1.Authority",
    default_service = "org.freedesktop.PolicyKit1",
    default_path = "/org/freedesktop/PolicyKit1/Authority"
)]
trait Authority {
    fn check_authorization(
        &self,
        subject: &(&str, HashMap<&str, Variant<'_>>),
        action_id: &str,
        details: &HashMap<&str, &str>,
        flags: u32,
        cancellation_id: &str,
    ) -> zbus::Result<AuthorizationResult>;
}

/// polkit, on the system bus: the subject is the caller's unique bus name, polkit's
/// identification of a D-Bus caller (`system-bus-name`), and the flags are 0, so it never asks
/// for a password (Phase 7 plan, Recommendation).
pub struct PolkitAuthorizer {
    authority: AuthorityProxy<'static>,
}

impl PolkitAuthorizer {
    /// polkit's authority on `conn`'s bus.
    ///
    /// # Errors
    ///
    /// When the proxy can't be made.
    pub async fn new(conn: &Connection) -> zbus::Result<PolkitAuthorizer> {
        Ok(PolkitAuthorizer {
            authority: AuthorityProxy::new(conn).await?,
        })
    }
}

impl Authorizer for PolkitAuthorizer {
    fn allowed<'a>(&'a self, sender: &'a str, action: Action) -> Answer<'a, bool> {
        Box::pin(async move {
            let subject = (
                "system-bus-name",
                HashMap::from([("name", Variant::from(sender))]),
            );
            let result = self
                .authority
                .check_authorization(&subject, action.id(), &HashMap::new(), 0, "")
                .await
                .map_err(|e| format!("polkit: {e}"))?;
            Ok(result.is_authorized)
        })
    }
}

/// The bus's own answer, `GetConnectionCredentials` (D-Bus specification): a missing
/// `UnixGroupIDs` stays `None`, which the dev interface reads as no.
pub struct BusCredentials {
    dbus: DBusProxy<'static>,
}

impl BusCredentials {
    /// # Errors
    ///
    /// When the proxy can't be made.
    pub async fn new(conn: &Connection) -> zbus::Result<BusCredentials> {
        Ok(BusCredentials {
            dbus: DBusProxy::new(conn).await?,
        })
    }
}

impl Credentials for BusCredentials {
    fn of<'a>(&'a self, sender: &'a str) -> Answer<'a, Identity> {
        Box::pin(async move {
            let name = BusName::try_from(sender).map_err(|e| e.to_string())?;
            let credentials = self
                .dbus
                .get_connection_credentials(name)
                .await
                .map_err(|e| format!("the bus: {e}"))?;
            Ok(Identity {
                user: credentials.unix_user_id(),
                groups: credentials.unix_group_ids().cloned(),
                pid: credentials.process_id(),
            })
        })
    }
}
