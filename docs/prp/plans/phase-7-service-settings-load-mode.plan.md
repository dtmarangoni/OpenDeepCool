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
- A user at the PC can change mode, rotation, unit, GPU and fan inputs over D-Bus: today with `busctl`, from Phase 10 with the app. Other sessions can only read.

**Invariant:**
- Every frame equals the spec's picture for the current mode, readings, rotation and tick (`docs/spec/frames/` is the oracle).
- A value that can't be read, or a reading no sensor can give, is drawn as dashes, never as a number (rule 11, `display.md` Unavailable values).
- A value is drawn in full up to 9999, and capped at 9999 above it, everywhere (owner, 2026-10-25). A picture with a value of 100 or more scrolls, and starts, keeps moving and stops exactly as `display.md` "Movement, precisely" says.
- One frame per 100 ms tick, without drift.
- A mode switch shows on the next frame.
- The service runs as the locked `opendeepcool` account, sandboxed, and changes settings only for a caller polkit authorizes.

**Success signal:** The service, enabled for boot and running with nobody logged in, shows the configured mode with no action (PRD Phase 7). The start check proves it unattended from the journal and the recorded frames, without rebooting (owner, 2026-10-25: no boot testing).

**Approach:** four new crates on top of Phases 5 and 6:
- **`odc-render`:** pure: glyphs, value rules, the static layout and the scrolling strip, the movement procedure, the mode table, and the Load Mode pictures.
- **`odc-ipc`:** the D-Bus names, types and client proxy, shared by the service, the tests and Phase 10's app.
- **`odc-service`:** a library plus the `opendeepcoold` binary.
  - An engine thread runs the 100 ms tick over the Phase 5 `DisplayPort` and the Phase 6 `Readings`.
  - A zbus blocking connection serves the interface and asks polkit before every change.
  - Settings are a versioned TOML file in the state directory.
- **Packaging files:** the unit, the D-Bus policy, the polkit action, and the service account.

`scripts/dev/setup.sh` installs them as the dev copy, recorded in the dev manifest.

## Recommendation

**One engine thread owns the display; D-Bus never touches it directly.**
- The engine owns the `Lp360`, the `Registry`, the current mode state and the tick clock.
- The D-Bus side owns `SettingsStore` (validate, persist) and sends the engine an `Apply(Change)` message over a channel: new settings, or `Refresh`. The engine picks it up at the next tick, which is exactly the "switch on the next frame" rule (`display.md` Mode switches).
- Status flows back through one `Arc<RwLock<Status>>`: connected, availability, bindings.
- Nothing is shared mutably in both directions, so a slow polkit check can never delay a frame.

**Pure drawing, in two parts** (`display.md` Movement, precisely).
- `picture(mode, &Values, unit) -> Result<Picture>`: the strip of the mode's values (both bands aligned), its width, cycle length *L*, home column *h*, and whether it must scroll (a value of 100 or more).
- `Marquee`: the movement procedure as a pure state machine: `reset(Picture)`, `update(Picture)`, `tick() -> Frame`. It holds *x* and whether the strip moves. No clock and no I/O, so every test scenario runs as a unit test.
- Rotation is applied last with Phase 5's `Frame::rotated` (`display.md` Rotation).
- The PRD's layer 1 contract was updated to match on 2026-10-24: the picture is a pure function of mode, readings and unit, and the movement a pure function of the changes and ticks. The old single `render(mode, values, unit, rotation, tick)` can't express a strip that keeps its position when a reading changes.

**The 100 ms tick on absolute deadlines.**
- The next frame is due at `start + n × 100 ms` on the monotonic clock. A late frame doesn't shift the ones after it.
- This matches DeepCreative's long-run rate of 100.0 ms (`protocol.md` Timing, corrected 2026-10-24).
- Readings refresh every 10th tick (`sensors.md`: once per second).

**D-Bus with zbus's blocking API.**
- The owner chose D-Bus on 2026-10-24.
- zbus 6's blocking `connection::Builder::system().name(…).serve_at(…).build()` runs the object server on zbus's own executor thread, so the service needs no async runtime of its own (zbus book, "blocking").
- polkit's `CheckAuthorization` takes a `system-bus-name` subject, the caller's unique name from the message header (`#[zbus(header)]`, `Header::sender()`). This is polkit's identification for D-Bus callers (polkit Authority interface docs).

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
- it registers a second interface, `…OpenDeepCool1.Dev`. Its D-Bus policy allows only members of group `opendeepcool`. It overrides readings, so the end-to-end test can drive every matrix case through the real, sandboxed, boot-started service.
- it records every report written to `/var/lib/opendeepcool/dev-frames.log`, using Phase 5's `Recording`.

