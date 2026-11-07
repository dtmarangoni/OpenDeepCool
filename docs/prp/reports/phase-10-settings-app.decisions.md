# Phase 10 decisions for the owner's review

The owner asked for Phase 10 to run unattended (2026-11-06): wherever a question comes up, the recommended choice is taken and recorded here, with the question, every option and what was implemented. The owner reviews them all at the phase's end; any change follows, then the sign-off.

Every entry below is **taken, pending the owner's review** unless it says otherwise.

## Pending real runs

The owner decided (2026-11-06) that the dev copy is rebuilt when the owner is back, with one `odc-dev setup` (one password), and the real checks that need it then run in one go. Each check here waits for that; the work it checks is committed with its in-process tests.

| Check | Why it waits | Command |
|---|---|---|
| `--only service`, every mode | Task 2 changed `odc-service` and `odc-ipc`, and Task 5 `odc-render`, which the service links; the installed dev copy is older | `cargo run -p odc-dev -- setup`, then `cargo run -p odc-e2e -- --only service --require dev-service` |
| The dev copy's app, launcher entry and icon | Task 9 added them to `odc-dev setup` (D80–D82) | `cargo run -p odc-dev -- setup`, then `cargo run -p odc-dev -- check` (exits 0), and the launcher lists OpenDeepCool with its icon and opens the dev copy's app |
| The real app against the real dev service | Task 9's real run needs this build's dev service (D83, D84); the installed one predates Task 2, so `--only app` reports `app_real` failed with the setup fix until then | after `odc-dev setup`: `cargo run -p odc-e2e -- --only app --require dev-service,weston` |
| CI: `cargo deny check` and the build with gtk4 | `cargo deny` isn't installed on this PC, and the branch is pushed only at sign-off (D40, D41; the zbus 5 skips, D73) | the first push of `phase-10-settings-app` |

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

### D30 — How a theme's border widths are read (Task 3)

- **Question:** `shell.toml`'s own comment says a border width takes "one CSS-style scalar/list: N, "Y X", "T X B", or "T R B L"", with per-side keys such as `normal-border-width-left` over the list. The design has one token, `--ctl-normal-border-width`, and P10-05 lists only colours and alphas.
- **Options:** 1. take only a number, or a list's first number; 2. read the whole four-sides list and the per-side keys, as the comment describes, and write them as CSS's four sides; and in both, cap a width: at 16 px, or none.
- **Chosen:** 2, with each width a number from 0 to 16 px. Anything else, including `"1px"` or five numbers, takes the default theme's 1 px with a log line, like a rejected colour. GTK's `border-width` takes the same four sides (docs.gtk.org, GTK CSS Properties, read 2026-11-06).
- **Implemented in:** `crates/odc-app/src/theme.rs` (`four_sides`, `Reader::widths`, `MAX_BORDER_WIDTH`); Task 3's commit.

### D31 — The default theme's selection colour (Task 3)

- **Question:** the design's `--selection` token (selected text in the command label) has no value in its Default theme table.
- **Options:** the accent at Omarchy's own default `selection-fill-alpha`, 0.35, over the background, opaque; or the selected control fill.
- **Chosen:** the accent at 0.35 over the background. Text on it keeps 4.5:1 in both default palettes, which a test checks. With an Omarchy theme, `colors.toml` `selection` is used as given.
- **Implemented in:** `crates/odc-app/src/theme.rs` (`SELECTION_ALPHA`); Task 3's commit.

### D32 — What a missing `shell.toml` key falls back to (Task 3)

- **Question:** the design says a missing key takes "that token from the default theme". A control token is a colour key and an alpha key together (`normal-color` with `normal-fill-alpha`), and the default theme's control colour is its own foreground, which may clash with an Omarchy theme. Four more points were open: the colour of `--ctl-pressed-fill` (Omarchy gives only `pressed-fill-alpha`), and the default theme's focus fill and border, selected border, and border width, which its table doesn't give.
- **Options:** for a missing colour key, the default theme's foreground, or this theme's own foreground, as Omarchy's template writes every control colour (`$OMARCHY_PATH/default/themed/shell.toml.tpl`, read 2026-11-06); for a missing alpha, the default theme's alpha in either case.
- **Chosen:** this theme's own foreground for a colour key, and the default theme's alpha (dark or light by `mode`) for an alpha. The pressed fill uses `normal-color`. The default theme follows Omarchy's template where its table is silent: focus as hover, the selected border opaque, borders 1 px. A missing `shell.toml`, or one without `[controls]`, gives the default controls in the theme's foreground, with one log line.
- **Implemented in:** `crates/odc-app/src/theme.rs` (`controls`, `default_controls`); Task 3's commit.

### D33 — A `CoolerState` value the app doesn't know (Task 3)

- **Question:** the design shows any other `CoolerState` value, "one a later service adds", as `not-found`. But `odc-ipc`'s `State::from_dict` refuses a value it doesn't know (`BadValue`), so the app couldn't read the state at all, and the contract tests expect that refusal (Phase 7).
- **Options:** 1. the contract reads an unknown value as `not-found`; 2. the contract stays strict, and the app's client drops an unknown `CoolerState` before reading the rest, so the view model sees no cooler state and shows it as `not-found`.
- **Chosen:** 2. The service's other clients, the e2e checks among them, still catch a wrong value; the app alone shows it as the design says. The view model shows a missing state and `released` as `not-found` (Task 3); the client's part comes with the client (Task 4). Other unknown values, such as a later GPU kind, still refuse the whole state, as the contract says.
- **Implemented in:** `crates/odc-app/src/model.rs` (`shown`); `client.rs` in Task 4.

### D34 — A chosen input that's there but can't be read (Task 3)

