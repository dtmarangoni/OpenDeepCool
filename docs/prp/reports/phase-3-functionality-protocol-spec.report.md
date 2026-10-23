# Phase 3 report: functionality and protocol spec

**Plan:** [phase-3-functionality-protocol-spec.plan.md](../plans/phase-3-functionality-protocol-spec.plan.md)
**PRD phase:** 3 — Functionality & protocol spec
**Branch:** `phase-3-spec`
**Status:** implemented, 2026-10-23: the owner signed off the [parity checklist](../../spec/parity-checklist.md). The phase is complete in the PRD once its pull request is merged.

## Outcome

[`docs/spec/`](../../spec/) describes what DeepCreative 1.2.14 shows on the LP360, the USB messages that carry it and the readings behind it. It's precise enough to reproduce without running DeepCreative, and it says where OpenDeepCool deliberately differs.

| Document | Covers |
|---|---|
| [protocol.md](../../spec/protocol.md) | Message layout, checksum, frame bit layout, start-up command `03 08`, timing, what the cooler does when frames stop |
| [display.md](../../spec/display.md) | Modes 1–12 and Creative Mode, glyphs, value rules, scrolling, rotation, mode switches |
| [music.md](../../spec/music.md) | Modes 13–22: spectrum layout (our own analysis), emoji animations, value area |
| [sensors.md](../../spec/sensors.md) | What each metric means, and the Linux source on any PC |
| [settings.md](../../spec/settings.md) | Every setting that changes what the cooler shows: values, defaults, restore at start-up |
| [frames/](../../spec/frames/) | Expected frames for modes 1–22, 100 and 101, every rotation and unit |
| [parity-checklist.md](../../spec/parity-checklist.md) | 73 rows, each with its spec section, evidence and the owner's check |

## Acceptance

| Criterion | Result | Evidence |
|---|---|---|
| AC1 Protocol | Met | Re-run on 2026-10-23 over the whole local capture log: 159,372 messages, 0 with a bad checksum or terminator; 12 are `03 08` (once per app start), the other 159,360 are frames. Every message is output report 16 |
| AC2 Display | Met | `render-check.py` over every sweep: 466 steps checked, 0 differ; one recorded one-off (display.md, "Observed one-off") |
| AC3 Music | Met | The same render check covers modes 13–22; music.md for audio source, layout, emoji frames and timing |
| AC4 Sensors | Met | sensors.md defines each metric and its source on any PC; the owner's devices and fan mapping are only in `docs/local/` |
| AC5 Settings | Met | settings.md; checklist rows T1–T6 |
| AC6 Parity checklist | Met | 73 rows, no row pending, every mode 1–22 and 100 present (mode coverage gate prints nothing); signed off 2026-10-23 |
| AC7 Safety and privacy | Met | Only DeepCreative, in its network-blocked prefix, wrote to the LP360. The commit guard passed on every commit. System changes were temporary and are in the PRD footprint (device access ACL, board sensor driver loaded by hand) |
| AC8 Records | Met | PRD open questions and capabilities updated with the owner's OK; README status; AGENTS.md already says plans exist from Phase 3 on |
| AC9 Expected frames | Met | Matrix coverage: 116 of 116 mode, rotation and unit cases pass; their frames are in `docs/spec/frames/` |

## Findings worth carrying into the build

- **Every metric shows itself.** In mode 18 with °F, DeepCreative draws the GPU temperature. OpenDeepCool draws the CPU temperature the mode names (music.md, checklist M10).
- **Stale scrolling text after a static mode** (found 2026-10-23). When a static picture takes over from a scrolling mode, DeepCreative's scrolling text pauses with its readings. At the next switch into any scrolling mode, it resumes one column after where it stopped and runs to the end of its cycle before the new text. Seen in 22 of 22 such switches. OpenDeepCool keeps no scrolling state across a switch, and the transition tests of Phases 7–8 must fail on any column from a third text (display.md, "Mode switches"; checklist D28).
- **Switch delay.** DeepCreative applies a switch on its next sensor snapshot, 35 ms to 1.9 s later. OpenDeepCool switches on the next frame.
- **Start-up.** DeepCreative shows 3 s of blank frames before a scrolling mode after a restart. OpenDeepCool doesn't wait (settings.md).
- **Music Mode spectrum** is OpenDeepCool's own analysis, Winamp style, over the whole audible range, in DeepCreative's layout (owner decision, 2026-10-22).
- **CPU power** needs a way past the root-only energy counter; Phase 6 decides it.
- **Still unverified:** what `03 08` means. OpenDeepCool sends it once at start-up, as DeepCreative does.

## How the evidence was gathered

- DeepCreative ran under Wine (11.20) in its own network-blocked prefix against the real cooler. Every USB write was logged with the system real-time clock.
- Chosen sensor values were injected where DeepCreative's main process receives its readings, and test tones were played for Music Mode.
- Analysis tools in `research/hook/` (git-ignored, local): `decode-frames.py`, `render-check.py`, `expected-frames.py`, `sweep.mjs`, and two added on 2026-10-23:
  - `transition-probe.mjs` runs scripted mode switches and records the time of each input;
  - `transition-analyze.py` lines the inputs up with every frame sent and labels each piece of text as old, new, stale, blank or unexplained, without anyone watching the cooler.
- Owner checks on the cooler were limited to pictures that hold still; anything faster was read from the frames.

## Commits

26 commits on `phase-3-spec`, from `69e4721` (research sessions get temporary LP360 access) to the record updates of 2026-10-23. Every spec document was committed on its own after the owner's check.

## Left for the owner

- The PRD footprint says the Wine prefix is deleted after Phase 3. The research rig is still running, the LP360 device access ACL is still granted, and the board sensor driver is loaded. All three go away at the next reboot, except the prefix itself (`research/wine/`), which needs the owner's decision.
- The branch `phase-3-spec` is ready for a pull request to `main`.