### Evidence

- `docs/spec/display.md`: Modes (modes 1–6 and their metrics, top first), Values (usage rounds up; Memory Load rounds to nearest; no cap in OpenDeepCool), Glyphs, Bands, Static layout. Line numbers changed with the 2026-10-24 decisions, so sections are named instead.
- `docs/spec/display.md`: Updates; Unavailable values (dashes, added 2026-10-24); Values, OpenDeepCool: no caps (automatic scrolling, one smooth movement); Static layout, OpenDeepCool: alignment; Movement, precisely; Mode switches (a mode switch jumps to home); Test scenarios S1–S26; Rotation. All owner decisions of 2026-10-24.
- `docs/spec/frames/` regenerated 2026-10-24 with OpenDeepCool's rules (172 of 292 cases changed); `research/hook/odc_rules.py` (local) draws them.
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

### Alternatives considered

- **An async runtime (tokio) for the whole service:** one more large dependency. The service has one periodic loop and a handful of rare calls, and zbus's blocking API runs the server on its own thread.
- **The engine and D-Bus sharing one `Mutex<State>`:** a polkit round trip or a slow client could hold the lock across a tick. The channel keeps the tick independent.
- **Testing the real service with real readings only:** the values can't be chosen, so frames can't be compared with the expected frames. The dev-only readings override keeps the comparison exact.
- **JSON settings:** works equally well, but it's less readable in the state directory. TOML is the conventional choice for configuration.

## Visuals