- **Question:** the design gives five values for the fan and pump rows: an RPM, "Choose input", "Input not found", "No fan sensors found" and "—". A chosen input can be there (`CoolerFanInputPresent` true) while its speed can't be read (NaN, for example a permission error).
- **Options:** "No reading" under the preview and "no reading" in the picker, as the mockups' picker already writes it; or "Input not found" for it too.
- **Chosen:** "No reading", and "no reading" in the picker. The input is found, so "not found" would be wrong, and no other input's RPM ever stands in for it.
- **Implemented in:** `crates/odc-app/src/model.rs` (`Speed::NoReading`); Task 3's commit. The design gains the value with Task 4's window.

### D35 — The metric names in the service's reasons (Task 3)

- **Question:** the design says reasons come from the service unchanged and "the app adds only the metric name in front", in the display's names ("GPU usage: …"). The service already puts the metric in front, by the spec's DeepCreative names: "Gpu usage: no reading for this driver", "Memory Load: …" (Phase 7's `availability`).
- **Options:** 1. show the service's text as it is; 2. the app rewrites only the name in front to the display's (`CPU usage`, `GPU usage`, `RAM usage`, `CPU temperature`, …) and keeps the rest; 3. the service writes the display's names.
- **Chosen:** 2. It keeps rule 11 and the design's Text rule in the app, needs no service change, and leaves a text without a known name ("comes in a later version") as it is. The same rewrite applies to `ModeUnavailable`'s message when a change is refused (Task 4).
- **Implemented in:** `crates/odc-app/src/model.rs` (`reason_text`); Task 3's commit.

### D36 — The GPU's product name, and identical GPUs (Task 3)

- **Question:** the design labels a GPU `<kind> · <product name>` from `pci.ids`. A device line there reads like "Chip 12 [Graphics R870]": a chip name, then the product in brackets. And two identical cards would get the same label.
- **Options:** the whole device name, or the last bracketed part when the name ends with one; for identical labels, nothing, or each one's PCI address after it.
- **Chosen:** the last bracketed part, else the whole name. Identical labels get ` · <address>`, so each entry can be told apart. A GPU without an entry stays `<kind> · <driver> · <address>`.
- **Implemented in:** `crates/odc-app/src/pci_ids.rs` (`product`), `crates/odc-app/src/model.rs` (`gpu_labels`); Task 3's commit.

### D37 — The fake service is built with its first user (Task 3)

- **Question:** the plan puts `odc-testkit/src/fake_service.rs` in Task 3, but lists no test for it there. Its only users are Task 8's scenarios, whose needs decide its shape: refusals, leaving the bus, the state file, the call log.
- **Options:** build it now, untested; or build it in Task 8, with its tests and its first user.
- **Chosen:** Task 8. Task 3 builds the weston harness and the pure modules, each with its tests.
- **Implemented in:** the plan's Task 3 note; `fake_service.rs` in Task 8.

### D38 — How the headless weston runs (Task 3)

- **Question:** the plan names `Weston::headless()`, with a socket of its own, removed when dropped, and "a killed test leaves no socket". Open: where the socket lives, which shell and renderer, and how a killed test's weston ends, since the workspace forbids `unsafe` and Rust's standard library can't set a parent-death signal.
- **Options:** the socket in the user's own runtime folder, or in a private one the test gives, with `WAYLAND_DISPLAY` holding its full path (libwayland takes one since 1.15); weston's desktop shell, or its kiosk shell; for a killed test, start weston through util-linux's `setpriv --pdeathsig KILL`, or leave it running.
- **Chosen:** a private runtime folder (`Weston::headless(dir)`, created 0700), the kiosk shell (no panel, no lock screen), the pixman renderer (no GPU needed in CI), a 1280 × 800 output and `--no-config`, all through `setpriv --pdeathsig KILL`. Weston dies with the thread that started it; a killed test's socket file can't be removed by the dead test, so the next start in the same folder removes every socket that takes no connection. The `weston` capability is `tool()`'s probe, unusable with the fix when missing, as Task 8 asks.
- **Implemented in:** `crates/odc-testkit/src/weston.rs`, `crates/odc-testkit/src/hardware.rs` (`Capability::Weston`), `crates/odc-app/tests/weston.rs`; Task 3's commit.

### D39 — A colour's own alpha (Task 3)

- **Question:** P10-05 accepts `#RRGGBBAA` and `rgba(RRGGBBAA)`, with an alpha of their own. The design wants the window opaque.
- **Options:** keep the alpha everywhere; drop it everywhere; or drop it for `colors.toml`'s base colours and multiply it by the key's alpha for `shell.toml`'s control colours.
- **Chosen:** the third. The window, its text and its surfaces stay opaque, and a control colour with its own alpha keeps it, times its `*-alpha` key.
- **Implemented in:** `crates/odc-app/src/theme.rs` (`theme`, `Reader::colour`); Task 3's commit.

### D40 — target-lexicon's licence (Task 3)

- **Question:** gtk4's build scripts use `system-deps`, which brings `cfg-expr` and `target-lexicon`. Its licence is "Apache-2.0 WITH LLVM-exception", which `deny.toml` doesn't allow, so `cargo deny check` would fail.
- **Options:** allow that licence everywhere; or an exception for that one crate, with the reason.
- **Chosen:** the one-crate exception (cargo-deny's `exceptions`, embarkstudios.github.io/cargo-deny, licenses config, read 2026-11-06). The LLVM exception only relaxes Apache's terms for binaries built with it, and the crate is a build dependency only: `cargo tree -i target-lexicon -e normal` finds nothing. `cargo deny` isn't installed on this PC, so CI checks it at the first push (Pending real runs).
- **Implemented in:** `deny.toml`; Task 3's commit.

### D41 — CI installs gtk4 from Task 3 (Task 3)

- **Question:** the plan adds CI's packages in Task 10, but the workspace build links gtk4 from Task 3 on, so CI's test job would fail until then.
- **Options:** add `gtk4` to the test job now, and the rest (`weston`, `at-spi2-core`, `hwdata`) with their jobs; or all of them now.
- **Chosen:** `gtk4` now, in the commit that adds the dependency; the rest with the UI job in Task 10.
- **Implemented in:** `.github/workflows/ci.yml`; Task 3's commit.

### D42 — What the view model's window input holds (Task 3)

- **Question:** the plan's `Ui` holds "the page, the open dropdown".
- **Options:** keep the open dropdown in it; or leave it to GTK, which owns the popup, with every entry always carrying its note and RPM.
- **Chosen:** the page and the `write-failed` wait's end (`sending_failed_long`), which the design asks to be an input like a state change. The open dropdown changes nothing the view model decides.
- **Implemented in:** `crates/odc-app/src/model.rs` (`Ui`, `SendingFails`); Task 3's commit.

### D43 — How a disabled control looks without `opacity` (Task 3)

- **Question:** the mockups dim disabled controls with `opacity: .45`, but the plan forbids `opacity` in the app's CSS.
- **Options:** muted text on the normal fill and border; or a computed dimmer fill.
- **Chosen:** muted text on the normal fill and border. The unavailable tiles' reasons are visible text under each group heading anyway, and Task 5 draws an insensitive tile's picture itself.
- **Implemented in:** `crates/odc-app/src/app.css`; Task 3's commit.

### D44 — AGENTS.md gains the app's crate and its weston tests now (Task 3)

- **Question:** the plan updates AGENTS.md in Task 10, but AGENTS.md says a changed rule or command is written there in the same commit, and Task 3 adds a crate and a test command.
- **Options:** wait for Task 10; or add the crate to the layout and the weston test command now, and the rest (`--only app`, the design-first rule) in Task 10.
- **Chosen:** now, as AGENTS.md asks; Task 10 adds the rest.
- **Implemented in:** `AGENTS.md`; Task 3's commit.

### D45 — Where the client runs (Task 4)

- **Question:** the plan puts "the `odc-ipc` proxy on a zbus thread", with updates reaching GTK's main loop "through a glib channel". glib-rs no longer has `MainContext::channel`; the gtk4-rs book now uses the `async-channel` crate, and says I/O from crates of the `futures`/`smol` family can run on the main loop with no channel at all (gtk4-rs book, The Main Event Loop, read 2026-11-06).
- **Options:** 1. a thread of the app's own with the blocking proxy, and `async-channel` to the main loop (a new dependency); 2. the async proxy awaited on GLib's main loop with `glib::spawn_future_local`: zbus's default runtime runs a connection's I/O, timers and tasks on two threads of its own (zbus README, Compatibility with async runtimes, read 2026-11-06), so nothing blocks the window.
- **Chosen:** 2. No thread, channel or crate of the app's own, and the window is only ever touched from its thread. The streams are subscribed before `Watch()`, then `GetState()` is applied, then every `StateChanged` in the order it's read, as `odc-audio` does; a `StateChanged` sent before the reply but read after it only replays an older value that a later one, or the reply, already holds.
- **Implemented in:** `crates/odc-app/src/client.rs`; Task 4's commit.

### D46 — A state the app can't read (Task 4)

- **Question:** `GetState()` succeeds but its state doesn't read (a value of another type, or an unknown value other than `CoolerState`'s, D33): what does the window show?
- **Options:** the service-down state; the not-available message; or a new banner, "The service's state can't be read", which the design doesn't have.
- **Chosen:** the service-down state, with the reason in the journal (`opendeepcool: the service's state can't be read: …`). Only a service and an app of different versions can cause it, and they ship together; a new banner would need the designer first.
- **Implemented in:** `crates/odc-app/src/client.rs` (`connect_once`); Task 4's commit.

### D47 — The app started by a test never hands over (Task 4)

- **Question:** the app is one instance per user (GTK's default). A test's app (`--service-address` or `--omarchy-state`) that finds the user's own window open would hand over to it and exit, and the test would drive the user's app.
- **Options:** keep one instance always; or `G_APPLICATION_NON_UNIQUE` when a test flag is given.
- **Chosen:** non-unique with a test flag; one instance per user otherwise.
- **Implemented in:** `crates/odc-app/src/main.rs`, `crates/odc-app/src/flags.rs` (`Flags::testing`); Task 4's commit.

### D48 — The accessible role of a named box (Task 4)

- **Question:** the design gives the banners and the not-available message, each a `GtkBox`, an accessible name, and says roles are GTK's defaults. A `GtkBox` defaults to the generic role, which ARIA lets carry no name, so a screen reader may skip it.
- **Options:** keep the generic role; group; or alert and status, as the mockups write them, though GTK fixes a widget's role when it's built, and the one cooler banner is an error or a warning by turns.
- **Chosen:** group for the three named boxes. The banners' words are their descriptions. The design's answered points say so, and the AT-SPI driver (Task 8) reads what screen readers get.
- **Implemented in:** `crates/odc-app/src/ui/banners.rs`, `crates/odc-app/src/ui/window.rs`; `docs/design/settings-app.md`; Task 4's commit.

### D49 — Announcements crash GTK's test accessibility backend (Task 4, found)

- **Question:** the layer 3 tests run GTK with `GTK_A11Y=test`, its test backend (docs.gtk.org, Running GTK applications), so the accessible properties can be checked in the test's process. There `gtk_accessible_announce` calls a null function and the process dies (SIGSEGV, backtrace through `gtk_accessible_announce`): the test backend seems to have no announcements.
- **Options:** run the tests without an accessibility backend, so no property can be checked; or send announcements through a function the window holds, GTK's own by default and a recorder in the tests.
- **Chosen:** the second: the tests also check what's announced and when (the not-available message once, each refusal's words). GTK's own call stays for the real app; AT-SPI in Task 8 is where it can be seen. Not reported to GTK: no outside service is contacted without the owner.
- **Implemented in:** `crates/odc-app/src/ui/window.rs` (`announce_with`), `crates/odc-app/tests/window.rs`; Task 4's commit.

### D50 — The page switcher (Task 4)

- **Question:** the design names the header's switcher a `GtkStackSwitcher` "(or grouped `GtkToggleButton`)". A `GtkStackSwitcher` builds its own buttons, whose accessible names and sensitivity the app doesn't set.
- **Options:** `GtkStackSwitcher`; or two grouped `GtkToggleButton`s that switch the page.
- **Chosen:** the two toggles, named Display and Configuration, insensitive while the service is down and hidden when not available, as the design's names table allows.
- **Implemented in:** `crates/odc-app/src/ui/window.rs`; Task 4's commit.

### D51 — The preview's size (Task 4)

- **Question:** the design calls the preview "a `GtkDrawingArea`, 226 × 226 px, in a frame with the surface colour"; the mockups draw a 210 px grid (15 px per dot) inside a 226 px frame with 8 px of padding.
- **Options:** a 226 px drawing; or the mockups' 210 px drawing in a 226 px frame.
- **Chosen:** the mockups': the same picture as the design shows, and the tests' dot centres follow from 15 px per dot. The design's text now says so.
- **Implemented in:** `crates/odc-app/src/ui/preview.rs`; `docs/design/settings-app.md`; Task 4's commit.

### D52 — Where the layer 3 tests run (Task 4)

- **Question:** the plan runs layer 3 "in-process, in `Weston::headless()`". GTK must run on the main thread, reads its display and settings from the environment, and the workspace forbids the `unsafe` that changing a test process's environment takes in Rust 2024. Started as it is, GTK would reach the user's own session: its display, session bus, GTK settings and `gtk.css`.
- **Options:** run the checks in the test process as started; or, with a harness of the test's own, have it start weston and run itself again inside it, with an environment that reaches only weston: its socket, the Cairo renderer, GTK's test accessibility backend, no session bus, in-memory GSettings, and configuration, cache and runtime folders of its own.
- **Chosen:** the second. The app's code still runs in the test's own process, against a fake client, and nothing of the owner's session is touched.
- **Implemented in:** `crates/odc-app/tests/window.rs`; Task 4's commit.

### D53 — Whether "Refreshed" is announced (Task 4)

- **Question:** the design has screen readers get a refusal's words; it doesn't say whether "Refreshed" is announced.
- **Options:** announce it too; or only refusals.
- **Chosen:** only refusals, as the design writes it; the label itself carries "Refreshed", named Change feedback.
- **Implemented in:** `crates/odc-app/src/ui/window.rs`; Task 4's commit.

### D54 — How the tiles draw a fixed spectrum (Task 5)

- **Question:** the Music tiles show "a fixed sample spectrum", drawn by the service's own code. `odc_render::Spectrum` keeps its levels private, and reaching a given pair of bars and peaks through `update()` takes a contrived run of ticks.
- **Options:** 1. the app works out the drawn columns itself from the sample levels, a second copy of `Spectrum::draw`'s rule; 2. a run of `update()` ticks that ends near a sample; 3. a constructor in `odc_render`, `Spectrum::at(bars, peaks)`, a spectrum standing at those levels, which changes nothing the service draws.
- **Chosen:** 3, with the mockups' sample levels (`app.js`, `SAMPLE_SPECTRUM`). The tiles then use the service's own `draw()` and `with_spectrum`. Its test: `at(levels, levels)` draws as one tick at those levels, and a peak above its bar draws its dot. As AGENTS.md asks for a change to `odc-render`, layer 2 and `--only display` ran before the commit.
- **Implemented in:** `crates/odc-render/src/spectrum.rs`, `crates/odc-render/tests/spectrum.rs`, `crates/odc-app/src/ui/tiles.rs`; Task 5's commit.

### D55 — How the four tile boxes act as one radio group (Task 5)

- **Question:** the design makes each `GtkFlowBox` single-selection and has choosing a tile unselect the other three. A single-selection box moves its selection with keyboard focus, so a tile could look chosen though only focus moved; and a click would select a tile before the service confirms it, against the design's Refused changes.
- **Options:** single selection, put back after each move or click; or no selection by GTK at all, the app marking the saved mode's tile (a `selected` style class and the accessible selected state).
- **Chosen:** the second. Activation (Enter, Space, a click) still emits `child-activated`, which asks for the mode; the mark follows the service's state only. The design's text now says so.
- **Implemented in:** `crates/odc-app/src/ui/display_page.rs`, `crates/odc-app/src/ui/tiles.rs`, `crates/odc-app/src/app.css`; `docs/design/settings-app.md`; Task 5's commit.

### D56 — How a tile's dots are coloured (Task 5)

- **Question:** the preview's unlit dots are `--fg` at 0.12, blended over its surface. A tile's background changes with its state (normal, hover, selected), and the design doesn't say how an unavailable tile's picture looks.
- **Options:** unlit dots blended over one fixed colour, or `--fg` at 0.12 drawn over whatever is behind; an unavailable tile's lit dots in `--fg` or in `--muted`.
- **Chosen:** unlit dots at 0.12 alpha over the tile's own fill, and an unavailable tile's lit dots in `--muted`, so it reads as unavailable before its words are read.
- **Implemented in:** `crates/odc-app/src/ui/dots.rs`, `crates/odc-app/src/ui/tiles.rs`; `docs/design/settings-app.md`; Task 5's commit.

### D57 — The tile picture's accessible name (Task 5)

- **Question:** the design's answered points build every tile picture with the image role "and their accessible label", but its names table names only the tile (`Load: CPU`).
- **Options:** the picture named as its tile; a name of its own ("Load: CPU picture"); or no name, hidden from screen readers.
- **Chosen:** named as its tile, kept out of the app's own list of names, so the tests and the AT-SPI driver find each tile by its name and role, a grid cell, and its picture by the image role.
- **Implemented in:** `crates/odc-app/src/ui/tiles.rs`; Task 5's commit.

### D58 — The plan's GPU "not found" (Task 6, a correction)

- **Question:** the plan's Task 6 still asks for the GPU dropdown's "— not found". Since the owner's change of 2026-11-04 the service clears the choice at start when a GPU is added or removed, so a saved GPU is never missing, and the design has no such state.
- **Options:** keep the plan's line; or follow the design and the owner's later decision.
- **Chosen:** the design: no GPU "not found". The dropdown has Automatic, the GPUs with their notes, the line naming the GPU in use and the no-readings hint.
- **Implemented in:** the plan's Task 6 note; `crates/odc-app/src/ui/config_page.rs`; Task 6's commit.

### D59 — A saved fan input that wasn't found at all (Task 6)

- **Question:** a saved input missing since the service started isn't among `FanInputs`, so the picker had no entry to show as chosen, and a `GtkDropDown` with nothing chosen shows an empty button, not the design's "<chip> · <input> — not found".
- **Options:** add the saved input to the list, last, with the note "not found"; or a button drawn apart from the list.
- **Chosen:** the saved input last in the list, chosen, with "not found". Choosing it again asks for nothing new.
- **Implemented in:** `crates/odc-app/src/model.rs` (`Fan::row`) and its test; Task 6's commit.

### D60 — How the dropdowns are built (Task 6)

- **Question:** the design's dropdowns draw a check mark, a label and a note per entry, with live RPM. The usual gtk-rs way is an item type of the app's own, a GObject subclass whose properties the rows bind to. Its macros expand to `unsafe` code; whether the workspace's `forbid(unsafe_code)` lets that through is unverified.
- **Options:** a GObject item subclass; or a `GtkStringList` of the labels with `SignalListItemFactory` rows that look their entry up by position in the app's own list.
- **Chosen:** the string list. The button shows the view's own words (so "— not found" reads there), each open row is named by its label and described by its note (`ListItem::set_accessible_label` and `set_accessible_description`, GTK 4.12), the open rows' notes are set again in place on every state, and the check mark shows by opacity so the labels line up. A pick goes to the client, and the confirmed entry shows again until the service confirms.
- **Implemented in:** `crates/odc-app/src/ui/dropdowns.rs`; Task 6's commit.

### D61 — How a scale asks for a stop (Task 6)

- **Question:** the design calls `SetMusicBarFall` or `SetMusicPeakFall` "when the value lands on a new stop", and a control shows a new value only once the service confirms it. Putting a scale back while the user drags it fights the drag.
- **Options:** put it back at once, as the toggles do; or let it follow the drag, ask once for each new stop, and go back to the saved stop when the service refuses.
- **Chosen:** the second. The window shows its whole state again after every refusal, so a refused stop goes back; a confirmed one arrives in `StateChanged`.
- **Implemented in:** `crates/odc-app/src/ui/config_page.rs`, `crates/odc-app/src/ui/window.rs` (`refusal`); Task 6's commit.

### D62 — The Configuration cards' names (Task 6)

- **Question:** the design's names table has no entry for the three cards; the mockups label them "Display settings", "Cooler settings" and "Music settings".
- **Options:** no names; or the mockups' labels, as groups.
- **Chosen:** the mockups' labels, each card a group; the design's names table now lists them.
- **Implemented in:** `crates/odc-app/src/ui/config_page.rs`; `docs/design/settings-app.md`; Task 6's commit.

### D63 — The peak dots switch until the service confirms (Task 6)

- **Question:** a `GtkSwitch` flips at a click. The design wants the new value only once confirmed.
- **Options:** flip it back by hand; or GTK's delayed state: the `state-set` handler returns `Stop`, so the switch keeps its state until the app sets it (`GtkSwitch`, docs.gtk.org).
- **Chosen:** the delayed state: the handle moves, the state follows the service's `MusicPeaks`.
- **Implemented in:** `crates/odc-app/src/ui/config_page.rs`; Task 6's commit.

### D64 — The scale's knob (Task 6)

- **Question:** GTK's own theme draws a scale with marks with a pin-shaped knob. Restyling it (a round knob in `--fg`) made GTK warn that the knob's minimum height was negative: its built-in theme's margins, which aren't documented, fight the app's sizes.
- **Options:** restyle the knob against GTK's undocumented theme; or leave the knob to GTK and give only the trough and its filled part the theme's tokens.
- **Chosen:** GTK's knob; the trough takes the control fill and border, its filled part the accent (checked in the saved picture: `#4fc3b4`).
- **Implemented in:** `crates/odc-app/src/app.css`; Task 6's commit.

### D65 — Which folders the theme watches (Task 7)

- **Question:** the plan watches `current/`. That misses an edit inside `current/theme/`, and a `current/` that doesn't exist yet: away from Omarchy, or before its first theme. Whether GLib's monitor on a missing folder reports its creation, and how soon, is unverified.
- **Options:** only `current/`; or each of the state folder, `current/` and `current/theme/` that exists, the nearest existing folder above while none does, all watched again after each reload, and the theme applied only when its tokens changed, so an unrelated change in a watched folder costs one read.
- **Chosen:** the second. The layer 3 test finds a theme written into a state folder that didn't exist within its wait, and an Omarchy switch within 1 s.
- **Implemented in:** `crates/odc-app/src/ui/theme.rs` (`watched`, `LiveTheme`); `docs/design/settings-app.md`; Task 7's commit.

### D66 — Where the theme's log lines go (Task 7)

- **Question:** the design has the journal name each missing or rejected key. The app is a desktop program, not a unit of its own.
- **Options:** standard error; or the journal's own protocol, a new dependency.
- **Chosen:** standard error, `opendeepcool: theme: …`, one line each: the source at start, each reload and each problem. Whether the session's journal gets a desktop app's standard error on Omarchy depends on how the session starts it and is unverified; the attended tour (Task 9) can look.
- **Implemented in:** `crates/odc-app/src/main.rs` (`log`), `crates/odc-app/src/ui/theme.rs`; Task 7's commit.

### D67 — Choosing a tile through AT-SPI (Task 8, found)

- **Question:** the plan has every tile chosen through AT-SPI. GTK gives a `GtkFlowBoxChild` no AT-SPI action of its own, and the flow box's AT-SPI selection selects nothing, since the boxes select nothing (D55). GTK does offer assistive technology a widget's own enabled actions that take no parameter, and only while the widget is sensitive (`widget_action_vtable` in `gtk/a11y/gtkatspiaction.c`, GTK's source, read 2026-11-07).
- **Options:** keep the tiles keyboard-only and their 23 calls in layer 3; or give each tile an action group with `activate`, which runs the tile's own activation, as Enter, Space or a click does.
- **Chosen:** the action, `tile.activate`. A screen reader user gets it in the tile's action list, an unavailable tile offers none, and the driver chooses all 23 tiles through it.
- **Implemented in:** `crates/odc-app/src/ui/tiles.rs`; the layer 3 checks in `crates/odc-app/tests/display_page.rs` (every tile's action asks for its mode once, an unavailable tile's asks for nothing); `docs/design/settings-app.md` (Accessible names); Task 8's commit.

### D68 — A button's accessible name (Task 8, found)

- **Question:** through AT-SPI the unit toggles read "°C" and "°F", and both choose-input links "Choose input", not the design's "Celsius", "Fahrenheit", "Choose cooler fans input" and "Choose pump input". `gtk_button_set_label` gives a button a `LABELLED_BY` relation to its label child (GTK's source, `gtk/gtkbutton.c`), and a labelled-by relation wins over the accessible label, as in ARIA's name computation. Layer 3 could only check that a label was set, not the name AT-SPI computes.
- **Options:** change the design's names to the visible labels; or drop the relation when the app names a widget.
- **Chosen:** drop it: `Names::name` resets `LabelledBy` after setting the label, so every widget carries the design's name ("Celsius" reads better than "degree C").
- **Implemented in:** `crates/odc-app/src/ui/names.rs`; checked by the layer 4 scenario `names`; Task 8's commit.

### D69 — Picking a dropdown entry through AT-SPI (Task 8, found)

- **Question:** an open dropdown's rows offer AT-SPI only `listitem.scroll-to`: GTK's `listitem.select` and `list.activate-item` take parameters, which GTK leaves out. The list's AT-SPI selection moves the open list's highlight, but the dropdown takes an entry only when the list is activated, and then from that list's own selection, ignoring the position it's given (`row_activated` in `gtk/gtkdropdown.c`, GTK's source). Through AT-SPI no entry could be picked.
- **Options:** keep the three pickers in layer 3 only, as the plan's risk row allows; or give each open row an action that does what a click does: the row's `listitem.select`, then the list's `list.activate-item` for its position.
- **Chosen:** the action, `row.activate`, on the row's own box. Given to GTK's row widget while the row was bound, it broke GTK (criticals, then an abort: "Unable to create shared memory pool"), so it stays on the app's own widget.
- **Implemented in:** `crates/odc-app/src/ui/dropdowns.rs` (and `Dropdown::activate_row` for the layer 3 check in `crates/odc-app/tests/config_page.rs`); `docs/design/settings-app.md`; Task 8's commit.

### D70 — The switcher while the service is down (Task 8, found)

- **Question:** with the service down, AT-SPI showed the Display and Configuration toggles as sensitive: the app made their box insensitive, and `gtk_widget_set_sensitive` marks only the widget itself disabled for assistive technology, not its children (GTK's source, `gtk/gtkwidget.c`). They looked usable to a screen reader.
- **Options:** keep the box; or set each toggle's own sensitivity.
- **Chosen:** each toggle's own. The tiles and the dropdowns already set their own.
- **Implemented in:** `crates/odc-app/src/ui/window.rs` (`Header::switcher`, and the test getter `Window::header`); Task 8's commit.

### D71 — The keyboard shortcuts in layer 4 (Task 8)

- **Question:** the plan tries the shortcuts through AT-SPI's key events, and else keeps them in layer 3 with the report saying so. The registry accepts `GenerateKeyboardEvent`, but on Wayland nothing reaches the app (F5 tried on the development PC, 2026-11-07: no Refresh). The window does offer AT-SPI its actions: `win.page-display`, `win.page-config`, `win.refresh`, `win.close`, with no key bindings in the answer.
- **Options:** the shortcuts in layer 3 only; or the actions through AT-SPI in layer 4, and each action's keys checked in layer 3.
- **Chosen:** both halves: the layer 4 scenario `shortcuts` runs the three actions through AT-SPI and tries F5 every run, saying in its note whether it arrived; the layer 3 check `the_window_s_requests` now checks every action's keys (`accels_for_action`).
- **Implemented in:** `crates/odc-ui-e2e/src/scenarios.rs` (`shortcuts`), `crates/odc-app/tests/window.rs`; Task 8's commit.

### D72 — A fixed name for a label (Task 8, found)

- **Question:** the design names eleven labels ("Cooler status", "Pump speed", "Change feedback", "Unavailable reading", "GPU in use", each group's reason line), so tests find them by name and read their values. GTK sets a label's accessible label to its own text at every change of it, unless its role forbids a name (`gtk_label_set_text_internal`, GTK's source, `gtk/gtklabel.c`): through AT-SPI "Cooler status" read "Cooler connected".
- **Options:** let labels carry their text as their name and drop them from the names table; set the name again after every text change, so the name says "Cooler status" and the text the value, which a screen reader may read without the value; or a group around each label, named as the design names the label, as for the banners (D48).
- **Chosen:** the named group: the status items, the two speed rows, the reason under the preview, the feedback, the GPU-in-use line and the four groups' reason lines. A screen reader reads "Cooler status, Cooler connected", and the name never holds a value.
- **Implemented in:** `crates/odc-app/src/ui/names.rs` (`group`), `window.rs`, `display_page.rs`, `config_page.rs`; the layer 3 names check (their role is now group); `docs/design/settings-app.md` (Accessible names); Task 8's commit.

### D73 — atspi on zbus 5, and cargo-deny (Task 8)

- **Question:** `atspi` 0.30.0 is still the newest (crates.io, 2026-11-07) and is built on zbus 5; the workspace uses zbus 6. The plan keeps the driver a binary crate of its own, which links no workspace crate.
- **Options:** atspi, as the plan says, with cargo-deny skips for the second zbus; or the few AT-SPI proxies the driver needs written on zbus 6, with no second zbus but more code of our own.
- **Chosen:** atspi, with only its `proxies` and `zbus` features (no connection or peer-to-peer code). The workspace then has zbus 5.19.0 and zbus_macros 5.19.0 beside zbus 6, the only new duplicates (`cargo tree -d`); `deny.toml` skips the two, with the reason, until atspi moves to zbus 6, which each sign-off checks. Every new crate is MIT or Apache-2.0 (`cargo metadata`). `cargo deny check` itself isn't installed here: CI runs it at the first push (Pending real runs).
- **Implemented in:** `crates/odc-ui-e2e/Cargo.toml`, `deny.toml`; Task 8's commit.

### D74 — The accessibility bus of the test (Task 8, found)

- **Question:** the app's AT-SPI tree must be on a bus the driver reaches and that nothing of the user's session shares. Three tries on the development PC, 2026-11-07: a private session bus that starts at-spi2-core's own launcher started the bus through dbus-broker, which starts the registry only through systemd ("unit failed"); a private dbus-daemon as the accessibility bus, starting the registry on demand, worked, but the registry outlived its bus, one process left per run.
- **Options:** the user's own accessibility bus (not isolated, and absent in CI); the launcher; the registry started on demand; or a private dbus-daemon as the accessibility bus, the registry started by the harness from the program its installed service file names, both through `setpriv --pdeathsig KILL`, and GTK pointed at it with `AT_SPI_BUS_ADDRESS`.
- **Chosen:** the last (`odc_testkit::bus::PrivateBus::accessibility`), without the service file's `--use-gnome-session`, since there's no session manager. `AT_SPI_BUS_ADDRESS` is GTK's own environment (GTK's source reads it first), like `GSK_RENDERER`, not an option of ours (AGENTS.md rule 5). A test checks that the bus holds the registry, starts nothing on demand, and leaves no process when dropped.
- **Implemented in:** `crates/odc-testkit/src/bus.rs`, `crates/odc-testkit/tests/fake_service.rs`; Task 8's commit.

### D75 — The weston of the end-to-end run (Task 8, found)

- **Question:** under weston's kiosk shell, which the in-process tests use, GTK hid the window's header bar (the switcher and Refresh): the kiosk shell fullscreens every window. And with no seat, the window never turned active, so the reconnect on activation couldn't be reached.
- **Options:** the kiosk shell, the header bar left out of layer 4; or the desktop shell with headless weston's `--fake-seat` (`weston --help`, weston 15).
- **Chosen:** the desktop shell and a fake seat (`Weston::desktop`): the header bar shows, the window is active, and it turns inactive and active again when another window opens and closes (D78). The in-process tests keep the kiosk shell.
- **Implemented in:** `crates/odc-testkit/src/weston.rs`; Task 8's commit.

### D76 — The fake service's files (Task 8)

- **Question:** the plan gives the fake its state "from a JSON file" and logs its calls to a file. JSON needs `serde_json`, new to the workspace; the state must also say what the fake answers: on the bus or not, reads allowed or not, changes refused or not.
- **Options:** JSON; or TOML, which the workspace already reads (`toml`), with a `[fake]` table for the answers and a `[state]` table in the contract's keys, each read by its signature from `STATE_KEYS`, so any value can be sent, even one the app doesn't know (D33).
- **Chosen:** TOML, written whole and moved into place. The fake never applies a change itself: it answers and logs it, and the driver confirms it by writing the new state, which the fake sends its watchers as `StateChanged` with the changed keys (`FanInputs` to frame watchers only, as the service does). So every control is checked to show its new value only after the service's word. The call log has one line per call, such as `SetMode(5)` or `GetState() refused NotAuthorized`, and `state <serial>` for each file taken.
- **Implemented in:** `crates/odc-testkit/src/fake_service.rs` and its tests, `crates/odc-ui-e2e/src/fake.rs`; Task 8's commit.

### D77 — Where `--only app` gets the app and the driver (Task 8)

- **Question:** the area runs the real `opendeepcool` and the driver `odc-ui-e2e`, two binaries `cargo run -p odc-e2e` doesn't build.
- **Options:** expect them built beforehand, and risk testing an old build; or build both at the area's start, with the profile `odc-e2e` was built with, next to it.
- **Chosen:** build them (`cargo build -p odc-app -p odc-ui-e2e --bins`), so the plan's one command tests the code at hand.
- **Implemented in:** `crates/odc-e2e/src/app.rs` (`build`); Task 8's commit.

### D78 — Making the window active again (Task 8)

- **Question:** "not available, then allowed again" needs the window to become active again, which no AT-SPI call does.
- **Options:** leave it to the service's new owner, another trigger the app follows; or another window that takes the focus and closes.
- **Chosen:** both are checked: the service's return in `service_stops`, and for `not_available` weston's own demo client `weston-simple-shm`, whose window takes the focus and is closed again; the app's window turns active about a second later, connects again, and shows the pages. Before that, the scenario waits 1.5 s and checks that the app asked nothing: no polling.
- **Implemented in:** `crates/odc-ui-e2e/src/scenarios.rs` (`activate_again`); Task 8's commit.

### D79 — What layer 4 leaves to the other layers (Task 8)

- **Question:** some coverage items can't be read through AT-SPI.
- **Options:** none: AT-SPI gives no pixels.
- **Chosen, and where each is checked:** the preview's dots and every tile's picture (layer 3; the real frames in Task 9), the theme's colours (layer 3; layer 4 checks the reload and that the page stays), the arrow keys between toggles and the shortcuts' keys (layer 3, D71), and the 2 s `write-failed` wait on a fake clock (layer 1; layer 4 measures it on the real one: the banner after about 2.1 s). For the owner's review: through AT-SPI the peak dots switch reads its new value at the press, before the service answers, since AT-SPI reads GTK's handle, not its delayed state (D63); and a scale follows the drag until a refusal (D61). The layer 4 checks test both as built: the call, then the confirmed state, and a refusal putting each back.
- **Implemented in:** `crates/odc-ui-e2e/src/scenarios.rs` (`music`, `refused`); this log; Task 8's commit.

### D80 — The dev copy's launcher entry (Task 9)

- **Question:** the package's entry starts `opendeepcool` from `/usr/bin` (`Exec=opendeepcool`); the dev copy's app is `/usr/local/libexec/opendeepcool-dev/opendeepcool`, which isn't on `PATH`.
- **Options:** rewrite the `Exec` line while installing, a step unlike every other file `odc-dev` installs; or a dev copy of the entry in `tools/dev/`, as the units' dev drop-ins are, with a test that it's the package's entry but for `Exec`.
- **Chosen:** the dev copy and its test (`the_dev_launcher_entry_is_the_product_s_but_for_exec`). Both entries pass `desktop-file-validate`.
- **Implemented in:** `packaging/applications/`, `tools/dev/io.github.dtmarangoni.OpenDeepCool.desktop`, `tools/dev/src/system.rs` (`APP_FILES`); the manifest's folders (`crates/odc-manifest`); Task 9's first commit.

### D81 — The app's icon (Task 9)

- **Question:** the plan asks for "a simple 14 × 14 dot motif, our own".
- **Options:** a ring of lit dots; the letters; or spectrum bars, the most display-like picture the app shows.
- **Chosen:** a 14 × 14 dot display on a rounded square, lit dots forming spectrum bars of made-up heights, in the default dark theme's background and accent; an SVG of two dot patterns and fourteen bars, written by hand. It renders as intended (`rsvg-convert`, looked at on the development PC).
- **Implemented in:** `packaging/icons/io.github.dtmarangoni.OpenDeepCool.svg`; Task 9's first commit.

### D82 — Desktop and icon caches (Task 9)

- **Question:** whether the dev setup runs `update-desktop-database` and `gtk-update-icon-cache` after installing the entry and the icon.
- **Options:** run both; or neither.
- **Chosen:** neither. The entry declares no MIME type, which is what the desktop database indexes, and launchers read entries from the folders in `XDG_DATA_DIRS`, `/usr/local/share` among them on this PC; an icon cache needs an `index.theme` in `/usr/local/share/icons/hicolor`, which nobody installs there, and GTK finds an icon without a cache. Whether the launcher lists the app and shows the icon is checked at the owner's setup (Pending real runs).
- **Implemented in:** `tools/dev/src/install.rs` (nothing run); Task 9's first commit.

### D83 — The real run and the test session (Task 9, found)

- **Question:** the plan's real run has the app change settings while the harness reads the dev recording ("each tile → the dev recording shows that mode's frame"). The recording runs only while a test session is held, and the dev service then answers every other caller's change with `Busy` (Phase 7; `odc-service` dbus.rs, `changer`). As written, the app's changes would all be refused.
- **Options:** (1) a dev-only allowance: the session's holder names another caller, by PID, whose changes the session takes, a change to the dev interface, the service and `docs/spec/dbus.md`; (2) split the run: the app's own changes against the real service with no session held, each counted once the app shows it, which it does only after the service's `StateChanged`; then a session in which the harness itself shows a few modes at fixed readings, and every frame written to the cooler must reach the app, in order, within 0.5 s; (3) only the app showing the real service's state, no frames.
- **Chosen:** (2), with no change to the service. Each link is checked: the app to the service (no session), the service to the cooler (`--only service`, and the session here), the service to the app's preview (the session here; how the preview draws a frame, layer 3). The owner's settings are read first and set back by Phase 7's `Restore` guard inside the session, however the run ends. A dev copy whose state this build can't read fails with the setup fix. Pending the owner's review; it runs at the owner's setup (Pending real runs).
- **Implemented in:** `crates/odc-e2e/src/app.rs` (`against_the_service`, `preview`, `compare`), `crates/odc-ui-e2e/src/real.rs`; Task 9's second commit.

### D84 — The app's frame log (Task 9)

- **Question:** the plan compares "the Frame signals the app receives (logged by the app)" with the recording, but gives the app two test flags only, and the app logs no readings (Delivery Considerations, Observability).
- **Options:** compare the recording with the frames another watcher receives, which proves nothing of the app; or a third test-only flag, `--frame-log FILE`, which makes the app add each frame it receives to a file: the time it came, in ns of `CLOCK_MONOTONIC` as the recording's, and its 196 dots.
- **Chosen:** the flag. Like the other two, it's for tests only: it makes the app non-unique (D47), and the package's entry passes none. The fake service now also sends a `frame` from its state file to its frame watchers, so the fake run's scenario `frame_log` proves the log before any real run.
- **Implemented in:** `crates/odc-app/src/flags.rs`, `client.rs` (`log_frames`), `main.rs`; `crates/odc-testkit/src/fake_service.rs` (`frame`) and its test; `crates/odc-ui-e2e/src/scenarios.rs` (`frame_log`); the plan's test flags; Task 9's second commit.
