# OpenDeepCool

A Linux app, written in Rust, that drives the 14×14 pixel display of a **DeepCool LP360** AIO cooler on **Omarchy** (Arch Linux). It replaces DeepCool's Windows-only DeepCreative app.

> **Not affiliated with DeepCool.** "DeepCool", "LP360" and "DeepCreative" are trademarks of their respective owners. This repository contains no DeepCool code, firmware or documents; facts about the device are described in our own words.

## Status

Pre-alpha: planning and research. No installable app yet.

- **Phase 1** (done, 2026-10-22): DeepCreative 1.2.14 runs under Wine on the target PC and drives the real cooler; every display mode was recorded. See [the report](docs/research/phase1-wine-spike.md).
- **Phase 2** (done, 2026-10-22): this repository, with agent instructions and a commit guard.
- **Phase 3** (done, 2026-10-23): the full functionality and protocol spec in [docs/spec/](docs/spec/), with the expected frame of every mode, rotation and unit, and a parity checklist the owner checked on the cooler.
- **Phase 4** (done, 2026-10-25): one implementation plan for each of Phases 5–11.
- **Now**: the plans' [solution reviews](docs/prp/solution-reviews/README.md) are crossed with the owner. **Next, Phase 5**: the Rust workspace, the frame encoder and the cooler's USB writer.
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

Enable the commit guard once per clone. It rejects vendor files, DeepCreative code and large binaries:

```sh
git config core.hooksPath .githooks
```

Build and test commands arrive with the Rust workspace (Phase 5).

## License

[MIT](LICENSE)