```mermaid
flowchart LR
  subgraph svc["opendeepcoold (new), User=opendeepcool, sandboxed"]
    DB["D-Bus object (zbus blocking)<br/>io.github.dtmarangoni.OpenDeepCool1"] -->|Apply(Settings)| CH((channel))
    DB --> ST[(SettingsStore<br/>settings.toml v1)]
    DB -->|CheckAuthorization| PK[[polkit]]
    CH --> EN["Engine thread<br/>100 ms absolute ticks"]
    EN --> RD["odc-render: picture + Marquee<br/>(pure, new)"]
    EN --> LP["Lp360 DisplayPort (Phase 5)"]
    EN --> RG["Registry Readings (Phase 6)<br/>every 10th tick"]
    EN --> STAT[(Status: connected,<br/>availability, bindings)]
    STAT --> DB
    DEV["…OpenDeepCool1.Dev<br/>(only with --dev)"] -.->|readings override| EN
  end
  APP["busctl now / settings app (Phase 10)"] -->|system bus| DB
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

- `odc_ipc`: the contract Phase 10's app compiles against: names, the property and method list, error names.
- `odc_render::{Mode, mode_info, picture, Marquee}`: Phases 8–9 add their modes' pictures; the strip and the movement serve them unchanged. The app's preview (Phase 10) reuses `picture` and `Marquee`.
- `opendeepcool.service`: `Wants=` and `After=` on `opendeepcool-gpu-rank.service` and `opendeepcool-sensors-detect.service` (Phase 6).
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
- Settings v1, the D-Bus interface, polkit, diagnostics, journal logging.
- The `opendeepcoold` binary, unit, sandbox, D-Bus policy, polkit action, service account, and the dev copy with its frame recording.
- Tests in all four layers, plus a start check and a resource baseline.

### Not building

- **Modes 7–23:** Phases 8–9. `SetMode` rejects them with "not available yet". The mode table already describes them, for availability.
- **Temperature and power pictures** (the `°C`, `°F` and `W` glyphs, modes 7–12): Phase 8. The strip and the movement built here serve them unchanged.
- **Audio input and its D-Bus path:** Phase 9.
- **The settings app and its client UI:** Phase 10. The client proxy ships in `odc-ipc` now.
- **The final package, `/usr` paths, uninstall removal script, cold-boot series:** Phase 11.

## Delivery Considerations

| Concern | Decision and owned work |
|---|---|
| Rollout / reversibility | From Task 8 the dev copy drives the owner's cooler at every boot. `scripts/dev/teardown.sh` removes it entirely, from the manifest. Every install or upgrade through `pkexec` is shown to the owner first (rule 7) |
| Observability | The journal: start, bindings at start and on each rebind, cooler connect and disconnect, rejected requests with the caller's reason, and settings file problems. Lines carry `sd-daemon` priority prefixes, so `journalctl -p` filters them |
| Documentation | README: how to change the mode with `busctl` until Phase 10, and the journal command. AGENTS.md Commands: new tests. PRD footprint: the dev copy rows already exist (2026-10-24); add the D-Bus and polkit file names |

## Implementation

Each task ends with its own commit (rule 4) after its tests pass. **Each Load Mode combination** (modes 1–6) is implemented, tested in all layers, and committed on its own (Task 3), as rule 4 requires.

### 1. The oracle gets the dash cases and the movement scenarios

**Files and integration points**
- `research/hook/odc_rules.py` and `expected-frames.py` (local, never committed): UPDATE.
  - `odc_rules.py` already draws OpenDeepCool's pictures (it generated the 2026-10-24 frames). It gets the movement procedure of `display.md` "Movement, precisely" as a small state machine, written from the spec text, not from any Rust code.
  - `expected-frames.py` writes the files below.
- `docs/spec/frames/unavailable.txt`: CREATE, generated. Dash cases, each in rotations 0–3:
  - mode 1 with CPU Usage unavailable, and with a negative, a not-a-number and an infinite reading;
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
  - the existing `mode-*.txt` cases are reproduced unchanged;
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
  - Phase 7 draws 1–6; `picture` returns `Err(NotYetImplemented)` for the rest.
  - `Metric` is Phase 6's type.
- **Value rules** (`display.md` Values, OpenDeepCool: no caps), on `f64` readings:
  - usage: round up, no cap;
  - Memory Load: round to nearest, halves up, no cap;
  - a number of 10 000 or more → 9999, the cap that holds everywhere in OpenDeepCool (owner, 2026-10-25); the journal and D-Bus show the same number;
  - a negative, not-a-number or infinite reading → `Unavailable`, drawn as dashes.
- **`picture(mode, &Values, unit) -> Result<Picture>`** (`display.md` Static layout and Movement, precisely):
  - the strip: digit places of 4 columns, then the symbol; dashes take two places; bands at rows 6–10 for one value, and rows 2–6 and 9–13 for two, both ending at the same column;
  - *w*, *L* = *w* + max(4, 21 − *w*), the home column *h* (one value centred, an odd spare column on the left; two values ending at column 14; 2 when *w* > 14), and whether it must scroll.
- **`Marquee`:** the procedure of `display.md` "Movement, precisely", step for step: `reset(Picture)`, `update(Picture)`, `tick() -> Frame` (unrotated). The engine rotates the frame last.

**Tests** (layer 1, CI)
- **Glyphs:** each glyph equals the spec's text picture, compared row by row with a copy typed into the test from `display.md`. Typing them independently makes a copy error show up as a mismatch.
- **Value rules:** every evidence row where OpenDeepCool keeps DeepCreative's rule (0.4 → 1, 9.5 → 10 for usage; 0.4 → 0, 0.5 → 1 for Memory Load), the no-cap rows (100 → 100, 1000 → 1000, 9999 → 9999), the cap (10000 → 9999, 123456 → 9999), the boundaries of S20 (0, 0.001, 9 ↔ 10, 99 ↔ 100, 999 ↔ 1000, 9999 ↔ 10000) and the impossible readings of S19. Property: any finite reading of 0 or more gives a whole number from 0 to 9999, and anything else gives dashes.
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
  - for every captured step of this mode **where OpenDeepCool's rules agree with DeepCreative's** (two-digit values, and the rows of two-value modes whose values are all two digits), every frame inside the step's steady window equals the picture at home: 0 differing pixels;
  - steps where they differ on purpose (one-digit values centred, values of 100 or more) are skipped and listed with the reason;
  - the test prints how many steps and frames it compared and skipped, and fails if none were compared.

**Validation**
- `cargo test -p odc-render --test matrix_load --test scenarios_load`: passes.
- `cargo test -p odc-render --test differential_load -- --ignored`: passes.

### 4. `odc-ipc`: the D-Bus contract

**Files and integration points**
- `crates/odc-ipc/`: CREATE, a library: `serde`, `zbus = "6"`.

**Implementation**
- **Names:**
  - bus name and interface `io.github.dtmarangoni.OpenDeepCool1`; object path `/io/github/dtmarangoni/OpenDeepCool1`;
  - dev interface `io.github.dtmarangoni.OpenDeepCool1.Dev`;
  - polkit action `io.github.dtmarangoni.OpenDeepCool1.change-settings`.
- **Properties** (readable by everyone):
  - `Mode` (u), `Rotation` (y), `TemperatureUnit` (s: `celsius` or `fahrenheit`);
  - `CoolerConnected` (b);
  - `AvailableModes` (au) and `UnavailableReasons` (a{us}: mode → one-line reason);
  - `Gpus` (a(sss): PCI address, driver, kind, in rank order) and `ChosenGpu` (s, empty when none was chosen);
  - `FanInputs` (a(ssus): chip, device path, input, label);
  - `CoolerFanInput` and `PumpInput` (ssu each, empty when unset);
  - `CoolerFanRpm` and `PumpRpm` (d, NaN when unavailable, capped at 9999 like every value; owner, 2026-10-25).
- **Methods:**
  - **changes, polkit-checked:** `SetMode(u)`, `RotateScreen()` (one press, `settings.md` T3), `SetRotation(y)`, `SetTemperatureUnit(s)`, `ChooseGpu(s)`, `SetCoolerFanInput(ssu)`, `SetPumpInput(ssu)`, `Refresh()` (fresh readings, picture restarted from home; owner, 2026-10-24);
  - **reading:** `Diagnostics() -> a(sss)`: metric, bound source, state.
- **Errors:** `…Error.NotAuthorized`, `…Error.ModeUnavailable` (with the reason), `…Error.NotYetAvailable` (modes of later phases), `…Error.InvalidArgument`.
- A `#[proxy]` client for the tests and Phase 10.

