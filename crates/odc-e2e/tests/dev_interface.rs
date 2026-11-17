//! The dev interface for the packaged service (Phase 11 plan, Task 9; owner, 2026-11-17): the
//! root step against a fake tree and a fake `systemctl`, the run's guard over in-process pipes, and
//! what the step must refuse. Nothing here starts `pkexec` or touches the real system.

use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use odc_e2e::dev_interface::{
    DEV_LINES, DROP_IN, Guard, Paths, READY, REMOVED, Refusal, StepError, Systemctl, UNIT,
    check_caller, command, drop_in_dir, drop_in_text, leftover, step,
};
use odc_e2e::{Area, Options, run};
use odc_testkit::fake_tree::FakeTree;

/// The package's unit as the repository has it, and the service binary it names.
const VENDOR_UNIT: &str = include_str!("../../../packaging/systemd/opendeepcool.service");
const PACKAGED_BINARY: &str = "/usr/lib/opendeepcool/opendeepcoold";
/// The dev copy's drop-in, whose lines the runtime drop-in repeats.
const DEV_CONF: &str = include_str!("../../../tools/dev/opendeepcool-dev.conf");

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dev-interface")
        .join(name)
}

/// A fake `systemctl`: it records each call with whether the drop-in was there at the time, and
/// answers `show` with a state; one chosen `restart` fails.
struct Fake {
    drop_in: PathBuf,
    state: String,
    fail_restart: Option<usize>,
    restarts: AtomicUsize,
    calls: Mutex<Vec<String>>,
}

impl Fake {
    fn new(paths: &Paths, state: &str) -> Arc<Fake> {
        Arc::new(Fake {
            drop_in: paths.drop_in_dir.join(DROP_IN),
            state: state.to_owned(),
            fail_restart: None,
            restarts: AtomicUsize::new(0),
            calls: Mutex::new(Vec::new()),
        })
    }

    fn failing_restart(paths: &Paths, nth: usize) -> Arc<Fake> {
        Arc::new(Fake {
            fail_restart: Some(nth),
            ..Arc::into_inner(Fake::new(paths, "active")).expect("one owner")
        })
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("the calls").clone()
    }
}

impl Systemctl for Fake {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        let there = if self.drop_in.exists() {
            "drop-in there"
        } else {
            "no drop-in"
        };
        self.calls
            .lock()
            .expect("the calls")
            .push(format!("{} [{there}]", args.join(" ")));
        match args {
            ["show", ..] => Ok(self.state.clone()),
            ["restart", _] => {
                let nth = self.restarts.fetch_add(1, Ordering::SeqCst) + 1;
                if self.fail_restart == Some(nth) {
                    Err("systemctl restart: exit status: 1: Job failed".into())
                } else {
                    Ok(String::new())
                }
            }
            _ => Ok(String::new()),
        }
    }
}

const SHOW: &str = "show --property=ActiveState --value opendeepcool.service [no drop-in]";

/// A fake package under a case's folder: the binary, the vendor unit naming it, and the dev
/// service's two recordings.
fn package(name: &str) -> Paths {
    let root = dir(name);
    let _ = fs::remove_dir_all(&root);
    let binary = root.join("usr/lib/opendeepcool/opendeepcoold");
    let unit = root.join("usr/lib/systemd/system").join(UNIT);
    let state = root.join("var/lib/opendeepcool");
    for path in [&binary, &unit, &state.join("x")] {
        fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
    }
    fs::write(&binary, "").expect("the binary");
    fs::write(
        &unit,
        VENDOR_UNIT.replace(PACKAGED_BINARY, &binary.to_string_lossy()),
    )
    .expect("the unit");
    let recordings = vec![state.join("dev-frames.log"), state.join("dev-frames.log.1")];
    for recording in &recordings {
        fs::write(recording, "frames").expect("a recording");
    }
    Paths {
        vendor_unit: unit,
        binary,
        drop_in_dir: drop_in_dir(&root.join("run")),
        recordings,
    }
}

/// The step on its own thread, with its standard input and output as in-process pipes.
struct Running {
    to_step: io::PipeWriter,
    from_step: BufReader<io::PipeReader>,
    thread: thread::JoinHandle<Result<(), StepError>>,
}

fn start(paths: &Paths, systemctl: &Arc<Fake>) -> Running {
    let (mut stdin, to_step) = io::pipe().expect("a pipe");
    let (from_step, mut out) = io::pipe().expect("a pipe");
    let (paths, systemctl) = (paths.clone(), Arc::clone(systemctl));
    let thread = thread::spawn(move || step(&paths, &*systemctl, &mut stdin, &mut out));
    Running {
        to_step,
        from_step: BufReader::new(from_step),
        thread,
    }
}

