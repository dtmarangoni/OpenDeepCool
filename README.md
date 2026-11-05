# OpenDeepCool

A Linux app, written in Rust, that drives the 14×14 pixel display of a **DeepCool LP360** AIO cooler on **Omarchy** (Arch Linux). It replaces DeepCool's Windows-only DeepCreative app.

> **Not affiliated with DeepCool.** "DeepCool", "LP360" and "DeepCreative" are trademarks of their respective owners. This repository contains no DeepCool code, firmware or documents; facts about the device are described in our own words.

## Status

Pre-alpha: planning and research. No installable app yet.

- **Phase 1** (done, 2026-10-22): DeepCreative 1.2.14 runs under Wine on the target PC and drives the real cooler; every display mode was recorded. See [the report](docs/research/phase1-wine-spike.md).
- **Phase 2** (done, 2026-10-22): this repository, with agent instructions and a commit guard.
- **Phase 3** (done, 2026-10-23): the full functionality and protocol spec in [docs/spec/](docs/spec/), with the expected frame of every mode, rotation and unit, and a parity checklist the owner checked on the cooler.
- **Phase 4** (done, 2026-10-25): one implementation plan for each of Phases 5–11.
- **Solution reviews** (done, 2026-10-27): every plan's [review](docs/prp/solution-reviews/README.md) was crossed with the owner, and the PRD, plans and spec follow the decisions.
- **Phase 5** (done, 2026-10-28): the Rust workspace. Frames reach the cooler byte-exact, without root, and keep reaching it after a reconnect, all tested unattended on the real cooler. CI runs on every push. See [the report](docs/prp/reports/phase-5-device-protocol-core.report.md).
- **Phase 6** (done, 2026-10-29): the sensors. The display's seven readings come from built-in driver rules on any PC. A short-lived helper tells each GPU's kind, and a once-per-board detection loads the board's fan driver. Every test runs unattended: real where the PC has the device, through fakes where it hasn't. See [the report](docs/prp/reports/phase-6-sensors.report.md).
- **Phase 7** (done, 2026-11-05): the MVP. A sandboxed system service shows Load on the cooler from boot, keeps its settings in one file, and takes changes over D-Bus, each checked with polkit. On the development PC, its end-to-end checks drive the real service through every Load mode and each scenario type a run can drive, unattended. See [the report](docs/prp/reports/phase-7-service-settings-load-mode.report.md).
- **Phase 8** (built, awaiting sign-off): Temperature and Power. The service shows the CPU's and the GPU's temperatures in °C or °F and their power in W, each alone or one over the other, every value in full up to 9999. Every scenario of modes 1–12 runs in-process in all four rotations, and each mode ran on the real cooler before its commit.
- **Releases** (Phase 12, after the package and its final validation in Phase 11): an alpha, then a beta, each a GitHub pre-release for one month and until its open bugs are fixed; then `v1.0.0`, on GitHub and the AUR.

The [PRD phase table](docs/prp/prds/opendeepcool-lp360.prd.md#implementation-phases) is the only progress record.

## Scope

- **Hardware**: DeepCool LP360 (USB `3633:000c`) on any desktop PC running Arch Linux, with Omarchy first. It finds the PC's CPU, GPU and board fan sensors itself. Tested on real hardware only on the owner's PC; the other sensor drivers are tested against simulated sensor files.
- **Planned**:
  - a system service that starts at boot, the same for every user;
  - a GTK4 settings app that follows the current Omarchy theme live, with its own default theme on other desktops;
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
| [Solution reviews](docs/prp/solution-reviews/README.md) | Architecture, best-practice and security reviews of the whole solution and of each phase's plan, done after planning and before implementation |
| [Phase 1 report](docs/research/phase1-wine-spike.md) | History: the Wine spike that first recorded DeepCreative; the spec supersedes it |
| [LP360 notes](docs/hardware/deepcool-lp360.md) | Wiring, identity, what the manual does and doesn't say |
| [Reference documents](docs/reference/README.md) | Vendor manuals kept locally (not in git) and where to get them |
| [AGENTS.md](AGENTS.md) / [CLAUDE.md](CLAUDE.md) | Rules and workflow for coding agents working on this repository |

## What it will install

Every file, rule, module and service the finished app adds will be listed here before the first release. The planned list is in the PRD's [System Footprint](docs/prp/prds/opendeepcool-lp360.prd.md#system-footprint).

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

`cargo run -p odc-e2e -- --only service` drives the dev service through each drawn mode, in each unit it shows, and each scenario it can be driven through, unattended. It tests the installed copy, so run `odc-dev setup` again after changing the service or what it links. `--only display` writes each drawn mode's examples straight to the cooler, from the code just built, and `--modes 7-9` limits either to some modes. `--only baseline` measures its CPU and memory use over 10 minutes, and `--attended` is an optional tour to watch on the cooler.

## License

[MIT](LICENSE)