**Tests**
- The names are valid D-Bus names (zbus parsing). The property and method lists are written into a doc comment, and that list is what Phase 10's design reads.

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
- **Validation:**
  - mode in the table;
  - rotation 0–3;
  - a known unit;
  - fan input fields non-empty.
- **`SettingsStore::load(dir)`:**
  - a missing file → defaults, which are written;
  - an unreadable or invalid file → renamed to `settings.toml.invalid-<unix time>`, with an error logged, then defaults.
- **`store(&Settings)`:** temp file, `fsync`, rename.

**Tests** (temp dirs under `CARGO_TARGET_TMPDIR`)
- Defaults on a fresh dir.
- A round trip of every field.
- An invalid file is moved aside, and defaults are used.
- A future `version = 2` file is rejected the same way, never silently downgraded.
- An interrupted write (a temp file left behind) doesn't replace the good file.

**Validation**
- `cargo test -p odc-service --lib settings`: passes.

### 6. The engine: tick, readings, start-up, resets and updates, availability, dashes

**Files and integration points**
- `crates/odc-service/src/engine.rs`: CREATE.
- `crates/odc-service/src/status.rs`: CREATE.

**Implementation**
- `Engine<D: DisplayPort, R: Readings, C: Clock>` runs `loop { wait_until(deadline); tick(); deadline += 100 ms }`, with deadlines on the monotonic clock. A deadline missed by more than one tick is logged and skipped, and frames never bunch up.
- **Each tick:**
  1. Take any pending `Apply(Change)` and adopt it now.
  2. Every 10th tick, take a `Readings::snapshot()`. `Refresh` takes one at once.
  3. Build the `picture` from the mode, readings and unit; hand it to the `Marquee` as a reset or an update (below).
  4. `Marquee::tick()`, then `Frame::rotated(rotation)`, then `display.show(frame)`.
  5. Update `Status`.
- **Reset or update** (`display.md` Mode switches and Movement, precisely):
  - **reset:** start-up, a mode switch, a rotation change, a unit change while the mode shows a temperature, another GPU, `Refresh`;
  - **update:** new readings;
  - **nothing:** a choice that is already set; a unit change while the mode shows no temperature (the unit is only saved). The D-Bus side doesn't even send these (Task 7).
  - When a settings change and new readings land on the same tick, the reset wins, drawn with the new readings (S21).
- **Start-up:** the first tick shows the saved mode at once. `Lp360` sends `03 08` first (Phase 5). The first second's rate metrics are `NotYet` → dashes (`display.md` Unavailable values).
- **Availability:** a metric is persistently unavailable when its `Unavailable` reason is `NoRule`, `NoDevice`, `PermissionDenied` or `IntegratedGpuPower`.
  - `AvailableModes` = the modes whose every metric is available; the rest carry their first reason.
  - A missing device makes all of its metrics unavailable, and so all of its modes (`settings.md` Availability).
