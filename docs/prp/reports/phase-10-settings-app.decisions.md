# Phase 10 decisions for the owner's review

The owner asked for Phase 10 to run unattended (2026-11-06): wherever a question comes up, the recommended choice is taken and recorded here, with the question, every option and what was implemented. The owner reviews them all at the phase's end; any change follows, then the sign-off.

Every entry below is **taken, pending the owner's review** unless it says otherwise.

## Pending real runs

The owner decided (2026-11-06) that the dev copy is rebuilt when the owner is back, with one `odc-dev setup` (one password), and the real checks that need it then run in one go. Each check here waits for that; the work it checks is committed with its in-process tests.

| Check | Why it waits | Command |
|---|---|---|
| `--only service`, every mode | Task 2 changed `odc-service` and `odc-ipc`; the installed dev copy is older | `cargo run -p odc-dev -- setup`, then `cargo run -p odc-e2e -- --only service --require dev-service` |

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

### D4 — The design's contrast figure for the dark accent on surface (Task 0)

- **Question:** the ported contrast check recomputes the design's table. Every figure matches to one decimal but one: dark "accent on surface" reads 7.6, while the WCAG ratio of `#4fc3b4` on `#1d2126` is 7.5495. The old script printed 7.55, and that was rounded again for the table.
- **Options:**
  1. correct the table to 7.5 and compare every figure with the exact ratio at one decimal;
  2. keep 7.6 and make the test round twice, as the table was made;
  3. compare with a tolerance of 0.06.
- **Chosen:** 1. The table should state the ratio itself. Both 7.5 and 7.6 are far above the 4.5 the check needs, so nothing in the design changes.
- **Implemented in:** `docs/design/settings-app.md` (Default theme), `tools/mockups/tests/mockups.rs`; Task 0's commit.

### D5 — How `odc-mockups` writes the mockups' JSON (Task 0)

- **Question:** each mockup embeds its state as JSON, written by the old generator with Python's defaults: `", "` and `": "` separators, keys in insertion order, non-ASCII characters as themselves. The port must give the same bytes.
- **Options:**
  1. a small JSON writer of the tool's own (about 60 lines), whose objects keep their order;
  2. `serde_json` with a custom `Formatter` for the separators, and `preserve_order` for the keys;
  3. write the JSON once and keep it as text in the tool.
- **Chosen:** 1. It's short, it needs no dependency in a dev tool, and a test pins its output. Option 3 would hide the states the design changes.
- **Implemented in:** `tools/mockups/src/lib.rs` (`Json`); Task 0's commit.

### D6 — Points of Task 1 that need the owner's desktop (Task 1)

- **Question:** two of the design's open points can only be seen in the owner's own session: whether Hyprland honours the window's minimum size when it tiles, and whether `GtkWindow:is-active` turns true again after a switch back from another user's session. The plan allows a few seconds of a throwaway window on the owner's desktop, but the owner is away and the agent notes say no test opens a window on the owner's desktop.
- **Options:**
  1. open the throwaway window anyway, unattended;
  2. leave both unverified, take the design's fallback where it has one, and look at both in the optional attended tour with the owner;
  3. stop until the owner is back.
- **Chosen:** 2. For `is-active`, the fallback is built in: the app also follows logind's `Active` property of its own session, so it connects again after a session switch either way, still without polling. For the minimum size, the app sets it as designed; whether Hyprland honours it changes nothing the app does.
- **Implemented in:** `docs/design/settings-app.md` "Unverified"; the app's connect triggers (Task 4).

### D7 — The dark or light base in GTK 4.20 and later (Task 1)

