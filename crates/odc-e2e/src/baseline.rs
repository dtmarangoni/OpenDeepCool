//! The resource baseline (Phase 7 plan, Task 10; PRD, Success Metrics, "measure a baseline in
//! Phase 7"), `--only baseline` and never part of the default run: the dev service in mode 4 on
//! its real readings for 10 minutes. Once a second the run samples the service's CPU time,
//! utime + stime of `/proc/<pid>/stat` in clock ticks (proc_pid_stat(5), fields 14 and 15), and
//! its `VmRSS` from `/proc/<pid>/status`; at the end, the writes' and snapshots' durations. It
//! prints the mean CPU, the peak RSS and the durations' p50, p99 and maximum; the owner set no
//! target. Afterwards the owner's settings are as they were.

use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use odc_testkit::hardware::{Capability, DEV_UNIT, Mode, Probe, Roots, Summary, probe};

use crate::service::{Restore, durations, state};
use crate::session::Session;

const LENGTH: Duration = Duration::from_secs(600);

pub fn run(roots: &Roots, summary: &mut Summary) {
    let result = match probe(Capability::DevService, roots) {
        Probe::Real => {
            summary.mode(Capability::DevService, Mode::Real);
            measure()
        }
        Probe::Absent { reason } => Err(format!("the baseline needs the dev service: {reason}")),
        Probe::Unusable { reason, fix } => Err(format!("{reason}; fix: {fix}")),
    };
    summary.check("baseline", result);
}

fn output(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// utime + stime, in clock ticks: the fields after the name, which is in parentheses and may hold
/// spaces, start with field 3.
fn cpu_ticks(stat: &str) -> Option<u64> {
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    Some(fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?)
}

/// `VmRSS`, in kB.
fn rss_kb(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

fn measure() -> Result<String, String> {
    let session = Session::begin()?;
    let _restore = Restore {
        state: state(&session)?,
        session: &session,
    };
    zbus::block_on(session.main().set_mode(4)).map_err(|e| format!("SetMode: {e}"))?;
    let pid = output(
        "systemctl",
        &["show", "--property=MainPID", "--value", DEV_UNIT],
    )?;
    let per_second: u64 = output("getconf", &["CLK_TCK"])?
        .parse()
        .map_err(|_| "getconf CLK_TCK isn't a number")?;
    let read = |file: &str| -> Result<String, String> {
        let path = format!("/proc/{pid}/{file}");
        fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))
    };
    let first = cpu_ticks(&read("stat")?).ok_or("an unreadable /proc/<pid>/stat")?;
    let began = Instant::now();
    let mut peak = 0;
    while began.elapsed() < LENGTH {
        thread::sleep(Duration::from_secs(1));
        peak = peak.max(rss_kb(&read("status")?).ok_or("no VmRSS")?);
    }
    let last = cpu_ticks(&read("stat")?).ok_or("an unreadable /proc/<pid>/stat")?;
    let cpu = (last - first) as f64 / per_second as f64 / began.elapsed().as_secs_f64() * 100.0;
    let times = durations(&session)?;
    Ok(format!(
        "mode 4, {} s: mean CPU {cpu:.2} % of one core, peak RSS {:.1} MiB; {times}",
        began.elapsed().as_secs(),
        peak as f64 / 1024.0
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_time_and_rss_come_from_proc() {
        let stat = "4242 (opendeep coold) S 1 4242 4242 0 -1 4194560 300 0 0 0 1234 567 0 0 20 0 \
                    6 0 999 0 0";
        assert_eq!(cpu_ticks(stat), Some(1234 + 567));
        assert_eq!(rss_kb("Name:\tx\nVmRSS:\t    2144 kB\n"), Some(2144));
        assert_eq!(cpu_ticks("4242 (x"), None);
    }
}
