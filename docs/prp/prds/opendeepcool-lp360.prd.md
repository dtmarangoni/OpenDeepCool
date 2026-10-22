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

- **A background service** that starts with the system and keeps the LP360 showing the configured mode with live data.
- **A GTK4 settings app** that follows the active Omarchy theme and offers every LP360 mode, option and effect in DeepCreative 1.2.14.

What each mode draws comes from DeepCreative itself:

- its readable UI code;
- running the real app under Wine on this PC, connected to the real cooler, and recording exactly what it sends over USB;
- the owner confirming each mode on the physical display.

The alternatives are worse. A Windows VM or dual boot isn't available, and a VM would show fake sensor readings anyway. Forking an existing Linux tool would give only the frame format and simple number modes, and would bring that project's GPL license with it.

## Key Hypothesis

We believe a Rust boot service plus an Omarchy-themed GTK4 app that reproduces DeepCreative's LP360 modes, options and effects will fully replace DeepCreative for the owner.

We'll know we're right when two things are true:

- every item on the parity checklist (built from DeepCreative 1.2.14) passes the owner's check on the real LP360;
- after a cold boot, the display goes back to the configured mode with no action from the owner.

## What We're NOT Building

- **Fan or pump speed control in v1.** v1 only reads speeds. DeepCreative has a separate fan-control page, and this may come after v1. It would need a driver that can write (the in-kernel driver for the board's sensor chip is read-only), and a bug could slow or stop the pump.
- **Hidden display settings and unused device reports in v1.** DeepCreative 1.2.14 stores `screenBrightness`, `screenStatus`, `noLoadScreen`, `rgbOptical` and `gyroStatus` for the LP360, but its LP page has no control for them, and Phase 1 saw no USB message for them. The LP360 also declares HID reports that DeepCreative never sent in Phase 1 ([LP360 notes](../../hardware/deepcool-lp360.md)). Supporting them may come after v1 (owner decision, 2026-10-22). It would first need research into what each one does; until then OpenDeepCool never sends them.
- **ARGB lighting.** The LP360's lighting cable plugs into the motherboard's 5V ARGB header (manual p. 16). OpenRGB, already installed, handles it.
- **Support for other hardware, other DeepCool devices, other distributions or other operating systems.** The owner can only test this machine.
- **DeepCreative features outside the LP360 page:** the fan-control page, firmware updates, online auto-update, screen casting and LCD-device features.
- **Compatibility with DeepCreative's import/export file format, and a user-facing command-line tool.** Owner decision.
- **Any DeepCool code, installer files or raw USB captures in the repository.** The repository is public.

## Success Metrics

| Metric | Target | How Measured |
|---|---|---|
| Parity with DeepCreative | 100% of parity-checklist items pass | Owner checks each item on the physical LP360 against the functionality spec and, where available, the Wine capture of DeepCreative |
| Starts with the system | Configured mode is on the display after every cold boot, with no action (proposed check: 5 consecutive cold boots) | Owner reboots; `systemctl --user status` and the journal show the service running |
| Correct values | Displayed values match the system's own readings within each metric's resolution | Integration and e2e tests compare rendered values with the sensor sources; owner spot-checks against `sensors` |
| Same timing as DeepCreative | Refresh interval and animation timing per mode match DeepCreative | Compare frame timestamps against the Wine capture |
| Tested | Unit, integration and e2e tests for each phase, where applicable | Test suites run locally and in CI; e2e on the real device with the owner |
| Resource use | TBD (the owner set no target) | Measure a baseline in Phase 7 (CPU %, memory) |

## Open Questions

- [x] Does DeepCreative 1.2.14 run under Wine, and can it reach the LP360 over USB through Wine? **Yes** (Phase 1 report, 2026-10-22). It needs Wine Mono and its sensor helper in console mode.
- [ ] Which USB messages does DeepCreative send for the LP page's features besides "draw a 14×14 frame"? Phase 1 saw only a start-up command `03 08`, meaning unknown; rotation is drawn by the PC.
- [x] Is each pixel only on/off? **Yes**: a frame carries one bit per pixel (Phase 1 report). Brightness levels and colour are after v1, with the hidden settings.
- [ ] *After v1 (owner decision, 2026-10-22):* what the hidden settings `screenBrightness`, `screenStatus`, `noLoadScreen`, `rgbOptical` and `gyroStatus` do, and what the device reports DeepCreative doesn't use are for.
- [ ] On the owner's board, which fan input is the pump and which is the CPU fan? Readings and candidates are in `docs/local/machine.md`.
- [ ] Which metrics does each mode use? For example, does Power Guardian use CPU power, and is CPU clock used anywhere?
- [ ] Music Mode: which audio output is analysed, and how does DeepCreative turn audio into the spectrum (bands, scaling, frame rate)?
- [ ] What should the display show when the service is stopped or the PC is idle?
- [ ] If Wine fails, can the main-process V8 bytecode (Electron 23.3.13) be decompiled?
- [ ] Does `energy_uj` (CPU power) stay readable without root across reboots and kernel updates? It is readable now, but no rule was found that sets this.

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
| Must | Starts with the system and shows the configured mode with live data | Owner requirement: keeps working after every restart |
| Must | Talks to the LP360 over USB without root | The service runs as the owner |
| Must | Load Mode: Full Screen and Split Screen layouts (Combined / Separate display) with CPU temperature, CPU usage, GPU temperature, GPU usage and RAM used | DeepCreative LP360 options; the owner's MVP |
| Must | Temperature Guardian and Power Guardian | DeepCreative LP360 modes |
| Must | Music Mode: emoji animation + spectrum, temperature + spectrum, usage + spectrum | DeepCreative LP360 modes |
| Must | Creative Mode: draw pixel art and animations; save, import and export them (own file format) | DeepCreative LP360 mode |
| Must | Display settings on DeepCreative's LP page: screen rotation, rotating screen, plus any others the spec finds there | DeepCreative LP360 settings. Hidden settings such as brightness come after v1 |
| Must | CPU fan and pump speed readings, with the fan inputs chosen as in DeepCreative's "CPU Fan I/O" / "Pump I/O" | DeepCreative LP360 card and settings |
| Must | GTK4 settings app that follows the Omarchy theme, including live theme changes | Owner requirement |
| Must | Settings kept across reboots | Owner requirement |
| Must | Clean install and uninstall, with every installed piece documented | Owner requirement |
| Must | Unit, integration and e2e tests for each phase, where applicable | Owner requirement |
| Should | Live preview of the 14×14 display in the app | Mode choice and Creative Mode work without looking at the pump |
| Should | CI build and test on GitHub | Keeps the public repo's main branch passing |
| Could | Fan and pump speed control, after v1 | Owner: possible later |
| Could | Hidden display settings (brightness, screen on/off, no-load screen, `rgbOptical`, gyro) and the device reports DeepCreative doesn't use, after v1 | Owner: possible later (2026-10-22) |
| Won't | Lighting, other devices, DeepCreative file compatibility, user-facing CLI | See [What We're NOT Building](#what-were-not-building) |

### MVP Scope

After a reboot, with no action from the owner, the LP360 shows the configured Load Mode layout with live values. The layout is one of CPU, GPU or CPU/GPU, showing temperature or usage %. It is drawn the way DeepCreative draws it. At this stage the configuration can be a settings file; the settings app arrives in Phase 11.

### User Flow

1. The PC boots, the owner's desktop session starts, and the OpenDeepCool user service starts with it. It reads the saved settings, opens the LP360 and draws the configured mode at each refresh interval.
2. To change the mode, open OpenDeepCool from the launcher, pick a mode and options (with a preview), and apply. The service switches right away and saves the choice.

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
- **Access without root**: a udev rule gives the logged-in user access to `3633:000c` (`TAG+="uaccess"`).
- **Sensors** (all checked readable by the owner's normal account on 2026-10-22, except where noted):
  - CPU temperature: the CPU's hwmon driver
  - CPU usage: `/proc/stat`
  - CPU clock: cpufreq `scaling_cur_freq`
  - CPU power: the powercap RAPL energy counter (`energy_uj`), readable now; whether that persists is an open question
  - GPU temperature and power: the GPU driver's hwmon device
  - GPU usage: the GPU driver's DRM busy-percent file
  - RAM: `/proc/meminfo`
  - Fan and pump RPM: the hwmon driver for the motherboard's sensor chip (which one, on the owner's PC: `docs/local/machine.md`). It must be loaded at every boot.

  Look devices up by name or driver, never by hwmon/DRM index, because indexes change between boots.
- **Service model**: a systemd **user** service that starts with the owner's desktop session. On the owner's PC that session starts automatically at boot (see `docs/local/machine.md`). Running inside the owner's session also gives Music Mode access to audio through PipeWire. The service restarts on failure and reconnects if the USB device disappears.
- **Music Mode**: capture the monitor of the default audio output through PipeWire and compute an FFT (crates chosen in the plan). Match DeepCreative's band layout using the Wine capture.
- **Settings app ↔ service**: local IPC within the session, either D-Bus or a Unix socket in `$XDG_RUNTIME_DIR` (chosen in the plan).
- **Theme**:
  - Ship an Omarchy template, `~/.config/omarchy/themed/opendeepcool.css.tpl`, using placeholders such as `{{ accent }}`.
  - On every theme change, Omarchy renders it to `~/.local/state/omarchy/current/theme/opendeepcool.css`. Omarchy already does this for `hyprland-preview-share-picker`, a Rust GTK4 app.
  - The app watches that file and reloads its CSS live. If the file is missing, it reads `colors.toml` directly.
- **Build and run**: `cargo build --release` produces native binaries, so running them needs no Rust toolchain. They link to the system GTK4, which Omarchy already ships.
- **Packaging**: a local PKGBUILD, so pacman tracks every installed file and `pacman -R` removes them all (proposed; decided in the Phase 12 plan).
- **Reverse-engineering environment** (development only, never shipped):
  - DeepCreative 1.2.14 is extracted from the owner's installer and run under Wine 11.17 in its own prefix, with networking blocked.
  - Its Electron build honours Node inspector arguments (fuse `EnableNodeCliInspectArguments`), so a small hook can log every USB write for each mode.
  - `usbmon` and `strace` serve as cross-checks.
  - Raw captures stay local, outside the repository.

**Technical Risks**

| Risk | Likelihood | Mitigation |
|---|---|---|
| DeepCreative won't start under Wine (Electron 23, its Windows services, WebView2, .NET helpers) | M | Run `DeepCool.exe` directly, without the installer; try Chromium flags (`--no-sandbox`, `--disable-gpu`) and winetricks components; then Proton. Last resort: static analysis plus recreation guided by the owner |
| Wine can't reach the LP360 over USB | M | Log frames with a hook inside the app; present a virtual HID device (Linux `uhid`) with the LP360's IDs; use `usbmon` to confirm |
| Main-process logic is compiled V8 bytecode | H (certain) | Capture at runtime under Wine; decompile only if needed |
| DeepCreative tries to update itself or the firmware | L | Network blocked during the Wine run; decline any prompt |
| Wrong fan input chosen as pump or CPU fan | L | Inputs are selectable, as in DeepCreative; the owner confirms the defaults |
| CPU power file becomes root-only after a reboot or kernel update | M | Detect at runtime; add a documented udev rule if needed |
| hwmon/DRM index changes between boots | H | Look up by name or driver, never by index |
| Omarchy changes its template mechanism | L | Fall back to `colors.toml`; take light/dark from its `mode` key |
| Automated e2e tests need hardware or root (`/dev/uhid` is root-only) | M | Fake transport for automated tests; real-device e2e runs with the owner |

---

## How Agents Work on This Project

This project spans many sessions and possibly several coding agents, so the way they work is part of the product plan, not an afterthought. The instruction files are drafted now and committed in Phase 2.

- **`AGENTS.md`** is the single rulebook for every coding agent. It holds:
  - the start-of-session checklist
  - the working method
  - the owner's rules: never guess; ask as soon as in doubt; the owner validates anything only the hardware shows; small commits; tests with every step; no DeepCool material in the repo; no undocumented system changes; v1 scope limits
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
- **One implementation, one commit** (owner's rule, 2026-10-22): each display mode and each combination, e.g. Load Mode → Combined CPU/GPU, gets its own implement → test → owner check on the cooler → exactly one commit. Several modes are never batched together.
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
| 3 | Functionality & protocol spec | Full spec of LP360 + DeepCreative + board sensors; parity checklist validated by the owner | pending | - | 1, 2 | - | - | - |
| 4 | Implementation plans | One PRP plan per implementation phase (5–12) | pending | - | 3 | - | - | - |
| 5 | Device & protocol core | Rust workspace, frame encoder, LP360 USB writer, udev rule, test patterns | pending | with 6 | 4 | - | - | - |
| 6 | Sensors | All metrics plus fan input selection, tested against fake sysfs trees | pending | with 5 | 4 | - | - | - |
| 7 | Service, settings & Load Mode (MVP) | User service, autostart, saved settings, Load Mode layouts | pending | - | 5, 6 | - | - | - |
| 8 | Guardian modes | Temperature Guardian, Power Guardian | pending | with 9, 10, 11 | 7 | - | - | - |
| 9 | Music Mode | Audio capture, spectrum, the three Music Mode variants | pending | with 8, 10, 11 | 7 | - | - | - |
| 10 | Creative Mode | Pixel art and animation engine, storage, import/export | pending | with 8, 9, 11 | 7 | - | - | - |
| 11 | Settings app | GTK4 app, Omarchy theme template, all settings screens, preview, pixel editor | pending | with 8, 9, 10 | 7 | - | - | - |
| 12 | Packaging & final validation | Install/uninstall, footprint docs, release build, full parity run with the owner | pending | - | 8, 9, 10, 11 | - | - | - |

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

**Phase 4: Implementation plans**
- **Goal**: An implementation-ready plan for each of phases 5–12 (owner's workflow step 9).
- **Scope**: `/prp-plan` for each phase. Each plan includes its unit, integration and e2e tests and the owner's validation steps.
- **Success signal**: Plans committed; each one lists files, tests and acceptance checks.

**Phase 5: Device & protocol core**
- **Goal**: Reliable frames to the LP360 without root.
- **Scope**: the workspace; frame encoder and checksum; a USB writer that reconnects; the udev rule; a developer test-pattern binary (not a user feature).
- **Tests**:
  - unit: encoding, checksum, rotation
  - integration: fake transport, optionally a virtual `uhid` device
  - e2e: the owner confirms the test patterns on the pump
- **Success signal**: Test patterns appear correctly and all tests pass.

**Phase 6: Sensors**
- **Goal**: Read every metric DeepCreative shows, correctly, as the owner's normal account.
- **Scope**:
  - CPU temperature, usage, clock and power
  - GPU temperature, usage and power
  - RAM
  - fan and pump RPM through the board's hwmon sensor driver, with selectable inputs
  - the sensor-driver autoload entry
- **Tests**:
  - unit: parsers
  - integration: fake sysfs and proc trees
  - e2e: values compared against `sensors`
- **Success signal**: Values match the system tools within each metric's resolution.

**Phase 7: Service, settings & Load Mode (MVP)**
- **Goal**: Deliver the MVP.
- **Scope**:
  - systemd user service and settings file
  - Load Mode layouts as defined in the spec: Full or Split Screen, Combined or Separate display; CPU and GPU temperature and usage; RAM
  - refresh timing matching DeepCreative
- **Tests**:
  - unit: renderers compared against spec frames
  - integration: service → fake device
  - e2e: cold boot → configured mode, checked by the owner
- **Success signal**: After cold boots, the configured mode appears with no action.

**Phase 8: Guardian modes**
- **Goal**: Temperature Guardian and Power Guardian as defined in the spec.
- **Scope**: renderers, thresholds and timing from the spec.
- **Tests**:
  - unit: renderers at and around thresholds
  - integration: service with simulated sensor values
  - e2e: check by the owner
- **Success signal**: These modes' checklist items pass.

**Phase 9: Music Mode**
- **Goal**: The three Music Mode variants.
- **Scope**: PipeWire capture, FFT, renderers.
- **Tests**:
  - unit: spectrum from synthetic signals
  - integration: audio file → frames
  - e2e: the owner plays music
- **Success signal**: These modes' checklist items pass, and the owner accepts the latency.

**Phase 10: Creative Mode**
- **Goal**: Pixel art and animations.
- **Scope**: frame and animation model, storage, playback timing, import/export in our own format.
- **Tests**:
  - unit: model and timing
  - integration: save and load
  - e2e: the owner draws and plays an animation
- **Success signal**: These modes' checklist items pass.

**Phase 11: Settings app**
- **Goal**: A GTK4 app, themed by Omarchy, that exposes every option.
- **Scope**: screens for all modes and settings; preview; pixel editor; theme template with live reload; IPC client.
- **Tests**:
  - unit: view models
  - integration: app ↔ service IPC
  - e2e: the owner switches themes and modes
- **Success signal**: The app follows `omarchy-theme-set` live, and every setting is reachable.

**Phase 12: Packaging & final validation**
- **Goal**: Clean install and uninstall, plus full parity sign-off.
- **Scope**: PKGBUILD (proposed); install and uninstall docs listing every file; release build; a full checklist run with the owner; a cold-boot test.
- **Tests**:
  - integration: install → uninstall leaves no files behind
  - e2e: fresh install → reboot → configured mode shows
- **Success signal**: The checklist is at 100%, and uninstalling leaves nothing behind.

### Parallelism Notes

Phases 1–4 run in sequence because each feeds the next. Phases 5 and 6 touch separate code (device and sensors) and can run in separate worktrees. After the MVP (Phase 7), the remaining mode families (Phases 8, 9 and 10) are independent renderers. The settings app (Phase 11) can be built against the settings schema from Phase 7, so all four can run in parallel worktrees.

---

## System Footprint

The owner asked for every installed or created item to be listed. Paths are proposed and get finalised in the Phase 12 plan.

**Finished app**

| Item | Path (proposed) | Why | Removed by |
|---|---|---|---|
| Service and settings-app binaries | `/usr/bin/` | The app | `pacman -R` |
| udev rule | `/usr/lib/udev/rules.d/70-opendeepcool.rules` | LP360 USB access for the logged-in user (plus the CPU power file, if needed) | `pacman -R` |
| Kernel module autoload | `/usr/lib/modules-load.d/opendeepcool.conf` (the board's sensor driver) | Fan and pump RPM at every boot | `pacman -R` |
| systemd user unit | `/usr/lib/systemd/user/opendeepcool.service`, enabled for the owner | Starts with the session | `systemctl --user disable` + `pacman -R` |
| Desktop entry and icon | `/usr/share/applications/`, `/usr/share/icons/` | Launch from the Omarchy menu | `pacman -R` |
| Omarchy theme template | `~/.config/omarchy/themed/opendeepcool.css.tpl` | Follow the theme | Uninstall step |
| Generated theme CSS | `~/.local/state/omarchy/current/theme/opendeepcool.css` | Written by Omarchy | Omarchy, on the next theme change |
| Settings and Creative Mode files | `~/.config/opendeepcool/` | Keep choices | Uninstall step (optionally kept) |
| Logs | User journal | Diagnostics | journald rotation |

**Development and research only (never shipped)**

| Item | Where | Notes |
|---|---|---|
| Rust toolchain | mise (already installed) | Build only |
| Wine 11.17, winetricks | Already installed | Phases 2–3 |
| Wine prefix for DeepCreative | `research/wine/` in the project (git-ignored) | Deleted after Phase 3 |
| Extracted DeepCreative files and captures | Outside the repository | Never committed |
| Board sensor driver loaded by hand | Kernel, until the next reboot | Done 2026-10-22 |

---

## Decisions Log

| Decision | Choice | Alternatives | Rationale |
|---|---|---|---|
| Language | Rust | — | Owner requirement |
| Source of truth for DeepCreative behaviour | Readable UI code + DeepCreative under Wine with the real cooler + owner checks | Windows VM; dual boot; Linux tools only | No Windows is available; a VM shows fake readings; Wine runs the real app against the real cooler on this PC |
| Wine or Proton | Plain Wine 11.17 (installed) | Proton / protontricks | Proton targets Steam games; kept as a fallback |
| Existing Linux projects | Learn the protocol from them; write our own code | Fork Nortank12/deepcool-digital-linux | They cover a small part, and copying their code brings GPL with it |
| License | MIT | GPL-3.0 | Owner decision |
| GUI toolkit | GTK4 (gtk4-rs), themed by an Omarchy template | Iced, Slint, egui, libadwaita | Omarchy already themes a Rust GTK4 app this way |
| Service model | systemd user service, started with the owner's desktop session | System service running as root | No root needed; Music Mode needs the owner's audio session; on the owner's PC the session starts automatically at boot |
| Fan and pump readings | In-kernel hwmon driver for the board's sensor chip (read-only), loaded at boot | Out-of-tree DKMS driver | Verified working on the owner's PC on 2026-10-22; nothing to build |
| Fan control | Not in v1 | In v1 | Owner decision; needs a driver that can write |
| Hidden display settings and unused device reports | Not in v1; documented, never sent | Research them in Phase 3 | Owner decision 2026-10-22: out of scope for now, kept as a possible later feature |
| Lighting | Out of scope (OpenRGB) | Include it | Wired to the motherboard ARGB header |
| Testing | Unit + integration + e2e for each phase, where applicable | — | Owner requirement |
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
