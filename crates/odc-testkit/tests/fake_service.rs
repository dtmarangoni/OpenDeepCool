//! The settings app's fake service (Phase 10 plan, Task 8) on a private bus: it serves the state
//! file's state, logs every call and never applies one, tells its watchers the keys that
//! changed, refuses as the file says, and leaves and takes the bus.

use std::collections::HashMap;
use std::fs;
use std::future::poll_fn;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::thread;
use std::time::{Duration, Instant};

use futures_core::Stream;
use odc_ipc::{BUS_NAME, CoolerState, Error, FanSpeed, OpenDeepCoolProxy, State};
use odc_testkit::bus::{PrivateBus, within};
use odc_testkit::fake_service::{CALL_LOG, FakeService, STATE_FILE};
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::names::BusName;
use zbus::wire::OwnedVariant;
use zbus::{Connection, MatchRule, Message, MessageStream, connection};

const ON: &str = "serial = 1\non_bus = true\nreads = true";

const FANS: &str = r#"FanInputs = [["acme6000", "devices/platform/acme6000.2592", 1, "", 1180.0], ["acme6000", "devices/platform/acme6000.2592", 2, "", nan]]"#;

const REST: &str = r#"
Rotation = 1
TemperatureUnit = "celsius"
CoolerState = "connected"
AvailableModes = [1, 7]
UnavailableReasons = { 2 = "Gpu usage: no device found" }
Gpus = [["0000:0a:00.0", "acmegpu", "dedicated", 4660, 22136, true]]
ChosenGpu = ""
CoolerFanInput = ["acme6000", "devices/platform/acme6000.2592", 1]
CoolerFanRpm = 1180
"#;

fn state(mode: u32, fans: &str) -> String {
    format!("Mode = {mode}\n{fans}\n{REST}")
}

fn dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("fake-service")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the test's folder");
    dir
}

fn write(dir: &Path, fake: &str, state: &str) {
    let next = dir.join("state.toml.new");
    fs::write(&next, format!("[fake]\n{fake}\n[state]\n{state}")).expect("written");
    fs::rename(next, dir.join(STATE_FILE)).expect("moved");
}

fn log(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join(CALL_LOG))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn logged(dir: &Path, line: &str) {
    let until = Instant::now() + Duration::from_secs(5);
    while !log(dir).iter().any(|l| l == line) {
        assert!(
            Instant::now() < until,
            "{line:?} not logged: {:?}",
            log(dir)
        );
        thread::sleep(Duration::from_millis(5));
    }
}

async fn connect(bus: &PrivateBus) -> Connection {
    connection::Builder::address(bus.address())
        .build()
        .await
        .expect("connected")
}

async fn owner(conn: &Connection) -> Option<String> {
    let dbus = DBusProxy::new(conn).await.expect("the bus");
    let name = BusName::try_from(BUS_NAME).expect("the name");
    dbus.get_name_owner(name).await.ok().map(|o| o.to_string())
}

/// The keys of the next `StateChanged`, within 5 s.
async fn changed_keys(stream: &mut MessageStream) -> Vec<String> {
    let next = poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx));
    let message: Message = within(Duration::from_secs(5), next)
        .await
        .expect("a StateChanged within 5 s")
        .expect("the stream goes on")
        .expect("a message");
    let (dict,): (HashMap<String, OwnedVariant>,) = message.body().deserialize().expect("a{sv}");
    let mut keys: Vec<String> = dict.into_keys().collect();
    keys.sort();
    keys
}

