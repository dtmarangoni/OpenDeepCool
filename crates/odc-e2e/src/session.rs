//! A test session on the dev service (Phase 7 plan, Task 9; `docs/spec/dbus.md`, Dev interface).
//! The direct checks hold one and release the cooler, so they share it with the running dev
//! service (owner, 2026-10-29). Dropping a session ends it, and a killed run ends it with its
//! connection.

use odc_ipc::{DevProxy, OpenDeepCoolProxy};
use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};
use zbus::Connection;

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
