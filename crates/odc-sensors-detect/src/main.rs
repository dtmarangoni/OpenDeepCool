//! `odc-sensors-detect`: on a PC without an lm-sensors setup, runs lm-sensors' `sensors-detect
//! --auto` once per motherboard, so the board's sensor-chip driver, which measures the cooler's
//! fans and pump, loads at every boot, and records what that run created (`docs/spec/sensors.md`,
//! Board sensor driver; Phase 6 plan, Task 10).
//!
//! It's OpenDeepCool's one root part (PRD, Service model), so it uses only the standard library,
//! never touches a file it didn't create, and keeps its record in a directory of its own that
//! only its account may use (owner, 2026-10-26; solution review OV-01). Its paths come from
//! flags, so the tests run it on fake roots with a fake `sensors-detect`.

mod record;

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use record::{Board, RECORD, Record, today};

const USAGE: &str = "usage: odc-sensors-detect [--etc <dir>] [--sys <dir>] [--state-dir <dir>]
                          [--sensors-detect <path>] [--systemctl <path>]

On a PC with no lm-sensors setup, runs `sensors-detect --auto` once per
motherboard and records what it created in the state directory.

  --etc <dir>              the configuration root (default /etc)
  --sys <dir>              the sysfs root (default /sys)
  --state-dir <dir>        the record's directory (default /var/lib/opendeepcool-detect)
  --sensors-detect <path>  lm-sensors' script (default /usr/bin/sensors-detect)
  --systemctl <path>       systemd's control program (default /usr/bin/systemctl)
  -h, --help               this text";

/// Where to look and what to run.
struct Options {
    etc: PathBuf,
    sys: PathBuf,
    state_dir: PathBuf,
    sensors_detect: PathBuf,
    systemctl: PathBuf,
}

/// The options, or `None` for `--help`. An unknown flag or a flag without its value is an error.
fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut options = Options {
        etc: PathBuf::from("/etc"),
        sys: PathBuf::from("/sys"),
        state_dir: PathBuf::from("/var/lib/opendeepcool-detect"),
        sensors_detect: PathBuf::from("/usr/bin/sensors-detect"),
        systemctl: PathBuf::from("/usr/bin/systemctl"),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--etc" => &mut options.etc,
            "--sys" => &mut options.sys,
            "--state-dir" => &mut options.state_dir,
            "--sensors-detect" => &mut options.sensors_detect,
            "--systemctl" => &mut options.systemctl,
            _ => return Err(format!("{arg:?} isn't an option")),
        };
        *slot = args
            .next()
            .ok_or_else(|| format!("{arg} needs a path"))?
            .into();
    }
    Ok(Some(options))
}