- **Unavailable values** in the current mode → dashes; the mode stays.
- **GPU:**
  - the chosen GPU comes from settings through Phase 6's `choose`;
  - `rank_is_stale` → delete `gpu-rank.json`, log it, and the helper ranks again at the next start (Phase 6 Task 7).

**Tests** (layer 3, CI; `FakeConnector`, `FakeReadings`, `FakeClock`)
- **Timing:** 50 ticks → exactly 50 frames, at `start + n × 100 ms`. A tick delayed 250 ms produces one frame, and later deadlines stay on the grid.
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
- **Reconnect (S18):** `FakeConnector` vanishes for 5 ticks while `100%` scrolls, then returns. No frames are lost from the schedule, the strip's position follows the ticks, and the returning connection starts with `03 08` (Phase 5).
- **Stale GPU ranking:** the rank file is deleted only when the set changed and no choice exists.

**Validation**
- `cargo test -p odc-service --test engine`: passes.

### 7. D-Bus interface, polkit and diagnostics

**Files and integration points**
- `crates/odc-service/src/dbus.rs`: CREATE.
- `crates/odc-service/src/polkit.rs`: CREATE.
  - `trait Authorizer { fn allowed(&self, sender: &str) -> Result<bool> }`;
  - `PolkitAuthorizer` calls `CheckAuthorization` with subject `("system-bus-name", {"name": sender})`, flags 0 (no interactive prompt), and the action from `odc-ipc`.
- `crates/odc-service/src/log.rs`: CREATE. Journal lines are stderr lines with `<N>` priority prefixes (`sd-daemon(3)`); there's no logging crate.

**Implementation**
- **Each change method:**
  1. `Authorizer::allowed(header.sender())`, else `NotAuthorized`.
  2. Validate the argument, else `InvalidArgument`.
  3. A later-phase mode → `NotYetAvailable`; an unavailable mode → `ModeUnavailable(reason)`.
  4. The value is already set → success, and nothing is stored, sent or emitted (`display.md`: choosing what is already set changes nothing).
  5. `SettingsStore::store`.
  6. Send `Apply` to the engine. A unit change while the mode shows no temperature is stored but sends nothing that touches the picture.
  7. Emit `PropertiesChanged`.
- **`Refresh()`:** authorized like a change; sends `Apply(Refresh)`; stores nothing.
- **Reads** take no authorization.
- **`Diagnostics`** returns `Registry::bindings()` plus each state. The same list goes to the journal at start and at each rebind (PRD Diagnostics).
- **The dev interface** (only when started with `--dev`):
  - `OverrideReadings(a{sd}, a{ss})`: values, and unavailable metrics with their reasons, replacing the `Readings` source until `ClearOverride()`;
  - authorization is by the D-Bus policy (group `opendeepcool`) **and** polkit;
  - it isn't registered at all without the switch.

**Tests** (layer 3, CI)
- zbus peer-to-peer connections over a socket pair: no bus daemon, and a fake `Authorizer`:
  - allowed → the setting is stored, the engine gets `Apply`, and `PropertiesChanged` fires;
  - denied → `NotAuthorized`, with nothing stored or applied;
  - mode 9 → `NotYetAvailable`;
  - mode 2 while Gpu usage is `NoDevice` → `ModeUnavailable`;
  - rotation 4 → `InvalidArgument`;
  - setting the mode, rotation, unit or GPU already set → success, nothing stored or applied, no signal;
  - `Refresh()` allowed → the engine gets `Apply(Refresh)`; denied → `NotAuthorized`;
  - `Diagnostics` lists all seven metrics;
  - without `--dev`, the dev interface doesn't exist.
- **`PolkitAuthorizer`:** a fake polkit object on a p2p connection checks the exact subject, action and flags it is sent.

**Validation**
- `cargo test -p odc-service --test dbus`: passes.

### 8. `opendeepcoold`, its unit and sandbox, the policies, and the dev copy on the owner's PC

