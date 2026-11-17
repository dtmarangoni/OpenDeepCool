//! The dev interface for the packaged service in a parity run (Phase 11 plan, Task 9; owner,
//! 2026-11-17: "One root step that stays").
//!
//! The end-to-end checks drive the service through its dev interface and read its frame
//! recording, which the packaged service has only when it is started with `--dev`. A drop-in in
//! `/run` adds that for as long as `odc-e2e --dev-interface` runs. Writing it and removing it both
//! need root, so one `pkexec` step does both and stays for the whole run: it writes the drop-in,
//! restarts the service, says `ready`, and waits until its standard input ends. The run holds the
//! pipe's writing end, so the run's end, a panic in it or a kill of it all end the wait, and the
//! step then removes what it wrote and restarts the service without `--dev`, which a guard
//! running as the owner could not do. `pkexec` runs the program by `execv`, keeps standard input,
//! output and error, and marks every other descriptor close-on-exec (polkit's `pkexec.c`, main
//! branch, read 2026-11-17); the owner's account can't signal a root process.
//!
//! The step, `odc-e2e dev-interface-root`, is started by `pkexec` with this program's own path, like
//! `odc-dev`'s root steps, and is left out of `USAGE`. It refuses a caller or a state it doesn't
//! expect, and takes nothing from its caller: the drop-in's text is fixed, with the service
//! binary's path read from the root-owned vendor unit.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

/// The root step's name, its first argument.
pub const ROOT_STEP: &str = "dev-interface-root";

/// The service's unit.
pub const UNIT: &str = "opendeepcool.service";

/// The drop-in's file name.
pub const DROP_IN: &str = "odc-e2e.conf";

/// What the step prints once the service runs with `--dev`, and once it has put things back.
pub const READY: &str = "ready";
pub const REMOVED: &str = "removed";

/// Exit codes: 2 for a refusal, before anything is written, and 1 for a failed step.
pub const REFUSED: u8 = 2;
pub const FAILED: u8 = 1;

const PKEXEC: &str = "/usr/bin/pkexec";
const SYSTEMCTL: &str = "/usr/bin/systemctl";

/// The drop-in's fixed lines after `ExecStart=`: the dev copy's (`tools/dev/opendeepcool-dev.conf`),
/// so the tests, which run as the owner in group `opendeepcool`, can read the recording.
pub const DEV_LINES: [&str; 2] = ["UMask=0027", "StateDirectoryMode=0750"];

/// Where things are: the real ones, or a test's.
#[derive(Clone, Debug)]
pub struct Paths {
    /// The package's unit, which root owns.
    pub vendor_unit: PathBuf,
    /// The service binary the unit starts.
    pub binary: PathBuf,
    /// The unit's drop-in folder in `/run`.
    pub drop_in_dir: PathBuf,
    /// The recordings the dev service writes in its state directory.
    pub recordings: Vec<PathBuf>,
}

impl Paths {
    pub fn system() -> Paths {
        let state = Path::new("/var/lib/opendeepcool");
        Paths {
            vendor_unit: PathBuf::from("/usr/lib/systemd/system").join(UNIT),
            binary: PathBuf::from("/usr/lib/opendeepcool/opendeepcoold"),
            drop_in_dir: drop_in_dir(Path::new("/run")),
            recordings: vec![
                state.join(odc_service::RECORDING),
                state.join(format!("{}.1", odc_service::RECORDING)),
            ],
        }
    }
}

/// The unit's drop-in folder under a `/run`.
pub fn drop_in_dir(run: &Path) -> PathBuf {
    run.join("systemd/system").join(format!("{UNIT}.d"))
}

/// A drop-in an earlier run's step left behind, which was killed outright (solution review
/// P11-18).
pub fn leftover(run: &Path) -> Option<PathBuf> {
    let path = drop_in_dir(run).join(DROP_IN);
    path.exists().then_some(path)
}