fn line(from: &mut impl BufRead) -> String {
    let mut line = String::new();
    from.read_line(&mut line).expect("a line");
    line.trim().to_owned()
}

fn lines(text: &str) -> Vec<&str> {
    text.lines().filter(|l| !l.starts_with('#')).collect()
}

#[test]
fn the_drop_in_is_the_packaged_service_with_dev_and_the_dev_copys_lines() {
    let text = drop_in_text(VENDOR_UNIT, Path::new(PACKAGED_BINARY)).expect("the real unit");
    assert_eq!(
        lines(&text),
        [
            "[Service]",
            "ExecStart=",
            "ExecStart=/usr/lib/opendeepcool/opendeepcoold --dev",
            "UMask=0027",
            "StateDirectoryMode=0750",
        ]
    );
    // The dev copy's drop-in says the same, apart from where its binary is: nothing there may
    // change without this one following.
    let dev = lines(DEV_CONF);
    assert_eq!(dev.len(), 5, "{dev:?}");
    assert_eq!(dev[..2], ["[Service]", "ExecStart="]);
    assert!(
        dev[2].starts_with("ExecStart=/usr/local/libexec/") && dev[2].ends_with(" --dev"),
        "{dev:?}"
    );
    assert_eq!(dev[3..], DEV_LINES);
}

#[test]
fn a_unit_that_isnt_exactly_the_packaged_one_is_refused_before_anything_is_written() {
    let binary = Path::new(PACKAGED_BINARY);
    let refused = |unit: &str| match drop_in_text(unit, binary) {
        Err(Refusal::VendorUnit { why }) => why,
        other => panic!("{unit:?}: {other:?}"),
    };
    assert!(refused("[Service]\nType=notify\n").contains("exactly one"));
    assert!(
        refused(&format!(
            "[Service]\nExecStart={PACKAGED_BINARY}\nExecStart={PACKAGED_BINARY}\n"
        ))
        .contains("exactly one")
    );
    for start in [
        "/usr/bin/other".to_owned(),
        format!("-{PACKAGED_BINARY}"),
        format!("@{PACKAGED_BINARY} x"),
        format!("{PACKAGED_BINARY} --dev"),
        format!("{PACKAGED_BINARY}x"),
        String::new(),
    ] {
        let why = refused(&format!("[Service]\nExecStart={start}\n"));
        assert!(why.contains("not exactly"), "{start:?}: {why}");
    }
    // A line that only starts like it, and a comment, aren't the line.
    let unit = format!(
        "[Service]\nExecStartPre=/bin/true\n# ExecStart=/x\n; ExecStart=/y\nExecStart = {PACKAGED_BINARY}\n"
    );
    assert!(drop_in_text(&unit, binary).is_ok());
}

#[test]
fn the_step_writes_waits_and_puts_everything_back() {
    let paths = package("whole");
    let fake = Fake::new(&paths, "active");
    let mut running = start(&paths, &fake);
    assert_eq!(line(&mut running.from_step), READY);

    // While it waits: the drop-in is there, the service was restarted on it, and the dev
    // service's recordings are the dev service's to hold.
    let written = fs::read_to_string(paths.drop_in_dir.join(DROP_IN)).expect("the drop-in");
    assert_eq!(
        lines(&written)[2],
        format!("ExecStart={} --dev", paths.binary.display())
    );
    let mode = fs::metadata(paths.drop_in_dir.join(DROP_IN))
        .expect("its metadata")
        .permissions();
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(&mode) & 0o777,
        0o644
    );
    assert!(!paths.drop_in_dir.join(".odc-e2e.conf.new").exists());
    assert!(paths.recordings.iter().all(|r| r.exists()));
    assert_eq!(
        fake.calls(),
        [
            SHOW,
            "daemon-reload [drop-in there]",
            "restart opendeepcool.service [drop-in there]"
        ]
    );

    // The run lets go of the pipe: the drop-in goes before the service is told to read its units
    // again and restart, and the recordings after the restart that closed them.
    drop(running.to_step);
    assert_eq!(line(&mut running.from_step), REMOVED);
    running
        .thread
        .join()
        .expect("the step")
        .expect("it ended well");
    assert_eq!(
        fake.calls()[3..],
        [
            "daemon-reload [no drop-in]",
            "restart opendeepcool.service [no drop-in]"
        ]
    );
    assert!(!paths.drop_in_dir.exists(), "the folder it made is gone");
    assert!(paths.recordings.iter().all(|r| !r.exists()));
}

