# Phase 7: the MVP — a sandboxed system service shows Load Mode on the LP360 from boot, with settings over D-Bus

**Plan ID:** `phase-7-service-settings-load-mode`
**Source PRD:** `docs/prp/prds/opendeepcool-lp360.prd.md` (repository-relative: an absolute path would name the owner's home folder, AGENTS.md rule 10)
**PRD Phase:** `7 — Service, settings & Load Mode (MVP)`
**Source Issue:** `None`
**Plan Publication:** `None`

## Outcome

**Problem:** Phases 5 and 6 give a byte-exact link to the cooler and correct readings, but nothing turns readings into pictures, runs at boot, remembers a choice, or lets anyone change it. The owner's display is still blank after every boot.

**Affected user:**
- The owner, and every user of the PC: the display is shared.
- The agents building Phases 8–10, which extend this service, its settings and its D-Bus interface.

**User outcome:**
- After every boot, before anyone logs in, the LP360 shows the saved Load Mode layout (modes 1–6) with live values, in the saved rotation, drawn as the spec defines: DeepCreative's glyphs and layouts, every value in full (no 99 cap), one-digit values centred, and `100%` scrolling smoothly until it fits again.
- The user at the PC can change mode, rotation, unit, GPU and fan inputs over D-Bus: today with `busctl`, from Phase 10 with the app. Only the active local session uses the service at all: other sessions can neither read nor change anything (owner, 2026-10-26).
- After the PC wakes from standby, the cooler shows the saved mode again with no action (owner, 2026-10-26).

**Invariant:**
- Every frame equals the spec's picture for the current mode, readings, rotation and tick (`docs/spec/frames/` is the oracle).
- A value that can't be read, or a reading no sensor can give, is drawn as dashes, never as a number (rule 11, `display.md` Unavailable values).
- A value is drawn in full up to 9999, and capped at 9999 above it, everywhere (owner, 2026-10-25). A picture with a value of 100 or more scrolls, and starts, keeps moving and stops exactly as `display.md` "Movement, precisely" says.
- One frame per 100 ms tick, without drift.
- A mode switch shows on the next frame.
- The service runs as the locked `opendeepcool` account, sandboxed, and answers only callers polkit authorizes: the active local session, for reads and changes alike. Nothing it sends is broadcast.

**Success signal:** The service, enabled for boot and running with nobody logged in, shows the configured mode with no action (PRD Phase 7). The start check proves it unattended from the journal and the recorded frames, without rebooting (owner, 2026-10-25: no boot testing).

**Approach:** four new crates on top of Phases 5 and 6:
- **`odc-render`:** pure: glyphs, value rules, the static layout and the scrolling strip, the movement procedure, the mode table, and the Load Mode pictures.
- **`odc-ipc`:** the D-Bus names, types and client proxy, shared by the service, the tests and Phase 10's app.
- **`odc-service`:** a library plus the `opendeepcoold` binary.
  - One thread per job (owner, 2026-10-26; solution review OV-03):
    - an engine thread runs the 100 ms tick and never waits for I/O;
    - a device thread writes the frames to the cooler through Phase 5's `DisplayPort`;
    - a sensor thread takes Phase 6's `Readings` snapshots;
    - zbus's own thread serves D-Bus and asks polkit before every call, reads included (owner, 2026-10-26).
  - After a wake-up from standby, logind's signal makes the device thread start a new conversation with the cooler and the sensor thread start its intervals over.
  - systemd supervises it: a panic ends the process, a watchdog restarts it when a thread stops making progress, and restarts back off but never give up.
  - Settings are a versioned TOML file in the state directory.
- **Packaging files:** the unit, the D-Bus policy, the polkit action, and the service account.

`scripts/dev/setup.sh` installs them as the dev copy, recorded in the dev manifest.

## Recommendation

**One thread per job; the tick never waits for I/O** (owner, 2026-10-26; solution review OV-03).
- **The engine thread** owns the drawing state (the mode, the `Marquee`) and the tick clock, and performs no I/O. Each tick it adopts pending changes, adopts the newest readings snapshot if a new one arrived, draws the frame, and posts it to the device thread.
- **The device thread** owns the `Lp360`. It takes the newest frame from a one-slot mailbox and writes it.
  - A write can block for up to 5 s on a stalled USB device (Phase 5: the kernel's `USB_CTRL_SET_TIMEOUT`).
  - A frame posted meanwhile replaces the unsent one instead of queueing, so the tick keeps its grid and nothing bunches up.
- **The sensor thread** owns the `Registry`. It takes a snapshot every second on its own monotonic timer, and at once when the engine asks (`Refresh`), and publishes the newest one. A slow driver delays only this thread.
- **The D-Bus side** (zbus's thread) owns `SettingsStore` (validate, persist) and puts the newest complete settings in a one-slot `Apply` mailbox, or raises its refresh flag (owner, 2026-10-27; solution review P7-15). The engine picks them up at the next tick, which is exactly the "switch on the next frame" rule (`display.md` Mode switches).
- Status flows back through one `Arc<RwLock<Status>>`: connected, availability, bindings.
- Nothing is shared mutably in both directions, so neither a slow polkit check, a stalled USB write nor a slow sensor can delay a frame.

**Supervision: a crash or a hang ends in a restart** (owner, 2026-10-26; solution review OV-03).
- **Panics.** `opendeepcoold` installs a panic hook that writes the message to the journal and aborts, and the release profile sets `panic = "abort"`. A panic in any thread ends the whole service, so systemd restarts it, instead of leaving a live process with a dead tick.
- **Hangs.** The device and sensor threads each record their last progress on the monotonic clock: a finished write attempt or rescan, a finished snapshot.
  - Every second the engine sends systemd `WATCHDOG=1`, but only if both threads progressed within the last 10 s.
  - A write attempt is bounded by the kernel's 5 s, so a failing but healthy device still progresses.
  - The unit has `WatchdogSec=15`.
  - The notification is one datagram to `$NOTIFY_SOCKET` (the `sd_notify` protocol), with no extra crate.
- **Readiness.** `Type=notify`: `READY=1` once the bus name is owned and the threads run.
- **Restarts.** `Restart=on-failure`, `RestartSec=1`, `RestartSteps=5` and `RestartMaxDelaySec=30` (systemd 254 and later), plus `StartLimitIntervalSec=0`. With systemd's default limit, 5 starts in 10 s, it would stop restarting for good.

**Pure drawing, in two parts** (`display.md` Movement, precisely).
- `picture(mode, &Values, unit) -> Result<Picture>`: the strip of the mode's values (both bands aligned), its width, cycle length *L*, home column *h*, and whether it must scroll (a value of 100 or more).
- `Marquee`: the movement procedure as a pure state machine: `reset(Picture)`, `update(Picture)`, `tick() -> Frame`. It holds *x* and whether the strip moves. No clock and no I/O, so every test scenario runs as a unit test.
- Rotation is applied last with Phase 5's `Frame::rotated` (`display.md` Rotation).
- The PRD's layer 1 contract was updated to match on 2026-10-24: the picture is a pure function of mode, readings and unit, and the movement a pure function of the changes and ticks. The old single `render(mode, values, unit, rotation, tick)` can't express a strip that keeps its position when a reading changes.

**The 100 ms tick on absolute deadlines.**
- The next frame is due at `start + n × 100 ms` on the monotonic clock. A late frame doesn't shift the ones after it.
- This matches DeepCreative's long-run rate of 100.0 ms (`protocol.md` Timing, corrected 2026-10-24).
- Readings refresh once per second (`sensors.md`): the sensor thread delivers a new snapshot every second, and the engine adopts the newest at its next tick.

**D-Bus with zbus's blocking API.**
- The owner chose D-Bus on 2026-10-24.
- zbus 6's blocking `connection::Builder::system().name(…).serve_at(…).build()` runs the object server on zbus's own executor thread, so the service needs no async runtime of its own (zbus book, "blocking").
- polkit's `CheckAuthorization` takes a `system-bus-name` subject, the caller's unique name from the message header (`#[zbus(header)]`, `Header::sender()`). This is polkit's identification for D-Bus callers (polkit Authority interface docs).
- **Arguments are checked against the service's own state** (owner, 2026-10-26; solution review OV-14). A GPU or a fan input is accepted only when it's on the lists the service found itself. Strings have length limits, and no file path is ever built from a caller's string (Task 7).

**Only the user at the PC, and nothing broadcast** (owner, 2026-10-26: no remote or inactive user uses the service, only the current logged-in user).
- **Every call is checked.** Reads need a new polkit action, `…OpenDeepCool1.use`; changes need `change-settings`. Both are `allow_active` yes, `allow_inactive` no, `allow_any` no. polkit counts an inactive local session under `allow_inactive` and any other client, a remote one included, under `allow_any` (polkit(8)), so only the active local session passes. A process in a systemd user unit gets its user's active session (checked on this PC, Phase 9 plan).
- **No D-Bus properties.** Anyone may `Get` a property through the standard `Properties` interface, and `PropertiesChanged` is a broadcast that any local process can receive, because the system bus lets everyone receive signals (`/usr/share/dbus-1/system.conf`). Instead:
  - `GetState() -> a{sv}` returns the whole state;
  - `Watch()` registers the caller. While it watches, each change arrives as `StateChanged(a{sv})` with the changed keys, a signal addressed to it alone (zbus 6.1: `SignalEmitter::set_destination`). The bus delivers such a signal to its destination only, whatever other connections' match rules say (D-Bus specification, Message Bus Message Routing). dbus-broker, Arch's default bus, implements no eavesdropping, and only root may monitor the system bus;
  - at most 8 watchers per Unix user and 32 in all. A watcher is dropped on `Unwatch()`, or when its name leaves the bus.
- **Re-checked every second**, on the D-Bus side's own timer: a watcher whose session is no longer the active local one gets nothing until it passes again, then the whole state in one `StateChanged`. Phase 9 adds the audio caller to the same check, and Phase 10 the frame watchers.
- `busctl` still works for the owner until Phase 10: `busctl call … GetState`.

**Back from standby** (owner, 2026-10-26: when the PC comes back, OpenDeepCool must be back; solution reviews P3-04, P5-05, OV-19).
- The D-Bus side watches logind's `PrepareForSleep`, sent with `false` right after suspend or hibernation (org.freedesktop.login1(5)). The match rule names `org.freedesktop.login1` as the sender, so no other process can fake a wake-up.
- On `false` it sends `Wake` to the device thread, which calls `Lp360::reset_connection()` (Phase 5): the next frame follows a new `03 08`, even when the device node survived. It also sends `Wake` to the sensor thread, which calls `Readings::restart_intervals()` (Phase 6), so no average spans the sleep. `true` changes nothing.
- The tick and the strip just continue. Neither the service's own progress check nor systemd's watchdog counts the sleep: both run on `CLOCK_MONOTONIC` (clock_gettime(2); systemd arms the service watchdog on it, `src/core/service.c`, read 2026-10-26).
- The monitor turning off or the screen locking isn't standby, and changes nothing (`protocol.md` Standby).

**Settings in TOML, written only by the service.**
- Readable when someone inspects the state directory.
- `version = 1` for later phases' fields, which are added with defaults.
- Atomic writes: a temp file plus rename.
- An unreadable file is renamed aside, logged, and replaced by defaults (`settings.md` OpenDeepCool's defaults).

**Names.**
- Bus name, interface and polkit action: `io.github.dtmarangoni.OpenDeepCool1`. The `1` is the interface's major version, as the D-Bus specification recommends (owner, 2026-10-24): an incompatible change would add `…OpenDeepCool2` alongside it, so older clients keep working.
- The D-Bus spec says names start with the author's reversed domain. The project's public address is its GitHub repository, and that account name is already public in `LICENSE` and the PRD.
- Object path: `/io/github/dtmarangoni/OpenDeepCool1`.
- Binary: `opendeepcoold`. Unit: `opendeepcool.service`.

**Testing the real service unattended, with known readings.** On the owner's PC the dev copy runs with a drop-in (dev only, never packaged) that starts the service with the `--dev` switch. `--dev` does two things, and the packaged service does neither:
- it registers a second interface, `…OpenDeepCool1.Dev`, at its own object path, `/io/github/dtmarangoni/OpenDeepCool1/Dev`. It overrides readings, so the end-to-end test can drive every matrix case through the real, sandboxed, boot-started service. Access is checked three ways (owner, 2026-10-26; solution review OV-13):
  - the D-Bus policy allows the dev path only to group `opendeepcool`. Every rule pairs `send_interface` or `send_path` with `send_destination`, because the interface field of a message is optional (dbus-daemon(1));
  - each dev method checks the caller itself: `GetConnectionCredentials` must report `UnixGroupIDs` containing the service's group, and a missing `UnixGroupIDs` means no (D-Bus specification);
  - polkit `change-settings`, as for every change.

  The interface stays in the shipped binary, off unless the service starts with `--dev`, so the final parity run (Phase 11) tests the binary that ships (owner, 2026-10-26).
- it records reports to `/var/lib/opendeepcool/dev-frames.log`, using Phase 5's `Recording` (owner, 2026-10-27; solution review P7-07):
  - the service's first 100 reports after each start, always, for the start check;
  - every report while a test session is held (Task 7), and nothing otherwise;
  - each with its tick number and its monotonic write time (`CLOCK_MONOTONIC`, ns; P7-13);
  - buffered, flushed once a second and at the end of each session, and rotated at 64 MB, keeping one previous file.

### Evidence

- `docs/spec/display.md`: Modes (modes 1–6 and their metrics, top first), Values (DeepCreative rounds usage up and Memory Load to the nearest; OpenDeepCool rounds every measurement to the nearest, with no cap below 9999), Glyphs, Bands, Static layout. Line numbers changed with the 2026-10-24 decisions, so sections are named instead.
- `docs/spec/display.md`: Updates; Unavailable values (dashes, added 2026-10-24); Values, OpenDeepCool: no caps (automatic scrolling, one smooth movement); Static layout, OpenDeepCool: alignment; Movement, precisely; Mode switches (a mode switch jumps to home); Test scenarios S1–S26; Rotation. All owner decisions of 2026-10-24.
- `docs/spec/frames/` regenerated 2026-10-24 with OpenDeepCool's rules (172 of 292 cases changed), and again on 2026-10-26 with the nearest rule (24 °F cases of 284 changed); `odc_rules.py` draws them, in `tools/oracle/` from Phase 5 Task 0.
- `docs/spec/protocol.md` Timing: one frame per 100 ms tick, corrected 2026-10-24.
- `docs/spec/settings.md`:
  - Settings: defaults are mode 1, rotation 0, Celsius, no fan inputs;
  - Start-up: `03 08`, then the saved picture at once;
  - Where the settings live: one file in the state directory;
  - What the settings app must offer: availability, GPU list, status, D-Bus/polkit.
- `docs/spec/frames/mode-001.txt` … `mode-006.txt`: 12 cases each (4 rotations × 3 value sets), 72 in all.
- `research/logs/sweep-*.json` (local): each step's `mode`, `values`, `unit`, `rot`, and steady window `start`/`end` in ms, lined up with `research/logs/lp360-hidraw.log`. `research/hook/render-check.py` (local) already compares them by the spec's rules.
- PRD:
  - Service model: system service, dedicated unprivileged user, sandboxed, restarts on failure;
  - Settings app ↔ service: D-Bus, polkit `allow_active`;
  - Diagnostics: journal plus a D-Bus method;
  - dev footprint: the dev copy and its manifest.
- zbus 6 (Context7, 2026-10-24): the blocking `connection::Builder`. Builder setters defer errors to `build()` in 6.0. `#[interface]`, `#[zbus(property)]`, `#[zbus(header)] Header` → `sender()`.
- polkit Authority docs (freedesktop.org, read 2026-10-24): `CheckAuthorization(subject, action_id, details, flags, cancellation_id)`. Subject kind `system-bus-name` with key `name`.
- This PC, 2026-10-24:
  - polkit (`man 8 polkit`) reads actions from `/etc/polkit-1/actions/` and `/usr/share/polkit-1/actions/`;
  - `/usr/share/dbus-1/system.conf` includes `/etc/dbus-1/system.d`;
  - `/proc/devices` names the `hidraw` and `drm` groups, for `DeviceAllow=char-hidraw` and `char-drm`;
  - systemd provides `systemd-analyze security`.
- crates.io, 2026-10-24: `zbus` 6.1.0, `serde` 1.0.229, `toml` 1.1.8.
- Access and wake-up sources, read 2026-10-26:
  - polkit(8) (polkit 128, this PC): `allow_any` applies to any client, `allow_inactive` to inactive sessions on local consoles, `allow_active` to active ones; an action id may use `[A-Z][a-z][0-9].-`;
  - D-Bus specification, Message Bus Message Routing: a signal with a `DESTINATION` is delivered to that recipient only, and match rules don't match it unless they eavesdrop;
  - dbus-broker wiki, Deviations: eavesdropping isn't implemented. This PC runs dbus-broker as the system bus, and its `system.conf` lets only root use the monitoring interface;
  - zbus 6.1.0 (docs.rs): `SignalEmitter::set_destination` unicasts a signal "to specific peers";
  - org.freedesktop.login1(5): `PrepareForSleep()` is sent "right before (with the argument 'true') or after (with the argument 'false')" suspend or hibernation (the manual's inner quotes shown as single quotes);
  - systemd `src/core/service.c`: the service watchdog timer runs on `CLOCK_MONOTONIC`.
- systemd.exec(5), read 2026-10-27 (overview cross-check): with `SystemCallFilter=` set and `SystemCallErrorNumber=` "not used, … the process will be terminated immediately when the filter is triggered"; with it, the call returns that error instead.

### Alternatives considered

- **An async runtime (tokio) for the whole service:** one more large dependency. The service has one periodic loop and a handful of rare calls, and zbus's blocking API runs the server on its own thread.
- **The engine and D-Bus sharing one `Mutex<State>`:** a polkit round trip or a slow client could hold the lock across a tick. The channel keeps the tick independent.
- **Testing the real service with real readings only:** the values can't be chosen, so frames can't be compared with the expected frames. The dev-only readings override keeps the comparison exact.
- **JSON settings:** works equally well, but it's less readable in the state directory. TOML is the conventional choice for configuration.

## Visuals

```mermaid
flowchart LR
  subgraph svc["opendeepcoold (new), User=opendeepcool, sandboxed"]
    DB["D-Bus thread (zbus blocking)<br/>io.github.dtmarangoni.OpenDeepCool1"] -->|Apply(Settings)| CH((channel))
    DB --> ST[(SettingsStore<br/>settings.toml v1)]
    DB -->|CheckAuthorization| PK[[polkit]]
    CH --> EN["Engine thread<br/>100 ms absolute ticks, no I/O"]
    EN --> RD["odc-render: picture + Marquee<br/>(pure, new)"]
    EN -->|"newest frame<br/>(one-slot mailbox)"| LP["Device thread<br/>Lp360 DisplayPort (Phase 5)"]
    RG["Sensor thread<br/>Registry Readings (Phase 6), every second"] -->|newest snapshot| EN
    EN --> STAT[(Status: connected,<br/>availability, bindings)]
    STAT --> DB
    EN -.->|"WATCHDOG=1 while<br/>every thread progresses"| SD[[systemd]]
    DEV["…OpenDeepCool1.Dev at its own path<br/>(only with --dev)"] -.->|readings override| EN
  end
  APP["busctl now / settings app (Phase 10)"] -->|"system bus: GetState, Watch, changes<br/>(active local session only)"| DB
  DB -.->|"StateChanged, addressed<br/>to each watcher alone"| APP
  LD[[logind]] -->|"PrepareForSleep(false)"| DB
  DB -.->|Wake| LP
  DB -.->|Wake| RG
  LP --> HW[(LP360)]
```

## Implementation Context

### Mandatory reading

| File | Why it matters |
|---|---|
| `docs/spec/display.md` (all) | Everything Load Mode draws: values with no cap, alignment, the strip, the movement procedure, switches, dashes, the test scenarios |
| `docs/spec/settings.md` (all) | Settings, defaults, start-up, availability, status, D-Bus/polkit |
| `docs/spec/protocol.md` Timing | The 100 ms tick |
| `docs/spec/frames/README.md`, `mode-001.txt`–`mode-006.txt` | The oracle and its format |
| `docs/prp/plans/phase-5-device-protocol-core.plan.md` | `DisplayPort`, `Lp360`, `Recording`, `odc-testkit` (expected, diff, capture, hardware probe), dev scripts and manifest |
| `docs/prp/plans/phase-6-sensors.plan.md` | `Readings`, `Registry`, `Snapshot`, GPU choice and `rank_is_stale`, fan inputs, `FakeReadings`, the GPU-rank and detection units ordered before this service |
| `docs/prp/prds/opendeepcool-lp360.prd.md` Service model, Settings app ↔ service, System Footprint | What the unit, sandbox and footprint must be |
| `research/hook/render-check.py`, `analyze-sweep.py` (local) | How the sweep steps line up with the captures |

### Existing patterns and primitives

- **Ports:** Phase 5 `DisplayPort` (`show(&Frame) -> Link`) and Phase 6 `Readings` (`snapshot()`), with their fakes. The engine depends only on these two traits and a `Clock`.
- **Oracle and diff:** `odc_testkit::{expected, diff}` (Phase 5 Task 3).
- **Hardware probe:** `odc_testkit::hardware` (Phase 5 Task 9, Phase 6 Task 11). This phase adds `DevService`.
- **Dev setup and manifest:** Phase 5 Task 8. Every file this phase installs goes through it.

### Integration points

- `odc_ipc`: the contract Phase 10's app compiles against: names, the state keys and method list, error names, as `docs/spec/dbus.md` describes them.
- `odc_render::{Mode, mode_info, picture, Marquee}`: Phases 8–9 add their modes' pictures; the strip and the movement serve them unchanged. The app's preview (Phase 10) reuses `picture` and `Marquee`.
- `opendeepcool.service`: `Wants=` both Phase 6 one-shots; `After=` only `opendeepcool-gpu-rank.service` and `lm_sensors.service` (ordering only), never the detection, so a slow probe never delays the cooler (owner, 2026-10-27; solution review P6-02, P6-03, P6-04).
- `settings.toml` v1: Phases 8–10 add fields with defaults; nothing else writes it.

## Scope

### In scope

- Load Mode, modes 1–6:
  - pictures and value rules for usage and Memory Load, with no cap;
  - the digit, `%` and dash glyphs;
  - the static layout with its alignment (one value centred, two values ending at column 14);
  - the scrolling strip for values of 100 or more (`100%`), and the movement procedure;
  - rotation;
  - dashes for unavailable and impossible readings.
- The mode table for every spec mode (metrics and kind), used for availability now and by later phases' renderers.
- The engine: 100 ms tick, readings every second, start-up, resets and updates as the spec defines (mode, rotation, unit, GPU, Refresh; nothing for a choice already set), reconnect continuity, availability, status.
- Settings v1, the D-Bus interface checked for every call, polkit, diagnostics, journal logging, and the wake-up from standby.
- The `opendeepcoold` binary, unit, sandbox, D-Bus policy, polkit action, service account, and the dev copy with its frame recording.
- Tests in all four layers, plus a start check and a resource baseline.

### Not building

- **Modes 7–23:** Phases 8–9. `SetMode` rejects them with "not available yet". The mode table already describes them, for availability.
- **Temperature and power pictures** (the `°C`, `°F` and `W` glyphs, modes 7–12): Phase 8. The strip and the movement built here serve them unchanged.
- **Audio input and its D-Bus path:** Phase 9.
- **The settings app and its client UI:** Phase 10. The client proxy ships in `odc-ipc` now.
- **The final package, `/usr` paths, uninstall removal script:** Phase 11.

## Delivery Considerations

| Concern | Decision and owned work |
|---|---|
| Rollout / reversibility | From Task 8 the dev copy drives the owner's cooler at every boot. `scripts/dev/teardown.sh` removes it entirely, from the manifest. Every install or upgrade through `pkexec` is shown to the owner first (rule 7) |
| Observability | The journal: start, bindings at start and on each rebind, cooler connect and disconnect with the reason (one line per change), each wake-up from standby, rejected requests with the caller's reason, and settings file problems. Lines carry `sd-daemon` priority prefixes, so `journalctl -p` filters them |
| Documentation | README: how to change the mode with `busctl` until Phase 10, and the journal command. AGENTS.md Commands: new tests. PRD footprint: the dev copy rows already exist (2026-10-24); add the D-Bus and polkit file names |

## Implementation

Each task ends with its own commit (rule 4) after its tests pass. **Each Load Mode combination** (modes 1–6) is implemented, tested in all layers, and committed on its own (Task 3), as rule 4 requires.

### 1. The oracle gets the dash cases and the movement scenarios

**Files and integration points**
- `tools/oracle/odc_rules.py` and `expected-frames.py` (committed since Phase 5 Task 0; owner, 2026-10-27, solution review P3-01): UPDATE, in the same commit as the frames they regenerate.
  - `odc_rules.py` already draws OpenDeepCool's pictures (it generated the 2026-10-24 frames). It gets the movement procedure of `display.md` "Movement, precisely" as a small state machine, written from the spec text, not from any Rust code.
  - Its value rules already use exact fractions (Python's `fractions.Fraction`), never floats, and round every measurement to the nearest whole number, halves up, as `display.md` Values requires (done 2026-10-26, at the owner's request). They agree with the spec's whole-number formulas on every millidegree from 0 to 6000 °C in both units, and on sampled jiffy, kB, energy and µW inputs.
  - With the nearest rule it regenerated `docs/spec/frames/` on 2026-10-26: exactly the 24 expected °F cases changed, 57 °C → `135°F` and 7 °C → `45°F`, and nothing else (`frames/README.md`).
  - The scenario file writes every reading as an exact decimal, which the Rust loader parses into an exact fraction, never through `f64`.
  - `expected-frames.py` writes the files below.
- `docs/spec/frames/unavailable.txt`: CREATE, generated. Dash cases, each in rotations 0–3:
  - mode 1 with CPU Usage unavailable, and with a negative reading (with exact readings, not-a-number and infinite can't occur, `display.md` Unavailable values);
  - mode 4 with CPU Usage 42 and Gpu usage unavailable, and with CPU Usage 100 and Gpu usage unavailable (scrolling);
  - mode 6 with both unavailable.
- `docs/spec/frames/scenarios/load.txt`: CREATE, generated. Every test scenario of `display.md` (S1–S26) that applies to Load Mode, for each of modes 1–6, in rotation 0:
  - each scenario is a list of ticks; each tick lists what changes (readings, a reset and its cause, or nothing) and the frame expected;
  - for the two-value modes 4–6, each scenario runs with the change in the top value, the bottom value and both;
  - the cycle-position scenarios (S6, S13, S14, S16, S24–S26) are listed for every *k* from 0 to *L* − 1.
  - Rotations 1–3 aren't stored: their expected frames are the rotation-0 frames turned, which the tests do (`display.md` Rotation).
- `docs/spec/frames/README.md`: UPDATE, describing `unavailable.txt` and the scenario file format.
- `docs/spec/parity-checklist.md`: UPDATE, a new row: "Unavailable values: dashes (OpenDeepCool's own rule)", reviewed as an owner decision, 2026-10-24.

**Tests**
- The generator's own checks:
  - the existing `mode-*.txt` cases are reproduced unchanged (as regenerated on 2026-10-26);
  - the owner's scenarios come out as the owner described them: a drop one column after home runs *L* − 1 more frames and stops at home; up, down and up again before home never stops; a value reaching 100 keeps its first digit place; nothing ever moves by more than one column per frame except at a reset.

**Validation**
- `git diff --stat docs/spec/frames/` shows only the new files and the README.

### 2. `odc-render`: glyphs, values, pictures, the mode table and the movement

**Files and integration points**
- `crates/odc-render/`: CREATE, a library: `lints.workspace = true`; it depends on `odc-protocol`.
  - `glyph.rs`: the digits 0–9, `%` and the dash, typed as `#`/`.` rows straight from `display.md`.
  - `value.rs`
  - `picture.rs`
  - `marquee.rs`
  - `mode.rs`
  - `lib.rs`

**Implementation**
- **`Mode`:** the spec's numbers. `mode_info(mode) -> ModeInfo { metrics: &[Metric], kind: Load | Temperature | Power | Music }` for every mode in `display.md` and `music.md`.
  - Phase 7 draws 1–6; `picture` returns `Err(NotYetImplemented)` for the rest. `draws(mode) -> bool` says which, true exactly where `picture` is implemented, and it's the one source for both the D-Bus refusal and `AvailableModes` (owner, 2026-10-27; solution review P8-03).
  - `Metric` is Phase 6's type.
- **Value rules** (`display.md` Values: OpenDeepCool's no caps, and exact whole-number arithmetic), on Phase 6's exact readings (`Exact { num, den }`), in integer arithmetic only, with intermediates in `i128` so that no reading can overflow (owner, 2026-10-26; solution review OV-08):
  - usage and Memory Load: the nearest whole number, halves up, (2 · num + den) div (2 · den), no cap (owner, 2026-10-26: every measurement rounds to the nearest);
  - a number of 10 000 or more → 9999, the cap that holds everywhere in OpenDeepCool (owner, 2026-10-25); the journal and D-Bus show the same number;
  - a negative reading → `Unavailable`, drawn as dashes. Not-a-number and infinite readings can't occur with exact readings.
- **`picture(mode, &Values, unit) -> Result<Picture>`** (`display.md` Static layout and Movement, precisely):
  - the strip: digit places of 4 columns, then the symbol; dashes take two places; bands at rows 6–10 for one value, and rows 2–6 and 9–13 for two, both ending at the same column;
  - *w*, *L* = *w* + max(4, 21 − *w*), the home column *h* (one value centred, an odd spare column on the left; two values ending at column 14; 2 when *w* > 14), and whether it must scroll.
- **`Marquee`:** the procedure of `display.md` "Movement, precisely", step for step: `reset(Picture)`, `update(Picture)`, `tick() -> Frame` (unrotated). The engine rotates the frame last.

**Tests** (layer 1, CI)
- **Glyphs:** each glyph equals the spec's text picture, compared row by row with a copy typed into the test from `display.md`. Typing them independently makes a copy error show up as a mismatch.
- **Value rules,** all given as exact fractions:
  - the nearest rule at its halves, for usage and Memory Load: 0.4 → 0, 0.5 → 1, 9.49 → 9, 9.5 → 10, 99.49 → 99, 99.5 → 100. DeepCreative's usage rule (0.4 → 1) is no longer followed;
  - the no-cap rows (100 → 100, 1000 → 1000, 9999 → 9999) and the cap (10000 → 9999, 123456 → 9999);
  - the boundaries of S20 (0, 0.001, 9 ↔ 10, 99 ↔ 100, 999 ↔ 1000, 9999 ↔ 10000);
  - the negative readings of S19;
  - raw counters: 23 busy jiffies out of 40 → 57.5 → `58`, never `57`; floating point gives 57.49999999999999.

  Property: any reading of 0 or more gives a whole number from 0 to 9999, and any negative reading gives dashes.
- **Picture:** *w*, *L*, *h* and "must scroll" for every value length from dashes to 4 digits, one and two values.
- **Marquee properties** (random pictures and change sequences):
  - between resets, every frame is the previous one shifted one column left, or the same frame;
  - a strip that needs no scrolling stops only at *h*, within *L* frames;
  - an update never moves the copy whose first column is on the screen;
  - after a reset, the first frame is the picture at *h*.
- **Mode table:** `mode_info` lists the metrics `display.md` gives for modes 1–12 and `music.md` gives for 13–22, top first, for all 22 modes.
- **Matrix and scenarios:** Task 3.

**Validation**
- `cargo test -p odc-render`: passes.

### 3. Load Mode, one combination at a time (modes 1, 2, 3, 4, 5, 6)

Six sub-steps, **each its own commit** (rule 4): Separate CPU (1), Separate GPU (2), Separate RAM (3), Combined CPU/GPU (4), CPU/RAM (5), GPU/RAM (6).

**Files and integration points**
- `crates/odc-render/src/load.rs`: UPDATE, per mode.
- `crates/odc-render/tests/matrix_load.rs`: UPDATE, per mode.
- `crates/odc-render/tests/scenarios_load.rs`: UPDATE, per mode.
- `crates/odc-render/tests/differential_load.rs`: UPDATE, per mode (layer 2, local).

**Implementation**
- Each mode maps its metrics (top first) to the picture of Task 2. A CPU option reads only CPU metrics, a GPU option only GPU ones (rule 11).

**Tests per mode**
- **Layer 1, matrix:** all 12 cases of `mode-00N.txt` (4 rotations × 3 value sets) match through `odc_testkit::diff`. A scrolling case (value set 3: `100%`) is compared frame by frame from the reset, over its whole cycle.
  - Each case's `readings` are mapped to a `Values` with **every** metric set to a distinct non-zero value. The metrics the mode doesn't show also get values, so drawing a wrong metric fails (rule 5).
- **Dash cases:** from `unavailable.txt`, where that mode has some.
- **Layer 1, scenarios:** every scenario of `scenarios/load.txt` for this mode, in all four rotations (the expected frames turned), driven through `Marquee` with `picture`: every tick's frame must match.
- **Properties:** for every value, only the band's rows change.
- **Layer 2 (local, `#[ignore]`):**
  - a sweep reader (`odc_testkit::sweep`, added with mode 1) reads `research/logs/sweep-*.json`;
  - for every captured step of this mode **where OpenDeepCool's rules agree with DeepCreative's** (two-digit values from whole readings, and the rows of two-value modes whose values are all such), every frame inside the step's steady window equals the picture at home: 0 differing pixels;
  - steps where they differ on purpose are skipped and listed with the reason: one-digit values centred; values of 100 or more; fractional readings, which DeepCreative rounds up and OpenDeepCool rounds to the nearest (owner, 2026-10-26);
  - the test prints how many steps and frames it compared and skipped, and fails if none were compared.

**Validation**
- `cargo test -p odc-render --test matrix_load --test scenarios_load`: passes.
- `cargo test -p odc-render --test differential_load -- --ignored`: passes.

### 4. `odc-ipc`: the D-Bus contract

**Files and integration points**
- `crates/odc-ipc/`: CREATE, a library: `serde`, `zbus = "6"`.
- `docs/spec/dbus.md`: CREATE (owner, 2026-10-27; solution review P4-04). The contract, in one document:
  - the introspection XML of both interfaces, every method and signal with its argument types;
  - a table of the state keys with their types, which the XML can't show inside an `a{sv}`;
  - the errors, and the polkit action each member needs.
  - Later phases update it in the same commit as their code. From `v1.0.0` it's frozen: an incompatible change gets a new interface, `…OpenDeepCool2`.
- `docs/spec/README.md`: UPDATE, the Documents table lists `dbus.md`.

**Implementation**
- **Names:**
  - bus name and interface `io.github.dtmarangoni.OpenDeepCool1`; object path `/io/github/dtmarangoni/OpenDeepCool1`;
  - dev interface `io.github.dtmarangoni.OpenDeepCool1.Dev`, at object path `/io/github/dtmarangoni/OpenDeepCool1/Dev`;
  - polkit actions `io.github.dtmarangoni.OpenDeepCool1.use` (reads) and `io.github.dtmarangoni.OpenDeepCool1.change-settings` (changes).
- **The state, with no D-Bus properties** (owner, 2026-10-26: nothing readable without a check, nothing broadcast). It's a dictionary, `a{sv}`, read with `GetState()` and sent in `StateChanged`; the proxy parses it into a typed `State`, and a client ignores keys it doesn't know, so later phases add keys:
  - `Mode` (u), `Rotation` (y), `TemperatureUnit` (s: `celsius` or `fahrenheit`);
  - `CoolerConnected` (b), and `CoolerState` (s: `connected`, `not-found`, `no-access` or `write-failed`), from Phase 5's `Link` and its reason (owner, 2026-10-27; solution review P5-02, P7-14);
  - `AvailableModes` (au) and `UnavailableReasons` (a{us}: mode → one-line reason);
  - `Gpus` (a(sss): PCI address, driver, kind, in rank order; kind is `dedicated`, `integrated`, `unknown` or `query-failed`, the last ranked as unknown, owner, 2026-10-27, solution review P6-07) and `ChosenGpu` (s, empty when none was chosen);
  - `FanInputs` (a(ssus): chip, device path, input, label);
  - `CoolerFanInput` and `PumpInput` (ssu each, empty when unset);
  - `CoolerFanRpm` and `PumpRpm` (d, NaN when unavailable, capped at 9999 like every value; owner, 2026-10-25).
- **Methods:**
  - **changes, polkit-checked:** `SetMode(u)`, `RotateScreen()` (one press, `settings.md` T3), `SetRotation(y)`, `SetTemperatureUnit(s)`, `ChooseGpu(s)`, `SetCoolerFanInput(ssu)`, `SetPumpInput(ssu)`, `Refresh()` (fresh readings, picture restarted from home; owner, 2026-10-24);
  - **reading, polkit `use`:** `GetState() -> a{sv}`; `Watch()` and `Unwatch()`; `Diagnostics() -> a(sss)`: metric, bound source, state.
- **Signal:** `StateChanged(a{sv})`: the changed keys, or the whole state after a watcher passes its check again. It's addressed to each authorized watcher alone, never broadcast.
- **Errors:** `…Error.NotAuthorized`, `…Error.ModeUnavailable` (with the reason), `…Error.NotYetAvailable` (modes of later phases), `…Error.InvalidArgument`, `…Error.TooManyWatchers`, `…Error.Busy` (dev copy only, while a test session is held; P7-16).
- A `#[proxy]` client for the tests and Phase 10.

**Tests**
- The names are valid D-Bus names (zbus parsing). `docs/spec/dbus.md` holds the state keys and the member list, and it's what Phase 10's design reads.
- `State` round-trips through `a{sv}`, and a dictionary with an unknown key still parses.

**Validation**
- `cargo test -p odc-ipc`: passes.

### 5. Settings v1: defaults, validation, atomic persistence

**Files and integration points**
- `crates/odc-service/src/settings.rs`: CREATE. `crates/odc-service/` is a library plus binary: `serde`, `toml`.

**Implementation**
- **`Settings`:**
  - `version = 1`, `mode = 1`, `rotation = 0`, `temperature_unit = "celsius"`;
  - `gpu_choice` (optional PCI address; present means "the user chose");
  - `cooler_fan_input`, `pump_input` (optional `{chip, device_path, input}`).
- **Defaults:** `settings.md` OpenDeepCool's defaults.
- **Validation** (owner, 2026-10-26; solution review OV-14). The file is checked for form only, because a saved GPU or input may be missing from the PC and is kept for when it's back (`sensors.md` GPU choice, Fan and pump speed):
  - mode in the table;
  - rotation 0–3;
  - a known unit;
  - a GPU choice in canonical PCI form, `dddd:bb:dd.f` in lower-case hex;
  - a fan input whose chip name has at most 64 bytes, whose device path is absolute, under `/sys/devices/`, without `.` or `..` components and at most 1024 bytes long, and whose input number is 1 or more.
- **`SettingsStore::load(dir)`:**
  - a missing file → defaults, which are written;
  - an unreadable or invalid file → renamed to `settings.toml.invalid-<unix time>`, with an error logged, then defaults. Only the last three such copies are kept (solution review P7-12).
- **Unknown fields** (owner, 2026-10-27; solution review P7-12): a version 1 file may hold fields that a later build added, such as Phase 9's Music settings. They're kept and written back unchanged, so going back to an older build, as the alpha and beta may, and forward again loses nothing. Only a different `version` is rejected.
- **`store(&Settings)`:** temp file, `fsync`, rename, then an `fsync` of the directory, so the rename itself survives a power cut (P7-12).

**Tests** (temp dirs under `CARGO_TARGET_TMPDIR`)
- Defaults on a fresh dir.
- A round trip of every field.
- An invalid file is moved aside, and defaults are used. That includes a malformed GPU address and a path-like device path (`/sys/devices/../../etc`).
- A future `version = 2` file is rejected the same way, never silently downgraded.
- An interrupted write (a temp file left behind) doesn't replace the good file.
- A v1 file with a field this build doesn't know keeps that field, with its value unchanged, through a load and a store (P7-12).
- A fourth invalid file removes the oldest of the three kept copies.

**Validation**
- `cargo test -p odc-service --lib settings`: passes.

### 6. The engine: tick, readings, start-up, resets and updates, availability, dashes

**Files and integration points**
- `crates/odc-service/src/engine.rs`: CREATE.
- `crates/odc-service/src/status.rs`: CREATE.

**Implementation**
- **Two threads around the engine** (Recommendation):
  - the device thread owns the `Lp360` and writes the newest frame from a one-slot mailbox;
  - the sensor thread owns the `Registry`, takes a snapshot every second and on request, and publishes the newest.

  The engine owns neither port.
- `Engine<C: Clock>` runs `loop { wait_until(deadline); tick(); deadline += 100 ms }`, with deadlines on the monotonic clock. It talks to the other threads only through the mailbox, the newest-snapshot slot and the `Apply` channel. A deadline missed by more than one tick is logged and skipped, and frames never bunch up.
- **Each tick:**
  1. Take the newest settings and the refresh flag from the `Apply` mailbox, if set, and adopt them now. Each change carries the complete settings, so only the newest matters, and a quick change and change back adopts nothing new (`display.md` D41; P7-15).
  2. Adopt the newest snapshot if a new one arrived. `Refresh` asks the sensor thread for a snapshot at once; its reset happens on the first tick that has the fresh snapshot, normally the next one.
  3. Build the `picture` from the mode, readings and unit; hand it to the `Marquee` as a reset or an update (below).
  4. `Marquee::tick()`, then `Frame::rotated(rotation)`, then post the frame, with its tick number, to the device mailbox.
  5. Update `Status`.
  6. Once a second, send `WATCHDOG=1` if the device and sensor threads both progressed within the last 10 s (Recommendation, Supervision).
- **Reset or update** (`display.md` Mode switches and Movement, precisely):
  - **reset:** start-up, a mode switch, a rotation change, a unit change while the mode shows a temperature, another GPU, `Refresh`;
  - **update:** new readings;
  - **nothing:** a choice that is already set; a unit change while the mode shows no temperature (the unit is only saved). The D-Bus side doesn't even send these (Task 7).
  - When a settings change and new readings land on the same tick, the reset wins, drawn with the new readings (S21).
- **Start-up:** the first tick shows the saved mode at once. `Lp360` sends `03 08` first (Phase 5). The first second's rate metrics are `NotYet` → dashes (`display.md` Unavailable values).
- **Availability:** a metric is persistently unavailable when its `Unavailable` reason is `NoRule`, `NoDevice`, `PermissionDenied`, `IntegratedGpuPower`, `UnknownGpuKind`, `DriverUpdated` or `LibraryFailed` (overview cross-check, 2026-10-27). `NoDevice` counts only after the first two minutes after start-up or a wake-up, while the registry still looks for late drivers; until then the metric shows dashes and its modes stay available (owner, 2026-10-27; solution review P6-04).
  - `AvailableModes` = the modes `draws` accepts whose every metric is available; the rest carry their first reason (P8-03).
  - A missing device makes all of its metrics unavailable, and so all of its modes (`settings.md` Availability).
- **Unavailable values** in the current mode → dashes; the mode stays.
- **GPU:**
  - the chosen GPU comes from settings through Phase 6's `choose`;
  - `rank_is_stale` → delete `gpu-rank.json`, log it, and the helper ranks again at the next start (Phase 6 Task 7).

**Tests** (layer 3, CI; `FakeConnector`, `FakeReadings`, `FakeClock`)
- **Timing:** 50 ticks → exactly 50 frames, at `start + n × 100 ms`. A tick delayed 250 ms produces one frame, and later deadlines stay on the grid.
- **The tick never waits:** a fake device whose write sleeps 5 s, and a fake registry whose snapshot sleeps 2 s, leave every tick on its 100 ms grid. Frames posted during the slow write replace each other in the mailbox, and the device thread then writes the newest one.
- **Watchdog:** a fake device thread that stops progressing makes the engine stop sending `WATCHDOG=1` within 10 s; a slow but progressing one doesn't.
- **Panic:** a renderer that panics ends the process with `SIGABRT` (run in a child process).
- **Refresh:** the reset lands on the first tick after the fresh snapshot.
- **Start-up:** the reports are `03 08`, then the saved mode's picture in the first frame.
- **Scenarios S14–S18 and S21–S26 for modes 1–6**, from `scenarios/load.txt` (Task 1), driven through the engine with the fakes, in all four rotations:
  - mode switches: all 30 ordered pairs among modes 1–6, at every *k*, with values below and above 100; the frame after the switch is the new picture at home and nothing of the old one appears;
  - rotation changes (each value, and `RotateScreen` presses 0→1→2→3→0) and unit changes, while static and while scrolling;
  - another GPU and `Refresh` at every *k*; a choice already set changes nothing;
  - start-up into each mode with values of 99 or less, 100 or more, and dashes turning into 100 or more;
  - a reading change and a settings change on the same tick;
  - a refused change (an unavailable mode, a later phase's mode) changes nothing.
- **Missed tick while scrolling (S22):** the strip moves one column per frame sent.
- **Readings:**
  - a changed reading shows within 10 ticks;
  - `NotYet` → dashes, then digits;
  - a GPU metric becoming `NoDevice` mid-run → dashes; mode 4 stays the mode, and `AvailableModes` drops 2, 4 and 6.
  - **Late sources** (P6-04, fake clock): Gpu usage `NoDevice` at start and found 30 s later → modes 2, 4 and 6 never leave `AvailableModes`; still missing after two minutes → they leave it, and come back when the source is found later.
  - **Newest settings** (P7-15): two changes before one tick → only the second is adopted; mode 1 → 2 → 1 before one tick → no reset.
  - **Drawable only** (P8-03): mode 13 is never in `AvailableModes`, even with every reading available, because `draws(13)` is false.
- **Reconnect (S18):** `FakeConnector` vanishes for 5 ticks while `100%` scrolls, then returns. No frames are lost from the schedule, the strip's position follows the ticks, and the returning connection starts with `03 08` (Phase 5).
- **Wake-up (S18; owner, 2026-10-26):** with the device present all along, a `Wake` while `100%` scrolls. The next report is `03 08`, then that tick's frame; the strip's position continues; `FakeReadings` counts one `restart_intervals()`, and the rate metrics it then reports as `NotYet` show dashes for their first second, then values. A `Wake` while the device is absent changes nothing but the readings.
- **Stale GPU ranking:** the rank file is deleted only when the set changed and no choice exists.

**Validation**
- `cargo test -p odc-service --test engine`: passes.

### 7. D-Bus interface, polkit and diagnostics

**Files and integration points**
- `crates/odc-service/src/dbus.rs`: CREATE.
- `crates/odc-service/src/polkit.rs`: CREATE.
  - `trait Authorizer { fn allowed(&self, sender: &str, action: Action) -> Result<bool> }`, with `Action::{Use, ChangeSettings}`;
  - `PolkitAuthorizer` calls `CheckAuthorization` with subject `("system-bus-name", {"name": sender})`, flags 0 (no interactive prompt), and the action's id from `odc-ipc`.
- `crates/odc-service/src/watchers.rs`: CREATE. The watcher list, its caps and the one-second re-check.
- `crates/odc-service/src/sleep.rs`: CREATE. `trait SleepWatcher`, with `LogindSleepWatcher` (a match rule on sender `org.freedesktop.login1`, interface `org.freedesktop.login1.Manager`, member `PrepareForSleep`) and a fake for tests.
- `crates/odc-service/src/log.rs`: CREATE. Journal lines are stderr lines with `<N>` priority prefixes (`sd-daemon(3)`); there's no logging crate.

**Implementation**
- **Each change method:**
  1. `Authorizer::allowed(header.sender(), ChangeSettings)`, else `NotAuthorized`.
  2. Validate the argument against the service's own state, else `InvalidArgument` (owner, 2026-10-26; solution review OV-14):
     - a mode from the table; a rotation from 0 to 3; `celsius` or `fahrenheit`;
     - for `ChooseGpu`, an address listed in `Gpus` now;
     - for `SetCoolerFanInput` and `SetPumpInput`, the empty triple (none) or a triple listed in `FanInputs` now;
     - a string over its limit (64 bytes; 1024 for a device path) is refused before any lookup.

     No file path is ever built from a caller's string: arguments are only compared with entries the service found itself.
  3. A mode `draws` doesn't accept → `NotYetAvailable`; an unavailable mode → `ModeUnavailable(reason)`.
  4. The value is already set → success, and nothing is stored, sent or emitted (`display.md`: choosing what is already set changes nothing).
  5. `SettingsStore::store`.
  6. Send `Apply` to the engine. A unit change while the mode shows no temperature is stored but sends nothing that touches the picture.
  7. Send `StateChanged` with the changed keys to every authorized watcher, each signal addressed to that watcher alone.
- **`Refresh()`:** authorized like a change; sends `Apply(Refresh)`; stores nothing.
- **Reads** (`GetState`, `Watch`, `Unwatch`, `Diagnostics`) check `Authorizer::allowed(sender, Use)`, else `NotAuthorized` (owner, 2026-10-26).
- **Watchers:** `Watch()` adds the caller's unique name, or `TooManyWatchers` beyond 8 per Unix user (from `GetConnectionCredentials`) or 32 in all. A name that leaves the bus (`NameOwnerChanged`) is dropped. A second `Watch()` from the same name changes nothing.
- **Re-check every second**, on the D-Bus side's own timer, never on the engine thread: each watcher is checked for `Use` again. A failing one is skipped by every `StateChanged` until it passes again, and then gets the whole state.
- **Wake-up:** on `PrepareForSleep(false)` the `SleepWatcher` sends `Wake` to the device thread (`Lp360::reset_connection()`) and to the sensor thread (`Readings::restart_intervals()`), and the journal gets one line.
- **`Diagnostics`** returns `Registry::bindings()` plus each state. The same list goes to the journal at start and at each rebind (PRD Diagnostics).
- **The dev interface** (only when started with `--dev`), at its own object path `/io/github/dtmarangoni/OpenDeepCool1/Dev`:
  - `OverrideReadings(a{s(xt)}, a{ss})`: exact values (metric → numerator and denominator, like Phase 6's `Exact`), and unavailable metrics with their reasons. They replace the sensor thread's snapshots until `ClearOverride()`;
  - `BeginTest()` and `EndTest()`, a test session (owner, 2026-10-27; solution review P7-16, P7-07). While one caller holds it, every change from any other caller is refused with `…Error.Busy` ("a test is running on this dev copy"), and the frame recording runs. The session, every override and the recording end on `EndTest()` or when the holder's connection leaves the bus (`NameOwnerChanged`), so a killed test cleans up too. `OverrideReadings` needs the session;
  - authorization, three ways (owner, 2026-10-26; solution review OV-13):
    - the D-Bus policy (group `opendeepcool`);
    - a check inside each method of the caller's credentials: `GetConnectionCredentials`, whose `UnixGroupIDs` must contain the service's group, with a missing `UnixGroupIDs` meaning no. It sits behind a `Credentials` trait, like `Authorizer`, so tests use a fake;
    - polkit;
  - it isn't registered at all without the switch.

**Tests** (layer 3, CI)
- zbus peer-to-peer connections over a socket pair: no bus daemon, and a fake `Authorizer`:
  - allowed → the setting is stored, the engine gets `Apply`, and each authorized watcher gets `StateChanged` with the changed key;
  - denied → `NotAuthorized`, with nothing stored or applied;
  - mode 9 → `NotYetAvailable`;
  - mode 2 while Gpu usage is `NoDevice` → `ModeUnavailable`;
  - rotation 4 → `InvalidArgument`;
  - setting the mode, rotation, unit or GPU already set → success, nothing stored or applied, no signal;
  - `Refresh()` allowed → the engine gets `Apply(Refresh)`; denied → `NotAuthorized`;
  - `Diagnostics` lists all seven metrics;
  - without `--dev`, the dev interface and its object path don't exist;
  - an unknown PCI address, a fan triple not in `FanInputs`, a path-like device path (`../../etc/shadow`) and an over-long string → `InvalidArgument`, with nothing stored or applied;
  - with `--dev`, a caller outside the group is refused, both through a normal call and through a raw method call without an interface field; a caller inside the group passes;
  - **test session** (P7-16): during a session, another caller's `SetMode` → `Busy`; the holder's connection closing, as when the test is killed, clears the override and ends the session, and changes pass again; `OverrideReadings` without a session → refused;
  - the introspection data lists no properties;
  - the contract (solution review P4-04): the service's introspection equals the XML in `docs/spec/dbus.md`, and `GetState()` returns exactly its state table's keys, each with its type;
  - `CoolerState` follows the fake connector (P5-02): absent → `not-found`, an open error → `no-access`, a failed write → `write-failed`, then `connected` again; each change reaches watchers once, and the journal gets one line per change;
  - `GetState`, `Watch` and `Diagnostics` from a caller the fake authorizer refuses → `NotAuthorized`, and nothing is registered;
  - a watcher the authorizer stops allowing gets no `StateChanged`; once allowed again, it gets the whole state in one;
  - a ninth watcher of one user, or a 33rd in all → `TooManyWatchers`; a watcher whose connection closes is dropped;
  - the fake `SleepWatcher`'s `PrepareForSleep(false)` → `Wake` reaches the device and sensor threads; `true` → nothing.
- On a private `dbus-daemon` (as Phase 10's tests use): a second connection with a match rule for `StateChanged`, but no `Watch()`, receives nothing while a watcher receives every change.
- **A faked wake-up** (overview cross-check, 2026-10-27; AGENTS.md rule 5): on a private `dbus-daemon`, the real `LogindSleepWatcher` gets `PrepareForSleep(false)` from a connection that doesn't own `org.freedesktop.login1` → no `Wake`; the same signal from the connection that owns the name → one `Wake`.
- **`PolkitAuthorizer`:** a fake polkit object on a p2p connection checks the exact subject, action (both actions) and flags it is sent.

**Validation**
- `cargo test -p odc-service --test dbus`: passes.

### 8. `opendeepcoold`, its unit and sandbox, the policies, and the dev copy on the owner's PC

**Files and integration points**
- `crates/odc-service/src/main.rs`: CREATE. It:
  - installs the panic hook (a journal line, then abort);
  - reads its only switch, `--dev` (any other argument is an error, so a typo can't start a different service);
  - loads settings, and builds the `Registry` and `Lp360<HidrawConnector>` (wrapped in `Recording` when started with `--dev`);
  - starts the sensor, device and engine threads and the zbus connection;
  - sends `READY=1` once the bus name is owned;
  - stops cleanly on SIGTERM.
- `Cargo.toml` (workspace): UPDATE, `[profile.release] panic = "abort"`.
- `packaging/systemd/opendeepcool.service`: CREATE.
  - `Type=notify`, `NotifyAccess=main`, `WatchdogSec=15`;
  - `User=opendeepcool`, `Group=opendeepcool`, `StateDirectory=opendeepcool`;
  - `Restart=on-failure`, `RestartSec=1`, `RestartSteps=5`, `RestartMaxDelaySec=30`, and `StartLimitIntervalSec=0` in `[Unit]` (Recommendation, Supervision);
  - `Wants=` the two Phase 6 one-shots; `After=opendeepcool-gpu-rank.service lm_sensors.service`, not the detection (P6-02, P6-04); `WantedBy=multi-user.target`;
  - sandbox: `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp=yes`, `NoNewPrivileges=yes`, `CapabilityBoundingSet=`, `ProtectKernelTunables=yes`, `ProtectKernelModules=yes`, `ProtectKernelLogs=yes`, `ProtectControlGroups=yes`, `ProtectClock=yes`, `ProtectHostname=yes`, `RestrictAddressFamilies=AF_UNIX`, `RestrictNamespaces=yes`, `LockPersonality=yes`, `MemoryDenyWriteExecute=yes`, `RestrictRealtime=yes`, `SystemCallArchitectures=native`, `SystemCallFilter=@system-service`, `SystemCallErrorNumber=EPERM`;
  - `SystemCallErrorNumber=EPERM` is fixed (owner, 2026-10-27; overview cross-check): without it a call outside the filter kills the service at once (systemd.exec(5)). NVIDIA's NVML runs inside the service and is untested in this sandbox, so a call it makes outside the list must fail, not crash the service at every start on NVIDIA PCs;
  - **tried one at a time** (owner, 2026-10-27; solution review P7-09), each kept only if `systemd-analyze security` and the real-cooler run still pass; the task's report lists every one dropped, with the reason: `PrivateNetwork=yes` (the system bus is a file-system socket and stays reachable, systemd.exec), `IPAddressDeny=any`, `RestrictSUIDSGID=yes`, `RemoveIPC=yes`, `UMask=0077`, `StateDirectoryMode=0700`, `SystemCallFilter=~@privileged @resources`, and `ProtectProc=invisible`, since Phase 9 logs the audio helper from its D-Bus credentials, never another user's `/proc` entry (owner, 2026-10-27; solution review P9-11);
  - devices: `DevicePolicy=closed`, `DeviceAllow=char-hidraw rw`. NVIDIA nodes are added in Phase 11 after verification (Phase 6).
- `packaging/dbus/io.github.dtmarangoni.OpenDeepCool1.conf`: CREATE. Every rule names `send_destination`, paired with `send_interface` or `send_path`, because the interface field of a message is optional (dbus-daemon(1)):
  - user `opendeepcool` may own the name;
  - everyone may call the main interface at the main object path, and introspect it. The service checks every caller itself with polkit, because a D-Bus policy can't tell sessions apart;
  - only group `opendeepcool` may send to the dev object path;
  - nothing else is allowed for this destination.
- `packaging/polkit/io.github.dtmarangoni.OpenDeepCool1.policy`: CREATE. `use` and `change-settings`, each `allow_active` yes, `allow_inactive` no, `allow_any` no (PRD; owner, 2026-10-26).
- `packaging/sysusers/opendeepcool.conf`: UPDATE. `g` becomes `u! opendeepcool - "OpenDeepCool display service"`, with no login and no home.
- `scripts/dev/opendeepcool-dev.conf`: CREATE, the drop-in that runs the dev binary with `--dev`, installed as `/etc/systemd/system/opendeepcool.service.d/dev.conf`. Never packaged. With `--dev`, the recording writes each report with its tick number and write time (Task 9). If the product keeps `UMask=0077` and `StateDirectoryMode=0700` (P7-09), the drop-in relaxes them to `0027` and `0750`, so the tests, which run as the owner in group `opendeepcool`, can read the recording.
- `scripts/dev/setup.sh`, `install-root.sh`, `teardown.sh`, `check.sh`: UPDATE.
  - **setup**, as the user, builds `--release` (solution review P5-03); then **install-root**, its only `pkexec` step, which never builds, installs the binary to `/usr/local/libexec/opendeepcool-dev/` and the files to their `/etc` places, points the unit's `ExecStart` there through that drop-in, re-runs `systemd-sysusers`, reloads D-Bus and systemd, enables and (re)starts the service, and records everything in the manifest.
  - **teardown** reverses all of it from the manifest.
  - **check** verifies the service is active, owns its bus name, and runs as `opendeepcool`.

**Tests**
- `systemd-analyze verify packaging/systemd/opendeepcool.service` passes.
- A unit test reads `opendeepcool.service` and checks the supervision values: `Type=notify`, `WatchdogSec=15`, `StartLimitIntervalSec=0`, `RestartSteps=5`, `RestartMaxDelaySec=30`. It also checks the ordering: `After=` the rank helper and `lm_sensors.service`, never the detection (P6-02, P6-04).
- `systemd-analyze security opendeepcool.service` on the dev copy gives an exposure level of "OK" or better. The output is pasted into the task's report, and any setting that can't be tightened is explained there.
- `xmllint --noout` on the D-Bus and polkit files. They're validated against their DTDs where the DTDs are installed; otherwise the check is "well-formed" only, and the report says so.

**Validation**
- After the owner approves the `scripts/dev/setup.sh` run and its `pkexec` step:
  - `scripts/dev/check.sh` exits 0;
  - `busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 io.github.dtmarangoni.OpenDeepCool1 GetState` from the owner's session shows `Mode` as `u 1`.

### 9. Layer 4: the real service drives the cooler through every Load Mode case and scenario, unattended; the start check; and the attended tour

**Files and integration points**
- `crates/odc-testkit/src/hardware.rs`: UPDATE, with two capabilities:
  - `DevService`: Real when the dev copy is active, owns its name, runs with `--dev`, and the LP360 probes Real;
- `crates/odc-e2e/src/service.rs`: CREATE, this phase's checks in the `odc-e2e` program (Phase 5 Task 9), run with `--only service`.
- `crates/odc-e2e/src/attended.rs`: CREATE, the attended tour behind the `--attended` flag.

**Implementation**
- **On the real cooler, one example of each mode and of each test type** (owner, 2026-10-26; solution review OV-04). The whole matrix and every scenario, at every position and in every rotation, run in-process in layers 1 and 3, where they take seconds; at the real 100 ms tick the whole cross would take many hours.
- **One test session for the whole run** (P7-16): `BeginTest()` first; a drop guard calls `EndTest()`, and if the run is killed, the session ends with its connection.
- **Timing** (owner, 2026-10-27; solution review P7-13): about 5 minutes of mode 1 at fixed readings, some 3,000 frames. The mean interval between write times lies within DeepCreative's captured 99.98–100.06 ms (`protocol.md` Timing); every interval lies between 50 and 150 ms, so no frame was dropped or doubled; the delay from `03 08` to the first frame is recorded. The summary prints the intervals' p50, p99 and maximum.
- **Mode examples** (`DevService` Real):
  - for each of modes 1–6, one case (value set 2, rotation 0): `OverrideReadings`, then `SetMode` through `odc-ipc`'s proxy (the owner's active session passes polkit);
  - wait 3 ticks, then read the recorded frames since the call; the last 2 frames must match the expected case;
  - one example of each further type: a rotated case (`SetRotation`), a scrolling case (`100%`, its whole cycle), and one dash case of `unavailable.txt`.
- **Durations:** with `--dev`, the device thread records each write's duration and the sensor thread each snapshot's. The summary prints their p50, p99 and maximum, and the report records them.
- **Scenario examples** (`DevService` Real): one variant of each scenario type of `scenarios/load.txt` that applies to Load, in one mode and rotation 0, on the real, sandboxed service.
  - To line changes up with ticks exactly, the dev interface's `OverrideReadings` takes effect on the next tick (not the next snapshot) and returns that tick's number, and the dev recording writes each report with its tick number. Settings changes go through the normal methods, and their tick is read from the recording.
  - Every recorded frame from each change on must match the scenario's expected frame for that tick.
  - Unattended, every run; the summary prints the time it took.
  - **Afterwards:** `ClearOverride` and restore the owner's saved settings, which were read at the start, so the run leaves the PC as it found it.
- **Wake-up:** the device side runs real in Phase 5 (`reset`). The real service's logind subscription can't be triggered without a real suspend, which needs root to wake an unattended PC, so it's tested with the fake `SleepWatcher` (Task 7) and, optionally, in the attended tour.
- **Fake mode** (`DevService` Absent, for example on another computer): the same examples run against an in-process `Engine` with the fakes. A `DevService` that's present but unusable (for example the service is inactive) fails with the fix.
- **Attended tour** (`odc-e2e --attended`; owner, 2026-10-24): a short run for the owner to watch on the cooler, never a gate (AGENTS.md rule 3: every required test stays unattended).
  - One case of each kind: each Load Mode (modes 1–6) with one-digit, two-digit and `100%` values, and dashes.
  - One case of each edge case: one variant of each scenario S1–S26 that applies to Load Mode.
  - For each, it shows the case on the real cooler through the dev service, prints the expected picture as text and one line saying what should happen ("`100%` scrolls left from where `99%` was, then stops at home"), and waits: `y` it looked right, `n` it didn't (with an optional note), `r` show it again.
  - Optionally, one real standby: the tour asks the owner to suspend the PC and wake it. Then it checks the recording shows `03 08` again before the next frame, and asks whether the cooler shows the mode again.
  - The answers go to `target/e2e/attended-<unix time>.txt`, and a `n` makes the tour's exit code non-zero. Afterwards it restores the owner's settings, like the unattended run.
  - Later phases add one case of each of their modes and edge cases.
- **Start check** (`DevService` Real; no reboot, owner 2026-10-25):
  - `systemctl is-enabled opendeepcool.service` is `enabled`, and the unit's `WantedBy=` is `multi-user.target` with no `After=`, `Requires=` or `Wants=` on a login, a session or `graphical.target`;
  - in the running instance's recording (from its own start, whenever that was), the first report is `03 08` and the next is the saved mode's picture with the readings of that moment, its values checked against the journal's first snapshot line of that instance.
  - Absent → it runs against a recorded journal and recording fixture and says so.

**Validation**
- `cargo run -p odc-e2e -- --only service`: passes, and prints the probe summary.
- With `--require dev-service` on the owner's PC, the mode and scenario examples run real.
- `cargo run -p odc-e2e -- --attended` works through the tour; the owner runs it when they want to look. The start check runs real once Task 8's dev copy is running.

### 10. Records, CI and the resource baseline

**Files and integration points**
- `AGENTS.md` Commands: the new layer 2 and layer 4 commands, and when to run them.
- `README.md`: Development (the dev service, `busctl` examples to change mode and rotation, `journalctl -u opendeepcool`) and Status.
- `docs/prp/prds/opendeepcool-lp360.prd.md`:
  - Success Metrics: "Resource use" gets the measured baseline;
  - System Footprint: exact file names of the D-Bus policy and polkit action, in both the product and dev tables.

**Implementation**
- **Baseline:** with the dev service in mode 4 for 10 minutes, sample `/proc/<pid>/stat` (utime + stime) and `/proc/<pid>/status` (`VmRSS`). Record mean CPU %, peak RSS, and the p50, p99 and maximum durations of frame writes and snapshots. This is the PRD's "measure a baseline in Phase 7"; the owner set no target.

**Validation**
- `.githooks/pre-commit` and CI are green.
- The baseline numbers are in the PRD.

## Acceptance

1. **AC1 — Load Mode as the spec draws it:**
   - all 72 Load Mode cases (modes 1–6 × 4 rotations × 3 value sets) match the regenerated `docs/spec/frames/` in unit tests, `100%` scrolling over its whole cycle;
   - every captured DeepCreative step of modes 1–6 where the two sets of rules agree matches with 0 differing pixels (local); the steps skipped on purpose are listed;
   - a metric the mode doesn't show never appears (Tasks 2, 3).
2. **AC2 — Dashes:** an unavailable value, or a negative, not-a-number or infinite reading, is drawn as dashes, with the symbol kept and the mode unchanged; `unavailable.txt` cases match (Tasks 1, 2, 6).
3. **AC3 — Timing, resets and movement:**
   - one frame per 100 ms on absolute deadlines, never delayed by a slow write or snapshot; readings once per second;
   - every scenario S1–S26 that applies to Load Mode passes for modes 1–6, single and combined values (change in the top, bottom and both), in all four rotations, at every cycle position the scenario lists (Tasks 1, 2, 3, 6);
   - a mode, rotation, unit (in a temperature mode) or GPU change, or `Refresh`, shows the new picture at home on the next frame; a choice already set changes nothing.
4. **AC4 — Start-up and wake-up:** `03 08`, then the saved mode in the first frame. A fresh install starts with mode 1, rotation 0, Celsius. After a wake-up from standby, a new `03 08` comes before the next frame and the interval readings start over (Tasks 5, 6, 7).
5. **AC5 — Settings:**
   - one versioned TOML file, written atomically, only by the service;
   - an invalid file is moved aside and defaults are used (Task 5).
6. **AC6 — Authorization:**
   - every call needs polkit, reads (`use`) and changes (`change-settings`) alike, granted only to the active local session (owner, 2026-10-26);
   - nothing is broadcast: state changes reach each authorized watcher alone, re-checked every second, at most 8 watchers per user and 32 in all;
   - unavailable and later-phase modes are refused with their reason;
   - `Refresh()` needs the same authorization as a change;
   - the dev interface exists only with `--dev`, at its own object path, and only for group `opendeepcool`, checked by the D-Bus policy, inside each call and by polkit;
   - every argument is validated against the service's own lists, and no path is built from a caller's string (Tasks 4, 5, 7, 8).
7. **AC7 — Availability:** `AvailableModes` excludes every mode with a persistently unavailable metric, and every mode of a missing device, with one-line reasons (Task 6).
8. **AC8 — Sandboxed system service:**
   - `opendeepcoold` runs as the locked `opendeepcool` account, at boot, with the listed sandbox;
   - `systemd-analyze security` rates it "OK" or better;
   - a panic or a hang ends in a restart (watchdog), and restarts back off but never stop (Tasks 6, 8).
9. **AC9 — Real cooler, unattended:**
   - the dev service, driven over D-Bus, writes the expected frame for one case of each Load mode and for one example of each test type: rotation, scrolling, dashes, and each scenario type (owner, 2026-10-26); every case runs in-process;
   - the running service's first frame after `03 08` was the saved mode, and its unit is enabled for boot with nothing tied to a login (the start check, no reboot);
   - other computers run the same checks against fakes (Task 9).
10. **AC10 — Records:**
    - README, AGENTS.md Commands, and the PRD (footprint file names, resource baseline) match what was built;
    - the dev manifest lists every dev file (Tasks 8, 10).

## Validation

| Gate | Command or procedure | Proves |
|---|---|---|
| Format, lint | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings` | Style; no warnings |
| Layers 1 + 3 | `cargo test --workspace` | AC1 (matrix), AC2, AC3, AC4, AC5, AC6, AC7 |
| Layer 2 (local) | `cargo test -p odc-render --test differential_load -- --ignored` | AC1 (captures) |
| Unit and policies | `systemd-analyze verify packaging/systemd/opendeepcool.service`; `xmllint --noout packaging/dbus/*.conf packaging/polkit/*.policy` | AC8 (syntax) |
| Sandbox | `systemd-analyze security opendeepcool.service` on the dev copy | AC8 |
| Dev copy | `scripts/dev/check.sh`; `busctl call … GetState` | AC8, AC10 |
| Layer 4 (local, unattended) | `cargo run -p odc-e2e -- --only service --require dev-service` | AC9 |
| Attended tour (optional, the owner) | `cargo run -p odc-e2e -- --attended` | The owner's own look; never a gate |
| CI | GitHub Actions on the pushed branch | Layers 1 + 3 |

### Test matrix covered in this phase

| Matrix cases | Layer 1 | Layer 2 | Layer 4 |
|---|---|---|---|
| Modes 1–6 × rotations 0–3 × value sets 1–3 (24 of the 112 mode, rotation and unit cases; 72 sections) | `matrix_load` | every captured step where both rules agree | one case per mode, plus one rotated and one scrolling case, on the dev service, or fakes where absent |
| Dash and impossible-reading cases (`unavailable.txt`) | `matrix_load` | — (DeepCreative has no dashes) | the dev service, or fakes |
| Movement scenarios S1–S13, S19, S20, S22 × modes 1–6 × top, bottom, both × 4 rotations × every listed cycle position | `scenarios_load` | — (OpenDeepCool's own rules) | one variant of each scenario type |
| Resets and settings: S14–S18, S21, S23–S26 (all 30 ordered mode pairs, rotation, unit, GPU, Refresh, start-up, reconnect and wake-up, refused and repeated choices) × 4 rotations | `engine` | — | one variant of each scenario type |

## Risks and Decisions

| Decision or risk | Recommendation | Evidence / mitigation | Consequence if different |
|---|---|---|---|
| No caps, automatic scrolling, the movement procedure, Refresh (owner, 2026-10-24) | Done: the spec, the expected frames and this plan follow them (revised 2026-10-24) | `display.md` Values, Movement, precisely, Test scenarios; parity checklist D35–D41 | — |
| The movement procedure is new and exact; a mismatch between the Rust code and the oracle generator could hide in rare cycle positions | Both follow the spec text independently; scenarios cover every cycle position; Marquee properties run on random sequences | `display.md` Movement, precisely | A bug would show as a jump or a strip that never stops |
| D-Bus names use the GitHub account and the major version (`io.github.dtmarangoni.OpenDeepCool1`) | Keep (owner, 2026-10-24): the D-Bus spec's reversed-domain rule and its advice to version interface names; the account name is already public in `LICENSE` and the PRD | D-Bus specification, Bus names | Another prefix (for example `org.opendeepcool`) would claim a domain the project doesn't own |
| The dev readings override lives in the shipped binary | Keep (owner, 2026-10-26): off unless the service starts with `--dev`, which only the dev drop-in passes; the packaged unit never sets it; at its own object path, checked by the D-Bus policy, inside each call and by polkit | Without it, real-service frames can't be compared with expected ones | Testing only in-process would leave the sandbox, D-Bus policy and boot path untested until Phase 11 |
| `systemd-analyze security` may flag settings that zbus or hidraw need | Tighten everything else; document each exception in the report | `RestrictAddressFamilies=AF_UNIX` is needed for the system bus | A weaker rating, explained |
| Threads add coordination | A one-slot mailbox, a newest-snapshot slot and the one-slot `Apply` mailbox; nothing shared mutably in both directions; engine tests with stalling fakes (owner, 2026-10-26) | Recommendation, one thread per job | — |
| Reads need polkit too (owner, 2026-10-26) | `use`, `allow_active` only; the app reads once, then watches, so a polkit round trip of a few ms per call doesn't matter | polkit(8); Task 7 | Open reads would let remote and inactive sessions see the settings |
| Signals addressed to each watcher | Keep: the bus delivers them to their destination only; root can still monitor the bus, and root is trusted anyway | D-Bus specification; dbus-broker has no eavesdropping | A broadcast would reach every local process |
| A wake-up can't be tested by a real suspend, unattended | Fake `SleepWatcher` in CI; the real watcher's sender check on a private bus; the device side real in Phase 5; a real suspend only in the optional attended tour | Waking an unattended PC needs root | — |
| Readings refresh once per second, so a value can lag by up to 1 s | As DeepCreative: it drew a changed value 957 ms after the change (`display.md` Updates) | `sensors.md` refresh | — |

## Agent Notes

- Installing or upgrading the dev copy is a `pkexec` system change. Show the owner the script's commands the first time; later upgrades of the same script follow the same approval (rule 7).
- The end-to-end run changes the cooler's picture for a few minutes, then restores the owner's settings. It must never leave the override active (the drop guard calls `ClearOverride`).
- Mode names in code and in anything a user reads (journal lines, D-Bus error texts, `--help`): Load, Temperature, Power, Music, never "Guardian" or "… Mode" (owner, 2026-10-24). DeepCreative's names appear only where the spec describes DeepCreative.
- The D-Bus state keys and method list in `odc-ipc` are the settings app's contract, described in `docs/spec/dbus.md`. If it changes, update `docs/spec/dbus.md` in the same commit, and `docs/spec/settings.md` "What the settings app must offer" so the UI designer sees it (owner's rule, 2026-10-24).
