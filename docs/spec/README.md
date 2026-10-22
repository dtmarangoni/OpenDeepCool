# OpenDeepCool spec

What DeepCool's DeepCreative app does with the LP360's 14×14 display, written so that OpenDeepCool can reproduce it without running DeepCreative. Phase 3 of the [PRD](../prp/prds/opendeepcool-lp360.prd.md) writes it; Phases 4–12 build from it.

## Documents

| Document | Covers | Used by |
|---|---|---|
| [protocol.md](protocol.md) | The USB messages, their timing, and what the display does without them | Phase 5 |
| `sensors.md` (to come) | What each metric means and where Linux has it | Phase 6 |
| [display.md](display.md) | Load Mode, Temperature Guardian, Power Guardian, Creative Mode, rotation | Phases 7, 8, 10 |
| `music.md` (to come) | Music Mode | Phase 9 |
| `settings.md` (to come) | Settings that change what the cooler shows | Phases 7, 11 |
| `parity-checklist.md` (to come) | Every item above, checked by the owner on the cooler | Every phase; Phase 12 |

## What the spec describes

- **DeepCreative 1.2.14**, run under Wine (11.17 in Phase 1, 11.20 in Phase 3) against the owner's real LP360.
- Only what DeepCreative's LP page offers. Features beyond it are listed in the PRD as after v1.

## Evidence labels

Every fact carries one of these labels, or says **unverified**:

| Label | Meaning |
|---|---|
| `capture: <log> <step>` | Seen in a USB capture of DeepCreative; `<step>` names the walkthrough step or session |
| `code: <path:line>` or `code: <string>` | Read in DeepCreative's own files (described, never copied) |
| `owner: <date>` | Checked by the owner on the physical cooler |
| `unverified` | Not yet checked; must not be relied on |

Captures, screenshots and DeepCreative's files stay in the git-ignored `research/` folder and are never committed. Facts about the owner's own PC are in `docs/local/` (also git-ignored).