/// Why the step refuses, before it writes anything.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    NotRoot { euid: u32 },
    NoPkexecUid,
    BadPkexecUid,
    Arguments { count: usize },
    NotInstalled { what: String },
    VendorUnit { why: String },
    NotActive { state: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotRoot { euid } => {
                write!(
                    f,
                    "refused: it runs through pkexec as root, not as user {euid}"
                )
            }
            Refusal::NoPkexecUid => write!(
                f,
                "refused: PKEXEC_UID isn't set, so pkexec didn't start it"
            ),
            Refusal::BadPkexecUid => write!(f, "refused: PKEXEC_UID isn't a user ID"),
            Refusal::Arguments { count } => {
                write!(f, "refused: it takes no argument, and was given {count}")
            }
            Refusal::NotInstalled { what } => {
                write!(f, "refused: the package isn't installed: {what}")
            }
            Refusal::VendorUnit { why } => write!(f, "refused: the package's unit: {why}"),
            Refusal::NotActive { state } => write!(
                f,
                "refused: {UNIT} is {state}, not active; turn OpenDeepCool on first, with the \
                 app's Start at boot switch"
            ),
        }
    }
}

/// How the step ends when it doesn't.
#[derive(Debug, PartialEq, Eq)]
pub enum StepError {
    Refused(Refusal),
    Failed(String),
}

impl StepError {
    pub fn code(&self) -> u8 {
        match self {
            StepError::Refused(_) => REFUSED,
            StepError::Failed(_) => FAILED,
        }
    }
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepError::Refused(refusal) => refusal.fmt(f),
            StepError::Failed(why) => f.write_str(why),
        }
    }
}

/// The effective user ID in `/proc/self/status`; see `odc_boot::effective_uid`.
pub use odc_boot::effective_uid;

/// Whether the caller is `pkexec` on a user's behalf, and gave nothing: root, a `PKEXEC_UID` of
/// digits only (`u32`'s parser would also take a leading `+`), and no argument.
pub fn check_caller(
    euid: u32,
    pkexec_uid: Option<&OsStr>,
    args: &[OsString],
) -> Result<(), Refusal> {
    if euid != 0 {
        return Err(Refusal::NotRoot { euid });
    }
    let uid = pkexec_uid.ok_or(Refusal::NoPkexecUid)?;
    let user_id = uid.to_str().is_some_and(|uid| {
        !uid.is_empty() && uid.bytes().all(|b| b.is_ascii_digit()) && uid.parse::<u32>().is_ok()
    });
    if !user_id {
        return Err(Refusal::BadPkexecUid);
    }
    if !args.is_empty() {
        return Err(Refusal::Arguments { count: args.len() });
    }
    Ok(())
}

/// The drop-in's text: the package's one `ExecStart=` line, which must name exactly `binary`,
/// again with `--dev`, and the dev copy's lines.
pub fn drop_in_text(vendor_unit: &str, binary: &Path) -> Result<String, Refusal> {
    let binary = binary.to_string_lossy();
    let mut starts = vendor_unit.lines().filter_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "ExecStart").then_some(value.trim())
    });
    let (Some(start), None) = (starts.next(), starts.next()) else {
        return Err(Refusal::VendorUnit {
            why: "it doesn't have exactly one ExecStart= line".into(),
        });
    };
    if start != binary {
        return Err(Refusal::VendorUnit {
            why: format!("ExecStart= is {start:?}, not exactly {binary:?}"),
        });
    }
    Ok(format!(
        "# Written by `odc-e2e --dev-interface` (Phase 11 plan, Task 9) and removed when the run\n\
         # ends: the packaged service with the dev interface and the frame recording.\n\
         [Service]\nExecStart=\nExecStart={binary} --dev\n{}\n{}\n",
        DEV_LINES[0], DEV_LINES[1]
    ))
}

/// What the step asks systemd.
pub trait Systemctl {
    /// The command's standard output; a failure says the command and why.
    fn run(&self, args: &[&str]) -> Result<String, String>;
}

/// `/usr/bin/systemctl` with an emptied environment.
pub struct RealSystemctl;

