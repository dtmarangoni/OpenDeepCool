//! The resource baseline (Phase 7 plan, Task 10; PRD, Success Metrics, "measure a baseline in
//! Phase 7"), `--only baseline` and never part of the default run: the dev service in mode 4 on
//! its real readings for 10 minutes. Once a second the run samples the service's CPU time,
//! utime + stime of `/proc/<pid>/stat` in clock ticks (proc_pid_stat(5), fields 14 and 15), and
//! its `VmRSS` from `/proc/<pid>/status`; at the end, the writes' and snapshots' durations. It
//! prints the mean CPU, the peak RSS and the durations' p50, p99 and maximum; the owner set no
//! target. Afterwards the owner's settings are as they were.

use std::collections::BTreeMap;
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

/// Each thread's name and exact run time so far in ns, by thread ID: the name from the thread's
/// `stat` and the run time from the first field of its `schedstat` (proc_pid_schedstat in
/// proc(5)), which the scheduler keeps exactly where `utime` and `stime` are sampled by the tick.
/// A thread that ends while it is read is left out.
pub(crate) fn thread_run_times(pid: &str) -> Result<BTreeMap<u32, (String, u64)>, String> {
    let folder = format!("/proc/{pid}/task");
    let mut found = BTreeMap::new();
    for entry in fs::read_dir(&folder)
        .map_err(|e| format!("{folder}: {e}"))?
        .flatten()
    {
        let Some(tid) = entry
            .file_name()
            .to_str()
            .and_then(|t| t.parse::<u32>().ok())
        else {
            continue;
        };
        let read = |file: &str| fs::read_to_string(entry.path().join(file));
        if let (Ok(stat), Ok(sched)) = (read("stat"), read("schedstat"))
            && let (Some(name), Some(ns)) = (thread_name(&stat), run_ns(&sched))
        {
            found.insert(tid, (name.to_owned(), ns));
        }
    }
    if found.is_empty() {
        return Err(format!("{folder}: no thread could be read"));
    }
    Ok(found)
}

/// The thread's name: the text in the parentheses of its `stat`, which may hold spaces and
/// parentheses itself, so up to the last closing one.
fn thread_name(stat: &str) -> Option<&str> {
    Some(&stat[stat.find('(')? + 1..stat.rfind(')')?])
}

/// The first field of a `schedstat`: the time on the CPU in ns.
fn run_ns(schedstat: &str) -> Option<u64> {
    schedstat.split_whitespace().next()?.parse().ok()
}

/// What each thread name used between two readings, in percent of one core over `seconds`, most
/// first. Threads of one name add up, and a thread in only one of the readings is left out.
fn thread_shares(
    first: &BTreeMap<u32, (String, u64)>,
    last: &BTreeMap<u32, (String, u64)>,
    seconds: f64,
) -> Vec<(String, f64)> {
    let mut by_name: BTreeMap<&str, u64> = BTreeMap::new();
    for (tid, (name, ns)) in last {
        if let Some((_, before)) = first.get(tid) {
            *by_name.entry(name).or_default() += ns.saturating_sub(*before);
        }
    }
    let mut shares: Vec<(String, f64)> = by_name
        .into_iter()
        .map(|(name, ns)| (name.to_owned(), ns as f64 / 1e9 / seconds * 100.0))
        .collect();
    shares.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    shares
}