**Files and integration points**
- `crates/odc-service/src/main.rs`: CREATE. It reads its only switch, `--dev` (any other argument is an error, so a typo can't start a different service), loads settings, builds the `Registry` and `Lp360<HidrawConnector>` (wrapped in `Recording` when started with `--dev`), starts the engine thread and the zbus connection, and stops cleanly on SIGTERM.
- `packaging/systemd/opendeepcool.service`: CREATE.
  - `User=opendeepcool`, `Group=opendeepcool`, `StateDirectory=opendeepcool`, `Restart=on-failure`, `RestartSec=1`;
  - `Wants=`/`After=` the two Phase 6 one-shots; `WantedBy=multi-user.target`;
  - sandbox: `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp=yes`, `NoNewPrivileges=yes`, `CapabilityBoundingSet=`, `ProtectKernelTunables=yes`, `ProtectKernelModules=yes`, `ProtectKernelLogs=yes`, `ProtectControlGroups=yes`, `ProtectClock=yes`, `ProtectHostname=yes`, `RestrictAddressFamilies=AF_UNIX`, `RestrictNamespaces=yes`, `LockPersonality=yes`, `MemoryDenyWriteExecute=yes`, `RestrictRealtime=yes`, `SystemCallArchitectures=native`, `SystemCallFilter=@system-service`;
  - devices: `DevicePolicy=closed`, `DeviceAllow=char-hidraw rw`. NVIDIA nodes are added in Phase 11 after verification (Phase 6).
- `packaging/dbus/io.github.dtmarangoni.OpenDeepCool1.conf`: CREATE.
  - user `opendeepcool` may own the name;
  - everyone may call the main interface and read properties;
  - only group `opendeepcool` may call the `.Dev` interface.
- `packaging/polkit/io.github.dtmarangoni.OpenDeepCool1.policy`: CREATE. `change-settings`: `allow_active` yes, `allow_inactive` no, `allow_any` no (PRD).
- `packaging/sysusers/opendeepcool.conf`: UPDATE. `g` becomes `u! opendeepcool - "OpenDeepCool display service"`, with no login and no home.
- `scripts/dev/opendeepcool-dev.conf`: CREATE, the drop-in that runs the dev binary with `--dev`, installed as `/etc/systemd/system/opendeepcool.service.d/dev.conf`. Never packaged. With `--dev`, the recording writes each report with its tick number (Task 9).
- `scripts/dev/setup.sh`, `teardown.sh`, `check.sh`: UPDATE.
  - **setup** builds `--release`, installs the binary to `/usr/local/libexec/opendeepcool-dev/` and the files to their `/etc` places, points the unit's `ExecStart` there through that drop-in, re-runs `systemd-sysusers`, reloads D-Bus and systemd, enables and (re)starts the service, and records everything in the manifest.
  - **teardown** reverses all of it from the manifest.
  - **check** verifies the service is active, owns its bus name, and runs as `opendeepcool`.

**Tests**
- `systemd-analyze verify packaging/systemd/opendeepcool.service` passes.
- `systemd-analyze security opendeepcool.service` on the dev copy gives an exposure level of "OK" or better. The output is pasted into the task's report, and any setting that can't be tightened is explained there.
- `xmllint --noout` on the D-Bus and polkit files. They're validated against their DTDs where the DTDs are installed; otherwise the check is "well-formed" only, and the report says so.

**Validation**
- After the owner approves the `pkexec scripts/dev/setup.sh` run:
  - `scripts/dev/check.sh` exits 0;
  - `busctl get-property io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 io.github.dtmarangoni.OpenDeepCool1 Mode` prints `u 1`.

### 9. Layer 4: the real service drives the cooler through every Load Mode case and scenario, unattended; the start check; and the attended tour

**Files and integration points**
- `crates/odc-testkit/src/hardware.rs`: UPDATE, with two capabilities:
  - `DevService`: Real when the dev copy is active, owns its name, runs with `--dev`, and the LP360 probes Real;
- `crates/odc-e2e/src/service.rs`: CREATE, this phase's checks in the `odc-e2e` program (Phase 5 Task 9), run with `--only service`.
- `crates/odc-e2e/src/attended.rs`: CREATE, the attended tour behind the `--attended` flag.

**Implementation**
- **Matrix run** (`DevService` Real):
  - for each of modes 1–6 × rotations 0–3 × value sets 1–3: `OverrideReadings`, then `SetMode` and `SetRotation` through `odc-ipc`'s proxy (the owner's active session passes polkit);
  - wait 3 ticks, then read the recorded frames since the call;
  - the last 2 frames must match the expected case;
  - plus the dash cases of `unavailable.txt`.
