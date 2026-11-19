//! The fake XDG desktop portal on a private bus (Phase 11a plan, Task 11): its name is
//! activatable on the bus that lists it, `ReadAll` and `ReadOne` give the appearance
//! namespace's `color-scheme` as set, and a change sends `SettingChanged`; on the plain bus the
//! name isn't activatable, which is what makes GDK read `Unsupported`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_core::Stream;
use odc_testkit::bus::{PrivateBus, within};
use odc_testkit::portal::{FakePortal, PORTAL_NAME, PORTAL_PATH};
use zbus::message::Type;
use zbus::wire::OwnedVariant;
use zbus::{Connection, MatchRule, MessageStream, connection};

const SETTINGS: &str = "org.freedesktop.portal.Settings";
const WAIT: Duration = Duration::from_secs(5);

fn dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("portal")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the test's folder");
    dir
}

async fn client(address: &str) -> Connection {
    connection::Builder::address(address)
        .build()
        .await
        .expect("a client")
}

async fn activatable(conn: &Connection) -> Vec<String> {
    zbus::fdo::DBusProxy::new(conn)
        .await
        .expect("the bus")
        .list_activatable_names()
        .await
        .expect("the names")
        .into_iter()
        .map(|n| n.to_string())
        .collect()
}

async fn scheme(conn: &Connection) -> u32 {
    let all: HashMap<String, HashMap<String, OwnedVariant>> = conn
        .call_method(
            Some(PORTAL_NAME),
            PORTAL_PATH,
            Some(SETTINGS),
            "ReadAll",
            &(vec!["org.freedesktop.appearance"],),
        )
        .await
        .expect("ReadAll answers")
        .body()
        .deserialize()
        .expect("a{sa{sv}}");
    let value = &all["org.freedesktop.appearance"]["color-scheme"];
    u32::try_from(value).expect("a number")
}

#[test]
fn the_portal_name_is_activatable_only_on_the_bus_that_lists_it() {
    let plain = PrivateBus::start(&dir("plain")).expect("a bus");
    let listed = PrivateBus::start_with_portal(&dir("listed")).expect("a bus");
    zbus::block_on(async {
        let names = activatable(&client(plain.address()).await).await;
        assert!(!names.iter().any(|n| n == PORTAL_NAME), "{names:?}");
        let names = activatable(&client(listed.address()).await).await;
        assert!(names.iter().any(|n| n == PORTAL_NAME), "{names:?}");
    });
}

#[test]
fn the_portal_reads_as_set_and_tells_a_change() {
    let bus = PrivateBus::start_with_portal(&dir("reads")).expect("a bus");
    let portal = FakePortal::start(bus.address(), 1).expect("the portal");
    zbus::block_on(async {
        let conn = client(bus.address()).await;
        assert_eq!(scheme(&conn).await, 1, "prefer dark");
        // ReadAll with no namespace asked gives it too; another namespace gives nothing.
        let none: HashMap<String, HashMap<String, OwnedVariant>> = conn
            .call_method(
                Some(PORTAL_NAME),
                PORTAL_PATH,
                Some(SETTINGS),
                "ReadAll",
                &(vec!["org.gnome.desktop.interface"],),
            )
            .await
            .expect("answers")
            .body()
            .deserialize()
            .expect("a{sa{sv}}");
        assert!(none.is_empty());

        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(SETTINGS)
            .member("SettingChanged")
            .build()
            .expect("a match rule");
        let mut changes = MessageStream::for_match_rule(rule, &conn, None)
            .await
            .expect("a stream");
        portal.announce(2).await.expect("sent");
        let message = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal")
            .expect("a message");
        let (namespace, key, value): (String, String, OwnedVariant) =
            message.body().deserialize().expect("(ssv)");
        assert_eq!(
            (namespace.as_str(), key.as_str()),
            ("org.freedesktop.appearance", "color-scheme")
        );
        assert_eq!(u32::try_from(&value).expect("a number"), 2);
        assert_eq!(scheme(&conn).await, 2, "prefer light, read again");
    });
}

async fn next<S>(stream: &mut S) -> Option<zbus::Result<zbus::Message>>
where
    S: Stream<Item = zbus::Result<zbus::Message>> + Unpin,
{
    use std::future::poll_fn;
    use std::pin::Pin;
    poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await
}