#[test]
fn it_serves_the_file_s_state_and_logs_every_call() {
    let dir = dir("serves");
    let bus = PrivateBus::start(&dir.join("bus")).expect("the bus");
    let _fake = FakeService::start(bus.address(), &dir).expect("the fake");
    zbus::block_on(async {
        let conn = connect(&bus).await;
        assert_eq!(owner(&conn).await, None, "no state file yet: no service");
        write(&dir, ON, &state(7, FANS));
        logged(&dir, "state 1");
        assert!(owner(&conn).await.is_some());
        let proxy = OpenDeepCoolProxy::new(&conn).await.expect("the proxy");
        let read = State::from_dict(&proxy.get_state().await.expect("the state"))
            .expect("the contract's types");
        assert_eq!(read.mode, Some(7));
        assert_eq!(read.rotation, Some(1));
        assert_eq!(read.cooler_state, Some(CoolerState::Connected));
        assert_eq!(read.gpus.as_ref().map(|g| g[0].vendor), Some(4660));
        let inputs = read.fan_inputs.expect("the inputs");
        assert_eq!(inputs[1].rpm, FanSpeed::Unavailable);
        assert_eq!(
            read.unavailable_reasons.and_then(|r| r.get(&2).cloned()),
            Some("Gpu usage: no device found".to_owned())
        );
        proxy.set_mode(5).await.expect("answered");
        proxy
            .set_cooler_fan_input("acme6000", "devices/platform/acme6000.2592", 2)
            .await
            .expect("answered");
        proxy.choose_gpu("").await.expect("answered");
        assert!(matches!(
            proxy.attach_audio().await,
            Err(Error::AudioNotWanted(_))
        ));
        let read = State::from_dict(&proxy.get_state().await.expect("the state")).expect("read");
        assert_eq!(read.mode, Some(7), "a change is never applied by the fake");
    });
    assert_eq!(
        log(&dir),
        [
            "state 1",
            "GetState()",
            "SetMode(5)",
            "SetCoolerFanInput(\"acme6000\", \"devices/platform/acme6000.2592\", 2)",
            "ChooseGpu(\"\")",
            "AttachAudio() refused AudioNotWanted",
            "GetState()",
        ]
    );
}

#[test]
fn watchers_get_the_changed_keys_and_fan_inputs_only_while_they_watch_frames() {
    let dir = dir("watchers");
    let bus = PrivateBus::start(&dir.join("bus")).expect("the bus");
    let _fake = FakeService::start(bus.address(), &dir).expect("the fake");
    write(&dir, ON, &state(7, FANS));
    logged(&dir, "state 1");
    zbus::block_on(async {
        let conn = connect(&bus).await;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .member("StateChanged")
            .build()
            .expect("a rule");
        let mut changes = MessageStream::for_match_rule(rule, &conn, None)
            .await
            .expect("subscribed");
        let proxy = OpenDeepCoolProxy::new(&conn).await.expect("the proxy");
        proxy.watch().await.expect("watching");
        let fans = FANS.replace("1180.0", "1200.0");
        write(
            &dir,
            "serial = 2\non_bus = true\nreads = true",
            &state(8, &fans),
        );
        assert_eq!(changed_keys(&mut changes).await, ["Mode"]);
        proxy.watch_frames().await.expect("watching frames");
        write(
            &dir,
            "serial = 3\non_bus = true\nreads = true",
            &state(9, FANS),
        );
        assert_eq!(changed_keys(&mut changes).await, ["FanInputs", "Mode"]);
    });
}

