//! A test session on the dev service (Phase 7 plan, Task 9; `docs/spec/dbus.md`, Dev interface).
//! The direct checks hold one and release the cooler, so they share it with the running dev
//! service (owner, 2026-10-29). Dropping a session ends it, and a killed run ends it with its
//! connection.

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use odc_ipc::{DevProxy, OpenDeepCoolProxy};
use odc_protocol::{Message, decode};
use odc_testkit::capture::parse_recorded;
use odc_testkit::hardware::{Capability, DEV_RECORDING, Mode, Probe, Roots, Summary, probe};
use zbus::Connection;

/// How long the service may take to write again after a session that released the cooler.
const RETURN_WITHIN: Duration = Duration::from_secs(3);

pub struct Session {
    dev: DevProxy<'static>,
    main: OpenDeepCoolProxy<'static>,
    ended: bool,
}

fn said(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Session {
    /// `BeginTest()` on the system bus, from this process's own connection.
    ///
    /// # Errors
    ///
    /// When the bus can't be reached or the service refuses the session.
    pub fn begin() -> Result<Session, String> {
        zbus::block_on(async {
            let connection = Connection::system().await.map_err(said)?;
            let dev = DevProxy::new(&connection).await.map_err(said)?;
            let main = OpenDeepCoolProxy::new(&connection).await.map_err(said)?;
            dev.begin_test()
                .await
                .map_err(|e| format!("BeginTest: {e}"))?;
            Ok(Session {
                dev,
                main,
                ended: false,
            })
        })
    }

    /// `ReleaseCooler()`: the service closes the cooler and writes nothing until the session
    /// ends.
    pub fn release_cooler(&self) -> Result<(), String> {
        zbus::block_on(self.dev.release_cooler()).map_err(|e| format!("ReleaseCooler: {e}"))
    }

    pub fn dev(&self) -> &DevProxy<'static> {
        &self.dev
    }

    pub fn main(&self) -> &OpenDeepCoolProxy<'static> {
        &self.main
    }

    /// `EndTest()`, its error said.
    pub fn end(mut self) -> Result<(), String> {
        self.ended = true;
        zbus::block_on(self.dev.end_test()).map_err(|e| format!("EndTest: {e}"))
    }

    /// `EndTest()` for a session that released the cooler; then the recording must show the
    /// service take it back: its new `03 08`, then a frame (owner, 2026-11-04).
    pub fn end_seeing_return(self) -> Result<String, String> {
        let before = recorded()?.len();
        self.end()?;
        let since = Instant::now();
        loop {
            let messages = recorded()?;
            let back = messages
                .iter()
                .skip(before)
                .position(|m| *m == Message::Startup)
                .map(|i| before + i);
            match back.map(|at| messages.get(at + 1)) {
                Some(Some(Message::Frame(_))) => {
                    return Ok(format!(
                        "the service took the cooler back after {} ms: 03 08, then its frame",
                        since.elapsed().as_millis()
                    ));
                }
                Some(Some(Message::Startup)) => {
                    return Err(format!(
                        "{DEV_RECORDING}: two 03 08 in a row after the session"
                    ));
                }
                _ if since.elapsed() > RETURN_WITHIN => {
                    return Err(format!(
                        "{DEV_RECORDING}: the service didn't take the cooler back within {} s",
                        RETURN_WITHIN.as_secs()
                    ));
                }
                _ => thread::sleep(Duration::from_millis(100)),
            }
        }
    }
}

/// The dev recording's reports so far. The service may be writing its last line: one without its
/// newline is left out.
fn recorded() -> Result<Vec<Message>, String> {
    let text = fs::read_to_string(DEV_RECORDING).map_err(|e| format!("{DEV_RECORDING}: {e}"))?;
    text.split_inclusive('\n')
        .filter_map(|line| line.strip_suffix('\n'))
        .enumerate()
        .map(|(i, line)| {
            let report = parse_recorded(i + 1, line)
                .map_err(|why| format!("{DEV_RECORDING}:{}: {why}", i + 1))?;
            let bytes: [u8; 64] = report
                .bytes
                .as_slice()
                .try_into()
                .map_err(|_| format!("{DEV_RECORDING}:{}: not 64 bytes", i + 1))?;
            decode(&bytes).map_err(|e| format!("{DEV_RECORDING}:{}: {e}", i + 1))
        })
        .collect()
}

/// A direct check's result, with its session ended: while the dev service ran, the check passes
/// only if the service then took the cooler back.
pub fn handed_back(
    session: Option<Session>,
    result: Result<String, String>,
) -> Result<String, String> {
    let Some(session) = session else {
        return result;
    };
    let back = session.end_seeing_return();
    let note = result?;
    Ok(format!("{note}; {}", back?))
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.ended
            && let Err(e) = zbus::block_on(self.dev.end_test())
        {
            eprintln!("odc-e2e: EndTest: {e}");
        }
    }
}

/// Before a direct check writes to the real cooler: while the dev service runs, a session that
/// released the cooler; `None` without a dev service. A dev service that's there but not usable
/// fails the check with its fix, since it would write between the check's frames.
pub fn release_for_direct(roots: &Roots, summary: &mut Summary) -> Result<Option<Session>, String> {
    match probe(Capability::DevService, roots) {
        Probe::Real => {
            summary.mode(Capability::DevService, Mode::Real);
            let session = Session::begin()?;
            session.release_cooler()?;
            Ok(Some(session))
        }
        Probe::Absent { .. } => Ok(None),
        Probe::Unusable { reason, fix } => Err(format!(
            "the display service may write to the cooler: {reason}; fix: {fix}"
        )),
    }
}
