# OpenDeepCool

A Linux app, written in Rust, that drives the 14×14 pixel display of a **DeepCool LP360** AIO cooler on **Omarchy** (Arch Linux). It replaces DeepCool's Windows-only DeepCreative app.

> **Not affiliated with DeepCool.** "DeepCool", "LP360" and "DeepCreative" are trademarks of their respective owners. This repository contains no DeepCool code, firmware or documents; facts about the device are described in our own words.

## Status

Pre-alpha: planning and research. No installable app yet.

- **Phase 1** (done, 2026-10-22): DeepCreative 1.2.14 runs under Wine on the target PC and drives the real cooler; every display mode was recorded. See [the report](docs/research/phase1-wine-spike.md).
- **Phase 2** (done): this repository, with agent instructions and a commit guard.
- **Next, Phase 3**: the full functionality and protocol spec, validated on the cooler.

The [PRD phase table](docs/prp/prds/opendeepcool-lp360.prd.md#implementation-phases) is the only progress record.

## Scope

- **Hardware**: DeepCool LP360 (USB `3633:000c`) on any desktop PC running Arch Linux, with Omarchy first. It finds the PC's CPU, GPU and board fan sensors itself. Tested on real hardware only on the owner's PC; the other sensor drivers are tested against simulated sensor files.
- **Planned**:
  - a system service that starts at boot, the same for every user;
  - a GTK4 settings app that follows the current Omarchy theme live, with its own default theme on other desktops;
  - every LP360 display mode from DeepCreative 1.2.14: Load Mode, Temperature Guardian, Power Guardian, Music Mode, Creative Mode and screen rotation;
  - CPU fan and pump speed readings.
- **Not in v1**: fan/pump speed control, display settings DeepCreative's LP page doesn't offer (such as brightness), ARGB lighting (use OpenRGB), other devices.

## Documentation

| Document | What it covers |
|---|---|
| [PRD](docs/prp/prds/opendeepcool-lp360.prd.md) | Problem, scope, decisions, phases, planned system footprint |
| [Phase 1 report](docs/research/phase1-wine-spike.md) | Wine spike, USB message format, full list of display modes |
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

Enable the commit guard once per clone. It rejects vendor files, DeepCreative code and large binaries:

```sh
git config core.hooksPath .githooks
```

Build and test commands arrive with the Rust workspace (Phase 5).

## License

[MIT](LICENSE)
