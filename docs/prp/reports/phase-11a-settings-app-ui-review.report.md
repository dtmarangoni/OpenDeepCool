# Phase 11a report: the settings app's UI review

**Plan:** [phase-11a-settings-app-ui-review.plan.md](../plans/phase-11a-settings-app-ui-review.plan.md)
**Review:** [phase-11a-settings-app-ui-review.review.md](../solution-reviews/phase-11a-settings-app-ui-review.review.md)
**Status:** in progress (branch `phase-11a-settings-app-ui-review`)

## Tasks

One line for each finished task: the commit, what was checked, and anything the plan didn't foresee.

| Task | Commit | Result |
|---|---|---|
| 1. Settle the open points | `5b50f16` | Answers in `docs/design/settings-app.md` (Phase 11a Task 1). GTK reads the system preference from the portal and needs no code of ours; a fake portal in the tests needs an activatable name on the bus; `resizable` false floats the window on Hyprland; a frame-clock tick still runs for a hidden window, so the app stops it explicitly. Unverified: the frame clock for a window on another workspace with the screen on (repeated at sign-off) |
| 2. Keep the facts about fans and pump | the next commit | `docs/spec/fans-and-pump.md`: what DeepCreative does, how the app measured and showed them, the driver detection with its sandbox and undo, the saved keys, the contract, the screens, the parity rows, the decisions, and where the code stands in history (`1a6a4e9`). Machine facts stay in `docs/local/` |