- **Question:** which GTK switch picks a dark or light base for the default theme.
- **Found** (docs.gtk.org, read 2026-11-06): `GtkSettings:gtk-application-prefer-dark-theme` is deprecated since 4.20, "use `GtkCssProvider` properties instead": `GtkCssProvider:prefers-color-scheme` (`GTK_INTERFACE_COLOR_SCHEME_LIGHT` or `_DARK`, since 4.20) sets the scheme a provider's `@media (prefers-color-scheme: …)` rules see, and changing it reloads the style sheet. `GtkSettings:gtk-interface-color-scheme` only carries the system's preference.
- **Options:** 1. set `prefers-color-scheme` on the app's own provider, from the theme's `mode` or the default theme's; 2. keep the deprecated setting; 3. set the system-wide `gtk-interface-color-scheme`.
- **Chosen:** 1, the documented replacement; the app's CSS gives every colour from the theme tokens anyway, so the base touches only what GTK draws itself. Whether GTK's own built-in theme follows the app provider's scheme is **unverified**; the layer 3 tests look at it.
- **Implemented in:** the design's "Unverified" answers (this task); the theme code (Task 7).

### D8 — The CSS provider call (Task 1)

- **Question:** whether `gtk_style_context_add_provider_for_display` is deprecated, and what replaces it.
- **Found** (docs.gtk.org): it carries no deprecation notice and is now declared in `gtkstyleprovider.h`, apart from the deprecated `GtkStyleContext`; gtk4-rs has it as `gtk::style_context_add_provider_for_display`.
- **Options:** 1. use it; 2. look for another way to style the whole display. **Chosen:** 1.
- **Implemented in:** the design's "Unverified" answers; the theme code (Task 7).

### D9 — Gradient border colours (Task 1)

- **Question:** whether GTK CSS can draw Hyprland's gradient border colours.
- **Found** (docs.gtk.org, CSS Properties): `border-color` follows CSS Backgrounds and Borders 3 and takes colours; `border-image` and `border-image-source` take an image, a gradient included.
- **Options:** 1. keep the gradient's first colour, as the design and review P10-05 already decided; 2. draw the border with `border-image` and the whole gradient.
- **Chosen:** 1: one colour everywhere is what the design specifies, and a `border-image` border wouldn't follow the controls' rounded corners in CSS.
- **Implemented in:** the design's "Unverified" answers; the theme's parser (Task 3).

### D10 — Accessible roles of the preview and the tiles (Task 1)

- **Question:** the default roles of `GtkFlowBoxChild`, `GtkDrawingArea` and labels.
- **Found** (docs.gtk.org, Accessibility): `GtkFlowBoxChild` is `GRID_CELL` (its box `GRID`), `GtkLabel` is `LABEL`; `GtkDrawingArea`'s default role isn't documented.
- **Options:** 1. rely on the defaults; 2. set the drawing areas' role to `IMG` when they're built, with their accessible label, and keep the others' defaults.
- **Chosen:** 2: the preview and each tile's picture are images, and setting the role removes the undocumented default from the question. The AT-SPI tests (Task 8) read every role.
- **Implemented in:** the design's "Unverified" answers; the window and tiles (Tasks 4–5).

### D11 — Behaviour the docs don't state, and how it's handled (Task 1)

- **Question:** four widget behaviours aren't stated in the docs: arrow keys between grouped `GtkToggleButton`s, live updates inside an open `GtkDropDown`, `GtkScale` while dragging, and tooltips on insensitive widgets.
- **Found:** `GtkRange:round-digits` rounds the value "when it changes", user drags included (docs.gtk.org), so a scale with 0 digits over 0–4 only ever holds whole stops.
- **Options per point:** check each now with a throwaway program in headless weston read through AT-SPI, or build the robust form now and let the layer 3 and 4 tests (Tasks 3–8) show which way GTK behaves.
- **Chosen:** the robust form for each, checked by the later tests:
  - each unit and rotation toggle is a Tab stop as well; arrow keys work too if GTK moves between them, which the tests record;
  - the dropdown's rows bind their RPM to the item's property, so they update while open if GTK redraws bound rows, and in any case when it reopens;
  - the falls' scales have `round-digits` 0 and apply each whole stop as it's reached (at most five calls a drag);
  - tooltips on unavailable tiles are extra: the reason line under each group is what the design relies on.