#[test]
fn another_admins_drop_in_in_the_folder_stays_and_an_old_one_of_ours_is_replaced() {
    let paths = package("others");
    fs::create_dir_all(&paths.drop_in_dir).expect("the folder");
    let other = paths.drop_in_dir.join("other.conf");
    fs::write(&other, "[Service]\nNice=5\n").expect("an admin's drop-in");
    fs::write(paths.drop_in_dir.join(DROP_IN), "left by a killed run").expect("a leftover");
    let fake = Fake::new(&paths, "active");
    let mut running = start(&paths, &fake);
    assert_eq!(line(&mut running.from_step), READY);
    let written = fs::read_to_string(paths.drop_in_dir.join(DROP_IN)).expect("the drop-in");
    assert!(written.contains("--dev"), "{written}");
    drop(running.to_step);
    assert_eq!(line(&mut running.from_step), REMOVED);
    running.thread.join().expect("the step").expect("ok");
    assert!(!paths.drop_in_dir.join(DROP_IN).exists());
    assert_eq!(
        fs::read_to_string(&other).expect("theirs stays"),
        "[Service]\nNice=5\n"
    );
}

#[test]
fn a_restart_that_fails_backs_the_drop_in_out_and_says_why() {
    let paths = package("restart-fails");
    let fake = Fake::failing_restart(&paths, 1);
    let mut running = start(&paths, &fake);
    // No `ready`: the pipe ends.
    assert_eq!(line(&mut running.from_step), "");
    let StepError::Failed(why) = running
        .thread
        .join()
        .expect("the step")
        .expect_err("it failed")
    else {
        panic!("a refusal");
    };
    assert!(why.contains("Job failed"), "{why}");
    assert!(!paths.drop_in_dir.exists());
    assert!(paths.recordings.iter().all(|r| !r.exists()));
    assert_eq!(
        fake.calls(),
        [
            SHOW,
            "daemon-reload [drop-in there]",
            "restart opendeepcool.service [drop-in there]",
            "daemon-reload [no drop-in]",
            "restart opendeepcool.service [no drop-in]",
        ]
    );
}

#[test]
fn a_restart_that_fails_at_the_end_still_removes_the_drop_in_and_says_so() {
    let paths = package("end-restart-fails");
    let fake = Fake::failing_restart(&paths, 2);
    let mut running = start(&paths, &fake);
    assert_eq!(line(&mut running.from_step), READY);
    drop(running.to_step);
    // It doesn't say `removed`, because the service isn't back.
    assert_eq!(line(&mut running.from_step), "");
    let StepError::Failed(why) = running
        .thread
        .join()
        .expect("the step")
        .expect_err("failed")
    else {
        panic!("a refusal");
    };
    assert!(why.contains("Job failed"), "{why}");
    assert!(!paths.drop_in_dir.exists());
}