- **Scenario run** (`DevService` Real): the scenarios of `scenarios/load.txt` on the real, sandboxed service.
  - To line changes up with ticks exactly, the dev interface's `OverrideReadings` takes effect on the next tick (not the next snapshot) and returns that tick's number, and the dev recording writes each report with its tick number. Settings changes go through the normal methods, and their tick is read from the recording.
  - Every recorded frame from each change on must match the scenario's expected frame for that tick.
  - Every scenario in every rotation, every run: the whole cross, unattended (owner, 2026-10-24). It takes a few hours; the summary prints the time it took.
  - **Afterwards:** `ClearOverride` and restore the owner's saved settings, which were read at the start, so the run leaves the PC as it found it.
- **Fake mode** (`DevService` Absent, for example on another computer): the same matrix and scenario runs against an in-process `Engine` with the fakes. A `DevService` that's present but unusable (for example the service is inactive) fails with the fix.
- **Attended tour** (`odc-e2e --attended`; owner, 2026-10-24): a short run for the owner to watch on the cooler, never a gate (AGENTS.md rule 3: every required test stays unattended).
  - One case of each kind: each Load Mode (modes 1–6) with one-digit, two-digit and `100%` values, and dashes.
  - One case of each edge case: one variant of each scenario S1–S26 that applies to Load Mode.
  - For each, it shows the case on the real cooler through the dev service, prints the expected picture as text and one line saying what should happen ("`100%` scrolls left from where `99%` was, then stops at home"), and waits: `y` it looked right, `n` it didn't (with an optional note), `r` show it again.
  - The answers go to `target/e2e/attended-<unix time>.txt`, and a `n` makes the tour's exit code non-zero. Afterwards it restores the owner's settings, like the unattended run.
  - Later phases add one case of each of their modes and edge cases.
- **Start check** (`DevService` Real; no reboot, owner 2026-10-25):
  - `systemctl is-enabled opendeepcool.service` is `enabled`, and the unit's `WantedBy=` is `multi-user.target` with no `After=`, `Requires=` or `Wants=` on a login, a session or `graphical.target`;
  - in the running instance's recording (from its own start, whenever that was), the first report is `03 08` and the next is the saved mode's picture with the readings of that moment, its values checked against the journal's first snapshot line of that instance.
  - Absent → it runs against a recorded journal and recording fixture and says so.

**Validation**
- `cargo run -p odc-e2e -- --only service`: passes, and prints the probe summary.
- With `--require dev-service` on the owner's PC, the matrix and the scenarios run real.
- `cargo run -p odc-e2e -- --attended` works through the tour; the owner runs it when they want to look. The start check runs real once Task 8's dev copy is running.

### 10. Records, CI and the resource baseline

**Files and integration points**
- `AGENTS.md` Commands: the new layer 2 and layer 4 commands, and when to run them.
- `README.md`: Development (the dev service, `busctl` examples to change mode and rotation, `journalctl -u opendeepcool`) and Status.
- `docs/prp/prds/opendeepcool-lp360.prd.md`:
  - Success Metrics: "Resource use" gets the measured baseline;
  - System Footprint: exact file names of the D-Bus policy and polkit action, in both the product and dev tables.

**Implementation**
- **Baseline:** with the dev service in mode 4 for 10 minutes, sample `/proc/<pid>/stat` (utime + stime) and `/proc/<pid>/status` (`VmRSS`). Record mean CPU % and peak RSS. This is the PRD's "measure a baseline in Phase 7"; the owner set no target.

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
   - one frame per 100 ms on absolute deadlines; readings every 10th tick;
   - every scenario S1–S26 that applies to Load Mode passes for modes 1–6, single and combined values (change in the top, bottom and both), in all four rotations, at every cycle position the scenario lists (Tasks 1, 2, 3, 6);
   - a mode, rotation, unit (in a temperature mode) or GPU change, or `Refresh`, shows the new picture at home on the next frame; a choice already set changes nothing.
4. **AC4 — Start-up:** `03 08`, then the saved mode in the first frame. A fresh install starts with mode 1, rotation 0, Celsius (Tasks 5, 6).
5. **AC5 — Settings:**
   - one versioned TOML file, written atomically, only by the service;
   - an invalid file is moved aside and defaults are used (Task 5).
6. **AC6 — Authorization:**
   - changes need polkit `change-settings` (active local session);
   - reads are open;
   - unavailable and later-phase modes are refused with their reason;
   - `Refresh()` needs the same authorization as a change;
   - the dev interface exists only with `--dev` and only for group `opendeepcool` (Tasks 4, 7, 8).