- **Implemented in:** the design's "Unverified" answers; Tasks 4–6 and their tests.

### D12 — Accessible names and value text (Task 1)

- **Question:** whether `update_property(&[Property::Label(…)])`, `Property::Description` and `Property::ValueText` reach AT-SPI.
- **Found** (gtk4-rs book, Accessibility): `AccessibleExtManual::update_property` sets `Label` (what screen readers announce) and `Description`; GTK's accessibility guide gives `ValueText` as ARIA's `aria-valuetext` (docs.gtk.org, Accessibility). What AT-SPI shows for each is **unverified** until the AT-SPI driver reads them.
- **Options:** 1. use them as designed and let Task 8's AT-SPI run prove each name; 2. check now with a throwaway program.
- **Chosen:** 1: Task 8 builds the AT-SPI driver anyway, and every coverage item reads a name through it.
- **Implemented in:** the design's "Unverified" answers; Tasks 4–8.

### D13 — Automatic, and how it names the GPU it reads (Task 1, the designer)

- **Question:** the wording of the GPU dropdown's first entry, the default, and how it shows which GPU it reads.
- **Options:** (a) "Automatic (Dedicated · …)" on the button and in the list; (b) "Automatic" with the GPU's name greyed beside it; (c) "Automatic" on the button and in the list, and a muted line under the dropdown, "Using Dedicated · Contoso Graphics R870.", from `GpuInUse`.
- **Chosen:** (c). Product names from `pci.ids` can be long: a line wraps, a button would cut them. The line is hidden when a GPU is chosen by hand, since the button names it then.
- **Implemented in:** the design (Configuration → Display, Accessible names, Text), the mockups' script, `configuration.html`; the app in Task 6.

### D14 — Saying why the GPU choice went back to Automatic (Task 1, the designer)

- **Question:** after a GPU is added or removed, the service clears the user's choice at its next start, with only a journal line. Should the app say why?
- **Options:** (a) say nothing; (b) a one-line note, which needs a new state key from the service; (c) the app remembers the old choice.
- **Chosen:** (a). The app stores nothing, (c) would break one setup for the whole PC, and the "Using …" line still shows which GPU is read. The old "saved GPU missing" state is gone from the design, since it can't happen.
- **Implemented in:** the design; `missing-input.html` replaces `missing-input-gpu.html`.

### D15 — The `no-access` banner (Task 1, the designer)

- **Question:** what the app shows when the service finds the cooler but can't open it.
- **Options:** a red or a yellow banner; the fix as words, as a command with a copy button, or as a button.
- **Chosen:** a red banner with the fix in words: "The cooler is plugged in, but the service can't open it. The udev rule that gives the service access is missing or not applied yet. Restart the PC; if that doesn't help, reinstall OpenDeepCool. The journal says more." Status "Cooler: no access". Nothing is disabled. No single command fixes both causes for both ways of installing, and the app never changes the system.
- **Implemented in:** the design (Banners, States, Text), `cooler-no-access.html`; the app in Task 4.

### D16 — `write-failed` (Task 1, the designer)

- **Question:** a failed write can last one tick, for example while the cooler is unplugged. What does the app show?
- **Options:** (a) as not connected; (b) its own banner at once; (c) its own banner after 2 s of `write-failed`.
- **Chosen:** (c): a yellow banner, "The cooler is connected, but sending it the picture fails. The service keeps trying by itself, and changes are still saved. The journal says more.", through a one-shot GLib timer, so nothing runs while the state is steady. Accepted gap: a cooler where every other write fails never reaches 2 s, so the app shows it as connected and only the journal has it.
- **Implemented in:** the design, `cooler-write-failed.html`; the app in Task 4.

### D17 — Which cooler state the app reads (Task 1, the designer)