#[test]
fn the_step_refuses_what_it_does_not_expect_and_writes_nothing() {
    let refused = |paths: &Paths, state: &str| -> (Refusal, Vec<String>) {
        let fake = Fake::new(paths, state);
        let mut out = Vec::new();
        let result = step(paths, &*fake, &mut io::empty(), &mut out);
        assert!(out.is_empty(), "it says nothing: {out:?}");
        assert!(!paths.drop_in_dir.exists(), "it wrote nothing");
        assert!(paths.recordings.iter().all(|r| r.exists()));
        match result {
            Err(StepError::Refused(refusal)) => (refusal, fake.calls()),
            other => panic!("{other:?}"),
        }
    };

    let paths = package("no-binary");
    fs::remove_file(&paths.binary).expect("no binary");
    let (refusal, calls) = refused(&paths, "active");
    assert!(matches!(refusal, Refusal::NotInstalled { .. }), "{refusal}");
    assert!(calls.is_empty(), "{calls:?}");

    let paths = package("no-unit");
    fs::remove_file(&paths.vendor_unit).expect("no unit");
    let (refusal, _) = refused(&paths, "active");
    assert!(matches!(refusal, Refusal::NotInstalled { .. }), "{refusal}");

    let paths = package("odd-unit");
    fs::write(&paths.vendor_unit, "[Service]\nExecStart=/usr/bin/other\n").expect("a unit");
    let (refusal, calls) = refused(&paths, "active");
    assert!(matches!(refusal, Refusal::VendorUnit { .. }), "{refusal}");
    assert!(calls.is_empty(), "{calls:?}");

    for state in ["inactive", "activating", "failed", "deactivating", ""] {
        let paths = package("not-active");
        let (refusal, calls) = refused(&paths, state);
        assert_eq!(
            refusal,
            Refusal::NotActive {
                state: state.to_owned()
            }
        );
        assert!(refusal.to_string().contains("Start at boot switch"));
        assert_eq!(calls, [SHOW], "it only asked");
    }
}

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[test]
fn only_pkexec_on_a_users_behalf_with_no_argument_gets_past_the_caller_check() {
    let uid = OsString::from("1000");
    assert_eq!(check_caller(0, Some(&uid), &[]), Ok(()));
    assert_eq!(
        check_caller(1000, Some(&uid), &[]),
        Err(Refusal::NotRoot { euid: 1000 })
    );
    assert_eq!(check_caller(0, None, &[]), Err(Refusal::NoPkexecUid));
    for bad in [
        "",
        "+1000",
        "-1",
        "10x0",
        " 1000",
        "1000 ",
        "99999999999",
        "0x10",
    ] {
        assert_eq!(
            check_caller(0, Some(&OsString::from(bad)), &[]),
            Err(Refusal::BadPkexecUid),
            "{bad:?}"
        );
    }
    assert_eq!(
        check_caller(0, Some(&OsString::from_vec(vec![0xff, b'1'])), &[]),
        Err(Refusal::BadPkexecUid)
    );
    for args in [
        &["on"][..],
        &["--help"],
        &["/etc/passwd"],
        &["a", "b"],
        &[""],
    ] {
        assert_eq!(
            check_caller(0, Some(&uid), &os(args)),
            Err(Refusal::Arguments { count: args.len() }),
            "{args:?}"
        );
    }
    // The first check that fails is the one told.
    assert_eq!(
        check_caller(1000, None, &os(&["x"])),
        Err(Refusal::NotRoot { euid: 1000 })
    );
}

/// The program started as `pkexec` would start it, but without the root `pkexec` gives: whoever
/// runs it as itself, as root or not, is refused with exit 2 before the step does anything. (A
/// root caller that is also `pkexec`'s, with no argument, is not tried here: it would act.)
#[test]
fn the_program_refuses_a_caller_that_pkexec_did_not_start_with_exit_2() {
    let run = |uid: Option<&str>, args: &[OsString]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_odc-e2e"));
        command.arg("dev-interface-root").args(args);
        match uid {
            Some(uid) => command.env("PKEXEC_UID", uid),
            None => command.env_remove("PKEXEC_UID"),
        };
        command.output().expect("the program")
    };
    for (uid, args) in [
        (None, os(&[])),
        (Some("+5"), os(&[])),
        (Some("x"), os(&[])),
        (Some("1000"), os(&["--help"])),
        (Some("1000"), os(&["a", "b"])),
        (Some("1000"), vec![OsString::from_vec(vec![0xff, 0xfe])]),
    ] {
        let out = run(uid, &args);
        let said = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{uid:?} {args:?}: {said}");
        assert!(said.contains("refused"), "{uid:?} {args:?}: {said}");
        assert!(out.stdout.is_empty(), "{uid:?} {args:?}");
    }
}

/// A guard over the step on a thread: its `finish` joins the thread.
fn guard(
    paths: &Paths,
    fake: &Arc<Fake>,
) -> Result<Guard<BufReader<io::PipeReader>, io::PipeWriter>, String> {
    let (mut stdin, to_step) = io::pipe().expect("a pipe");
    let (from_step, mut out) = io::pipe().expect("a pipe");
    let (paths, fake) = (paths.clone(), Arc::clone(fake));
    let thread = thread::spawn(move || step(&paths, &*fake, &mut stdin, &mut out));
    Guard::open(
        BufReader::new(from_step),
        to_step,
        Box::new(move || {
            thread
                .join()
                .expect("the step")
                .map_err(|e| format!("exit {}: {e}", e.code()))
        }),
    )
}

#[test]
fn the_guard_holds_the_step_until_the_run_closes_it_and_then_it_is_all_back() {
    let paths = package("guard-close");
    let fake = Fake::new(&paths, "active");
    let guard = guard(&paths, &fake).expect("ready");
    assert!(paths.drop_in_dir.join(DROP_IN).exists());
    let note = guard.close().expect("closed");
    assert!(
        note.contains("removed") && note.contains("without --dev"),
        "{note}"
    );
    assert!(!paths.drop_in_dir.exists());
    assert_eq!(fake.calls().len(), 5);
}