#[test]
fn it_refuses_as_told_and_leaves_the_bus() {
    let dir = dir("refuses");
    let bus = PrivateBus::start(&dir.join("bus")).expect("the bus");
    let _fake = FakeService::start(bus.address(), &dir).expect("the fake");
    let refusing = format!("{ON}\nrefuse = \"ModeUnavailable: Gpu usage: no device found\"");
    write(&dir, &refusing, &state(7, FANS));
    logged(&dir, "state 1");
    zbus::block_on(async {
        let conn = connect(&bus).await;
        let proxy = OpenDeepCoolProxy::new(&conn).await.expect("the proxy");
        match proxy.set_mode(2).await {
            Err(Error::ModeUnavailable(text)) => assert_eq!(text, "Gpu usage: no device found"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            proxy.refresh().await,
            Err(Error::ModeUnavailable(_))
        ));
        write(
            &dir,
            "serial = 2\non_bus = true\nreads = false",
            &state(7, FANS),
        );
        logged(&dir, "state 2");
        assert!(matches!(
            proxy.get_state().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(proxy.watch().await, Err(Error::NotAuthorized(_))));
        write(
            &dir,
            "serial = 3\non_bus = false\nreads = true",
            &state(7, FANS),
        );
        logged(&dir, "state 3");
        assert_eq!(owner(&conn).await, None);
    });
    let log = log(&dir);
    for line in [
        "SetMode(2) refused ModeUnavailable",
        "Refresh() refused ModeUnavailable",
        "GetState() refused NotAuthorized",
        "Watch() refused NotAuthorized",
    ] {
        assert!(log.iter().any(|l| l == line), "{line:?} in {log:?}");
    }
}

#[test]
fn a_file_it_can_t_take_is_logged_and_the_fake_goes_on() {
    let dir = dir("bad-file");
    let bus = PrivateBus::start(&dir.join("bus")).expect("the bus");
    let _fake = FakeService::start(bus.address(), &dir).expect("the fake");
    write(&dir, ON, "Mood = 1");
    logged(&dir, "state error: Mood isn't a state key");
    write(
        &dir,
        "serial = 2\non_bus = true\nreads = true",
        &state(7, FANS),
    );
    logged(&dir, "state 2");
}

/// The processes whose command line names `dir`.
fn running_in(dir: &Path) -> Vec<String> {
    let needle = dir.display().to_string();
    let mut found = Vec::new();
    for entry in fs::read_dir("/proc").expect("/proc").flatten() {
        let Ok(cmdline) = fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let cmdline = String::from_utf8_lossy(&cmdline).replace('\0', " ");
        let environ = fs::read(entry.path().join("environ")).unwrap_or_default();
        if cmdline.contains(&needle) || String::from_utf8_lossy(&environ).contains(&needle) {
            found.push(cmdline);
        }
    }
    found
}

#[test]
fn the_accessibility_bus_holds_the_registry_and_takes_it_along() {
    if !Path::new(odc_testkit::bus::REGISTRY_SERVICE).exists() {
        // at-spi2-core isn't everywhere the workspace's tests run.
        eprintln!("skipped: no {}", odc_testkit::bus::REGISTRY_SERVICE);
        return;
    }
    let dir = dir("a11y");
    let runtime = dir.join("runtime");
    fs::create_dir_all(&runtime).expect("a runtime folder");
    let bus = PrivateBus::accessibility(&dir.join("bus"), &runtime).expect("the bus");
    zbus::block_on(async {
        let conn = connect(&bus).await;
        let dbus = DBusProxy::new(&conn).await.expect("the bus");
        let registry = BusName::try_from("org.a11y.atspi.Registry").expect("a name");
        assert!(dbus.get_name_owner(registry).await.is_ok());
        let activatable = dbus
            .list_activatable_names()
            .await
            .expect("the activatable names");
        assert!(
            activatable
                .iter()
                .all(|n| n.as_str().starts_with("org.freedesktop.DBus")),
            "nothing starts on demand: {activatable:?}"
        );
    });
    assert_eq!(running_in(&runtime).len(), 2, "the daemon and the registry");
    drop(bus);
    let until = Instant::now() + Duration::from_secs(5);
    while !running_in(&runtime).is_empty() {
        assert!(
            Instant::now() < until,
            "left running: {:?}",
            running_in(&runtime)
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// The bytes of the next `Frame`, within 5 s, or none.
async fn next_frame(stream: &mut MessageStream, wait: Duration) -> Option<Vec<u8>> {
    let next = poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx));
    let message: Message = within(wait, next).await??.ok()?;
    let (pixels,): (Vec<u8>,) = message.body().deserialize().ok()?;
    Some(pixels)
}

#[test]
fn a_new_frame_goes_to_frame_watchers_once() {
    let dir = dir("frames");
    let bus = PrivateBus::start(&dir.join("bus")).expect("the bus");
    let _fake = FakeService::start(bus.address(), &dir).expect("the fake");
    write(&dir, ON, &state(7, FANS));
    logged(&dir, "state 1");
    let dots = |lit: usize| {
        let mut dots = vec!['0'; 196];
        dots[lit] = '1';
        dots.into_iter().collect::<String>()
    };
    let pixels = |lit: usize| {
        let mut pixels = vec![0_u8; 196];
        pixels[lit] = 1;
        pixels
    };
    zbus::block_on(async {
        let conn = connect(&bus).await;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .member("Frame")
            .build()
            .expect("a rule");
        let mut frames = MessageStream::for_match_rule(rule, &conn, None)
            .await
            .expect("subscribed");
        let proxy = OpenDeepCoolProxy::new(&conn).await.expect("the proxy");
        proxy.watch().await.expect("watching");
        let with = |serial: u32, lit: usize| {
            format!(
                "serial = {serial}\non_bus = true\nreads = true\nframe = \"{}\"",
                dots(lit)
            )
        };
        write(&dir, &with(2, 0), &state(7, FANS));
        logged(&dir, "state 2");
        assert_eq!(
            next_frame(&mut frames, Duration::from_millis(300)).await,
            None,
            "no frame without WatchFrames()"
        );
        proxy.watch_frames().await.expect("watching frames");
        write(&dir, &with(3, 14), &state(7, FANS));
        assert_eq!(
            next_frame(&mut frames, Duration::from_secs(5)).await,
            Some(pixels(14))
        );
        // The same frame again sends nothing.
        write(&dir, &with(4, 14), &state(8, FANS));
        logged(&dir, "state 4");
        assert_eq!(
            next_frame(&mut frames, Duration::from_millis(300)).await,
            None
        );
    });
}