impl Systemctl for RealSystemctl {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new(SYSTEMCTL)
            .env_clear()
            .args(args)
            .output()
            .map_err(|e| format!("{SYSTEMCTL}: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        } else {
            Err(format!(
                "systemctl {}: {}: {}",
                args.join(" "),
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
}

/// The step: checks the package and the unit, writes the drop-in, restarts the service, says
/// [`READY`], waits until `stdin` ends, then removes what it wrote and says [`REMOVED`].
pub fn step(
    paths: &Paths,
    systemctl: &dyn Systemctl,
    stdin: &mut dyn Read,
    out: &mut dyn Write,
) -> Result<(), StepError> {
    if !paths.binary.is_file() {
        return Err(StepError::Refused(Refusal::NotInstalled {
            what: format!("no {}", paths.binary.display()),
        }));
    }
    let unit = fs::read_to_string(&paths.vendor_unit).map_err(|e| {
        StepError::Refused(Refusal::NotInstalled {
            what: format!("{}: {e}", paths.vendor_unit.display()),
        })
    })?;
    let text = drop_in_text(&unit, &paths.binary).map_err(StepError::Refused)?;
    let state = systemctl
        .run(&["show", "--property=ActiveState", "--value", UNIT])
        .map_err(StepError::Failed)?;
    if state != "active" {
        return Err(StepError::Refused(Refusal::NotActive { state }));
    }

    let drop_in = paths.drop_in_dir.join(DROP_IN);
    let written = (|| -> Result<(), String> {
        write_drop_in(&paths.drop_in_dir, &text)
            .map_err(|e| format!("{}: {e}", drop_in.display()))?;
        systemctl.run(&["daemon-reload"])?;
        systemctl.run(&["restart", UNIT])?;
        writeln!(out, "{READY}")
            .and_then(|()| out.flush())
            .map_err(|e| format!("telling the run: {e}"))
    })();
    if let Err(why) = written {
        // Whatever was written, back out of it; the first failure is the one to tell first.
        let why = match remove(paths, systemctl) {
            Ok(()) => why,
            Err(more) => format!("{why}; {more}"),
        };
        return Err(StepError::Failed(why));
    }
    // Until the run lets go of the pipe, which it does at its end, on a panic and when killed.
    let _ = io::copy(stdin, &mut io::sink());
    remove(paths, systemctl).map_err(StepError::Failed)?;
    let _ = writeln!(out, "{REMOVED}").and_then(|()| out.flush());
    Ok(())
}

/// The drop-in, atomically: a file systemd ignores, renamed into place.
fn write_drop_in(dir: &Path, text: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join(".odc-e2e.conf.new");
    fs::write(&tmp, text)?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644))?;
    fs::rename(&tmp, dir.join(DROP_IN))
}

/// Takes the drop-in away (and its folder if nothing else is in it), has systemd read the unit
/// again and restart the service without `--dev`, then removes the recordings the dev service
/// wrote, which the restart has closed. Every part runs; what failed is told together.
fn remove(paths: &Paths, systemctl: &dyn Systemctl) -> Result<(), String> {
    let mut failed = Vec::new();
    let gone = |result: io::Result<()>, path: &Path, failed: &mut Vec<String>| match result {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => failed.push(format!("{}: {e}", path.display())),
    };
    let drop_in = paths.drop_in_dir.join(DROP_IN);
    gone(fs::remove_file(&drop_in), &drop_in, &mut failed);
    // A folder with another admin's drop-in in it stays.
    match fs::remove_dir(&paths.drop_in_dir) {
        Ok(()) => {}
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
            ) => {}
        Err(e) => failed.push(format!("{}: {e}", paths.drop_in_dir.display())),
    }
    for args in [&["daemon-reload"][..], &["restart", UNIT][..]] {
        if let Err(e) = systemctl.run(args) {
            failed.push(e);
        }
    }
    for recording in &paths.recordings {
        gone(fs::remove_file(recording), recording, &mut failed);
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; "))
    }
}

/// The step as a process: the checks of the caller, then [`step`] on the real system, with the
/// exit code `main` returns.
pub fn root_main(args: &[OsString]) -> u8 {
    let euid = fs::read_to_string("/proc/self/status")
        .ok()
        .as_deref()
        .and_then(effective_uid);
    let Some(euid) = euid else {
        eprintln!("odc-e2e: /proc/self/status gives no effective user ID: refused");
        return REFUSED;
    };
    if let Err(refusal) = check_caller(euid, std::env::var_os("PKEXEC_UID").as_deref(), args) {
        eprintln!("odc-e2e {ROOT_STEP}: {refusal}");
        return REFUSED;
    }
    let result = step(
        &Paths::system(),
        &RealSystemctl,
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
    );
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("odc-e2e {ROOT_STEP}: {e}");
            e.code()
        }
    }
}

