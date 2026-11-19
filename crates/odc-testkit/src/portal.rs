//! A fake XDG desktop portal for the settings app's tests (Phase 11a plan, Task 11): the Settings
//! interface's `org.freedesktop.appearance` `color-scheme` (0 no preference, 1 prefer dark, 2
//! prefer light), which GDK reads with `ReadAll` and follows with `SettingChanged`, and so fills
//! `GtkSettings:gtk-interface-color-scheme` (Phase 11a Task 1 measured it).
//!
//! GDK uses the portal only when its name is **activatable**: the bus must list a service
//! directory holding a file for `org.freedesktop.portal.Desktop`, whose `Exec` may be `/bin/true`,
//! and the fake must own the name before GTK starts. [`PrivateBus::start_with_portal`] writes the
//! file; a bus without it, or a fake not started, leaves the preference `Unsupported`, as a PC
//! with no portal does.
//!
//! [`PrivateBus::start_with_portal`]: crate::bus::PrivateBus::start_with_portal

use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use zbus::object_server::SignalEmitter;
use zbus::wire::{OwnedVariant, Variant};
use zbus::{Connection, connection, interface};

/// The portal's bus name and object path.
pub const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";

struct Settings {
    scheme: Arc<AtomicU32>,
}

impl Settings {
    fn value(&self) -> OwnedVariant {
        OwnedVariant::try_from(Variant::from(self.scheme.load(Ordering::Relaxed)))
            .expect("a number has no file descriptor")
    }
}

#[interface(name = "org.freedesktop.portal.Settings")]
impl Settings {
    /// The interface's version, as GDK asks for it first (`version`, in lower case).
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }

    /// Every setting of the namespaces asked for: none asked, or one that matches, gives the
    /// appearance namespace.
    async fn read_all(
        &self,
        namespaces: Vec<String>,
    ) -> HashMap<String, HashMap<String, OwnedVariant>> {
        let wanted = namespaces.is_empty()
            || namespaces.iter().any(|n| {
                n == NAMESPACE
                    || n.is_empty()
                    || n.strip_suffix(".*")
                        .is_some_and(|p| NAMESPACE.starts_with(p))
            });
        let mut all = HashMap::new();
        if wanted {
            all.insert(
                NAMESPACE.to_owned(),
                HashMap::from([(KEY.to_owned(), self.value())]),
            );
        }
        all
    }

    async fn read_one(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedVariant> {
        if namespace == NAMESPACE && key == KEY {
            Ok(self.value())
        } else {
            Err(zbus::fdo::Error::Failed(format!(
                "{namespace} {key}: no such setting"
            )))
        }
    }

    #[zbus(signal)]
    async fn setting_changed(
        emitter: &SignalEmitter<'_>,
        namespace: &str,
        key: &str,
        value: Variant<'_>,
    ) -> zbus::Result<()>;
}

/// The fake's own interface, on the same object: `Set(u)` changes the preference and sends
/// `SettingChanged`, so a process other than the one that started the fake (a test's child in a
/// headless weston) can change it while the app runs.
pub const CONTROL: &str = "io.github.odc.FakePortal";

struct Control {
    scheme: Arc<AtomicU32>,
}

#[interface(name = "io.github.odc.FakePortal")]
impl Control {
    async fn set(
        &self,
        scheme: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.scheme.store(scheme, Ordering::Relaxed);
        Settings::setting_changed(&emitter, NAMESPACE, KEY, Variant::from(scheme))
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
}

/// Changes the preference of the fake portal on the bus at `address`, from any process.
///
/// # Errors
///
/// When the bus can't be reached or the fake doesn't answer.
pub fn set_over(address: &str, scheme: u32) -> io::Result<()> {
    zbus::block_on(async {
        let conn = connection::Builder::address(address).build().await?;
        conn.call_method(
            Some(PORTAL_NAME),
            PORTAL_PATH,
            Some(CONTROL),
            "Set",
            &(scheme,),
        )
        .await
        .map(|_| ())
    })
    .map_err(io::Error::other)
}

/// The fake, serving until dropped.
pub struct FakePortal {
    connection: Connection,
    scheme: Arc<AtomicU32>,
}

impl FakePortal {
    /// Connects to the bus at `address`, owns the portal's name and serves `scheme` as the
    /// preference.
    ///
    /// # Errors
    ///
    /// When the bus can't be reached or the name can't be owned.
    pub fn start(address: &str, scheme: u32) -> io::Result<FakePortal> {
        let scheme = Arc::new(AtomicU32::new(scheme));
        let settings = Settings {
            scheme: Arc::clone(&scheme),
        };
        let control = Control {
            scheme: Arc::clone(&scheme),
        };
        let connection = zbus::block_on(async {
            connection::Builder::address(address)
                .name(PORTAL_NAME)
                .serve_at(PORTAL_PATH, settings)
                .serve_at(PORTAL_PATH, control)
                .build()
                .await
        })
        .map_err(io::Error::other)?;
        Ok(FakePortal { connection, scheme })
    }

    /// Changes the preference and sends `SettingChanged`, as a desktop does.
    ///
    /// # Errors
    ///
    /// When the signal can't be sent.
    pub fn set(&self, scheme: u32) -> io::Result<()> {
        zbus::block_on(self.announce(scheme)).map_err(io::Error::other)
    }

    /// [`FakePortal::set`] for a caller that is already in an async task.
    ///
    /// # Errors
    ///
    /// When the signal can't be sent.
    pub async fn announce(&self, scheme: u32) -> zbus::Result<()> {
        self.scheme.store(scheme, Ordering::Relaxed);
        let emitter = SignalEmitter::new(&self.connection, PORTAL_PATH)?;
        Settings::setting_changed(&emitter, NAMESPACE, KEY, Variant::from(scheme)).await
    }
}