#[test]
fn the_guard_dropped_without_a_close_still_has_the_step_put_everything_back() {
    let paths = package("guard-drop");
    let fake = Fake::new(&paths, "active");
    let guard = guard(&paths, &fake).expect("ready");
    assert!(paths.drop_in_dir.join(DROP_IN).exists());
    // A panic in the run, or a run killed: the pipe's end goes with it.
    drop(guard);
    assert!(!paths.drop_in_dir.exists());
    assert!(paths.recordings.iter().all(|r| !r.exists()));
    assert_eq!(fake.calls().len(), 5);
}

#[test]
fn a_step_that_refuses_ends_the_guards_open_with_how_it_ended() {
    let paths = package("guard-refused");
    let fake = Fake::new(&paths, "inactive");
    let Err(why) = guard(&paths, &fake) else {
        panic!("it opened");
    };
    assert!(why.contains("without saying \"ready\""), "{why}");
    assert!(
        why.contains("exit 2") && why.contains("not active"),
        "{why}"
    );
}

#[test]
fn a_step_that_says_something_else_or_ends_early_is_a_failure() {
    let opened = |said: &'static str| {
        let (from_step, mut out) = io::pipe().expect("a pipe");
        let (_stdin, to_step) = io::pipe().expect("a pipe");
        let thread = thread::spawn(move || out.write_all(said.as_bytes()));
        Guard::open(
            BufReader::new(from_step),
            to_step,
            Box::new(move || {
                thread.join().expect("the thread").expect("written");
                Ok(())
            }),
        )
    };
    let Err(why) = opened("oops\n") else {
        panic!("it opened")
    };
    assert!(why.contains("said \"oops\""), "{why}");
    let Err(why) = opened("") else {
        panic!("it opened")
    };
    assert!(why.contains("ended without saying"), "{why}");
    // The line ends at the pipe's end too.
    let _ = opened(READY).expect("opened").close();

    // A step that said `ready` but ends without `removed`, or with a failure, is told.
    let closing = |after: &'static str, finish: Result<(), String>| {
        let (from_step, mut out) = io::pipe().expect("a pipe");
        let (mut stdin, to_step) = io::pipe().expect("a pipe");
        let thread = thread::spawn(move || {
            writeln!(out, "{READY}").expect("ready");
            let _ = stdin.read(&mut [0; 1]);
            out.write_all(after.as_bytes()).expect("the rest");
        });
        let guard = Guard::open(
            BufReader::new(from_step),
            to_step,
            Box::new(move || {
                thread.join().expect("the thread");
                finish
            }),
        )
        .expect("ready");
        guard.close()
    };
    assert!(closing("removed\n", Ok(())).is_ok());
    assert!(
        closing("", Ok(()))
            .expect_err("no removed")
            .contains("without saying \"removed\"")
    );
    assert!(
        closing("removed\n", Err("exit status: 1".into()))
            .expect_err("failed")
            .contains("exit status: 1")
    );
}

#[test]
fn the_step_is_started_by_pkexec_with_an_absolute_path_and_piped_standard_streams() {
    let me = std::env::current_exe().expect("this program");
    let command = command(&me);
    assert_eq!(command.get_program(), "/usr/bin/pkexec");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(
        args,
        [
            OsString::from("--disable-internal-agent"),
            me.clone().into_os_string(),
            OsString::from("dev-interface-root")
        ]
    );
    assert!(
        Path::new(args[1]).is_absolute(),
        "pkexec runs in root's home"
    );
}

#[test]
fn a_leftover_drop_in_is_reported_at_the_start_and_in_the_summary() {
    let tree = FakeTree::create(dir("leftover")).expect("a fake tree");
    let roots = tree.roots();
    assert_eq!(leftover(&roots.run), None);
    let options = Options {
        only: Some(Area::GpuRank),
        ..Options::default()
    };
    let quiet = run(
        &options,
        &roots,
        &dir("leftover-out"),
        std::time::Duration::from_millis(1),
    );
    assert!(
        !quiet.table().contains("dev_interface"),
        "{}",
        quiet.table()
    );

    let folder = drop_in_dir(&roots.run);
    fs::create_dir_all(&folder).expect("the folder");
    fs::write(folder.join(DROP_IN), "left").expect("a leftover");
    assert_eq!(leftover(&roots.run), Some(folder.join(DROP_IN)));
    let summary = run(
        &options,
        &roots,
        &dir("leftover-out"),
        std::time::Duration::from_millis(1),
    );
    let table = summary.table();
    assert!(summary.passed(), "a note, not a failure: {table}");
    assert!(
        table.contains("dev_interface_leftover passed")
            && table.contains("keeps --dev until it is removed"),
        "{table}"
    );
}
