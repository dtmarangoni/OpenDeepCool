//! A headless weston for the settings app's tests (Phase 10 plan, Task 3; solution review
//! P4-05, P10-04): `weston --backend=headless` on a socket of its own, which a GTK client
//! reaches through `WAYLAND_DISPLAY`. Nothing reaches the user's own session: the socket lives
//! in a private runtime folder the test gives, and libwayland takes its full path in
//! `WAYLAND_DISPLAY` (since Wayland 1.15: wayland.freedesktop.org, Client API,
//! `wl_display_connect`).
//!
//! Weston is started through util-linux's `setpriv --pdeathsig KILL`, so it dies with the
//! thread that started it, even when the test is killed; a killed test's socket is removed by
//! the next start in the same folder. Stopped when dropped.

use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// How long weston may take to listen.
const START_WAIT: Duration = Duration::from_secs(10);

/// The prefix of every socket this module names.
const PREFIX: &str = "odc-test-";

/// The output's size: room for the window's default 1040 × 720 (`docs/design/settings-app.md`,
/// Window).
pub const SIZE: (u32, u32) = (1280, 800);

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct Weston {
    child: Child,
    socket: PathBuf,
}

impl Weston {
    /// Starts weston with its runtime folder `dir` (created 0700), on the socket
    /// `odc-test-<pid>-<n>`; returns once the socket takes connections. A socket left in `dir`
    /// by a killed test, which takes none, is removed first.
    ///
    /// # Errors
    ///
    /// When `weston` or `setpriv` can't start, or weston doesn't listen within 10 s; the error
    /// carries weston's log.
    pub fn headless(dir: &Path) -> io::Result<Weston> {
        Weston::headless_at(dir, SIZE)
    }

    /// The same, with an output of `size` px.
    ///
    /// # Errors
    ///
    /// As [`Weston::headless`].
    pub fn headless_at(dir: &Path, size: (u32, u32)) -> io::Result<Weston> {
        DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        remove_dead_sockets(dir)?;
        let name = format!(
            "{PREFIX}{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let socket = dir.join(&name);
        let log = dir.join(format!("{name}.log"));
        let mut child = Command::new("setpriv")
            .args(["--pdeathsig", "KILL", "--", "weston"])
            .args([
                "--backend=headless",
                "--renderer=pixman",
                "--shell=kiosk",
                "--no-config",
                &format!("--width={}", size.0),
                &format!("--height={}", size.1),
                &format!("--socket={name}"),
                &format!("--log={}", log.display()),
            ])
            .env("XDG_RUNTIME_DIR", dir)
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("DISPLAY")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let started = Instant::now();
        loop {
            if UnixStream::connect(&socket).is_ok() {
                return Ok(Weston { child, socket });
            }
            let failed = match child.try_wait()? {
                Some(status) => Some(format!("weston exited ({status})")),
                None if started.elapsed() > START_WAIT => {
                    Some(format!("weston didn't listen within {START_WAIT:?}"))
                }
                None => None,
            };
            if let Some(failed) = failed {
                let _ = child.kill();
                let _ = child.wait();
                let log = fs::read_to_string(&log).unwrap_or_default();
                remove(&socket);
                return Err(io::Error::other(format!("{failed}; its log:\n{log}")));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// The socket's full path, for a client's `WAYLAND_DISPLAY`.
    pub fn display(&self) -> &Path {
        &self.socket
    }

    /// Weston's process ID.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for Weston {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        remove(&self.socket);
    }
}

/// Removes a socket and the lock file libwayland keeps beside it.
fn remove(socket: &Path) {
    let _ = fs::remove_file(socket);
    let mut lock = socket.as_os_str().to_owned();
    lock.push(".lock");
    let _ = fs::remove_file(lock);
}

/// Every `odc-test-*` socket in `dir` that takes no connection: a killed test's.
fn remove_dead_sockets(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with(PREFIX)
            && !name.ends_with(".lock")
            && !name.ends_with(".log")
            && UnixStream::connect(&path).is_err()
        {
            remove(&path);
            let _ = fs::remove_file(dir.join(format!("{name}.log")));
        }
    }
    Ok(())
}
