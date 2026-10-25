# Phase 4 report: implementation plans

**Plan:** none of its own. Phase 4 is the planning phase, one PRP plan per implementation phase.
**PRD phase:** 4 — Implementation plans
**Branch:** `phase-4-plans`
**Status:** written 2026-10-25, waiting for the owner's sign-off; then the branch is merged into `main` (fast-forward) and pushed, as AGENTS.md says.

## Outcome

Every implementation phase has a plan an agent can execute from top to bottom, with its tests, its acceptance criteria and its HTML companion. Each plan's evidence was checked against the spec, this PC and primary sources, and spikes settled the architectural hinges.

| Phase | Plan | In one line |
|---|---|---|
| 5 | [Device & protocol core](../plans/phase-5-device-protocol-core.plan.md) | Byte-exact frames to the LP360 without root, through reconnects; the dev access setup; the expected-frame loader every later phase tests against |
| 6 | [Sensors](../plans/phase-6-sensors.plan.md) | Every display value read correctly on any PC by an unprivileged service: driver rules, GPU choice, fan inputs, board sensor detection |
| 7 | [Service, settings & Load](../plans/phase-7-service-settings-load-mode.plan.md) | The sandboxed system service: the 100 ms tick, the movement procedure, settings, D-Bus and polkit, Load (modes 1–6), the dev copy |
| 8 | [Temperature & Power](../plans/phase-8-temperature-power.plan.md) | Modes 7–12 in °C and °F, every value up to 9999 then capped; differential tests that compare scrolling captures in cycle order |
| 9 | [Music](../plans/phase-9-music.plan.md) | Modes 13–23: the session's sound through a tiny `pw-record` helper, a Winamp-style spectrum with peak dots, the emoji loops, and the Dancer (mode 23) |
| 10 | [Settings app](../plans/phase-10-settings-app.plan.md) | The GTK4 app as designed in [docs/design/](../../design/settings-app.md): a live preview, instant apply, the Omarchy theme followed live, tested through AT-SPI in headless weston |
| 11 | [Packaging & final validation](../plans/phase-11-packaging-final-validation.plan.md) | One PKGBUILD for the AUR and a GitHub clone, an install → uninstall test in a rootless throwaway root, the full parity run, v1.0.0, and DeepCreative's deletion |

The PRD's Phase 4 row first said "phases 5–12". Removing Creative (2026-10-25) dropped one phase, so the plans cover 5–11.

## Acceptance

| Criterion | Result | Evidence |
|---|---|---|
| One plan per implementation phase | Met: Phases 5–11, each linked from the PRD table | PRD Implementation Phases |
| Each plan has an HTML companion | Met | `docs/prp/plans/*.plan.html` |
| Plans follow the spec and the owner's decisions | Met; a final sweep on 2026-10-25 found and fixed two stale lines (Phase 5's frame count, a leftover cold-boot mention in Phase 7) | Commit history below |
| Hinges settled by evidence, not assumption | Met by spikes and checks on this PC | Spikes, below |
| Nothing implemented yet | Met: plans, spec, design and research only; no Rust code | `git diff main --stat` |

## The owner's decisions in Phase 4

All recorded in the spec, the PRD and the plans the same day.

- **Display (2026-10-24):**
  - no caps, and no layout setting: static while every value is 99 or less, scrolling by itself from 100;
  - one smooth movement with a home position, written as an exact procedure (`display.md`, Movement, precisely);
  - one value centred, two values ending at column 14;
  - a mode, rotation, unit or GPU change, and Refresh, reset to home; choosing what's already set changes nothing;
  - dashes for unavailable and impossible readings;
  - test scenarios S1–S26.
- **Names (2026-10-24):** Load, Temperature, Power, Music. Never "Guardian", and no "Mode" after the name.
- **Testing (2026-10-24):**
  - no person in any test from Phase 5 on, with an optional attended tour (`odc-e2e --attended`);
  - test options are flags, never environment variables;
  - real devices where present, fakes where absent.
- **Values (2026-10-25):** every value capped at 9999 everywhere: the cooler, the app, D-Bus and the journal, the fan and pump RPM included.
- **Music (2026-10-25, chosen on a preview page):**
  - peak dots, on by default and switchable;
  - bar fall and peak fall sliders with Winamp's five stops and defaults;
  - at most 150 ms from sound to frame for 95 % of frames;
  - a reset restarts the emoji loop;
  - the Dancer (mode 23) on the whole grid, standing still in silence, in poses the owner's proportions define.
- **App (2026-10-25):**
  - one configuration screen for every setting except mode and unit;
  - the UI design delegated to the design agent; instant apply; no app-only settings;
  - GPUs named from `hwdata`.
- **Scope (2026-10-25):** Creative mode removed from OpenDeepCool everywhere.
- **Validation and release (2026-10-25):**
  - no boot testing of any kind; "starts at boot" proven by the unit's enablement and a start check;
  - distribution through the AUR and a GitHub clone with `makepkg`, both tested in a rootless container;
  - the service enabled at install, against Arch's convention.

## Spikes and checks that settled the plans

| Question | Answer | Where |
|---|---|---|
| Do DeepCreative's Temperature and Power captures still prove OpenDeepCool's rules? | Yes for 92 steps: 24 static, and 68 scrolling, which match the expected cycle frame by frame | Phase 8 plan |
| Can a user-session helper capture the default output and follow its changes without a PipeWire library? | Yes: `pw-record` with `stream.capture.sink`; WirePlumber's `follow-default-target` is on by default | Phase 9 plan |
| Does polkit count a systemd user unit as the active session? | Yes: `pkcheck` passed for an `allow_active`-only action from a user unit (polkit 128) | Phase 9 plan |
| Can a GTK4 app be driven unattended, without showing a window? | Yes: in headless weston, the widget tree is on the AT-SPI bus and actions run handlers; one action at a time | Phase 10 plan |
| Can install and uninstall be tested without root? | Yes: `pacstrap -N` and `arch-chroot -N` build and enter an Arch root as a regular user | Phase 11 plan |

## What's new outside the plans

- `docs/design/`: the settings app's design, 14 mockups and their generator.
- `docs/spec/`: OpenDeepCool's spectrum and Dancer (`music.md`); the movement procedure, scenarios and value rules (`display.md`); the app's requirements (`settings.md`); parity rows M16–M20 and D35–D41.
- `docs/spec/frames/`: regenerated with OpenDeepCool's rules, 284 cases. Phase 9 adds mode 23.
- Development tools the owner installed for the tests: `weston` and `arch-install-scripts` (PRD dev footprint).
- On claude.ai, for the owner: the spectrum preview page and the Dancer drawing board. They aren't part of the repository.

## Commits

36 commits on `phase-4-plans`, all `docs:`, from `4a6bd9b` (testing needs no person; uninstall removes everything) to the commit that adds this report. `git log --oneline main..phase-4-plans` lists them.

## Left for the owner

- **Sign off Phase 4.** Then the branch is merged into `main` and pushed, and implementation starts with Phase 5. Phase 6 can run alongside it.
- **Later, one-time actions:**
  - approve each `pkexec` run of the dev setup (Phases 5–10);
  - create an AUR account and SSH key before the release (Phase 11);
  - confirm the DeepCreative deletion list at the very end.