7. **AC7 — Availability:** `AvailableModes` excludes every mode with a persistently unavailable metric, and every mode of a missing device, with one-line reasons (Task 6).
8. **AC8 — Sandboxed system service:**
   - `opendeepcoold` runs as the locked `opendeepcool` account, at boot, with the listed sandbox;
   - `systemd-analyze security` rates it "OK" or better;
   - it restarts on failure (Task 8).
9. **AC9 — Real cooler, unattended:**
   - the dev service, driven over D-Bus, writes the expected frame for every Load Mode case, the dash cases and the scenario run (default or full);
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
| Dev copy | `scripts/dev/check.sh`; `busctl get-property … Mode` | AC8, AC10 |
| Layer 4 (local, unattended) | `cargo run -p odc-e2e -- --only service --require dev-service` | AC9 |
| Attended tour (optional, the owner) | `cargo run -p odc-e2e -- --attended` | The owner's own look; never a gate |
| CI | GitHub Actions on the pushed branch | Layers 1 + 3 |

### Test matrix covered in this phase

| Matrix cases | Layer 1 | Layer 2 | Layer 4 |
|---|---|---|---|
| Modes 1–6 × rotations 0–3 × value sets 1–3 (24 of the 112 mode, rotation and unit cases; 72 sections) | `matrix_load` | every captured step | the dev service, or fakes where absent |
| Dash and impossible-reading cases (`unavailable.txt`) | `matrix_load` | — (DeepCreative has no dashes) | the dev service, or fakes |
| Movement scenarios S1–S13, S19, S20, S22 × modes 1–6 × top, bottom, both × 4 rotations × every listed cycle position | `scenarios_load` | — (OpenDeepCool's own rules) | every scenario in every rotation |
| Resets and settings: S14–S18, S21, S23–S26 (all 30 ordered mode pairs, rotation, unit, GPU, Refresh, start-up, reconnect, refused and repeated choices) × 4 rotations | `engine` | — | every scenario in every rotation |

## Risks and Decisions

| Decision or risk | Recommendation | Evidence / mitigation | Consequence if different |
|---|---|---|---|
| No caps, automatic scrolling, the movement procedure, Refresh (owner, 2026-10-24) | Done: the spec, the expected frames and this plan follow them (revised 2026-10-24) | `display.md` Values, Movement, precisely, Test scenarios; parity checklist D35–D41 | — |
| The movement procedure is new and exact; a mismatch between the Rust code and the oracle generator could hide in rare cycle positions | Both follow the spec text independently; scenarios cover every cycle position; Marquee properties run on random sequences | `display.md` Movement, precisely | A bug would show as a jump or a strip that never stops |
| D-Bus names use the GitHub account and the major version (`io.github.dtmarangoni.OpenDeepCool1`) | Keep (owner, 2026-10-24): the D-Bus spec's reversed-domain rule and its advice to version interface names; the account name is already public in `LICENSE` and the PRD | D-Bus specification, Bus names | Another prefix (for example `org.opendeepcool`) would claim a domain the project doesn't own |
| The dev readings override lives in the shipped binary | Keep it off unless the service starts with `--dev`, which only the dev drop-in passes; the packaged unit never sets it; group plus polkit gate it anyway | Without it, real-service frames can't be compared with expected ones | Testing only in-process would leave the sandbox, D-Bus policy and boot path untested until Phase 11 |
| `systemd-analyze security` may flag settings that zbus or hidraw need | Tighten everything else; document each exception in the report | `RestrictAddressFamilies=AF_UNIX` is needed for the system bus | A weaker rating, explained |
| Readings refresh once per second, so a value can lag by up to 1 s | As DeepCreative: it drew a changed value 957 ms after the change (`display.md` Updates) | `sensors.md` refresh | — |

## Agent Notes

- Installing or upgrading the dev copy is a `pkexec` system change. Show the owner the script's commands the first time; later upgrades of the same script follow the same approval (rule 7).
- The end-to-end run changes the cooler's picture for a few minutes, then restores the owner's settings. It must never leave the override active (the drop guard calls `ClearOverride`).
- Mode names in code and in anything a user reads (journal lines, D-Bus error texts, `--help`): Load, Temperature, Power, Music, never "Guardian" or "… Mode" (owner, 2026-10-24). DeepCreative's names appear only where the spec describes DeepCreative.
- The D-Bus property and method list in `odc-ipc` is the settings app's contract. If it changes, update `docs/spec/settings.md` "What the settings app must offer" so the UI designer sees it (owner's rule, 2026-10-24).
