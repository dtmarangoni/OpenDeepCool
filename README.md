# OpenDeepCool

A Linux app, written in Rust, that drives the 14×14 pixel display of a **DeepCool LP360** AIO cooler on **Arch Linux** and on Arch-based distributions that use systemd, Omarchy included. It replaces DeepCool's Windows-only DeepCreative app.

> **Not affiliated with DeepCool.** "DeepCool", "LP360" and "DeepCreative" are trademarks of their respective owners. This repository contains no DeepCool code, firmware or documents; facts about the device are described in our own words.

## Status

Pre-alpha. The service, the settings app and the Arch package are built and tested, but no release is published yet; the first alpha comes in Phase 12 (see Install).

- **Phase 1** (done, 2026-10-22): DeepCreative 1.2.14 runs under Wine on the target PC and drives the real cooler; every display mode was recorded. See [the report](docs/research/phase1-wine-spike.md).
- **Phase 2** (done, 2026-10-22): this repository, with agent instructions and a commit guard.
- **Phase 3** (done, 2026-10-23): the full functionality and protocol spec in [docs/spec/](docs/spec/), with the expected frame of every mode, rotation and unit, and a parity checklist the owner checked on the cooler.
- **Phase 4** (done, 2026-10-25): one implementation plan for each of Phases 5–11.
- **Solution reviews** (done, 2026-10-27): every plan's [review](docs/prp/solution-reviews/README.md) was crossed with the owner, and the PRD, plans and spec follow the decisions.
- **Phase 5** (done, 2026-10-28): the Rust workspace. Frames reach the cooler byte-exact, without root, and keep reaching it after a reconnect, all tested unattended on the real cooler. CI runs on every push. See [the report](docs/prp/reports/phase-5-device-protocol-core.report.md).
- **Phase 6** (done, 2026-10-29): the sensors. The display's seven readings come from built-in driver rules on any PC. A short-lived helper tells each GPU's kind, and a once-per-board detection loads the board's fan driver. Every test runs unattended: real where the PC has the device, through fakes where it hasn't. See [the report](docs/prp/reports/phase-6-sensors.report.md).
- **Phase 7** (done, 2026-11-05): the MVP. A sandboxed system service shows Load on the cooler from boot, keeps its settings in one file, and takes changes over D-Bus, each checked with polkit. On the development PC, its end-to-end checks drive the real service through every Load mode and each scenario type a run can drive, unattended. See [the report](docs/prp/reports/phase-7-service-settings-load-mode.report.md).
- **Phase 8** (done, 2026-11-05): Temperature and Power. The service shows the CPU's and the GPU's temperatures in °C or °F and their power in W, each alone or one over the other, every value in full up to 9999. Every scenario of modes 1–12 runs in-process in all four rotations, and each mode ran on the real cooler before its commit. See [the report](docs/prp/reports/phase-8-temperature-power.report.md).
- **Phase 9** (done, 2026-11-06): Music. The service shows the sound the PC plays as a 14-column spectrum with falling peak dots, alone, under one of four emoji animations, or under a value; and OpenDeepCool's own Dancer, who dances to the bass. A small helper in each user's session hands the service that sound, only while a Music mode is shown. Every mode from 1 to 23 is now drawn, and each ran on the real cooler before its commit. See [the report](docs/prp/reports/phase-9-music.report.md).
- **Phase 10** (done, 2026-11-07): the settings app. One GTK4 window mirrors the cooler with a live preview and changes every setting at once, as the service confirms it; it follows the Omarchy theme live and has its own theme elsewhere. Its tests drive the real app through the accessibility interface in an invisible display, against a fake service and against the real one. See [the report](docs/prp/reports/phase-10-settings-app.report.md).
- **Releases** (Phase 12, after the package and its final validation in Phase 11): an alpha, then a beta, each a GitHub pre-release for one month and until its open bugs are fixed; then `v1.0.0`, on GitHub and the AUR.

