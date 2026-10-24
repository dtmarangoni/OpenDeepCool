# OpenDeepCool — DeepCool LP360 display app for Omarchy

## Problem Statement

The owner moved this PC from Windows to Omarchy (Arch Linux), and the 14×14 dot-matrix display on the DeepCool LP360's pump has shown nothing since. DeepCool's control software, DeepCreative, runs only on Windows 10/11, and the existing Linux tools cover a small part of what DeepCreative draws on the LP360. The display sits idle, and the owner has no at-a-glance view of temperatures and load. There is no Windows to fall back on: no dual boot and no spare drive.

## Evidence

- The display is blank under Omarchy (owner's report, 2026-10-22).
- Linux sees the cooler: USB HID device `3633:000c` "DeepCool LP-Series" (lsusb and sysfs, 2026-10-21).
- DeepCool supports Windows 10/11 only. Sources: the LP240/LP360 manual (p. 18, "Download") and DeepCool's download page (DeepCreative 1.2.14, released 2026-09-28).
- Existing Linux projects cover only part of DeepCreative's LP360 features: numbers for CPU/GPU usage, temperature and power; custom patterns; video playback. None has Music Mode, emoji animations, Temperature/Power Guardian or the display settings. See [Research Summary](#research-summary).
- DeepCreative's LP360 page, read from the app's own UI code, offers five modes with sub-options plus several device settings. This set is the parity target (see [Core Capabilities](#core-capabilities-moscow)).

## Proposed Solution

Build OpenDeepCool in Rust, in two parts:

- **A background service** that starts at boot as a system service, before anyone logs in and the same for every user, and keeps the LP360 showing the configured mode with live data. It finds the PC's sensors itself, so it works on any PC with an LP360 (owner, 2026-10-23).
- **A GTK4 settings app** that follows the active Omarchy theme live, uses its own default theme where there is none, and offers every LP360 mode, option and effect in DeepCreative 1.2.14.

OpenDeepCool covers only the DeepCool cooler (owner, 2026-10-23): its display, and its fans and pump. It reads the CPU, GPU and RAM only for the values the display shows, and controls no other hardware. Users who want to monitor or control the CPU, GPU, other fans or power use dedicated software, as OpenRGB handles lighting.

What each mode draws comes from DeepCreative itself:

- its readable UI code;
- running the real app under Wine on this PC, connected to the real cooler, and recording exactly what it sends over USB;
- the owner confirming each mode on the physical display.

The alternatives are worse. A Windows VM or dual boot isn't available, and a VM would show fake sensor readings anyway. Forking an existing Linux tool would give only the frame format and simple number modes, and would bring that project's GPL license with it.

## Key Hypothesis

We believe a Rust boot service plus an Omarchy-themed GTK4 app that reproduces DeepCreative's LP360 modes, options and effects will fully replace DeepCreative for the owner.

We'll know we're right when two things are true:

- every item on the parity checklist (built from DeepCreative 1.2.14, checked by the owner in Phase 3) passes the unattended end-to-end run on the real LP360;
- after a cold boot, the display goes back to the configured mode with no action from the owner.

## What We're NOT Building

- **Speed control of the cooler's fans and pump in v1.** v1 only reads their speeds. They're DeepCool hardware, so controlling them may come after v1 (owner, 2026-10-23). It would need a driver that can write (the in-kernel driver for the board's sensor chip is read-only), and a bug could slow or stop the pump. Other fans stay out (see "Monitoring or controlling any hardware other than the cooler" below).
- **Creative Mode animations in v1.** DeepCreative 1.2.14's Creative Mode shows static 14×14 images only; its editor has no frames or timing. Animations may come after v1 (owner decision, 2026-10-22).
- **Hidden display settings and unused device reports in v1.** DeepCreative 1.2.14 stores `screenBrightness`, `screenStatus`, `noLoadScreen`, `rgbOptical` and `gyroStatus` for the LP360, but its LP page has no control for them, and Phase 1 saw no USB message for them. The LP360 also declares HID reports that DeepCreative never sent in Phase 1 ([LP360 notes](../../hardware/deepcool-lp360.md)). Supporting them may come after v1 (owner decision, 2026-10-22). It would first need research into what each one does; until then OpenDeepCool never sends them.
- **ARGB lighting.** The LP360's lighting cable plugs into the motherboard's 5V ARGB header (manual p. 16). OpenRGB, already installed, handles it.
- **Other DeepCool devices, other distributions or other operating systems.** OpenDeepCool supports only the LP360, on Arch Linux: Omarchy comes first, but any desktop works (owner, 2026-10-23). Its sensors aren't tied to the owner's PC (owner, 2026-10-23): it finds the CPU, GPU and board fan sensors of any PC. Only the owner's PC is tested on real hardware; the rules for other drivers are tested against fake sysfs trees built from their kernel documentation.
- **Following other desktops' themes in v1.** On a desktop other than Omarchy, the app uses OpenDeepCool's own default theme. Discovering other desktops' themes (for example Plasma, Pop!_OS, Ubuntu, and Hyprland setups such as HyDE, Caelestia or ML4W), and a setting that points the app at a theme's files, may come after v1 (owner, 2026-10-23).
- **Monitoring or controlling any hardware other than the cooler** (owner, 2026-10-23).
  - OpenDeepCool reads the CPU, GPU and RAM only for the values the display shows: usage, temperature, power and Memory Load. It shows no other readings, such as clocks, voltages or other fans.
  - It never changes a CPU, GPU, fan or power setting: no fan control except the cooler's own, after v1, and no power limits.
  - Users who want that use dedicated software, as OpenRGB handles lighting.
  - The one GPU setting chooses which GPU feeds the display, and appears only when the PC has more than one.
- **DeepCreative features outside the LP360 page:** the fan-control page, firmware updates, online auto-update, screen casting and LCD-device features.
- **Compatibility with DeepCreative's import/export file format, and a user-facing command-line tool.** Owner decision.
- **Any DeepCool code, installer files or raw USB captures in the repository.** The repository is public.

## Success Metrics

| Metric | Target | How Measured |
|---|---|---|
| Parity with DeepCreative | 100% of parity-checklist items pass | The unattended end-to-end run on the real LP360 checks every item against the expected frames, which match DeepCreative's captures the owner checked in Phase 3 |
| Starts with the system | Configured mode is on the display after every cold boot, with no action and before anyone logs in (proposed check: 5 consecutive cold boots) | An unattended boot test: after each boot, a check reads the service state, the journal and the recording of written frames. The Phase 12 plan decides how the power cycle is triggered without a person |
| Correct values | Displayed values match the system's own readings within each metric's resolution | Integration and e2e tests compare rendered values with the sensor sources and with `sensors` |
| Same timing as DeepCreative | Refresh interval and animation timing per mode match DeepCreative | Compare frame timestamps against the Wine capture |
| Tested | Unit, integration and e2e tests for each phase, where applicable | Test suites run locally and in CI; e2e on the real device, unattended |
| Resource use | TBD (the owner set no target) | Measure a baseline in Phase 7 (CPU %, memory) |

## Open Questions

- [x] Does DeepCreative 1.2.14 run under Wine, and can it reach the LP360 over USB through Wine? **Yes** (Phase 1 report, 2026-10-22). It needs Wine Mono and its sensor helper in console mode.
- [x] Which USB messages does DeepCreative send for the LP page's features besides "draw a 14×14 frame"? **Only the start-up command `03 08`** (2026-10-23): across all 8 captured app starts (106,075 messages), every message is output report 16, either `03 08` once at start-up or a frame. Rotation is drawn by the PC. What `03 08` means is unverified; OpenDeepCool sends it exactly as DeepCreative does ([protocol spec](../../spec/protocol.md)).
- [x] Is each pixel only on/off? **Yes**: a frame carries one bit per pixel (Phase 1 report). Brightness levels and colour are after v1, with the hidden settings.
- [ ] *After v1 (owner decision, 2026-10-22):* what the hidden settings `screenBrightness`, `screenStatus`, `noLoadScreen`, `rgbOptical` and `gyroStatus` do, and what the device reports DeepCreative doesn't use are for.
- [x] On the owner's board, which fan input is the pump and which is the CPU fan? **Answered** (2026-10-23): a load test, the driver's register layout and the owner's BIOS check agree. The mapping is in `docs/local/machine.md`. It serves only the owner's hardware checks: the program lets the user pick the inputs on any PC.
- [ ] Which signal tells a dedicated GPU from an integrated one, for each GPU driver? Phase 6 verifies it from primary sources; if a driver has none, the owner decides.
- [x] Theme: should the app install an Omarchy template in each user's `~/.config/omarchy/themed/`? **No** (2026-10-23): it reads the current theme's `colors.toml` directly, so nothing is installed per user, and uses its own default theme elsewhere (owner's theme requirements, Technical Approach → Theme).
- [x] Which metrics does each mode use? For example, does Power Guardian use CPU power, and is CPU clock used anywhere? **Answered** (2026-10-23): `display.md` and `music.md` map every mode. Power Guardian shows CPU Power and Gpu power; no mode shows the CPU clock, so OpenDeepCool doesn't read it.
- [x] Music Mode: how does DeepCreative turn audio into the spectrum? **Not copied** (owner decision, 2026-10-22). Under Wine its 14 columns cover only about 5–160 Hz; OpenDeepCool imitates Winamp's classic analyser over the whole audible range and keeps DeepCreative's layout ([display and music specs](../../spec/)).
- [x] What should the display show when the service is stopped or the PC is idle? **Answered** (owner, 2026-10-23): when the service stops, it just stops sending, and the cooler goes blank on its own after about 10 s (parity checklist P8); no extra message is sent. While the PC is idle, the display keeps showing live values, as DeepCreative does.
- [x] If Wine fails, can the main-process V8 bytecode (Electron 23.3.13) be decompiled? **Not needed** (2026-10-23): Wine worked (Phase 1), and watching DeepCreative's output with chosen inputs answered every Phase 3 question.
- [ ] How does the display service read CPU power? `energy_uj` became root-only after a reboot on 2026-10-22, and no rule was found that had made it readable before. Phase 6 decides how the service reads it ([sensors spec](../../spec/sensors.md#cpu-power), parity checklist S4).

---

## Users & Context

**Primary User**

- **Who**: The owner, a single Omarchy desktop user with a DeepCool LP360. Hardware details are in `docs/local/machine.md` (local only). Moved from Windows and has no Windows available.
- **Current behavior**: The LP360 display stays blank. Temperatures get checked with other tools, if at all.
- **Trigger**: The PC boots, or the owner wants a different mode or animation on the display.
- **Success state**: From boot, the display shows the mode configured in the app, with live values. Examples: CPU, GPU or CPU/GPU; temperature or usage %. Changing it takes a few clicks in an app that matches the Omarchy theme.

**Job to Be Done**

When my PC boots into Omarchy, I want the LP360 to show live stats and animations the way it did on Windows, and to switch modes from an app that matches my theme, so I can watch my system at a glance without Windows.

**Non-Users**

These can't be tested here, so the project doesn't target them: owners of other hardware or other DeepCool devices, users of other distributions, Windows and macOS users. The repository is public, but support covers this machine only.

---

## Solution Detail

### Core Capabilities (MoSCoW)

| Priority | Capability | Rationale |
|---|---|---|
| Must | Starts at boot and shows the configured mode with live data, before anyone logs in and for every user | Owner requirement: keeps working after every restart; a system service: owner, 2026-10-23 |
| Must | Talks to the LP360 over USB without root | The service runs as its own unprivileged system user |
| Must | Works on any PC: finds the sensors behind the display's values and the cooler's fan and pump inputs itself, and runs the board's sensor detection when the PC has none configured | Owner, 2026-10-23 |
| Must | Load Mode: Combined and Separate display with CPU usage, GPU usage and RAM used | DeepCreative LP360 options; the owner's MVP |
| Must | Temperature Guardian and Power Guardian | DeepCreative LP360 modes |
| Must | Music Mode: emoji animation + spectrum, temperature + spectrum, usage + spectrum | DeepCreative LP360 modes |
| Must | Creative Mode: draw 14×14 pixel images; save, import and export them (own file format) | DeepCreative LP360 mode. Animations come after v1 |
| Must | Display settings: Rotate Screen (one button, four quarter turns clockwise) and the global temperature unit (°C/°F) | What the spec found on DeepCreative's LP page and in its global settings ([settings spec](../../spec/settings.md)). Hidden settings such as brightness come after v1 |
| Must | The cooler's fan and pump speed readings, with the inputs chosen as in DeepCreative's "CPU Fan I/O" / "Pump I/O" | DeepCreative LP360 card and settings |
| Must | GPU choice: a dedicated GPU before an integrated one; a dropdown only when the PC has more than one GPU, set to the dedicated GPU at first; the choice kept across reboots | Owner, 2026-10-23 |
| Must | GTK4 settings app that follows the current Omarchy theme, including live theme changes, and uses its own default theme where it finds none | Owner requirement; default theme and other desktops: owner, 2026-10-23 |
| Could | Follow other desktops' themes, discovered automatically or from a theme folder the user sets, after v1 | Owner, 2026-10-23 |
| Must | Settings kept across reboots, one set for the whole PC | Owner requirement; the same for every user: owner, 2026-10-23 |
| Must | Clean install and uninstall, with every installed piece documented | Owner requirement |
| Must | Unit, integration and e2e tests for each phase, where applicable | Owner requirement |
| Should | Live preview of the 14×14 display in the app | Mode choice and Creative Mode work without looking at the pump |
| Should | CI build and test on GitHub | Keeps the public repo's main branch passing |
| Could | Speed control of the cooler's own fans and pump, after v1 | Owner: possible later; only DeepCool hardware (2026-10-23) |
| Could | Creative Mode animations (several frames played in sequence), after v1 | DeepCreative 1.2.14 shows only static images; owner: possible later (2026-10-22) |
| Could | Hidden display settings (brightness, screen on/off, no-load screen, `rgbOptical`, gyro) and the device reports DeepCreative doesn't use, after v1 | Owner: possible later (2026-10-22) |
| Won't | Lighting, other devices, monitoring or control of any hardware other than the cooler, DeepCreative file compatibility, user-facing CLI | See [What We're NOT Building](#what-were-not-building) |

### MVP Scope

After a reboot, with no action from the owner, the LP360 shows the configured Load Mode layout with live values. The layout is one of CPU, GPU or CPU/GPU, showing temperature or usage %. It is drawn the way DeepCreative draws it. At this stage the configuration can be a settings file; the settings app arrives in Phase 11.

### User Flow

1. The PC boots and the OpenDeepCool system service starts, before anyone logs in. It reads the saved settings, opens the LP360 and draws the configured mode at each refresh interval. If the PC has no sensor configuration yet, the board's sensor detection runs first, once per motherboard.
2. When someone logs in, their session's audio helper starts and feeds Music Mode. Until then, Music Mode shows silence.
3. To change the mode, any user at the PC opens OpenDeepCool from the launcher, picks a mode and options (with a preview), and applies. The service switches right away and saves the choice for the whole PC.

---

## Technical Approach

**Feasibility**: HIGH for USB communication, sensors, autostart and theming. MEDIUM for exact parity of every mode, until the Wine spike (Phase 2) settles it.

**Architecture Notes**

- **Rust workspace**, split by concern (names finalised in the plans):
  - protocol: frame encoding and checksum (pure code)
  - sensors
  - renderers: mode → 14×14 frames (pure code)
  - service: device I/O, scheduling, IPC
  - GTK4 settings app

  The pure parts can be unit-tested without hardware.
- **Device**: hidraw, through the `hidapi` crate or direct `/dev/hidraw` writes (chosen in the plan). The community projects document this "draw frame" message:
  - a 64-byte report with report ID `16`
  - header `104 1 5 29 1`
  - 28 pixel bytes: 14 columns × 2, odd rows then even rows, 7 bits each
  - checksum = sum of bytes 1–33 mod 256
  - terminator `22`

  We haven't verified it yet. Phases 2–3 confirm it and add any other messages.
- **Access without root**: a udev rule gives the display service's own group access to `3633:000c`. Only the sensor detection runs as root (Service model).
- **Sensors**, on any PC (owner, 2026-10-23; the rules per driver are in [sensors.md](../../spec/sensors.md#driver-rules)):
  - CPU temperature: the CPU's hwmon driver (`coretemp` on Intel, `k10temp` on AMD)
  - CPU usage: `/proc/stat`
  - CPU power: the powercap RAPL package energy counter (`energy_uj`). It's root-only, so an install-time change lets only the display service's account read it (Phase 6 decides how)
  - GPU temperature, usage and power: the chosen GPU's driver. `amdgpu` through hwmon and its DRM busy-percent file; NVIDIA through NVML, because its driver has no hwmon; other drivers where their documentation gives the reading
  - RAM: `/proc/meminfo`
  - The cooler's fan and pump RPM: the hwmon fan inputs the user assigns to them. lm-sensors' `sensors-detect` finds the driver for the board's sensor chip, and `lm_sensors.service` loads it at every boot (Service model)

  Nothing else is read: these are the display's seven values and the cooler's two speeds (owner, 2026-10-23).

  Look devices up by name or driver, never by hwmon/DRM index, because indexes change between boots.
- **Sensor architecture: ports and adapters** (owner, 2026-10-22). The display logic never touches files or drivers, so a driver change or new hardware means a new rule row, not new display code.
  - **Display logic** (modes, renderers, scheduler) asks for one set of readings per second and sends frames. It depends only on two ports: a readings port and a display port.
  - **Sensor registry:** scans hwmon, DRM, powercap and `/proc`, matches the driver rules, and binds one source per metric. It finds devices by name or driver on every scan and rescans when a read fails.
    - A metric with no matching source is reported as unavailable and logged, never drawn as a wrong number.
    - A `doctor` command prints which source each metric is bound to.
  - **Adapters**, each small and tested against fake sysfs trees:
    - one generic hwmon adapter for every hwmon driver (the kernel standardises hwmon file names: docs.kernel.org, hwmon sysfs interface);
    - procfs (CPU usage, memory);
    - powercap (CPU power);
    - DRM (GPU busy percentage);
    - on the display side, the LP360 hidraw adapter and a fake device.
  - **Driver rules** are data, built into the program, with no user override file. They cover the common drivers, each taken from its kernel documentation ([sensors.md](../../spec/sensors.md#driver-rules)). The kernel standardises file names but not which input means what, so a rule names a driver and its label. A driver without a rule leaves its metric unavailable.
  - **GPU choice:** the registry lists the PC's GPUs, ranks a dedicated GPU before an integrated one, and binds the chosen one ([sensors.md](../../spec/sensors.md#gpu-choice)).
  - **Fan inputs** need no rule: the user picks from every hwmon fan input, as in DeepCreative. The kernel documents that no standard ties an input to a board header.
  - **Writes:** only the LP360 is written, through the display port. Every other device is read-only: OpenDeepCool never changes a CPU, GPU, fan or power setting (owner, 2026-10-23). Speed control of the cooler's own fans and pump, after v1, would be one more port.
- **Service model** (owner, 2026-10-23: the hardware doesn't depend on who logs in, setting OpenDeepCool up for each user would be bad, and it must follow best security practice):
  - **Display service:** a systemd **system** service that starts at boot, before anyone logs in.
    - It runs as a dedicated unprivileged system user, never as root, sandboxed by systemd: for example a read-only system (`ProtectSystem=strict`), no access to home folders (`ProtectHome=`), `NoNewPrivileges=`, no capabilities, and a closed device policy that allows only the LP360's hidraw node and the GPU devices it reads. The Phase 12 plan sets the exact settings.
    - `ProtectKernelTunables=` makes `/sys` read-only for the service (systemd.exec), so it can't change any hardware setting, even by mistake.
    - That leaves it fewer rights than any user account. A user service would run with all the rights of whoever logged in.
    - It restarts on failure and reconnects if the USB device disappears.
  - **Sensor detection:** a separate one-shot unit, the only part that runs as root.
    - At boot, when the PC has no lm-sensors configuration (`/etc/conf.d/lm_sensors`), it runs `sensors-detect --auto`, at most once per motherboard.
    - That writes the configuration and enables `lm_sensors.service`, which loads the detected drivers at every boot.
    - An existing configuration is never overwritten.
  - **Audio helper:** audio belongs to each user's session. A small systemd user unit, enabled for all users at install (`systemctl --global enable`), captures the session's audio and passes it to the display service. The service accepts it only from the active local session. Nobody sets it up by hand.
- **Music Mode**: the audio helper captures the monitor of the default audio output through PipeWire; the plan decides whether the FFT runs in the helper or the service, and chooses the crates. The spectrum imitates Winamp's classic analyser: its bands span the whole audible range (user reports on the Winamp forums: tones up to about 15 kHz still show at its right edge), here across 14 columns. How its bars and peaks fall is unverified; the owner decides it in the Phase 9 plan, and the tests then check it automatically. The bars' size and position follow DeepCreative's layout.
- **Settings app ↔ service**: IPC to the system service, either the D-Bus system bus or a Unix socket under `/run` (chosen in the plan).
  - polkit authorizes changes. A user in an active local session changes settings without a password; remote and inactive sessions may only read them (polkit `allow_active`; owner, 2026-10-23).
  - Settings live in the service's state directory: one set for the whole PC.
- **Theme** (owner, 2026-10-23: follow the current system theme; work on other desktops; no per-user setup):
  - **Omarchy:** the app takes its colours from the current theme's `~/.local/state/omarchy/current/theme/colors.toml`, which holds the semantic colours and the light/dark `mode`. It installs no template, so nothing goes into users' home folders.
  - **Live changes:** a theme switch builds the new theme in a staging folder, then moves it over `current/theme` (Omarchy's `omarchy-theme-set` script, read 2026-10-23). So the app watches the `current` folder, not a single file, and reloads its colours when the theme folder is replaced.
  - **Opacity and blur** (owner, 2026-10-23: the app takes the theme's opacity, blur and so on):
    - Hyprland applies them to every window: Omarchy's `default/hypr/windows.lua` tags all windows for a default opacity (0.985 active, 0.96 inactive when read), and `default/hypr/looknfeel.lua` sets the blur. The app gets both by being an ordinary window, so it never sets its own window opacity, and users' own window rules still apply.
    - For its own surfaces, the app takes the alpha values of the current theme's `shell.toml`: background, and the hover, focus, selected and pressed fill and border alphas that Omarchy's shell uses.
    - Evidence: system: Omarchy's Hyprland defaults and the current theme folder, read 2026-10-23. Which `shell.toml` keys map to which widget is decided in the Phase 11 design.
  - **Elsewhere**, or when that file is missing or unreadable, the app uses OpenDeepCool's own default theme, designed with the screens in Phase 11.
  - **After v1, other desktops:** the XDG Desktop Portal's Settings interface gives a desktop's light/dark preference (`color-scheme`) and accent colour (`accent-color`), with a `SettingChanged` signal, on desktops that implement it. Setups without a standard need a theme folder that the user picks.
- **Build and run**: `cargo build --release` produces native binaries, so running them needs no Rust toolchain. They link to the system GTK4, which Omarchy already ships.
- **Packaging**: a local PKGBUILD, so pacman tracks every installed file and `pacman -R` removes them all (proposed; decided in the Phase 12 plan).
- **Reverse-engineering environment** (development only, never shipped):
  - DeepCreative 1.2.14 is extracted from the owner's installer and run under Wine (11.17 in Phase 1, 11.20 in Phase 3) in its own prefix, with networking blocked.
  - Its Electron build honours Node inspector arguments (fuse `EnableNodeCliInspectArguments`), so a small hook can log every USB write for each mode.
  - `usbmon` and `strace` serve as cross-checks.
  - Raw captures stay local, outside the repository.

**Testing Strategy** (owner's requirement, 2026-10-22: every combination is tested automatically, not by hand)

The LP360 only shows the 14×14 picture each message carries; mode, metric, unit, rotation and scrolling are all computed on the PC. So every combination can be checked automatically by comparing frames. The owner's eyes proved, once, in Phase 3, that DeepCreative's bytes show the intended pictures; OpenDeepCool sending the same bytes carries that proof over. From Phase 5 on, no test needs a person (owner, 2026-10-24: discovery needed the owner, implementation doesn't).

- **Test matrix:** every mode × the four rotations × °C/°F where a temperature is shown, scrolling included: 116 combinations, each with several values (Phase 3 plan, Test matrix). In every case each metric gets its own distinct, non-zero value, so a mode that draws the wrong metric fails.
- **Expected frames:** `docs/spec/frames/` holds the expected frame of every matrix case as a text picture. Phase 3 generates it from the spec's rules and confirms it against DeepCreative's captures. It's the oracle for the layers below, so Rust output is never compared with Rust output.
- **Layer 1, unit (CI):** drawing is one pure function of mode, readings, unit, rotation and scroll tick. Tests run every matrix case against the expected frames; a failure prints both pictures. Property tests add random inputs for these rules:
  - four quarter turns give back the original;
  - drawing with a rotation equals rotating the finished frame;
  - each scroll tick shifts the picture one column;
  - each value rule matches the spec's table.

  Crates are chosen in the Phase 5 plan.
- **Layer 2, differential (local):** re-encodes every captured DeepCreative message and expects identical bytes (Phase 5), and replays every captured DeepCreative sweep step from `research/` through the Rust renderer and expects 0 differing pixels. The captures are git-ignored, so this runs on the owner's PC before each mode's commit, not in CI.
- **Layer 3, integration (CI):** the service runs against a fake device, a fake clock, fake sensor files and a fake audio source. The fake sensor trees cover every driver rule, each built from its driver's kernel documentation, plus PCs with one GPU, several GPUs, and no board fan sensors. Decoding what it sent checks:
  - message format and checksum;
  - frame timing and the 100 ms scroll tick;
  - mode switches, and settings such as rotation changed while it runs;
  - reconnecting after the device disappears.
- **Layer 4, end to end on the real LP360:**
  - **automated:** the service drives the real cooler through the whole matrix, records every report it actually wrote, and compares them with the expected frames. Unattended.
  - **no person (rule 3):** nobody looks at the cooler. Layer 2's byte-for-byte match with the captures the owner checked in Phase 3 proves bytes → picture and orientation.
- **Music Mode spectrum:** OpenDeepCool's own analysis, so it isn't compared with DeepCreative. Synthetic tones must light the columns the Phase 9 design predicts, and every spectrum frame must follow the layout rules (solid bars from the bottom, bottom row lit, height limits) in every rotation.
- **When they run:** layers 1 and 3 on every push (GitHub Actions) and in the pre-commit guard from Phase 5; layers 2 and 4 before each mode's commit (rule 4).

**Technical Risks**

| Risk | Likelihood | Mitigation |
|---|---|---|
| DeepCreative won't start under Wine (Electron 23, its Windows services, WebView2, .NET helpers) | M | Run `DeepCool.exe` directly, without the installer; try Chromium flags (`--no-sandbox`, `--disable-gpu`) and winetricks components; then Proton. Last resort: static analysis plus recreation guided by the owner |
| Wine can't reach the LP360 over USB | M | Log frames with a hook inside the app; present a virtual HID device (Linux `uhid`) with the LP360's IDs; use `usbmon` to confirm |
| Main-process logic is compiled V8 bytecode | H (certain) | Capture at runtime under Wine; decompile only if needed |
| DeepCreative tries to update itself or the firmware | L | Network blocked during the Wine run; decline any prompt |
| Wrong fan input chosen as the cooler's pump or fans | L | No default: the user picks both inputs, as in DeepCreative, and the settings app shows each input's live RPM |
| CPU power counter is root-only (kernel fix for CVE-2020-8694) | H (certain) | An install-time change lets only the display service's account read it; Phase 6 decides how |
| `sensors-detect` probes hardware; lm-sensors warns that its SMBus probing "has been reported to cause trouble on some systems" | L | Runs only when the PC has no lm-sensors configuration, once per motherboard; never overwrites a configuration; its output goes to the journal |
| A driver behaves differently from its documentation on hardware we can't test | M | Rules come from kernel docs and are tested with fake trees; a reading that fails shows as unavailable, never as a wrong number; the spec marks such rules "not tested on hardware" |
| No reliable way to tell a dedicated GPU from an integrated one for some driver | M | Phase 6 verifies a signal per driver from primary sources; where none exists, the owner decides |
| hwmon/DRM index changes between boots | H | Look up by name or driver, never by index |
| Omarchy changes where or how it stores the current theme | L | The app falls back to its default theme and logs why; the spec records which files it reads |
| Automated e2e tests need hardware or root (`/dev/uhid` is root-only) | M | Fake transport in CI; the automated real-device run (Testing Strategy, layer 4) uses the udev rule's group, so it needs no root and no person |

---

## How Agents Work on This Project

This project spans many sessions and possibly several coding agents, so the way they work is part of the product plan, not an afterthought. The instruction files are drafted now and committed in Phase 2.

- **`AGENTS.md`** is the single rulebook for every coding agent. It holds:
  - the start-of-session checklist
  - the working method
  - the owner's rules: never guess; ask as soon as in doubt; testing needs no person; small commits; tests with every step; no DeepCool material in the repo; no undocumented system changes; v1 scope limits
  - verified facts and the repository layout
  - It stays under 200 lines, as Claude Code's docs recommend.
- **`CLAUDE.md`** starts with `@AGENTS.md`, so Claude Code loads the same rules at every session start. It then adds Claude-only notes: which PRP commands to use, the REA skill for reverse engineering, and Context7 for library docs.
- **Source-of-truth chain**, which agents read in this order:
  1. this PRD (scope, decisions, and the phase table as the only progress record)
  2. the current phase's plan in `docs/prp/plans/`
  3. the spec in `docs/spec/`
  4. the latest report in `docs/prp/reports/`
  5. git history
- **Start-of-session checklist**: read the PRD, find the current phase, open its plan and latest report, check `git status`/`git log`, and tell the owner the phase and step before changing anything.
- **One implementation, one commit** (owner's rule, 2026-10-22): each display mode and each combination, e.g. Load Mode → Combined CPU/GPU, gets its own implement → test (unattended end-to-end run on the cooler included) → exactly one commit. Several modes are never batched together.
- **Privacy** (owner's rule, 2026-10-22):
  - Nothing personal or specific to the owner's PC goes into git: no names or emails, serials, hardware models, BIOS/firmware, OS versions, login or disk setup, local paths.
  - Machine facts the implementation needs live in one git-ignored file, `docs/local/machine.md`. `AGENTS.md` references it, and `CLAUDE.md` imports it so agents always see it.
  - The commit guard rejects any commit containing the sensitive values listed in that file, and any commit made without the GitHub no-reply identity.
- **Staying on track**:
  - Work only on the current plan step.
  - Re-read the step after a context compaction or long pause.
  - If a plan is wrong, stop, ask the owner, and fix the document first.
  - Update the phase table whenever a phase changes state.
- **Enforcement**: Claude Code's docs say instruction files are context, not enforced configuration. A git pre-commit guard therefore backs up the critical rules for any agent or person:
  - from Phase 2, it rejects DeepCool material and oversized files;
  - from Phase 5, it also runs format, lint and tests.

---

## Implementation Phases

<!--
  STATUS: pending | in-progress | complete
  PARALLEL: phases that can run concurrently (e.g., "with 3" or "-")
  DEPENDS: phases that must complete first (e.g., "1, 2" or "-")
  PLAN: link to generated plan file once created
  REPORT: link to the implementation report once implemented
  PR: link to the pull request once opened
-->

| # | Phase | Description | Status | Parallel | Depends | Plan | Report | PR |
|---|-------|-------------|--------|----------|---------|------|--------|----|
| 1 | Wine reference spike | Run DeepCreative under Wine with the real LP360 and log what it sends; the result may change this plan | complete | - | - | - | [report](../../research/phase1-wine-spike.md) | - |
| 2 | Repository bootstrap | git, README, MIT license, .gitignore, agent instructions (AGENTS.md, CLAUDE.md), commit guard, PRD and research docs, public GitHub repo | complete | - | 1 | - | - | - |
| 3 | Functionality & protocol spec | Full spec of LP360 + DeepCreative + board sensors; parity checklist validated by the owner | complete | - | 1, 2 | [plan](../plans/phase-3-functionality-protocol-spec.plan.md) | [report](../reports/phase-3-functionality-protocol-spec.report.md) | - |
| 4 | Implementation plans | One PRP plan per implementation phase (5–12) | pending | - | 3 | - | - | - |
| 5 | Device & protocol core | Rust workspace, frame encoder, LP360 USB writer, udev rule, test patterns | pending | with 6 | 4 | - | - | - |
| 6 | Sensors | All metrics on any PC: driver rules, GPU choice, fan inputs, sensor detection; tested against fake sysfs trees | pending | with 5 | 4 | - | - | - |
| 7 | Service, settings & Load Mode (MVP) | System service, start at boot, saved settings, Load Mode layouts | pending | - | 5, 6 | - | - | - |
| 8 | Guardian modes | Temperature Guardian, Power Guardian | pending | with 9, 10, 11 | 7 | - | - | - |
| 9 | Music Mode | Audio capture, spectrum, the three Music Mode variants | pending | with 8, 10, 11 | 7 | - | - | - |
| 10 | Creative Mode | Pixel art and animation engine, storage, import/export | pending | with 8, 9, 11 | 7 | - | - | - |
| 11 | Settings app | GTK4 app, Omarchy theme and default theme, all settings screens, preview, pixel editor | pending | with 8, 9, 10 | 7 | - | - | - |
| 12 | Packaging & final validation | Install/uninstall that removes everything, footprint docs, release build, full unattended parity run | pending | - | 8, 9, 10, 11 | - | - | - |

### Phase Details

**Phase 1: Wine reference spike** (moved first by the owner on 2026-10-22: "test the deepcool with wine before doing anything. If it doesn't work it will change our plan")
- **Goal**: Prove or disprove that DeepCreative, running under Wine on this PC, can drive the real LP360, and that we can record what it sends.
- **Scope**:
  - run `DeepCool.exe` from the extracted files, without installing, in a dedicated Wine prefix with the network blocked
  - attach a logger to its USB HID calls
  - the owner confirms on the cooler what the display shows
  - capture at least one mode end to end
  - verdict: PROVEN, DISPROVEN or CONDITIONAL
- **Success signal**: A verdict backed by evidence. If proven, the logged frames for at least one mode match what the owner sees on the display.

**Phase 2: Repository bootstrap**
- **Goal**: Public repository holding the project's first state (owner's workflow step 1).
- **Scope**:
  - `git init`
  - README covering purpose, status and "not affiliated with DeepCool"
  - MIT LICENSE
  - `.gitignore` that excludes DeepCool files, captures, the Wine prefix and build output
  - agent instructions:
    - `AGENTS.md` holds the rules for every coding agent, plus the start-of-session checklist
    - `CLAUDE.md` imports it with `@AGENTS.md` and adds Claude Code specifics
  - a git pre-commit guard that rejects DeepCool material (installer, extracted app files, captures) and oversized files (proposed; it gains build/test/lint checks in Phase 5)
  - this PRD and the research notes under `docs/`
  - create the public GitHub repository and push
- **Success signal**: The repository is visible on GitHub with the docs, and nothing from DeepCool is committed.
- **Done 2026-10-22**: <https://github.com/dtmarangoni/OpenDeepCool> (public, branch `main`). The guard was tested by force-adding a vendor PDF, which it rejected.

**Phase 3: Functionality & protocol spec**
- **Goal**: A complete document of what DeepCreative does with the LP360 and the board (owner's workflow step 7).
- **Scope**:
  - every mode, option and effect, with captured frames and timing
  - every USB message
  - the sensor source for each metric
  - the fan input mapping on this board, confirmed with the board's manual and a check by the owner
  - the parity checklist, with each item validated by the owner on the device
- **Success signal**: The owner signs off the parity checklist as complete and correct.
- **Done 2026-10-23**: the owner signed off the [parity checklist](../../spec/parity-checklist.md) (73 rows). Outputs are in [`docs/spec/`](../../spec/): protocol, display (full test matrix), Music Mode, sensors, settings, expected frames, checklist. Report: [phase-3-functionality-protocol-spec.report.md](../reports/phase-3-functionality-protocol-spec.report.md).
  - Found in the final checks: after a static mode, DeepCreative replays the paused text of the last scrolling mode, with stale readings, before the new text. OpenDeepCool never does, and the transition tests of Phases 7–8 check it ([display spec](../../spec/display.md#mode-switches)).
  - Owner decisions on 2026-10-23: sensors work on any PC, a sandboxed system service, automatic sensor detection, the GPU choice rules, the cooler-only scope, and the display going blank on its own when the service stops.
  - Still open: `03 08`'s meaning is unverified (sent as DeepCreative does).

**Phase 4: Implementation plans**
- **Goal**: An implementation-ready plan for each of phases 5–12 (owner's workflow step 9).
- **Scope**: `/prp-plan` for each phase. Each plan includes its tests in the four layers of the Testing Strategy, with the matrix cases it covers, and the owner's validation steps.
- **Success signal**: Plans committed; each one lists files, tests and acceptance checks.

**Phase 5: Device & protocol core**
- **Goal**: Reliable frames to the LP360 without root.
- **Scope**: the workspace; frame encoder and checksum; a USB writer that reconnects; the udev rule; a developer test-pattern binary (not a user feature).
- **Tests**:
  - unit: encoding, checksum, rotation (property tests); the expected-frame loader and picture diff the later phases reuse
  - integration: fake transport, optionally a virtual `uhid` device
  - differential: every captured DeepCreative message re-encoded byte for byte
  - e2e, unattended: test patterns written to the real cooler in all four rotations, every written report recorded and compared with the expected frames
  - CI: GitHub Actions and the pre-commit guard run layers 1 and 3
- **Success signal**: All test layers pass unattended, on the real cooler, without root.

**Phase 6: Sensors**
- **Goal**: Read every metric the display shows, correctly, on any PC, as the display service's unprivileged account.
- **Scope**:
  - CPU temperature, usage and power, on Intel and AMD
  - GPU temperature, usage and power of the chosen GPU: `amdgpu`, NVIDIA through NVML, and other drivers where their documentation gives the reading
  - the GPU choice: telling dedicated from integrated GPUs (verified per driver), the priority rule, the saved choice
  - RAM
  - the cooler's fan and pump RPM, from the hwmon fan inputs the user assigns
  - the sensor detection unit (`sensors-detect` when the PC has no lm-sensors configuration)
  - the install-time change that lets the service's account read the CPU power counter
- **Tests**:
  - unit: parsers, rule matching, GPU ranking
  - integration: fake sysfs and proc trees for every driver rule, one and several GPUs, missing and failing readings
  - e2e: values compared against `sensors` on the owner's PC
- **Success signal**: Values match the system tools within each metric's resolution on the owner's PC, and every rule passes its fake-tree tests.

**Phase 7: Service, settings & Load Mode (MVP)**
- **Goal**: Deliver the MVP.
- **Scope**:
  - systemd system service (dedicated user, sandboxed) and the settings file in its state directory
  - Load Mode layouts as defined in the spec: Full or Split Screen, Combined or Separate display; CPU and GPU temperature and usage; RAM
  - refresh timing matching DeepCreative
- **Tests**:
  - unit: renderers against the expected frames for every Load Mode matrix case
  - differential: DeepCreative's Load Mode captures, 0 differing pixels
  - integration: service → fake device
  - e2e, unattended: the matrix run on the cooler; boot → configured mode, checked from the service state and the recorded frames
- **Success signal**: After cold boots, the configured mode appears with no action.

**Phase 8: Guardian modes**
- **Goal**: Temperature Guardian and Power Guardian as defined in the spec.
- **Scope**: renderers, thresholds and timing from the spec.
- **Tests**:
  - unit: renderers against the expected frames for every Guardian matrix case, at and around thresholds
  - differential: DeepCreative's Guardian captures, 0 differing pixels
  - integration: service with simulated sensor values
  - e2e, unattended: the matrix run on the cooler
- **Success signal**: These modes' checklist items pass.

**Phase 9: Music Mode**
- **Goal**: The three Music Mode variants.
- **Scope**: the audio helper and its link to the service, PipeWire capture, FFT, renderers.
- **Tests**:
  - unit: spectrum from synthetic signals; emoji and value areas against the expected frames in every rotation and unit; spectrum layout rules
  - differential: DeepCreative's Music Mode captures, top areas only
  - integration: audio file → frames
  - e2e, unattended: the matrix run on the cooler, with test audio played through a virtual output
- **Success signal**: These modes' checklist items pass, and the measured delay from sound to frame is within the target the Phase 9 plan sets with the owner.

**Phase 10: Creative Mode**
- **Goal**: Pixel art and animations.
- **Scope**: frame and animation model, storage, playback timing, import/export in our own format.
- **Tests**:
  - unit: model; saved images against the expected frames in every rotation
  - integration: save and load
  - e2e, unattended: the matrix run on the cooler, with images saved through the service
- **Success signal**: These modes' checklist items pass.

**Phase 11: Settings app**
- **Goal**: A GTK4 app that follows the Omarchy theme, with its own default theme elsewhere, and exposes every option.
- **Scope**: screens for all modes and settings, including the GPU dropdown (only when the PC has more than one GPU) and the cooler's fan and pump pickers with live RPM; cooler and service status; preview; pixel editor; the Omarchy theme with live reload, and the default theme; IPC client with polkit authorization ([settings.md](../../spec/settings.md#what-the-settings-app-must-offer)).
- **Design** (owner, 2026-10-22): feature parity with DeepCreative, not interface parity. The `ui-ux-designer` agent (`.claude/agents/ui-ux-designer.md`) designs the screens and mockups before the plan is written: modern, minimalist, clean, fast. DeepCreative's tabs and its Combined/Separate split aren't required; every mode must stay reachable. The app covers only the cooler, which keeps it small (owner, 2026-10-23).
- **Tests**:
  - unit: view models
  - integration: app ↔ service IPC
  - e2e, unattended: automated UI tests switch themes and modes, and start the app with no Omarchy theme
- **Success signal**: The app follows `omarchy-theme-set` live, shows the default theme when it finds no Omarchy theme, and every setting is reachable.

**Phase 12: Packaging & final validation**
- **Goal**: Clean install and uninstall, plus full parity sign-off.
- **Scope**: PKGBUILD (proposed) with the system user, units, udev rule and polkit policy; install and uninstall docs listing every file; release build; a full unattended checklist run; an unattended boot test, also with nobody logged in; an uninstall that removes everything (System Footprint).
- **Tests**:
  - integration: install → uninstall in a throwaway system leaves it exactly as before: files, users, groups, udev rules, enabled units, permissions and saved data
  - e2e, unattended: fresh install → reboot → configured mode shows, checked from the service state and the recorded frames
- **Last step of v1** (owner, 2026-10-23): delete DeepCreative completely from the owner's PC: the Wine prefix (`research/wine/`), the extracted app, the captures, the static-analysis extracts and the research tools that only work with it (all in `research/`), and the installer copy. The owner confirms the list before anything is deleted.
- **Success signal**: The checklist is at 100%, uninstalling leaves nothing behind, and no DeepCreative material is left on the owner's PC.

### Parallelism Notes

Phases 1–4 run in sequence because each feeds the next. Phases 5 and 6 touch separate code (device and sensors) and can run in separate worktrees. After the MVP (Phase 7), the remaining mode families (Phases 8, 9 and 10) are independent renderers. The settings app (Phase 11) can be built against the settings schema from Phase 7, so all four can run in parallel worktrees.

---

## System Footprint

The owner asked for every installed or created item to be listed. Paths are proposed and get finalised in the Phase 12 plan.

**Uninstalling removes everything** (owner, 2026-10-24). pacman removes the files the package installed. It doesn't remove what was created after install: accounts (`pacman -R` runs no `userdel`; Arch forum, unverified until the Phase 12 test), saved data, enable links. The package's removal script removes those, in the "Removed by" column below. An automated test installs and uninstalls in a throwaway system and requires it to end exactly as it started. Only two things outlast the uninstall: a sensor driver loaded by lm-sensors stays loaded until the next reboot, and old journal lines stay until the journal rotates them (they can't be deleted without deleting everyone's logs).

**Finished app**

| Item | Path (proposed) | Why | Removed by |
|---|---|---|---|
| Service and settings-app binaries | `/usr/bin/` | The app | `pacman -R` |
| System user and group | `/usr/lib/sysusers.d/opendeepcool.conf` | The unprivileged account the display service runs as | `pacman -R` removes the file; the removal script deletes the account and group |
| udev rule | `/usr/lib/udev/rules.d/70-opendeepcool.rules` | LP360 access for the service's group, plus read access to the CPU power counter if Phase 6 chooses a rule for it | `pacman -R`; the removal script reloads udev so the access ends at once, not at the next reboot |
| systemd system unit | `/usr/lib/systemd/system/opendeepcool.service`, enabled at install | Starts the display service at boot | The removal script stops and disables it; `pacman -R` |
| Sensor detection unit | `/usr/lib/systemd/system/opendeepcool-sensors-detect.service` (one-shot, runs as root) | Runs `sensors-detect --auto` when the PC has no lm-sensors configuration | The removal script disables it; `pacman -R` |
| lm-sensors | the `lm_sensors` package, as a dependency | Provides `sensors-detect` and `lm_sensors.service` | `pacman -Rs`, if nothing else needs it |
| lm-sensors configuration | `/etc/conf.d/lm_sensors`, with `lm_sensors.service` enabled | Written by `sensors-detect`; loads the board's sensor drivers at every boot | The removal script, only if OpenDeepCool's detection created them; a configuration that existed before is never touched |
| Audio helper | `/usr/lib/systemd/user/opendeepcool-audio.service`, enabled for all users (`systemctl --global enable`) | Music Mode audio from the active session | The removal script (`systemctl --global disable`); `pacman -R` |
| polkit policy | `/usr/share/polkit-1/actions/` (file name set in the plan) | Lets users at the PC change settings | `pacman -R` |
| D-Bus policy, if the plan picks D-Bus | `/usr/share/dbus-1/system.d/` | Lets the settings app reach the service | `pacman -R` |
| Desktop entry and icon | `/usr/share/applications/`, `/usr/share/icons/` | Launch from the app menu, on Omarchy or any desktop | `pacman -R` |
| Settings and Creative Mode files | `/var/lib/opendeepcool/`, the service's state directory | One set of choices for the whole PC | The removal script |
| Detection record | the same state directory | Which motherboard the sensor detection last ran for | The removal script |
| Logs | System journal | Diagnostics | journald rotation |

**Development and research only (never shipped)**

| Item | Where | Notes |
|---|---|---|
| Rust toolchain | mise (already installed) | Build only |
| Wine 11.20 (11.17 in Phase 1), winetricks | Already installed | Phases 1–3 |
| Wine prefix for DeepCreative | `research/wine/` in the project (git-ignored) | Kept until v1 is finished, in case research is needed again; then deleted with all DeepCreative material (owner, 2026-10-23; Phase 12) |
| Extracted DeepCreative files and captures | `research/` in the project (git-ignored) | Never committed; deleted at the end of v1 (Phase 12) |
| Board sensor driver loaded by hand | Kernel, until the next reboot | Loaded with `sudo` when a research task needs it (first on 2026-10-22) |
| Temporary access to the LP360 device node | ACL granted by the owner with `sudo setfacl` for a research session | Gone at reboot or re-plug; first granted 2026-10-22 |

---

## Decisions Log

| Decision | Choice | Alternatives | Rationale |
|---|---|---|---|
| Language | Rust | — | Owner requirement |
| Source of truth for DeepCreative behaviour | Readable UI code + DeepCreative under Wine with the real cooler + owner checks | Windows VM; dual boot; Linux tools only | No Windows is available; a VM shows fake readings; Wine runs the real app against the real cooler on this PC |
| Wine or Proton | Plain Wine 11.17 (installed) | Proton / protontricks | Proton targets Steam games; kept as a fallback |
| Existing Linux projects | Learn the protocol from them; write our own code | Fork Nortank12/deepcool-digital-linux | They cover a small part, and copying their code brings GPL with it |
| License | MIT | GPL-3.0 | Owner decision |
| GUI toolkit | GTK4 (gtk4-rs) | Iced, Slint, egui, libadwaita | Omarchy already ships GTK4 and themes a Rust GTK4 app |
| Theme | Omarchy's current `colors.toml` read directly and followed live; OpenDeepCool's own default theme elsewhere; other desktops after v1 | An Omarchy template in each user's `~/.config` (the 2026-10-22 plan) | Owner, 2026-10-23: follow the system theme, run on other desktops, no per-user setup |
| Service model | systemd system service as a dedicated unprivileged user, sandboxed; a root one-shot unit only for sensor detection; an audio helper enabled for every user | User service in each desktop session (the 2026-10-22 choice); system service running as root | Owner, 2026-10-23: the hardware doesn't depend on who logs in, and per-user setup would be bad. A sandboxed unprivileged service has fewer rights than a user account, which is best practice; root only where the detection needs it |
| Fan and pump readings | Any hwmon fan input, picked by the user; the board's driver found by `sensors-detect`, run automatically when the PC has no lm-sensors configuration | A fixed module for the owner's board (the 2026-10-22 choice); out-of-tree DKMS drivers | Owner, 2026-10-23: must work on any PC. The kernel documents that no standard ties an input to a header, so the user picks, as in DeepCreative |
| Product scope | Only the DeepCool cooler: its display, and its fans and pump in the app. CPU, GPU and RAM are read only for the display's values; no other hardware is monitored or controlled | A general hardware monitor or control tool | Owner, 2026-10-23: keeps OpenDeepCool and its app small; dedicated tools exist, as OpenRGB for lighting |
| Fan control | Not in v1; after v1 only for the cooler's own fans and pump | In v1; control of other fans | Owner decisions 2026-10-22 and 2026-10-23; needs a driver that can write |
| Creative Mode animations | Not in v1 | In v1 | DeepCreative 1.2.14's editor has no frames or timing (LP page code; Phase 1 saw static images only); owner decision 2026-10-22: possible later |
| Hidden display settings and unused device reports | Not in v1; documented, never sent | Research them in Phase 3 | Owner decision 2026-10-22: out of scope for now, kept as a possible later feature |
| Lighting | Out of scope (OpenRGB) | Include it | Wired to the motherboard ARGB header |
| Music Mode spectrum | Imitate Winamp's classic analyser (whole audible range); DeepCreative's layout kept | Copy DeepCreative's analysis | Owner decision 2026-10-22: under Wine DeepCreative's columns cover only about 5–160 Hz and show a second pattern at half the frequency, which may be a Wine artefact; nobody remembers its behaviour on Windows |
| Settings app design | Our own design by a senior UI/UX designer agent; every DeepCreative feature reachable, its layout not copied | Mirror DeepCreative's pages, tabs and Combined/Separate options | Owner decision 2026-10-22: a modern, minimalist, fast app; DeepCreative's structure has unnecessary steps |
| Sensor architecture | Ports and adapters: display logic behind a readings port; a registry binds one source per metric from built-in driver rules | Display code reading sysfs directly; a user-editable rules file | Owner decision 2026-10-22: a driver change or new hardware must not touch display code; rules stay built in and tested |
| Hardware coverage | Any PC with an LP360 on Arch Linux, Omarchy first: rules for the common CPU, GPU and board-sensor drivers; a driver without a rule leaves its metric unavailable | Only the owner's PC | Owner, 2026-10-23 |
| GPU | Only the GPU metrics the display shows. A dedicated GPU before an integrated one; a dropdown only when the PC has more than one GPU, set to the dedicated one at first; the choice kept across reboots | DeepCreative's GPU list on every PC; wider GPU monitoring | Owner, 2026-10-23: the cooler needs no GPU monitoring beyond its display |
| Who may change settings | Users in an active local session, without a password (polkit `allow_active`); remote and inactive sessions only read | Any local user; an admin password for every change | Owner, 2026-10-23: best practice for settings shared by the whole PC |
| Driver names in the repository | Allowed for every driver the rules cover (names, module names, sensor labels) in code, tests, packaging and spec; part models and other machine facts still banned | Naming only the owner's drivers | Owner decisions 2026-10-22 (the kernel doesn't standardise what each input means, so rules must name the driver; AGENTS.md rule 10 exception) and 2026-10-23 (sensors work on any PC) |
| Metric shown by each mode | Always the metric the mode names (CPU as CPU, GPU as GPU), everywhere in the implementation | Copy DeepCreative's swaps | Owner decision 2026-10-22, after DeepCreative was found drawing the GPU temperature in Music Mode → Temperature + Spectrum → CPU in °F (mode 18) |
| Testing | Every combination tested automatically against expected frames, in four layers (unit, differential, integration, end to end); from Phase 5 on, no test needs a person (Testing Strategy) | Owner checks every combination by hand; owner checks test patterns and samples (the 2026-10-22 choice) | Owner requirements, 2026-10-22: by hand would mean hundreds of checks, repeated after every change; 2026-10-24: discovery needed the owner, implementation doesn't |
| Uninstall | Removes everything OpenDeepCool installed or created: files, the system user and group, udev rules, enabled units, permissions, settings and saved images; proven by an automated test | Keep the account or the saved data | Owner, 2026-10-24 |
| PRP documents | Kept in the repository under `docs/prp/` (the PRP store under `~/.prp/` links there) | PRP default store outside the repo | Owner wants the PRP documents in git |
| DeepCool material | Never committed; captures stay local | — | Copyright; the repository is public |
| Agent instructions | `AGENTS.md` is the single source; `CLAUDE.md` imports it (`@AGENTS.md`) and adds Claude-only notes; critical rules also enforced by a git pre-commit guard | `CLAUDE.md` only; two separate copies | Works for any coding agent, avoids drift between two copies, and follows Claude Code's documented import pattern. Instruction files are context, not enforcement, so the guard backs up the critical rules |

---

## Research Summary

**Market Context**

| Project | Language / license | LP360 coverage | Last activity |
|---|---|---|---|
| Nortank12/deepcool-digital-linux | Rust, GPL-3.0, CLI | "Partially supported": CPU/GPU usage, temperature and power as numbers (one or two values), rotation; documents the frame format | v0.11.0-alpha, 2026-08-31 (427 stars) |
| Blaster4385/deepcool-display-linux | Go + Wails GUI, GPL-2.0 | Custom patterns (CSV export), CPU temperature and usage | 2025-01-13 |
| TP-Di/bad-apple-lp360 | Python, no license | Plays a video on the display | 2026-02-28 |
| XusMochizuki/DeepCool-LP360-RustDriver | — | Empty repository | 2026-04-30 |

Other projects (philling-dev/deepcool-digital-linux, RahnRazamai/deepcool_digital_dart, Algorithm0/deepcool-digital-info) target DeepCool digital devices. Their LP360 coverage wasn't checked.

**DeepCreative 1.2.14** (static analysis of the owner's installer on 2026-10-22; nothing was executed)

- **Packaging**: an NSIS 3.04 installer wrapping an Electron 23.3.13 app (Chrome 110). The UI is Vue, and USB goes through `node-hid`.
- **LP360 page** (readable UI code):
  - modes: Load Mode, Temperature Guardian, Power Guardian, Music Mode, Creative Mode
  - Music Mode variants: emoji animation + spectrum, temperature + spectrum, usage + spectrum
  - layouts: Full Screen / Split Screen, Combined / Separate display
  - metrics: CPU temperature, GPU temperature, CPU usage, GPU usage, RAM used
  - settings: screen rotation, rotating screen, CPU Fan I/O, Pump I/O
  - stored settings: `screenStatus`, `screenBrightness`, `screenRotate`, `noLoadScreen`, `rgbOptical`, `gyroStatus`
  - custom emoji save, import and export
  - spectrum data from an FFT
- **LP360 device card**: shows CPU temperature, CPU fan speed and pump speed.
- **Main process**: frame drawing and USB writes live in the main process, which ships as V8 bytecode (`out/main/index.jsc`). Function names are visible (`generateNumberMatrix`, `rotateMatrix`, `handleMusicMode`, `emojis`), but the logic is not.
- **Electron fuses**: Node inspector arguments and `NODE_OPTIONS` are honoured, and ASAR integrity isn't enforced. Instrumenting the app at runtime is therefore possible.
- **Bundled helpers**:
  - a sensor bridge service that includes HWiNFO's library
  - a fan-control helper using PawnIO, which sets motherboard and hub fan speeds according to the developer notes shipped inside the app
  - a display service
  - nssm, a Windows service manager
- **Sensor path** (strings and logs, 2026-10-22):
  - the sensor bridge serves readings to the app over a local named pipe and polls its HWiNFO worker every 1000 ms;
  - the main process receives them as sensor-snapshot events, and its strings include normalized metric names such as CPU Temperature, CPU Usage, CPU Power, Gpu temperature, Gpu usage, Gpu power, Memory Load and Memory Used;
  - the bridge has a built-in fake backend (its readings unverified), and the main process has a `SENSOR_THRESHOLDS` entry (purpose unverified).

**Hardware & system**

- **The owner's PC**: board, sensor chip and Linux driver, fan inputs and readings, CPU, GPU and OS are recorded in `docs/local/machine.md`. That file is local only and never committed.
- **LP360**: USB `3633:000c` "DeepCool LP-Series".
- **Wiring** (LP240/LP360 manual, pp. 16–17):
  - fans → 4-pin CPU_FAN/CPU_OPT
  - pump → AIO_PUMP/W_PUMP (or CPU_FAN/CPU_OPT)
  - 3-pin 5V/D/GND lighting plug → motherboard ARGB header
  - display → internal 9-pin USB 2.0 header
- **Omarchy theme**:
  - `~/.local/state/omarchy/current/theme/colors.toml` holds the semantic colours and the light/dark `mode`
  - templates in `~/.config/omarchy/themed/*.tpl` are rendered on every theme change

---

*Generated: 2026-10-22*
*Status: DRAFT - needs validation*