/// Lines with `<N>` in front reach the journal at that level (sd-daemon(3)): 3 an error, 4 a
/// warning, 6 information.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("odc-sensors-detect: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(note) => {
            println!("<6>odc-sensors-detect: {note}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("<3>odc-sensors-detect: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The steps of the plan's Task 10, in order. `Ok` says why it stopped or what it did.
fn run(options: &Options) -> Result<String, String> {
    let conf_d = options.etc.join("conf.d/lm_sensors");
    let modprobe_d = options.etc.join("modprobe.d/lm_sensors.conf");
    // An existing setup is never overwritten: sensors-detect would overwrite either file.
    for file in [&conf_d, &modprobe_d] {
        if exists(file) {
            return Ok(format!(
                "{} exists: lm-sensors is set up already",
                file.display()
            ));
        }
    }
    // The chip that measures the cooler's fans and pump is readable already (P6-13).
    if let Some(chip) = board_fan_chip(&options.sys) {
        return Ok(format!("{chip} has fan inputs already: nothing to find"));
    }
    root_only(&options.state_dir)?;
    let board = board(&options.sys);
    let record_path = options.state_dir.join(RECORD);
    if let Some(record) = read_record(&record_path)?
        && record.board == board
    {
        return Ok(format!(
            "it ran for this board on {}: once per motherboard",
            record.date
        ));
    }
    let enabled_before = is_enabled(&options.systemctl)?;
    let status = Command::new(&options.sensors_detect)
        .arg("--auto")
        .stdin(Stdio::null())
        .status()
        .map_err(|e| format!("{}: {e}", options.sensors_detect.display()))?;
    if !status.success() {
        // Recorded all the same, so it doesn't run again at every boot.
        eprintln!("<4>odc-sensors-detect: sensors-detect ended with {status}");
    }
    let record = Record {
        board,
        date: today(),
        created_conf_d: exists(&conf_d),
        created_modprobe_d: exists(&modprobe_d),
        lm_sensors_enabled_before: enabled_before,
    };
    write_record(&options.state_dir, &record)?;
    Ok(format!(
        "sensors-detect ran; it created {}{}",
        match (record.created_conf_d, record.created_modprobe_d) {
            (true, true) => "both lm-sensors files",
            (true, false) => "the lm-sensors configuration",
            (false, true) => "the module options only",
            (false, false) => "no file",
        },
        if record.lm_sensors_enabled_before {
            "; lm_sensors.service was enabled before"
        } else {
            ""
        }
    ))
}

/// Whether anything is at `path`, a dangling link included.
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// The `name` of the first hwmon device with a fan input that isn't a graphics card's: the fan
/// picker's rule (`docs/spec/sensors.md`, Fan and pump speed).
fn board_fan_chip(sys: &Path) -> Option<String> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(sys.join("class/hwmon"))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs.into_iter().find_map(|dir| {
        let has_fan = fs::read_dir(&dir).ok()?.flatten().any(|entry| {
            entry.file_name().to_str().is_some_and(|name| {
                name.strip_prefix("fan")
                    .and_then(|n| n.strip_suffix("_input"))
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
        });
        let card = dir
            .join("device")
            .canonicalize()
            .is_ok_and(|parent| is_display(&parent));
        (has_fan && !card).then(|| {
            fs::read_to_string(dir.join("name"))
                .map_or_else(|_| dir.display().to_string(), |n| n.trim().to_owned())
        })
    })
}

/// Whether a device folder's `class` is PCI base class 0x03, a display controller (the kernel's
/// `pci_ids.h`, `PCI_BASE_CLASS_DISPLAY`).
fn is_display(dir: &Path) -> bool {
    fs::read_to_string(dir.join("class"))
        .ok()
        .and_then(|class| u32::from_str_radix(class.trim().trim_start_matches("0x"), 16).ok())
        .is_some_and(|class| class >> 16 == 0x03)
}

/// Makes the state directory when it's missing, and refuses one that isn't this account's
/// alone: owned by it (root, in the unit and through `pkexec`), mode 0700, not a link.
fn root_only(dir: &Path) -> Result<(), String> {
    match fs::symlink_metadata(dir) {
        Err(e) if e.kind() == ErrorKind::NotFound => DirBuilder::new()
            .mode(0o700)
            .create(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?,
        Err(e) => return Err(format!("{}: {e}", dir.display())),
        Ok(_) => {}
    }
    let meta = fs::symlink_metadata(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let me = effective_uid()?;
    if !meta.is_dir() {
        return Err(format!("{} isn't a directory: refused", dir.display()));
    }
    if meta.uid() != me {
        return Err(format!(
            "{} is owned by uid {}, not by this account's {me}: refused",
            dir.display(),
            meta.uid()
        ));
    }
    if meta.mode() & 0o7777 != 0o700 {
        return Err(format!(
            "{} has mode {:o}, not 700: refused",
            dir.display(),
            meta.mode() & 0o7777
        ));
    }
    Ok(())
}

/// The effective user ID: the second number of `/proc/self/status`'s `Uid:` line (real,
/// effective, saved, filesystem; proc_pid_status(5)).
fn effective_uid() -> Result<u32, String> {
    let status =
        fs::read_to_string("/proc/self/status").map_err(|e| format!("/proc/self/status: {e}"))?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|ids| ids.split_whitespace().nth(1))
        .and_then(|id| id.parse().ok())
        .ok_or_else(|| "/proc/self/status has no effective uid".to_owned())
}

/// The board's identity from DMI; a file that can't be read gives an empty string.
fn board(sys: &Path) -> Board {
    let read = |name: &str| {
        fs::read_to_string(sys.join("class/dmi/id").join(name))
            .map(|text| text.trim_end_matches('\n').to_owned())
            .unwrap_or_default()
    };
    Board {
        vendor: read("board_vendor"),
        name: read("board_name"),
        version: read("board_version"),
    }
}

/// The record, `None` when there's none. A link or anything but a plain file is refused, and
/// never followed; one that doesn't read is replaced by this run's, with a warning.
fn read_record(path: &Path) -> Result<Option<Record>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if !meta.is_file() {
        return Err(format!("{} isn't a plain file: refused", path.display()));
    }
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match Record::parse(&text) {
        Ok(record) => Ok(Some(record)),
        Err(why) => {
            eprintln!("<4>odc-sensors-detect: {}: {why}; replaced", path.display());
            Ok(None)
        }
    }
}

/// Whether `lm_sensors.service` is enabled: `systemctl is-enabled` prints `enabled`.
fn is_enabled(systemctl: &Path) -> Result<bool, String> {
    let output = Command::new(systemctl)
        .args(["is-enabled", "lm_sensors.service"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", systemctl.display()))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "enabled")
}

/// Writes the record through a new file, created exclusively next to it, then renamed over it.
fn write_record(dir: &Path, record: &Record) -> Result<(), String> {
    let path = dir.join(RECORD);
    let new = dir.join(format!(".{RECORD}.new"));
    let failed = |e: std::io::Error| format!("{}: {e}", new.display());
    // Left by a run that stopped before its rename; the directory is this account's alone.
    if let Err(e) = fs::remove_file(&new)
        && e.kind() != ErrorKind::NotFound
    {
        return Err(failed(e));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&new)
        .map_err(failed)?;
    file.write_all(record.render().as_bytes()).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    fs::rename(&new, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| format!("{}: {e}", dir.display()))
}
