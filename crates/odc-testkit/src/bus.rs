//! A private D-Bus daemon for the tests (Phase 7 plan, Task 7; owner, 2026-10-29), and the two
//! waits they need without an async library: `dbus-daemon` with its own socket in a test
//! directory and a minimal policy that lets every connection own any name and call any other,
//! with no eavesdropping, as on the system bus. Started without root; stopped when dropped. The
//! same daemon, with the AT-SPI registry on it, is the settings app's accessibility bus in its
//! end-to-end run (Phase 10 plan, Task 8).

use std::fs;
use std::future::{Future, poll_fn};
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::pin::pin;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

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

/// The AT-SPI registry's installed service file, where at-spi2-core's own bus configuration
/// (`/usr/share/defaults/at-spi2/accessibility.conf`) finds it, and its name on the bus.
pub const REGISTRY_SERVICE: &str =
    "/usr/share/dbus-1/accessibility-services/org.a11y.atspi.Registry.service";
const REGISTRY: &str = "org.a11y.atspi.Registry";

/// How long the registry may take to own its name.
const REGISTRY_WAIT: Duration = Duration::from_secs(10);

pub struct PrivateBus {
    child: Child,
    address: String,
    /// The AT-SPI registry, on an accessibility bus.
    registry: Option<Child>,
}

impl PrivateBus {
    /// Starts a daemon whose socket and configuration live in `dir`; returns once it listens.
    ///
    /// # Errors
    ///
    /// When `dbus-daemon` can't start, or prints no address.
    pub fn start(dir: &Path) -> io::Result<PrivateBus> {
        PrivateBus::start_with(dir, Command::new("dbus-daemon"))
    }

    /// An accessibility bus for the settings app's end-to-end run (Phase 10 plan, Task 8), which
    /// GTK reaches through `AT_SPI_BUS_ADDRESS`: the same daemon, with the AT-SPI registry
    /// started on it, the program the installed service file names; returns once the registry
    /// owns its name. Both run through util-linux's `setpriv --pdeathsig KILL`, so they end with
    /// the thread that started them, as the headless weston does; with an environment of their
    /// own (`PATH`, `runtime` as `XDG_RUNTIME_DIR`, GLib's in-memory settings, no session bus),
    /// so nothing reaches the user's session. The registry's log goes to `dir/registry.log`.
    ///
    /// Not at-spi2-core's own launcher: it starts the bus through dbus-broker where that's
    /// installed, which starts the registry only through systemd ("unit failed" in a private
    /// session). Not the registry started on demand either: it outlives its bus (both seen on
    /// the development PC, 2026-11-07). And not its service file's `--use-gnome-session`: there's
    /// no session manager to register with.
    ///
    /// # Errors
    ///
    /// When the registry's service file is missing or names no program (at-spi2-core isn't
    /// installed), when the registry doesn't own its name within 10 s, or as
    /// [`PrivateBus::start`].
    pub fn accessibility(dir: &Path, runtime: &Path) -> io::Result<PrivateBus> {
        let program = registry_program()?;
        let own = |command: &mut Command| {
            command
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                .env("XDG_RUNTIME_DIR", runtime)
                .env("GSETTINGS_BACKEND", "memory");
        };
        let mut daemon = Command::new("setpriv");
        daemon.args(["--pdeathsig", "KILL", "--", "dbus-daemon"]);
        own(&mut daemon);
        let mut bus = PrivateBus::start_with(dir, daemon)?;
        let mut registry = Command::new("setpriv");
        registry.args(["--pdeathsig", "KILL", "--"]).arg(&program);
        own(&mut registry);
        bus.registry = Some(
            registry
                .env("AT_SPI_BUS_ADDRESS", &bus.address)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(fs::File::create(dir.join("registry.log"))?)
                .spawn()?,
        );
        let started = Instant::now();
        while !owned(&bus.address, REGISTRY) {
            if started.elapsed() > REGISTRY_WAIT {
                return Err(io::Error::other(format!(
                    "the AT-SPI registry didn't own {REGISTRY} within {REGISTRY_WAIT:?}; its log: {}",
                    dir.join("registry.log").display()
                )));
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(bus)
    }

    fn start_with(dir: &Path, mut daemon: Command) -> io::Result<PrivateBus> {
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
        let mut child = daemon
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
        Ok(PrivateBus {
            child,
            address,
            registry: None,
        })
    }

    /// The daemon's address, for a connection to it.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        for child in self.registry.iter_mut().chain([&mut self.child]) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The program the registry's installed service file starts: its `Exec=` line's first word.
fn registry_program() -> io::Result<String> {
    let text = fs::read_to_string(REGISTRY_SERVICE).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("{REGISTRY_SERVICE}: {e}; install at-spi2-core"),
        )
    })?;
    text.lines()
        .find_map(|line| line.strip_prefix("Exec="))
        .and_then(|exec| exec.split_whitespace().next())
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other(format!("{REGISTRY_SERVICE} names no program")))
}

/// Whether `name` has an owner on the bus at `address`.
fn owned(address: &str, name: &str) -> bool {
    zbus::block_on(async {
        let conn = zbus::connection::Builder::address(address).build().await?;
        let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
        let name = zbus::names::BusName::try_from(name)?;
        dbus.name_has_owner(name).await
    })
    .unwrap_or(false)
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