The [PRD phase table](docs/prp/prds/opendeepcool-lp360.prd.md#implementation-phases) is the only progress record.

## Scope

- **Hardware and system**: DeepCool LP360 (USB `3633:000c`) on any desktop PC running Arch Linux or an Arch-based distribution that uses systemd and Arch's packages. Distributions without systemd are out of scope. It finds the PC's CPU, GPU and board fan sensors itself. Tested on real hardware only on the owner's PC; the other sensor drivers are tested against simulated sensor files.
- **Planned**:
  - a system service that starts at boot, the same for every user;
  - a GTK4 settings app that follows the current Omarchy theme live when there is one, with its own default theme everywhere else;
  - DeepCreative 1.2.14's LP360 display modes under OpenDeepCool's names, Load, Temperature, Power and Music (DeepCreative's Load Mode, Temperature Guardian, Power Guardian and Music Mode), with screen rotation, plus OpenDeepCool's own Dancer;
  - the cooler's fan and pump speed readings.
- **Only the cooler**: OpenDeepCool reads the CPU, GPU and RAM only for what the display shows, and never controls other hardware. To monitor or control the CPU, GPU, other fans or power, use dedicated tools, as OpenRGB for lighting.
- **Not in v1**: speed control of the cooler's fans and pump, display settings DeepCreative's LP page doesn't offer (such as brightness), ARGB lighting (use OpenRGB), other devices.
- **Never**: DeepCreative's Creative Mode, its 14×14 image editor.

## Documentation

| Document | What it covers |
|---|---|
| [PRD](docs/prp/prds/opendeepcool-lp360.prd.md) | Problem, scope, decisions, phases, planned system footprint |
| [Spec](docs/spec/) | The contract: USB messages, display modes, sensors, settings, expected frames and the parity checklist |
| [Settings app design](docs/design/settings-app.md) | The settings app's screens, states, theme, keyboard and accessible names, with [mockups](docs/design/mockups/) |
| [Solution reviews](docs/prp/solution-reviews/README.md) | Architecture, best-practice and security reviews of the whole solution and of each phase's plan, done after planning and before implementation |
| [Phase 1 report](docs/research/phase1-wine-spike.md) | History: the Wine spike that first recorded DeepCreative; the spec supersedes it |
| [LP360 notes](docs/hardware/deepcool-lp360.md) | Wiring, identity, what the manual does and doesn't say |
| [Reference documents](docs/reference/README.md) | Vendor manuals kept locally (not in git) and where to get them |
| [AGENTS.md](AGENTS.md) / [CLAUDE.md](CLAUDE.md) | Rules and workflow for coding agents working on this repository |

## Install

There is no release yet: the first alpha, `0.1.z`, comes in Phase 12. The package builds the signed tag of the release it installs, so these steps work once a tag exists.

From GitHub, on Arch Linux or an Arch-based distribution:

```sh
git clone https://github.com/dtmarangoni/OpenDeepCool.git
cd OpenDeepCool/packaging/arch
makepkg -si
```

From the AUR, from `v1.0.0` (Phase 12): `yay -S opendeepcool`, or any AUR helper, or clone `https://aur.archlinux.org/opendeepcool.git` and run `makepkg -si` in it. Releases are signed, and the steps to import the signing key come with the first release.

`makepkg` builds one package with Arch's Rust toolchain and runs the tests first, which takes a few minutes. pacman installs what it needs: GTK 4, PipeWire's tools, polkit, lm-sensors and the rest of its dependencies. `nvidia-utils` is optional, for NVIDIA GPUs.

## What it needs

- **systemd is necessary.** The display service, its helpers, the account and the rules are systemd's units, sysusers and logind. An Arch-based distribution without systemd can't run OpenDeepCool, and supporting one is out of scope for now.
- **PipeWire, for the Music modes.** `pipewire-audio` is a dependency, so pacman installs it. The sound helper records through PipeWire's `pw-record`; on a system that runs only PulseAudio, the Music modes have no sound and the display shows a silent spectrum. The other modes don't use sound.
- **A polkit authentication agent, for the Start at boot switch.** The switch asks for your password through polkit. GNOME, KDE, Xfce, LXQt, MATE, Cinnamon and Hyprland setups have an agent; on another window manager install one (the [Arch wiki lists them](https://wiki.archlinux.org/title/Polkit#Authentication_agents)), or turn OpenDeepCool on from a terminal, as the next section shows. The other settings need no password.
- **Unprivileged user namespaces, for the sound helper.** Arch's kernels allow them, except `linux-hardened`. Where they're off, the helper fails to start (`systemctl --user status opendeepcool-audio` shows `status=217/USER` and "Failed to set up user namespacing"), and Music has no sound. Turn them on with `sudo sysctl kernel.unprivileged_userns_clone=1` (put the line in a file in `/etc/sysctl.d/` to keep it). That lets every user create user namespaces, which `linux-hardened` turns off on purpose, so it is your choice; the display service doesn't need it.

## Turn it on and off

Installing turns nothing on, as Arch's packages do. Turn OpenDeepCool on with the **Start at boot** switch in its settings app, which asks for your password once, or from a terminal:

```sh
sudo systemctl enable --now opendeepcool.service
sudo systemctl --global enable opendeepcool-audio.service
systemctl --user start opendeepcool-audio.service
```

Turn it off with the same switch, or:

```sh
sudo systemctl disable --now opendeepcool.service
sudo systemctl --global disable opendeepcool-audio.service
```

On a PC that has no lm-sensors configuration and no fan reading yet, the service's first start runs lm-sensors' `sensors-detect --auto` once, as root, to find the chip that measures the cooler's fans and pump. That detection probes the PC's sensor buses, loads the `i2c-dev` and `cpuid` drivers, and may write `/etc/conf.d/lm_sensors` and enable `lm_sensors.service`. The uninstall undoes only what the detection created.

NVIDIA support is untested on real hardware, NVML inside the service's sandbox included.

## Uninstall

```sh
sudo pacman -R opendeepcool
```

This removes everything OpenDeepCool installed or made, in the next section's four tables: its files, its system account and group, the links that turn it on, its saved settings, what the detection created, and any file an administrator made under its names in `/etc` and `/run`. A package test proves it: it installs and uninstalls the package in a throwaway Arch root and compares the root with how it started.

What stays, because it isn't OpenDeepCool's to delete:

- a sensor driver that lm-sensors loaded, and `i2c-dev` and `cpuid`, until the next boot;
- the journal's lines, until journald rotates them;
- a core dump of an OpenDeepCool program that crashed, until systemd's cleanup removes it, after two weeks by default;
- pacman's own log and cache;
- shadow-utils' backups of the account files, `/etc/passwd-`, `/etc/group-`, `/etc/shadow-` and `/etc/gshadow-`, which keep the `opendeepcool` lines until the next change to any account on the PC;
- users' home folders, which OpenDeepCool never writes to;
- software an administrator installed in `/usr/local`.

## Installed on your system

The exact list is [packaging/FILES.md](packaging/FILES.md), and a test fails when this section and that file differ.

**Installed by the package**, owned by root, and removed by `pacman -R`:

| Path | What it is |
|---|---|
| `/usr/bin/opendeepcool` | the settings app |
| `/usr/lib/opendeepcool/` | the folder for the programs users don't run themselves |
| `/usr/lib/opendeepcool/opendeepcoold` | the display service |
| `/usr/lib/opendeepcool/opendeepcool-audio` | the sound helper, run in each user's session |
| `/usr/lib/opendeepcool/odc-gpu-rank` | a one-shot helper that asks each GPU's driver for its kind |
| `/usr/lib/opendeepcool/odc-sensors-detect` | a one-shot helper that runs lm-sensors' detection once |
| `/usr/lib/opendeepcool/opendeepcool-boot` | the Start at boot switch's root program |
| `/usr/lib/systemd/system/opendeepcool.service` | the display service's unit |
| `/usr/lib/systemd/system/opendeepcool-gpu-rank.service` | the GPU helper's unit |
| `/usr/lib/systemd/system/opendeepcool-sensors-detect.service` | the detection's unit |
| `/usr/lib/systemd/system/opendeepcool-sensors-detect-modules.service` | loads the two drivers the detection may need |
| `/usr/lib/systemd/user/opendeepcool-audio.service` | the sound helper's user unit |
| `/usr/lib/sysusers.d/opendeepcool.conf` | the locked system account and group `opendeepcool` that the service runs as |
| `/usr/lib/udev/rules.d/70-opendeepcool.rules` | gives that group the cooler's device and read access to the CPU's power counters |
| `/usr/share/dbus-1/system.d/io.github.dtmarangoni.OpenDeepCool1.conf` | lets the service own its D-Bus name |
| `/usr/share/polkit-1/actions/io.github.dtmarangoni.OpenDeepCool1.policy` | the actions polkit checks before the service takes a change |
| `/usr/share/applications/io.github.dtmarangoni.OpenDeepCool.desktop` | the launcher entry |
| `/usr/share/icons/hicolor/scalable/apps/io.github.dtmarangoni.OpenDeepCool.svg` | the icon |
| `/usr/share/libalpm/hooks/opendeepcool-restart.hook` | restarts a running service on the new program after an upgrade |
| `/usr/share/licenses/opendeepcool/` | the license folder |
| `/usr/share/licenses/opendeepcool/LICENSE` | the MIT license |

The system account and group `opendeepcool` are created by systemd's sysusers hook when the package is installed, and deleted by the uninstall.

**Made when you turn it on**, removed by the uninstall:

| Path | What it is |
|---|---|
| `/etc/systemd/system/multi-user.target.wants/opendeepcool.service` → `/usr/lib/systemd/system/opendeepcool.service` | starts the service at boot |
| `/etc/systemd/user/default.target.wants/opendeepcool-audio.service` → `/usr/lib/systemd/user/opendeepcool-audio.service` | starts the sound helper at each login |

**Made while it runs**, removed by the uninstall:

| Path | What it is |
|---|---|
| `/var/lib/opendeepcool/` (opendeepcool:opendeepcool, 0700), with `settings.toml` and `gpu-rank.json` | your settings and each GPU's kind |
| `/var/lib/opendeepcool-detect/` (root:root, 0700), with `sensors-detect.json` | a record of what the detection created |
| `/etc/conf.d/lm_sensors`, `/etc/modprobe.d/lm_sensors.conf`, and `lm_sensors.service` enabled | written by the detection, only on a PC with no lm-sensors configuration and no fan reading; removed only if the record says the detection made them |

It also changes two things the udev rule controls, and the uninstall puts both back: the cooler's device node gets the group `opendeepcool` and mode 0660, and each CPU package's `energy_uj` power counter gets that group's read access.

**Made by an administrator under OpenDeepCool's names.** pacman never deletes a file it didn't install, but these are read in front of, or beside, the package's own files, so one left behind would change the next install. The uninstall deletes them; each lives in `/etc`, or in `/run` for the current boot:

| Path | What it is |
|---|---|
| `/etc/systemd/system/<unit>`, `<unit>.d/` and `/etc/systemd/system.control/<unit>.d/`, for each of the four system units; the same under `/run/systemd/` | a unit copy, a mask or a drop-in from `systemctl edit`, `set-property` or `mask` |
| `/etc/systemd/user/opendeepcool-audio.service` and `opendeepcool-audio.service.d/`; the same under `/run/systemd/user/` | the same for the sound helper |
| `/etc/sysusers.d/opendeepcool.conf`, `/etc/udev/rules.d/70-opendeepcool.rules`, `/etc/polkit-1/actions/io.github.dtmarangoni.OpenDeepCool1.policy`; the same three under `/run` | a file that overrides the package's, or masks it |
| `/etc/dbus-1/system.d/io.github.dtmarangoni.OpenDeepCool1.conf`, `/etc/pacman.d/hooks/opendeepcool-restart.hook` | likewise |

## Reporting a crash

The programs write to the journal. Attach the lines around the crash to a bug report, with the version from `pacman -Q opendeepcool`:

```sh
journalctl -u opendeepcool -b
journalctl --user -u opendeepcool-audio -b
```

A program that panics logs a message that names the source file, line and column, which is usually enough to find the fault. The package carries no debug symbols, and there is nothing for you to rebuild. If a program was killed without such a message, `coredumpctl list` followed by the program's name (`opendeepcoold`, for the service) shows when it died and by which signal: add that line.

Don't attach a core dump, which is a copy of the program's memory, or the output of `coredumpctl info`, which prints your account name, host name and machine ID.

## Development

Install the toolchain once per clone with [mise](https://github.com/jdx/mise). It sets up the Rust version pinned in `mise.toml`, with clippy and rustfmt:

```sh
mise install
```

Enable the commit guard once per clone. It's a Rust program (`tools/guard/`) that git starts at every commit and push. It rejects vendor files, DeepCreative code, binary files and another identity than the GitHub no-reply address, and it runs the checks below whenever a commit touches code:

```sh
git config core.hooksPath .githooks
```

Build, test, lint and format the workspace:

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

`cargo run -p odc-oracle -- docs/spec/frames` rebuilds the expected frames from the spec's rules. The tests on the real cooler need access to it without root, and so do the tests that read the CPU's power counter: `cargo run -p odc-dev -- setup` sets both up with one `pkexec` step, `check` reports them and `teardown` removes them. `cargo run -p odc-e2e` then runs the end-to-end checks, on the real cooler when one is attached.

While the Arch package is installed, `odc-dev setup` refuses and changes nothing, because the development copy's files in `/etc` would override the package's, and the package's removal would delete them: remove the package first with `sudo pacman -R opendeepcool`. `odc-dev setup --test-access` then gives the tests what they need from the installed package, with one `pkexec` step: your account in the package's group, and the reconnect rule. `odc-dev check` and `teardown` follow whichever is installed.

From Phase 7, `odc-dev setup` also builds a development copy of the display service and installs it as `opendeepcool.service`, enabled at boot. It drives the cooler from then on, as the finished app will, and starts with `--dev`, which adds the test session the end-to-end checks use. Until the settings app exists (Phase 10), change its mode, unit and rotation over D-Bus, and read its journal:

```sh
busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 \
  io.github.dtmarangoni.OpenDeepCool1 SetMode u 4
busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 \
  io.github.dtmarangoni.OpenDeepCool1 SetMode u 7
busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 \
  io.github.dtmarangoni.OpenDeepCool1 SetTemperatureUnit s fahrenheit
busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 \
  io.github.dtmarangoni.OpenDeepCool1 SetMode u 10
busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 \
  io.github.dtmarangoni.OpenDeepCool1 SetRotation y 1
busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 \
  io.github.dtmarangoni.OpenDeepCool1 GetState
journalctl -u opendeepcool
```

From Phase 9, `odc-dev setup` also installs the sound helper `opendeepcool-audio` as a user unit enabled for every user, so it starts at each login. While the display shows a Music mode (13–23), it hands the service the sound of the session's default output through PipeWire's `pw-record`; in every other mode it captures nothing and only waits. Check it, show the full-screen spectrum and turn its peak dots off:

```sh
systemctl --user status opendeepcool-audio
busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 \
  io.github.dtmarangoni.OpenDeepCool1 SetMode u 13
busctl call io.github.dtmarangoni.OpenDeepCool1 /io/github/dtmarangoni/OpenDeepCool1 \
  io.github.dtmarangoni.OpenDeepCool1 SetMusicPeaks b false
```

From Phase 10, `odc-dev setup` also installs the settings app `opendeepcool` with its launcher entry and icon under `/usr/local`, so the launcher lists OpenDeepCool; `odc-dev teardown` removes them with the rest. `cargo run -p odc-app` starts the app being built, against the dev service on the system bus. It follows the current Omarchy theme live, and uses its own default theme elsewhere. Its three flags are for the tests only: `--service-address` points it at a fake service on a private bus, `--omarchy-state` at another theme folder, and `--frame-log` makes it write each preview frame it receives to a file. `cargo test -p odc-app --features weston-tests` runs its widget tests in a headless weston (it needs `weston`), and `cargo run -p odc-e2e -- --only app` drives the real app through the accessibility interface (AT-SPI), against a fake service and, where the dev service runs, against that too; nothing appears on your screen.

`cargo run -p odc-e2e -- --only service` drives the dev service through each drawn mode, in each unit it shows, and each scenario it can be driven through, unattended. It tests the installed copy, so run `odc-dev setup` again after changing the service or what it links. `--only display` writes each drawn mode's examples straight to the cooler, from the code just built, and `--modes 7-9` limits either to some modes. `--only music` plays a test tone into a virtual output, checks what the cooler shows for it, and measures the delay from sound to frame. `--only baseline` measures its CPU and memory use over 10 minutes, and `--attended` is an optional tour to watch on the cooler.

## License

[MIT](LICENSE)