/// The shares as one phrase: each thread's, down to 0.005 %.
fn shares_text(shares: &[(String, f64)]) -> String {
    let shown: Vec<String> = shares
        .iter()
        .filter(|(_, share)| *share >= 0.005)
        .map(|(name, share)| format!("{name} {share:.3} %"))
        .collect();
    shown.join(", ")
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
    let threads_first = thread_run_times(&pid)?;
    let began = Instant::now();
    let mut peak = 0;
    while began.elapsed() < LENGTH {
        thread::sleep(Duration::from_secs(1));
        peak = peak.max(rss_kb(&read("status")?).ok_or("no VmRSS")?);
    }
    let last = cpu_ticks(&read("stat")?).ok_or("an unreadable /proc/<pid>/stat")?;
    let threads = thread_shares(
        &threads_first,
        &thread_run_times(&pid)?,
        began.elapsed().as_secs_f64(),
    );
    let cpu = (last - first) as f64 / per_second as f64 / began.elapsed().as_secs_f64() * 100.0;
    let times = durations(&session)?;
    Ok(format!(
        "mode 4, {} s: mean CPU {cpu:.2} % of one core, peak RSS {:.1} MiB; by thread (exact run \
         time): {}; {times}",
        began.elapsed().as_secs(),
        peak as f64 / 1024.0,
        shares_text(&threads)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_threads_name_and_run_time_are_read_as_the_kernel_writes_them() {
        assert_eq!(thread_name("4242 (sensors) S 1 4242"), Some("sensors"));
        // A name may hold spaces and parentheses; it ends at the last closing one.
        assert_eq!(thread_name("4242 (a (b) c) S 1 4242"), Some("a (b) c"));
        assert_eq!(thread_name("4242 no parentheses"), None);
        assert_eq!(run_ns("1234567 89 10\n"), Some(1_234_567));
        assert_eq!(run_ns(""), None);
        assert_eq!(run_ns("many 1 2"), None);
    }

    fn times(list: &[(u32, &str, u64)]) -> BTreeMap<u32, (String, u64)> {
        list.iter()
            .map(|(tid, name, ns)| (*tid, ((*name).to_owned(), *ns)))
            .collect()
    }

    #[test]
    fn each_thread_name_gets_its_share_of_the_interval_and_a_pool_adds_up() {
        let first = times(&[
            (1, "sensors", 0),
            (2, "device", 0),
            (3, "pool", 0),
            (4, "pool", 0),
            (5, "gone", 0),
        ]);
        // 10 s: sensors 40 ms, device 10 ms, the pool's two threads 5 ms each; thread 5 ended and
        // thread 6 began in between, so neither counts.
        let last = times(&[
            (1, "sensors", 40_000_000),
            (2, "device", 10_000_000),
            (3, "pool", 5_000_000),
            (4, "pool", 5_000_000),
            (6, "new", 900_000_000),
        ]);
        let shares = thread_shares(&first, &last, 10.0);
        let names: Vec<&str> = shares.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["sensors", "device", "pool"], "most first");
        let close = |got: f64, want: f64| (got - want).abs() < 1e-9;
        assert!(close(shares[0].1, 0.4), "{shares:?}");
        assert!(close(shares[1].1, 0.1), "{shares:?}");
        assert!(
            close(shares[2].1, 0.1),
            "two threads of one name add up: {shares:?}"
        );
        assert_eq!(
            shares_text(&shares),
            "sensors 0.400 %, device 0.100 %, pool 0.100 %"
        );
        // Under 0.005 % isn't shown.
        assert_eq!(
            shares_text(&[("idle".into(), 0.004), ("busy".into(), 0.5)]),
            "busy 0.500 %"
        );
        // A counter that went backwards, as when a thread's ID is reused, counts as nothing.
        assert!(thread_shares(&times(&[(1, "x", 9)]), &times(&[(1, "x", 3)]), 1.0)[0].1 < 1e-12);
    }

    /// The real `/proc`: this process's own threads, read as the baseline reads the service's.
    #[test]
    fn this_process_s_own_threads_are_read_with_names_and_run_times() {
        let pid = std::process::id().to_string();
        let first = thread_run_times(&pid).expect("this process's threads");
        assert!(first.values().all(|(name, _)| !name.is_empty()));
        // 100 ms of CPU on this thread shows as a share of its name.
        let began = Instant::now();
        while began.elapsed() < Duration::from_millis(100) {
            std::hint::spin_loop();
        }
        let last = thread_run_times(&pid).expect("again");
        let shares = thread_shares(&first, &last, began.elapsed().as_secs_f64());
        let total: f64 = shares.iter().map(|(_, s)| s).sum();
        assert!(
            total >= 80.0,
            "a busy thread is about 100 % of a core: {shares:?}"
        );
        assert!(thread_run_times("999999999").is_err(), "no such process");
    }

    #[test]
    fn cpu_time_and_rss_come_from_proc() {
        let stat = "4242 (opendeep coold) S 1 4242 4242 0 -1 4194560 300 0 0 0 1234 567 0 0 20 0 \
                    6 0 999 0 0";
        assert_eq!(cpu_ticks(stat), Some(1234 + 567));
        assert_eq!(rss_kb("Name:\tx\nVmRSS:\t    2144 kB\n"), Some(2144));
        assert_eq!(cpu_ticks("4242 (x"), None);
    }
}