- **Question:** `CoolerConnected` or `CoolerState`, and how to show other values (the dev copy's `released`, later ones).
- **Chosen:** `CoolerState` only (`CoolerConnected` stays for other clients); any other value shows as not connected.
- **Implemented in:** the design; the view model in Task 3.

### D18 — A GPU whose kind query failed (Task 1, the designer)

- **Question:** how the list shows a GPU of kind `query-failed`, ranked as unknown.
- **Options:** a kind word of its own; a hint under the dropdown; a note in the open list.
- **Chosen:** a note in the open list, "Unknown type · <name>" with the muted "driver couldn't be asked", listed with the unknown GPUs and choosable. The button and the list draw entries with separate factories (`GtkDropDown:list-factory`, docs.gtk.org), so the button shows the label alone. Only drivers with reading rules are asked for a kind, so a GPU can't carry both notes; if it ever did, "no readings" wins.
- **Implemented in:** the design, `configuration-gpu-list.html`; the app in Task 6.

### D19 — The mockups' sample GPU address (Task 1, the designer)

- **Question:** the made-up dedicated GPU in the mockups needed an address of its own.
- **Chosen:** `0000:0a:00.0`, the address the test fixtures already give their made-up card; and Automatic is now the base state's choice, so every page shows it.
- **Implemented in:** `tools/mockups/src/lib.rs`, every mockup. The odc-e2e chip-name test took the same made-up address (`fcda8de`).

### D20 — Which frames the preview gets (Task 1, from the designer)

- **Question:** Task 2 said the engine hands the D-Bus side "each written frame". While the cooler is away, nothing is written, but the design wants the preview to keep showing the saved picture.
- **Options:** 1. every frame the engine draws, written or not; 2. only written frames, and the preview freezes while the cooler is away.
- **Chosen:** 1, as the design asks: the preview shows what the cooler would show.
- **Implemented in:** the plan's Task 2 (this commit); the service in Task 2.

### D21 — Automatic and a GPU without a reading rule (Task 1, raised by the designer)

- **Question:** Automatic reads the first GPU of the ranking (kind, then address), not the first with a reading rule. With no ranking yet (first start, or the boot after a GPU change) every GPU is unknown, so Automatic could read a GPU without a rule while another has one.
- **Options:** 1. keep `sensors.md`'s rule in Phase 10: the app names the GPU in use, so the user sees it and can choose another; 2. change the service so Automatic skips GPUs without a rule, a spec change outside Phase 10's plan.
- **Chosen:** 1, for the owner to decide at the review whether the rule should change. The design works either way, because it names `GpuInUse`.
- **Implemented in:** nothing changed; recorded here.

### D22 — Where every fan input's speed is read (Task 2)

- **Question:** `FanInputs` gains each input's live RPM, about once a second. Which part reads them?
- **Options:**
  1. the sensor thread, in each snapshot, published to the status beside the bindings;
  2. the same thread, but inside the readings snapshot the engine draws from;
  3. the D-Bus side, reading sysfs when it builds the state.
- **Chosen:** 1. The sensor thread already owns every sysfs read, so a slow driver delays only it. Kept out of the snapshot, the speeds don't make the engine see new readings every second. `Sensors::fan_rpms` reads each input found at start, through `odc_sensors::fan_rpm`, and a change pulses the D-Bus side.
- **Implemented in:** `crates/odc-service/src/runtime.rs` (`Sensors::fan_rpms`, `RegistrySensors`), `service.rs`, `status.rs` (`fan_rpms`); Task 2's commit.

### D23 — Where the PCI IDs are read, and what a missing one is (Task 2)

- **Question:** D1 put the IDs in the GPU scan, `display_devices`, which five other callers use as `(address, driver)`. And sysfs may not give an ID.
- **Options:** read them in `display_devices` and change its callers, or in `gpus()`, the one place that builds `Gpu`; and for a missing ID, `None` in `odc-sensors` and 0 on the bus, or leave the GPU out.
- **Chosen:** `gpus()` reads each device's `vendor` and `device` files, `0x` and four hex digits, and anything else is `None`. On the bus `Gpus` carries 0, which the app treats as no product name: `<kind> · <driver> · <address>`.
- **Implemented in:** `crates/odc-sensors/src/gpu.rs`, `docs/spec/dbus.md`; Task 2's commit.

### D24 — What "the chosen input is present" means (Task 2)

- **Question:** the design needs `CoolerFanInputPresent` and `PumpInputPresent`. What do they say with no input chosen, and when does a chosen one count as gone?
- **Options:** with none chosen, true or false; gone when it isn't among the inputs found at start, or when its own reading says there's no device.
- **Chosen:** true with none chosen (the app shows "Choose input" from the empty `CoolerFanInput`), and false exactly when the chosen input's reading is "no device", so an input that vanishes while the service runs counts too.
- **Implemented in:** `crates/odc-service/src/dbus.rs` (`present`), `docs/spec/dbus.md`; Task 2's commit.

### D25 — How frame watchers count against the caps (Task 2)

- **Question:** watchers are capped at 8 per user and 32 in all, frame and state watchers together. Does the app, which does both, count once or twice?
- **Options:** one entry per caller with two watches, counted once; or each watch counted.
- **Chosen:** one entry per caller, counted once, dropped when it watches nothing any more or leaves the bus.
- **Implemented in:** `crates/odc-service/src/watchers.rs` (`Watch`), `docs/spec/dbus.md`; Task 2's commit.

### D26 — Who gets `FanInputs` in `StateChanged` (Task 2)

- **Question:** the plan sends `FanInputs`, whose speeds change every second, "only to frame watchers". What about a caller that watches frames but not the state, and the whole state a watcher gets after passing its check again?
- **Options:** `FanInputs` in `StateChanged` for callers that watch both; or a separate signal for the speeds.
- **Chosen:** `StateChanged` goes to state watchers as before, with `FanInputs` only for those that also watch frames, in the whole state after a check too. `GetState()` always has it. No new signal.
- **Implemented in:** `crates/odc-service/src/dbus.rs` (`tell_watchers`), `docs/spec/dbus.md`; Task 2's commit.

### D27 — A new frame watcher's first frame (Task 2)

- **Question:** `Frame` is sent "only on change". A caller that starts watching a still picture would then get nothing, and the preview would stay empty.
- **Options:** keep the last frame sent to each watcher, so a newcomer gets the next frame drawn (within a tick) and then only changes; or a `GetFrame()` method.
- **Chosen:** the first. A watcher that fails its check is forgotten, so it gets a frame as soon as it passes again.
- **Implemented in:** `crates/odc-service/src/dbus.rs` (`tell_frame_watchers`); Task 2's commit.

### D28 — A state value of another shape crashed the reader (Task 2, found)

- **Question:** testing the old `FanInputs` shape showed that zbus 6 panics, instead of returning an error, when it converts a structure with fewer fields than asked. A client reading another version's state would crash.
- **Options:** check each value's D-Bus signature against the contract before converting it; or catch the panic.
- **Chosen:** check the signature first: a value of another type is `WrongType`, as before, and nothing converts it. (Not reported upstream: no outside service is contacted without the owner.)
- **Implemented in:** `crates/odc-ipc/src/state.rs` (`from_dict`), `crates/odc-ipc/tests/contract.rs`; Task 2's commit.

### D29 — A CPU temperature check that failed once (Task 2, found)

- **Question:** the first `--only sensors` run after Task 2 failed `cpu_temp`: OpenDeepCool read 58 °C, `sensors` 36 °C. It ran right after several builds, and Task 2 doesn't touch the CPU temperature rules. A rerun after 20 s idle passed, 35 ≈ 35 °C.
- **Options:** 1. take the rerun, and record that the check reads OpenDeepCool and `sensors` at two moments, so a CPU cooling fast between them can fail it; 2. change the check to read `sensors` just before and after OpenDeepCool and pass within that range, outside Task 2's plan.
- **Chosen:** 1, for the owner to decide at the review whether the check should change (option 2).
- **Implemented in:** nothing changed; Task 2's commit message records both runs.
