//! A private D-Bus daemon for the tests (Phase 7 plan, Task 7; owner, 2026-10-29), and the two
//! waits they need without an async library: `dbus-daemon` with its own socket in a test
//! directory and a minimal policy that lets every connection own any name and call any other,
//! with no eavesdropping, as on the system bus. Started without root; stopped when dropped.

use std::fs;
use std::future::{Future, poll_fn};
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::pin::pin;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::thread;
use std::time::Duration;

/// The daemon's configuration (dbus-daemon(1), CONFIGURATION FILE): no service directories, so
/// nothing is started on demand; receiving allowed as the system bus allows it
/// (`/usr/share/dbus-1/system.conf`), with no `eavesdrop` rule.
const CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=SOCKET</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*"/>
    <allow own="*"/>
    <allow receive_type="method_call"/>
    <allow receive_type="method_return"/>
    <allow receive_type="error"/>
    <allow receive_type="signal"/>
  </policy>
</busconfig>
"#;

pub struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    /// Starts a daemon whose socket and configuration live in `dir`; returns once it listens.
    ///
    /// # Errors
    ///
    /// When `dbus-daemon` can't start, or prints no address.
    pub fn start(dir: &Path) -> io::Result<PrivateBus> {
        fs::create_dir_all(dir)?;
        let socket = dir.join("bus");
        match fs::remove_file(&socket) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        let config = dir.join("bus.conf");
        fs::write(
            &config,
            CONFIG.replace("SOCKET", &socket.display().to_string()),
        )?;
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--nopidfile", "--nosyslog", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()?;
        let mut address = String::new();
        let stdout = child.stdout.take().expect("a piped stdout");
        BufReader::new(stdout).read_line(&mut address)?;
        let address = address.trim().to_owned();
        if address.is_empty() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("dbus-daemon printed no address"));
        }
        Ok(PrivateBus { child, address })
    }

    /// The daemon's address, for a connection to it.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Waits `duration`: a thread of its own wakes the task.
pub fn sleep(duration: Duration) -> impl Future<Output = ()> {
    let state = Arc::new(Mutex::new((false, None::<Waker>)));
    let timer = Arc::clone(&state);
    thread::spawn(move || {
        thread::sleep(duration);
        let mut state = timer.lock().expect("the timer");
        state.0 = true;
        if let Some(waker) = state.1.take() {
            waker.wake();
        }
    });
    poll_fn(move |cx| {
        let mut state = state.lock().expect("the timer");
        if state.0 {
            return Poll::Ready(());
        }
        state.1 = Some(cx.waker().clone());
        Poll::Pending
    })
}

/// `future`'s output, or `None` when `duration` passes first.
pub async fn within<F: Future>(duration: Duration, future: F) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut timer = pin!(sleep(duration));
    poll_fn(|cx| {
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            return Poll::Ready(Some(output));
        }
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await
}
