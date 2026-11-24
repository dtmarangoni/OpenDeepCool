# Design

The settings app's design. What is built is the design document; everything else here supports it or is kept as history.

| Path | What it is |
|---|---|
| [settings-app.md](settings-app.md) | **The design**: goals, the decisions log (D1 onwards), each page, the theme, keyboard use and accessible names, the coverage check. It is long because it keeps every decision with its date and its owner's words; read its sections by name, and use the decisions log to find why something is as it is |
| [palette-winter.md](palette-winter.md) | The two own themes, Dusk (dark) and Snowday (light): the palettes, their contrast and how they were chosen |
| [mockups/](mockups/) | The 24 mockup pages, one per screen or state. **Generated** by `cargo run -p odc-mockups -- docs/design/mockups` from the states in `tools/mockups/` and the sheet and script in `mockups-src/`; never edit a page by hand |
| [mockups-src/](mockups-src/) | The mockups' style sheet (`app.css`) and script (`app.js`): the designer's own files |
| [emoji-options/](emoji-options/) | History: the twenty emoji pictures shown to the owner on 2026-11-22 (`index.html`) and the three chosen (`chosen.html`: options 1, 6 and 11), now OpenDeepCool's own Music modes 14 to 16 and 24 to 26 ([music.md](../spec/music.md)) |

The design follows [the spec](../spec/) and the PRD's decisions; the plan that changed it last is [Phase 11a's](../prp/plans/phase-11a-settings-app-ui-review.plan.md). The `ui-ux-designer` agent ([its brief](../../.claude/agents/ui-ux-designer.md)) owns these files; the code follows them, with tests that pin the palettes and the contrast.
