//! The fake systemd and logind of the Start at boot switch (Phase 11 plan, Task 3) on two
//! private buses: a new install before any file, the state file's answers after it, a failed
//! read, a slow one, the jobs and their `JobRemoved`, logind's session and its change; and the
//! fake `pkexec`, run as the app runs it, through a link in the fake's folder.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use odc_testkit::bus::{PrivateBus, within};
use odc_testkit::fake_systemd::{
    FakeSystemd, LOG, PKEXEC_ANSWER, PKEXEC_LINK, PKEXEC_LOG, SESSION_PATH, STATE_FILE,
};
use zbus::message::Type;
use zbus::wire::{OwnedObjectPath, OwnedVariant};
use zbus::{Connection, MatchRule, MessageStream, connection};

const SYSTEMD: &str = "org.freedesktop.systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const PATH: &str = "/org/freedesktop/systemd1";

fn dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("fake-systemd")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the test's folder");
    dir
}

fn write(dir: &Path, text: &str) {
    let next = dir.join("systemd.toml.new");
    fs::write(&next, text).expect("written");
    fs::rename(next, dir.join(STATE_FILE)).expect("moved");
}

fn log(dir: &Path, name: &str) -> Vec<String> {
    fs::read_to_string(dir.join(name))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn logged(dir: &Path, line: &str) {
    let until = Instant::now() + Duration::from_secs(5);
    while !log(dir, LOG).iter().any(|l| l == line) {
        assert!(
            Instant::now() < until,
            "{line:?} not logged: {:?}",
            log(dir, LOG)
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn state(
    serial: u32,
    service: (&str, &str, &str),
    sound: (&str, &str, &str),
    remote: bool,
) -> String {
    format!(
        "serial = {serial}\n\
         [service]\nfile = {:?}\nactive = {:?}\nsub = {:?}\n\
         [sound]\nfile = {:?}\nactive = {:?}\njob = {:?}\n\
         [session]\nactive = true\nremote = {remote}\n",
        service.0, service.1, service.2, sound.0, sound.1, sound.2
    )
}

async fn connect(bus: &PrivateBus) -> Connection {
    connection::Builder::address(bus.address())
        .build()
        .await
        .expect("a connection")
}

async fn file_state(conn: &Connection, unit: &str) -> zbus::Result<String> {
    let reply = conn
        .call_method(
            Some(SYSTEMD),
            PATH,
            Some(MANAGER),
            "GetUnitFileState",
            &(unit,),
        )
        .await?;
    reply.body().deserialize()
}

async fn property(
    conn: &Connection,
    name: &str,
    path: &str,
    iface: &str,
    prop: &str,
) -> OwnedVariant {
    let reply = conn
        .call_method(
            Some(name),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(iface, prop),
        )
        .await
        .expect("Get");
    reply.body().deserialize().expect("a variant")
}

async fn active_state(conn: &Connection, unit: &str) -> (String, String) {
    let reply = conn
        .call_method(Some(SYSTEMD), PATH, Some(MANAGER), "LoadUnit", &(unit,))
        .await
        .expect("LoadUnit");
    let path: OwnedObjectPath = reply.body().deserialize().expect("a path");
    let read = |prop| {
        property(
            conn,
            SYSTEMD,
            path.as_str(),
            "org.freedesktop.systemd1.Unit",
            prop,
        )
    };
    let active = String::try_from(read("ActiveState").await).expect("text");
    let sub = String::try_from(read("SubState").await).expect("text");
    (active, sub)
}

struct Stage {
    dir: PathBuf,
    _system: PrivateBus,
    _user: PrivateBus,
    _fake: FakeSystemd,
    system: Connection,
    user: Connection,
}

fn stage(name: &str) -> Stage {
    let dir = dir(name);
    let system = PrivateBus::start(&dir.join("system")).expect("the system bus");
    let user = PrivateBus::start(&dir.join("user")).expect("the session bus");
    let fake = FakeSystemd::start(system.address(), user.address(), &dir).expect("the fake");
    let (sys, usr) = zbus::block_on(async { (connect(&system).await, connect(&user).await) });
    Stage {
        dir,
        _system: system,
        _user: user,
        _fake: fake,
        system: sys,
        user: usr,
    }
}

#[test]
fn a_new_install_then_the_file_s_answers() {
    let s = stage("answers");
    zbus::block_on(async {
        // Before any file: a new install.
        assert_eq!(
            file_state(&s.system, "opendeepcool.service").await.unwrap(),
            "disabled"
        );
        assert_eq!(
            active_state(&s.system, "opendeepcool.service").await,
            ("inactive".to_owned(), "dead".to_owned())
        );
        assert_eq!(
            file_state(&s.user, "opendeepcool-audio.service")
                .await
                .unwrap(),
            "disabled"
        );
        // Each manager knows its own unit only.
        assert!(
            file_state(&s.system, "opendeepcool-audio.service")
                .await
                .is_err()
        );
        assert!(file_state(&s.user, "opendeepcool.service").await.is_err());
    });
    write(
        &s.dir,
        &state(
            1,
            ("enabled", "activating", "auto-restart"),
            ("masked", "active", "done"),
            false,
        ),
    );
    logged(&s.dir, "state 1");
    zbus::block_on(async {
        assert_eq!(
            file_state(&s.system, "opendeepcool.service").await.unwrap(),
            "enabled"
        );
        assert_eq!(
            active_state(&s.system, "opendeepcool.service").await,
            ("activating".to_owned(), "auto-restart".to_owned())
        );
        assert_eq!(
            file_state(&s.user, "opendeepcool-audio.service")
                .await
                .unwrap(),
            "masked"
        );
        assert_eq!(
            active_state(&s.user, "opendeepcool-audio.service").await.0,
            "active"
        );
    });
    // An empty file state fails the call, as for a unit that isn't installed.
    write(
        &s.dir,
        &state(
            2,
            ("", "inactive", "dead"),
            ("enabled", "active", "done"),
            false,
        ),
    );
    logged(&s.dir, "state 2");
    let failed = zbus::block_on(file_state(&s.system, "opendeepcool.service"));
    assert!(failed.is_err(), "{failed:?}");
    // A slow answer.
    write(
        &s.dir,
        &state(
            3,
            ("enabled", "active", "running"),
            ("enabled", "active", "done"),
            false,
        )
        .replace("[sound]", "delay_ms = 1500\n[sound]"),
    );
    logged(&s.dir, "state 3");
    let quick = zbus::block_on(within(
        Duration::from_millis(500),
        file_state(&s.system, "opendeepcool.service"),
    ));
    assert!(quick.is_none(), "answered within 0.5 s");
    // A broken file is logged, and the last state kept.
    write(&s.dir, "serial = 4\n[service]\nfile = 3\n");
    logged(&s.dir, "state error: no [sound] table");
    let calls = log(&s.dir, LOG);
    assert!(calls.contains(&"system GetUnitFileState(\"opendeepcool.service\")".to_owned()));
    assert!(calls.contains(&"user LoadUnit(\"opendeepcool-audio.service\")".to_owned()));
}

#[test]
fn a_job_ends_with_job_removed_and_moves_the_sound() {
    let s = stage("jobs");
    write(
        &s.dir,
        &state(
            1,
            ("enabled", "active", "running"),
            ("enabled", "inactive", "done"),
            false,
        ),
    );
    logged(&s.dir, "state 1");
    zbus::block_on(async {
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(SYSTEMD)
            .path(PATH)
            .interface(MANAGER)
            .member("JobRemoved")
            .build()
            .expect("a rule");
        let mut jobs = MessageStream::for_match_rule(rule, &s.user, None)
            .await
            .expect("the stream");
        s.user
            .call_method(Some(SYSTEMD), PATH, Some(MANAGER), "Subscribe", &())
            .await
            .expect("Subscribe");
        let reply = s
            .user
            .call_method(
                Some(SYSTEMD),
                PATH,
                Some(MANAGER),
                "StartUnit",
                &("opendeepcool-audio.service", "replace"),
            )
            .await
            .expect("StartUnit");
        let job: OwnedObjectPath = reply.body().deserialize().expect("a job");
        let message = within(Duration::from_secs(5), async {
            use std::future::poll_fn;
            use std::pin::Pin;
            poll_fn(|cx| futures_core::Stream::poll_next(Pin::new(&mut jobs), cx)).await
        })
        .await
        .flatten()
        .expect("JobRemoved")
        .expect("a message");
        let (_, path, unit, result): (u32, OwnedObjectPath, String, String) =
            message.body().deserialize().expect("JobRemoved's body");
        assert_eq!(
            (path, unit.as_str(), result.as_str()),
            (job, "opendeepcool-audio.service", "done")
        );
        assert_eq!(
            active_state(&s.user, "opendeepcool-audio.service").await.0,
            "active"
        );
        s.user
            .call_method(
                Some(SYSTEMD),
                PATH,
                Some(MANAGER),
                "StopUnit",
                &("opendeepcool-audio.service", "replace"),
            )
            .await
            .expect("StopUnit");
        assert_eq!(
            active_state(&s.user, "opendeepcool-audio.service").await.0,
            "inactive"
        );
    });
    // A failed job's result, and a call that fails.
    write(
        &s.dir,
        &state(
            2,
            ("enabled", "active", "running"),
            ("enabled", "inactive", "failed"),
            false,
        ),
    );
    logged(&s.dir, "state 2");
    zbus::block_on(async {
        s.user
            .call_method(
                Some(SYSTEMD),
                PATH,
                Some(MANAGER),
                "StartUnit",
                &("opendeepcool-audio.service", "replace"),
            )
            .await
            .expect("StartUnit");
        assert_eq!(
            active_state(&s.user, "opendeepcool-audio.service").await.0,
            "inactive"
        );
    });
    write(
        &s.dir,
        &state(
            3,
            ("enabled", "active", "running"),
            ("enabled", "inactive", ""),
            false,
        ),
    );
    logged(&s.dir, "state 3");
    let failed = zbus::block_on(s.user.call_method(
        Some(SYSTEMD),
        PATH,
        Some(MANAGER),
        "StartUnit",
        &("opendeepcool-audio.service", "replace"),
    ));
    assert!(failed.is_err());
    for line in [
        "user Subscribe()",
        "user StartUnit(\"opendeepcool-audio.service\", \"replace\") done",
        "user StopUnit(\"opendeepcool-audio.service\", \"replace\") done",
        "user StartUnit(\"opendeepcool-audio.service\", \"replace\") failed",
    ] {
        assert!(
            log(&s.dir, LOG).iter().any(|l| l == line),
            "{line:?}: {:?}",
            log(&s.dir, LOG)
        );
    }
}

#[test]
fn logind_s_session_and_its_change() {
    let s = stage("session");
    zbus::block_on(async {
        let reply = s
            .system
            .call_method(
                Some("org.freedesktop.login1"),
                "/org/freedesktop/login1",
                Some("org.freedesktop.login1.Manager"),
                "GetSession",
                &("auto",),
            )
            .await
            .expect("GetSession");
        let path: OwnedObjectPath = reply.body().deserialize().expect("a path");
        assert_eq!(path.as_str(), SESSION_PATH);
        let read = |prop| {
            property(
                &s.system,
                "org.freedesktop.login1",
                SESSION_PATH,
                "org.freedesktop.login1.Session",
                prop,
            )
        };
        assert!(bool::try_from(read("Active").await).expect("a flag"));
        assert!(!bool::try_from(read("Remote").await).expect("a flag"));
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .path(SESSION_PATH)
            .interface("org.freedesktop.DBus.Properties")
            .member("PropertiesChanged")
            .build()
            .expect("a rule");
        let mut changes = MessageStream::for_match_rule(rule, &s.system, None)
            .await
            .expect("the stream");
        write(
            &s.dir,
            &state(
                1,
                ("disabled", "inactive", "dead"),
                ("disabled", "inactive", "done"),
                true,
            ),
        );
        let changed = within(Duration::from_secs(5), async {
            use std::future::poll_fn;
            use std::pin::Pin;
            poll_fn(|cx| futures_core::Stream::poll_next(Pin::new(&mut changes), cx)).await
        })
        .await;
        assert!(changed.flatten().is_some(), "no PropertiesChanged");
        assert!(bool::try_from(read("Remote").await).expect("a flag"));
    });
}

#[test]
fn the_fake_pkexec_logs_its_arguments_and_ends_as_answered() {
    let dir = dir("pkexec");
    let link = dir.join(PKEXEC_LINK);
    symlink(env!("CARGO_BIN_EXE_odc-fake-pkexec"), &link).expect("the link");
    let args = [
        "--disable-internal-agent",
        "/usr/lib/opendeepcool/opendeepcool-boot",
        "on",
    ];
    for (answer, code, stderr) in [
        ("exit 0\n", 0, ""),
        ("exit 126\n", 126, ""),
        (
            "exit 1\nopendeepcool-boot: step 3 of 3 failed\n",
            1,
            "opendeepcool-boot: step 3 of 3 failed\n",
        ),
    ] {
        let mut child = Command::new(&link)
            .args(args)
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the fake starts");
        // It waits for its answer.
        thread::sleep(Duration::from_millis(100));
        assert!(
            child.try_wait().expect("waits").is_none(),
            "ended without an answer"
        );
        fs::write(dir.join(PKEXEC_ANSWER), answer).expect("the answer");
        let out = child.wait_with_output().expect("it ends");
        assert_eq!(out.status.code(), Some(code), "{answer:?}");
        assert_eq!(String::from_utf8_lossy(&out.stderr), stderr);
        assert!(!dir.join(PKEXEC_ANSWER).exists(), "the answer is taken");
    }
    assert_eq!(
        log(&dir, PKEXEC_LOG),
        ["pkexec --disable-internal-agent /usr/lib/opendeepcool/opendeepcool-boot on"; 3]
    );
}