/// How the step's process ended, in words.
pub fn describe(status: ExitStatus) -> String {
    match status.code() {
        Some(126) => "the password dialog was dismissed (pkexec 126)".into(),
        Some(127) => "pkexec wasn't authorised, or no polkit agent answered (127)".into(),
        Some(2) => "the step refused (its reason is above)".into(),
        Some(1) => "the step failed (its reason is above)".into(),
        _ => status.to_string(),
    }
}

/// Waits for the step's process and tells how it ended.
pub type Finish = Box<dyn FnOnce() -> Result<(), String>>;

/// The run's end of the step: held for the whole run, closed at its end or when dropped.
pub struct Guard<R: BufRead, W: Write> {
    to_step: Option<W>,
    from_step: R,
    finish: Option<Finish>,
}

/// The guard over a real `pkexec` process.
pub type Spawned = Guard<BufReader<ChildStdout>, ChildStdin>;

impl<R: BufRead, W: Write> Guard<R, W> {
    /// Waits for the step to say [`READY`]. Anything else, or the end of its output, is a
    /// failure, told with how the step ended.
    pub fn open(mut from_step: R, to_step: W, finish: Finish) -> Result<Guard<R, W>, String> {
        let mut line = String::new();
        let read = from_step.read_line(&mut line);
        if matches!(read, Ok(n) if n > 0) && line.trim() == READY {
            return Ok(Guard {
                to_step: Some(to_step),
                from_step,
                finish: Some(finish),
            });
        }
        drop(to_step);
        let ended = finish().err().unwrap_or_else(|| "it ended".into());
        Err(match read {
            Ok(n) if n > 0 => format!("the step said {:?}, not {READY:?}: {ended}", line.trim()),
            Ok(_) => format!("the step ended without saying {READY:?}: {ended}"),
            Err(e) => format!("reading the step: {e}: {ended}"),
        })
    }

    /// Lets go of the pipe, which ends the step's wait, then waits for it to say [`REMOVED`] and
    /// end.
    pub fn close(mut self) -> Result<String, String> {
        self.shut()
    }

    fn shut(&mut self) -> Result<String, String> {
        let Some(finish) = self.finish.take() else {
            return Err("the step was already closed".into());
        };
        self.to_step = None;
        let mut removed = false;
        let mut line = String::new();
        loop {
            line.clear();
            match self.from_step.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => removed |= line.trim() == REMOVED,
            }
        }
        let ended = finish();
        match (removed, ended) {
            (true, Ok(())) => Ok(format!(
                "the drop-in is removed and {UNIT} runs without --dev again"
            )),
            (_, Err(why)) => Err(format!("the step didn't end cleanly: {why}")),
            (false, Ok(())) => Err(format!("the step ended without saying {REMOVED:?}")),
        }
    }
}

impl<R: BufRead, W: Write> Drop for Guard<R, W> {
    fn drop(&mut self) {
        if self.finish.is_some()
            && let Err(e) = self.shut()
        {
            eprintln!("odc-e2e: the dev interface's step: {e}");
        }
    }
}

/// `pkexec` running this program's root step, with its standard input and output piped to the
/// run. `--disable-internal-agent`: a missing polkit agent fails at once instead of asking on a
/// terminal that nobody watches. The path is absolute, because `pkexec` runs the program in
/// root's home directory.
pub fn command(me: &Path) -> Command {
    let mut command = Command::new(PKEXEC);
    command
        .arg("--disable-internal-agent")
        .arg(me)
        .arg(ROOT_STEP)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    command
}

/// Starts the step through `pkexec` and waits for it to say [`READY`]: the owner types the
/// password meanwhile.
pub fn open() -> Result<Spawned, String> {
    let me = std::env::current_exe().map_err(|e| format!("this program's path: {e}"))?;
    println!(
        "odc-e2e will, through pkexec, add the dev interface to {UNIT} for this run: a drop-in in \
         /run/systemd/system/{UNIT}.d/, and a restart. It removes both when the run ends, however \
         it ends."
    );
    let mut child: Child = command(&me).spawn().map_err(|e| format!("{PKEXEC}: {e}"))?;
    let (Some(stdout), Some(stdin)) = (child.stdout.take(), child.stdin.take()) else {
        return Err("the step's pipes".into());
    };
    let finish: Finish = Box::new(move || match child.wait() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(describe(status)),
        Err(e) => Err(format!("waiting for the step: {e}")),
    });
    Guard::open(BufReader::new(stdout), stdin, finish)
}
