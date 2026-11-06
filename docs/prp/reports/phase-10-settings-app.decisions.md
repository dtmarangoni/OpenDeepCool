# Phase 10 decisions for the owner's review

The owner asked for Phase 10 to run unattended (2026-11-06): wherever a question comes up, the recommended choice is taken and recorded here, with the question, every option and what was implemented. The owner reviews them all at the phase's end; any change follows, then the sign-off.

Every entry below is **taken, pending the owner's review** unless it says otherwise.

## Pending real runs

The owner decided (2026-11-06) that the dev copy is rebuilt when the owner is back, with one `odc-dev setup` (one password), and the real checks that need it then run in one go. Each check here waits for that; the work it checks is committed with its in-process tests.

| Check | Why it waits | Command |
|---|---|---|

## Decisions

### D1 — Where the GPUs' PCI IDs come from (before Task 0)

- **Question:** Task 2 adds each GPU's PCI vendor and device ID to `Gpus`, and said the Phase 6 GPU scan already reads them. It doesn't: `odc_sensors::display_devices` reads each display controller's `class` file and `driver` link, and `Gpu` holds only the address, driver and kind.
- **Options:**
  1. The GPU scan reads the `vendor` and `device` files of the same PCI folder and carries them in `Gpu`.
  2. The service reads them separately when it builds the state, from the sysfs path of each GPU.
  3. No IDs: the app labels every GPU `<kind> · <driver> · <address>` and never shows a product name.
- **Chosen:** 1. The scan already opens that folder for each GPU, so one sysfs reader finds every GPU fact, and the design's product labels need the IDs.
- **Implemented in:** the plan's Task 2 (this commit); the code in Task 2.

### D2 — Five additions or six (before Task 0)

- **Question:** the plan's Approach lists five service additions; its Scope said "the six service and contract additions".
- **Options:** 1. five, the design's items 1–5 (its items 6 and 7 are behaviour: a refused `GetState()`, and watches that can be renewed); 2. six, counting the frame watch's two methods apart.
- **Chosen:** 1, matching the design's list.
- **Implemented in:** the plan's Scope (this commit).

### D3 — Which `odc_render` pieces draw the tiles (before Task 0)

- **Question:** the plan named `odc_render::{picture, music}` for the tiles, but `music` isn't a public module: Phase 9 built the Music frame from separate pieces.
- **Options:** 1. name the pieces as built: `picture` and `Marquee`, `with_spectrum` and `spectrum_height`, `EmojiLoop::of`, `Pose::new(0)`; 2. add a public `odc_render::tile(mode, values, unit, peaks)` that composes them, so the app makes one call.
- **Chosen:** 1 for now: the app's tile code composes them as the engine does. Whether a `tile` helper is worth adding is decided in Task 5, where the tiles are drawn, and recorded here then.
- **Implemented in:** the plan's Integration points (this commit).
