# Settings app design

The design of OpenDeepCool's GTK4 settings app (PRD, Phase 10): its structure, each screen's purpose, its GTK4 widgets, its states, its theme, keyboard use and accessible names, and the coverage check. The mockups are in [mockups/](mockups/); each one has a theme switch (a sample Omarchy theme and the default theme, dark and light). `odc-mockups` (`tools/mockups/`) writes them from the states they show and the style sheet and script in [mockups-src/](mockups-src/), and its tests check the palettes against the contrast tables below.

Sources: [settings.md](../spec/settings.md) "What the settings app must offer", [display.md](../spec/display.md), [music.md](../spec/music.md), [sensors.md](../spec/sensors.md), the PRD (Phase 10, Technical Approach → Theme, Decisions Log) and the designer brief (`.claude/agents/ui-ux-designer.md`). Every reading, chip name, GPU name and PCI address in this document and in the mockups is made up.

Widget names are GTK 4 classes. Anything not checked against docs.gtk.org is marked **unverified**; the Phase 10 plan checks those first.

## Goals

- **Feature parity, not interface parity.** Every LP360 feature is reachable. There are no DeepCreative tabs and no Combined/Separate split.
- **Few screens, few clicks.** There is one window with two pages, and every mode is one click away.
- **Fast.** It opens straight onto the current settings, with no animation in the app's own chrome. Idle CPU use is nil: the preview redraws only when the cooler's picture changes, and nothing polls.
- **Only the cooler.** CPU, GPU and RAM values appear only inside display pictures. The only RPM shown is the cooler's own fans and pump.
- **Themed.** The app follows the current Omarchy theme live and uses its own default theme elsewhere. It never sets window opacity or blur.

## Decisions

The owner delegated the design choices (2026-10-25). These are final for v1.

| # | Question | Choice | Why |
|---|---|---|---|
| D1 | Window structure | One window: a persistent preview column on the left, and a `GtkStack` on the right with two pages, **Display** and **Configuration** | The preview stays in view while rotation, GPU or Music settings change. There is no second window to tile in Hyprland |
| D2 | How the 23 modes are picked | A gallery of picture tiles in four groups, **Load**, **Temperature**, **Power** and **Music**. One tile is one mode | One click per mode, all of them visible, and each tile shows what the mode looks like. No Combined/Separate wording |
| D3 | Where the preview sits | The top of the left column, on both pages | Every setting's effect shows in place |
| D4 | Refresh and status | Refresh in the header bar; a status line of up to three items under the preview; a banner only for states that need attention | Status can be read at a glance, and the app only raises an alert when one is needed |
| D5 | How the configuration screen is reached | A two-segment switcher in the header bar: `Display \| Configuration` | One click each way, and it always shows where you are |
| D6 | Apply model | **Instant apply.** Every choice goes to the service, and so to the cooler, when it's made. There is no Apply button and no unsaved state | Fewest clicks; the preview simply mirrors the cooler (settings.md, Changes apply at once) |
| D7 | Toolkit | Plain GTK4 through gtk4-rs, no libadwaita (PRD Decisions Log) | Every widget needed exists in GTK4, and libadwaita's look would mostly be overridden for Omarchy anyway |
| D8 | Theming | CSS custom properties in one `GtkCssProvider` at APPLICATION priority, generated from `colors.toml` and the `shell.toml` `[controls]` alphas. A `GFileMonitor` on `current/` triggers a live reload: Omarchy keeps `current/` and replaces only `current/theme` inside it (its `omarchy-theme-set`, read 2026-10-27; solution review P10-06) | [Theme](#theme) |

Answers to the pass-1 questions, decided under the owner's delegation (2026-10-25):

- **Tiles:** fixed made-up sample values, drawn locally.
- **Remote or inactive session** (owner, 2026-10-26, replacing pass 1's view-only session): the window shows one message and nothing else ([Not available in this session](#not-available-in-this-session)). No control is ever shown disabled for lack of rights.
- **Rotation:** four direct choices.
- **Temperature unit:** once, at the top of the Display page.
- **Service not running:** the app explains, shows the start command, and has no start button.
- **Window background:** opaque. The `shell.toml` alphas apply to controls only.
- **Default theme:** dark and light variants. Outside Omarchy in v1, the dark one is used.
- **Preview dots:** lit dots in the foreground colour, unlit dots in a low-alpha foreground.
- **Language and app settings:** English only, with no app-only settings.
- **Fan and pump RPM:** under the preview, on both pages.
- **GPU label:** `<kind> · <product name>`.
- **Preview source:** the preview mirrors the service's current frame.

Answers decided on 2026-11-06, under the same delegation, before Phase 10's code (Phase 10 plan, Task 1):

- **Automatic in the GPU dropdown** (owner, 2026-11-04): the entry and the button read "Automatic". While it's chosen, a muted line under the dropdown names the GPU it reads, from `GpuInUse`: "Using Dedicated · Contoso Graphics R870." ([Display](#display)).
- **A GPU added or removed:** the app says nothing about why the choice went back to Automatic. The line under the dropdown names the GPU in use, and the journal has the reason.
- **Cooler found, but the service can't open it** (`CoolerState` `no-access`): an error banner of its own with the fix in words (restart the PC, then reinstall OpenDeepCool). It has no command and no button, and the status says "Cooler: no access" ([Banners](#banners)).
- **Sending to the cooler fails** (`write-failed`): a warning banner, shown only once the state has lasted 2 s. Before that, the window looks as if the cooler were connected, so unplugging the cooler doesn't flash a second banner.
- **A GPU whose kind query failed** (`query-failed`): listed as "Unknown type · <name>", with the muted note "driver couldn't be asked" in the open list.

## Window

```
┌ GtkApplicationWindow ──────────────────────────────────────────────────────┐
│ GtkHeaderBar   [OpenDeepCool]    [ Display | Configuration ]   [⟳ Refresh] │
├────────────────────────────────────────────────────────────────────────────┤
│ Banner (GtkRevealer > GtkBox): only when needed                            │
├──────────────────────┬─────────────────────────────────────────────────────┤
│ Preview column       │ GtkStack (transition: none)                         │
│  GtkDrawingArea      │  page "display": unit + tile gallery                │
│  caption, status,    │  page "config":  Display / Cooler / Music sections  │
│  fan and pump RPM    │  (each page in a GtkScrolledWindow)                 │
└──────────────────────┴─────────────────────────────────────────────────────┘
```

- **Size:** default 1040 × 720 px, minimum 720 × 560 px. The tile rows reflow (`GtkFlowBox`), and each page scrolls vertically. Whether Hyprland honours a GTK window's minimum size when it tiles is **unverified**.
- **Header bar:**
  - `GtkHeaderBar` with the switcher as its title widget, and the app name as a `GtkLabel` at the start;
  - window buttons follow the desktop's decoration layout (GtkHeaderBar's default);
  - Refresh is a `GtkButton` with an icon and the label "Refresh", packed at the end.
- **No motion in the chrome.** `GtkStack` uses `GTK_STACK_TRANSITION_TYPE_NONE`, and the app's stylesheet sets `transition: none` on everything. The preview is the only thing that moves.
- **Remote or inactive session:** the header bar keeps only the app name, and the body holds only one message ([Not available in this session](#not-available-in-this-session)).

## Preview column

**Purpose:** what the cooler shows right now, plus the short status (PRD Phase 10: preview, status).

- **Preview:** a `GtkDrawingArea`, 226 × 226 px, in a frame with the surface colour.
  - It draws 14 × 14 round dots: lit dots in `--fg`, unlit dots in `--fg` at 0.12 alpha.
  - It draws the frame the service sends the app over D-Bus, in a signal addressed to it alone ([Talking to the service](#talking-to-the-service)). It's the same frame written to the cooler, so rotation, scrolling, dashes, rounding, the 9999 cap, the spectrum with its peak and fall settings, the emoji loops and the Dancer are exactly the cooler's.
  - `queue_draw()` runs only when a new frame differs from the last one. A static picture costs nothing; a moving one redraws at the cooler's 100 ms tick.
  - It also redraws once on a theme change.
  - The app watches frames (`WatchFrames()`) only while the window is mapped, and stops watching when it's hidden or minimised.
- **Caption:** the current mode, `<group> · <option>` (for example "Temperature · CPU + GPU"). Below it, one line, from `CoolerState` ([Banners](#banners)):
  - "Shown on the cooler now" (`connected`, and `write-failed` in its first 2 s);
  - "Not on the cooler: it isn't connected" (`not-found`);
  - "Not on the cooler: the service can't open it" (`no-access`);
  - "Not on the cooler: sending it fails" (`write-failed` for 2 s or more);
  - "No picture", with "The service draws the picture" under it, while the service isn't running.
- **Reason line**, when the saved mode has lost a reading: a warning icon and `<metric>: <reason>. The cooler shows dashes until it's back.` The reason text comes from the service (`UnavailableReasons`).
- **Status line:** a `GtkBox` of up to three items. Each is a coloured dot plus words, never colour alone:
  - cooler, from `CoolerState`: "Cooler connected" (green), "Cooler not connected" (yellow), "Cooler: no access" (red) or "Cooler: sending fails" (yellow), with the same 2 s rule as the caption. While the service is down it says "Cooler: unknown" (hollow dot);
  - service: "Service running" or "Service not running";
  - Music sound, **only while a Music mode is chosen** (`AudioConnected`): "Sound connected" or "No sound: spectrum is silent".
- **Fan and pump RPM:** two rows, "Cooler fans" and "Pump". Speeds are capped at 9,999 RPM, like every value in OpenDeepCool (settings.md); the same holds for the RPM in the input pickers. Each value is one of:
  - `1,180 RPM`;
  - "Choose input": a link-style `GtkButton` that opens the Configuration page;
  - "Input not found";
  - "No fan sensors found";
  - "—" while the service is down.

  These speeds come from the chosen inputs and never reach the cooler.

## Display page

**Purpose:** choose the mode and the temperature unit (settings.md: the only two settings outside the configuration screen).

- **Temperature unit** at the top: "Temperature unit" and two grouped `GtkToggleButton`s, `°C` and `°F` (`gtk_toggle_button_set_group`, verified).
  - It changes the unit at once.
  - In a mode without a temperature, the unit is only saved and the cooler doesn't change (display.md, Mode switches). Nothing in the app needs to say so, because the preview stays the same.
  - The temperature tiles redraw in the chosen unit.
- **Tile gallery:** four groups, each a heading `GtkLabel` and a `GtkFlowBox`.
  - **Load** takes a full row. **Temperature** and **Power** share a row (a horizontal `GtkBox`). **Music** takes two rows.
  - **Tiles:**

    | Group | Tiles, with their spec modes |
    |---|---|
    | Load | CPU (1), GPU (2), RAM (3), CPU + GPU (4), CPU + RAM (5), GPU + RAM (6) |
    | Temperature | CPU (7), GPU (8), CPU + GPU (9) |
    | Power | CPU (10), GPU (11), CPU + GPU (12) |
    | Music | Spectrum (13), Emoji 1–4 (14–17), CPU temperature (18), GPU temperature (19), CPU usage (20), GPU usage (21), RAM usage (22), Dancer (23) |

  - **Tile content:**
    - each tile is a `GtkFlowBoxChild` holding a 63 × 63 px `GtkDrawingArea` and a caption `GtkLabel`;
    - the picture comes from the same drawing code as the service (`odc_render`), with fixed sample values: CPU usage 42 %, GPU usage 63 %, RAM 51 %, CPU 57 °C, GPU 38 °C, CPU power 65 W, GPU power 88 W;
    - Music tiles have a fixed sample spectrum, the emoji tiles their loop's first picture, and the Dancer its standing pose;
    - tiles are drawn upright (no rotation) in the current unit, with peak dots as currently set. They redraw only when the unit, the peak-dots setting or the theme changes.
  - **Selection:**
    - each `GtkFlowBox` is `GTK_SELECTION_SINGLE` with `activate-on-single-click`;
    - choosing a tile in one group unselects the other three, so the four boxes act as one radio group;
    - activating a tile calls `SetMode` at once. Moving keyboard focus never changes the mode; only Enter, Space or a click does;
    - the selected tile has the selected fill and an accent border. Its caption stays in the foreground colour, because muted text on the selected fill is below 4.5:1.
  - **Unavailable modes** (settings.md, Availability):
    - the tile is insensitive and labelled "Unavailable";
    - its accessible description and tooltip hold `<metric>: <reason>`;
    - each group shows every distinct reason once, as a visible line under its heading (for example "GPU usage: no reading for this GPU's driver"). The reason doesn't depend on a tooltip that keyboard users can't reach.
  - **The saved mode lost a reading:**
    - its tile stays selected and sensitive, flagged "Reading lost" in the warning colour;
    - the other tiles that show that metric are disabled, and the reason shows under the preview;
    - the preview, like the cooler, shows dashes (`--°C`).

## Configuration page

**Purpose:** every other setting, on one screen (settings.md, A configuration screen). There are three sections, each a surface-coloured `GtkBox` card with a heading and rows. A row is a label (and an optional hint) on the left and the control on the right.

### Display

- **Rotation:** "Turns the picture clockwise". Four grouped `GtkToggleButton`s, `0°`, `90°`, `180°`, `270°`, which call `SetRotation`. The preview turns with the next frame.
- **GPU:** "Where the display's GPU values come from".
  - A `GtkDropDown`, shown only when the PC has more than one GPU. Its `factory` draws the button, with the label alone. Its `list-factory` draws the popup's entries: a check mark, the label, and a muted note at the right (GtkDropDown:list-factory, "the factory for populating list items in the popup", docs.gtk.org, read 2026-11-06).
  - **Automatic** is the first entry and the default (owner, 2026-11-04). It reads "Automatic", in the list and on the button, and choosing it calls `ChooseGpu("")`. Its entry has no note.
  - **The GPU in use:** while Automatic is chosen, a muted `GtkLabel` under the dropdown names the GPU it reads: "Using Dedicated · Contoso Graphics R870."
    - The name comes from `GpuInUse`, with the same label as that GPU's entry, and the line follows `GpuInUse` when it changes.
    - It names the GPU instead of saying "the first in the list". The list moves the GPUs without a reading rule last, while the service's order (`Gpus`) doesn't, so the first in the list isn't always the one Automatic reads.
    - A GPU chosen by hand is named on the button, so the line is hidden then.
  - **The GPUs:** each entry is `<kind> · <product name>`, for example "Dedicated · Contoso Graphics R870". The product name comes from the system's PCI ID list (`hwdata`). Without an entry there, the label is `<kind> · <driver> · <PCI address>`.
    - The kind reads "Dedicated", "Integrated" or "Unknown type". `unknown` and `query-failed` both read "Unknown type".
    - **Order:** the service's (`Gpus`: dedicated, integrated, then unknown, each by PCI address), with the GPUs without a reading rule moved last (settings.md, GPU list order).
  - **Notes**, muted, at the right of an entry, in the open list only:
    - "no readings": a GPU without a reading rule;
    - "driver couldn't be asked": a GPU whose kind query failed (`Gpus` kind `query-failed`; settings.md, GPU list order; solution review P6-07). It's still listed with the unknown GPUs, after an integrated one, and can be chosen like any other.
    - Only drivers that have reading rules are asked for a kind today (amdgpu, xe, i915 and nvidia: `odc-gpu-rank`'s signals and `odc-sensors`' GPU rules, read 2026-11-06), so no GPU has both notes. If one ever does, it shows "no readings".
  - **Choosing a GPU** calls `ChooseGpu` with its address. When the GPU in use, chosen by hand or read by Automatic, has no reading rule, a hint says "No reading for this GPU's driver: the GPU options are unavailable", and the GPU tiles are disabled with that reason.
  - **A GPU added or removed** (owner, 2026-11-04): the service clears the choice at its next start (sensors.md, GPU choice), so the dropdown shows Automatic, and the line under it names the GPU in use.
    - The app adds no note of why. It keeps no record of the old choice, the service publishes only the result, and the journal has the reason.
    - A note would need a new state key from the service saying that it cleared the choice in this run. That isn't part of v1.
    - A saved GPU is never missing, so the GPU row has no "not found" state.

### Cooler

- **Cooler fans input** and **Pump input:** two `GtkDropDown`s with a custom list-item factory.
  - Entries: "None" first (the default), then every fan input except a graphics card's own fans. Each shows `<chip> · <label or fanN>` and its live RPM, right-aligned with tabular figures.
  - The RPM figures update in place while the list is open; how smooth that is in a `GtkDropDown` popover is **unverified**.
  - Choosing calls `SetCoolerFanInput` or `SetPumpInput`.
  - **Saved input missing:** the button reads `<chip> · <input> — not found` with a warning border. A hint says "The saved input isn't found. Pick another, or it comes back when the input does." The app never shows another input's RPM in its place.
  - **No fan sensors:** both dropdowns are insensitive and read "No fan sensors found". A hint says "No fan sensors were found. The board's sensor driver may not be loaded; the journal says more."
- **Hint** under the rows: "Pick the inputs your cooler's radiator fans and pump are plugged into. Their speeds show in the app only, never on the cooler."

### Music

- **Peak dots:** a `GtkSwitch`, on by default. It calls `SetMusicPeaks`.
- **Bar fall** and **Peak fall:** two horizontal `GtkScale`s on a 0–4 adjustment (step 1, no value drawn), with five labelled marks: Slower, Slow, Moderate, Fast, Faster (`gtk_scale_add_mark`, verified).
  - Bar fall defaults to Moderate, Peak fall to Slow.
  - A change calls `SetMusicBarFall` or `SetMusicPeakFall` with the stop's name when the value lands on a new stop.
  - Snapping to whole stops during a drag (with `round-digits` 0 and a `change-value` handler) is **unverified**.
- **Hint:** "For every Music option except Dancer. Changes show on the next frame." In a mode without a spectrum, the change is only saved (music.md, point 7).

## Banners

A banner is a `GtkRevealer` (transition none) holding a `GtkBox`: an icon, a `GtkLabel`, and an optional button. GtkInfoBar is deprecated since GTK 4.10, and GTK's docs suggest exactly this construction (verified). Banners stack in this order:

| Banner | When | Text | Effect on the window |
|---|---|---|---|
| Service (error) | the bus name `io.github.dtmarangoni.OpenDeepCool1` has no owner | "The OpenDeepCool service isn't running. The cooler shows nothing until it starts. Start it from a terminal:" `sudo systemctl start opendeepcool.service` (a selectable monospace label) "The app connects by itself once it runs." A "Copy" button copies the command | The switcher and Refresh are insensitive. The pages are replaced by a centred message: "Settings appear here when the service runs". The preview is all unlit, and the status says "Cooler: unknown" and "Service not running". The app watches `NameOwnerChanged` and connects by itself once the name has an owner ([Talking to the service](#talking-to-the-service)); it doesn't poll |
| Cooler not connected (warning) | `CoolerState` is `not-found` | "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back." | Nothing is disabled. The preview keeps mirroring the service's picture, captioned "Not on the cooler: it isn't connected". Status "Cooler not connected" |
| Cooler, no access (error) | `CoolerState` is `no-access`: the service finds the cooler but can't open its device node, a permissions problem such as a missing udev rule (settings.md, Status; solution review P5-02) | "**The cooler is plugged in, but the service can't open it.** The udev rule that gives the service access is missing or not applied yet. Restart the PC; if that doesn't help, reinstall OpenDeepCool. The journal says more." | Nothing is disabled, and changes are saved. The preview keeps mirroring the service's picture, captioned "Not on the cooler: the service can't open it". Status "Cooler: no access", with a red dot. There's no button and no command, because both fixes are the user's own and the app can't do either |
| Cooler, sending fails (warning) | `CoolerState` has been `write-failed` for 2 s | "**The cooler is connected, but sending it the picture fails.** The service keeps trying by itself, and changes are still saved. The journal says more." | Nothing is disabled. Caption "Not on the cooler: sending it fails". Status "Cooler: sending fails", with a yellow dot |

- **One cooler banner at a time,** chosen by `CoolerState` (dbus.md, State). The app reads `CoolerState`, never `CoolerConnected`, which stays in the state for other clients. While the service banner shows, there's no cooler banner, because the cooler's state is unknown then.
- **Why the fix for `no-access` reads as it does:**
  - At boot, udev applies its rules to every device: systemd's `systemd-udev-trigger.service`, "Coldplug All udev Devices", runs `udevadm trigger --type=all --action=add` (systemd's unit source, read 2026-11-06). So a restart applies a rule that's installed but wasn't applied.
  - Reinstalling puts the rule file back, and the systemd package's pacman hook reloads udev and replays its events (`35-systemd-udev-reload.hook`, as the Phase 11 review read it on 2026-10-26).
  - A cause that neither fixes, such as another rule overriding ours, is named in the journal, which the banner points to.
  - The service tries the cooler again on every tick (Phase 5 plan, Task 6), so once the node can be opened, the cooler state turns `connected` and the banner goes, with nothing to do in the app.
- **`write-failed` waits 2 s.**
  - A failed write drops the connection, and the service opens the cooler again on its next 100 ms tick (Phase 5 plan, Task 6). So unplugging the cooler can pass through `write-failed` for about one tick before `not-found` (the write to the vanished node fails first; the Phase 5 plan's hidraw adapter expects `ENODEV` then), and a single bad write lasts one tick before `connected`.
  - The app therefore shows `write-failed` as `connected` for its first 2 s. A one-shot timer starts when the state becomes `write-failed` and is dropped when the state changes: `glib::timeout_add_local_once` in glib-rs (docs.rs, glib 0.15 to 0.20, read 2026-11-06), over GLib's `g_timeout_add_once` (since GLib 2.74, docs.gtk.org, read 2026-11-06). Nothing runs while the state stays the same.
  - In the view model, the timer's end is an input like a state change, so the rule is a unit test with a fake clock.
- **Any other value,** such as the dev copy's `released` while a test holds the cooler, or one a later service adds, shows as `not-found`.

The saved mode's lost reading gets no banner. It shows under the preview and on its tile.

## Refresh

- It calls `Refresh()`: fresh readings, and the picture restarts from home (display.md, Mode switches).
- A small confirmation label, "Refreshed", shows in a `GtkOverlay` for about 1.5 s, without animation. A static picture often looks the same after a refresh, so the user needs this confirmation.
- Refresh shows only where OpenDeepCool can be used: it's absent when the app is [not available in this session](#not-available-in-this-session), and insensitive while the service isn't running. If the service refuses it anyway (`NotAuthorized`, for example under an administrator's own polkit rule that allows reading but refuses changes), that's shown like any [refused change](#refused-changes).

## Refused changes

- A control shows a new value only once the service confirms it (`StateChanged`), so the app never shows a change the service refused.
- When the service refuses a change or a Refresh, the control stays on the saved value. The overlay label that says "Refreshed" says instead, with a warning icon, "Not changed." or "Not refreshed." followed by the reason, for about 5 s, without animation.
  - The reason is the service's error message. For `NotAuthorized` it's the app's own text: "This session may not change settings."
  - Screen readers get the same text (`gtk_accessible_announce`, since GTK 4.14, verified).
- There's no separate state for it. An administrator's own polkit rule that allows reading but refuses changes simply ends every change this way.

## Not available in this session

**When:** the service refuses `GetState()` with `NotAuthorized`, because the app runs in a remote or inactive session (owner, 2026-10-26). The service refuses such a session every read, change and preview frame (polkit `allow_active` only), so the app has nothing to show. This state replaces the view-only state, which no longer exists.

```
┌ GtkApplicationWindow ──────────────────────────────────────────────────────┐
│ GtkHeaderBar   [OpenDeepCool]                                              │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│                                                                            │
│          OpenDeepCool can be used only from the session at the PC          │
│                This session is remote or not the active one.               │
│                                                                            │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

- **Text:** the heading "OpenDeepCool can be used only from the session at the PC", and under it, in `--muted`, "This session is remote or not the active one."
- **Widgets**, all plain GTK4 (no libadwaita, so no `AdwStatusPage`):
  - the same `GtkHeaderBar`, with only the app name. The switcher and Refresh are hidden (`visible` false), not just insensitive. The window buttons stay, following the desktop's decoration layout;
  - the body is a vertical `GtkBox`, centred in the window (`halign` and `valign` center), holding two `GtkLabel`s, the heading and the text. Both wrap (`wrap` true, `justify` center), so a narrow window wraps them instead of cutting them. They look like the service state's "Settings appear here when the service runs";
  - nothing else: no banner, no preview column, no pages, no status, no RPM, nothing that could show a value.
- **Keyboard and screen readers:** nothing in the content takes focus. Ctrl+W and Ctrl+Q still close the window; the page and Refresh shortcuts do nothing, because their actions are disabled. On entering the state, the app announces the heading and text once (`gtk_accessible_announce`, since GTK 4.14, verified).
- **Trying again,** without polling: when the window becomes active again (`notify::is-active` on `GtkWindow:is-active`, verified) and when the service's bus name gets a new owner (`NameOwnerChanged`). If `GetState()` succeeds then, the normal screens appear with the current state.
- **The service not running comes first.** Without the service, the app can't tell whether this session may use OpenDeepCool, so it shows the service state, which holds no settings and no picture.
- **An administrator's own polkit rule** might allow reading but refuse changes. That's no separate state: the app shows the normal screens, and each change fails with `NotAuthorized`, shown like any [refused change](#refused-changes).

## Talking to the service

The D-Bus system bus (settings.md, Status). Only the user of the active local session can use OpenDeepCool: the service refuses every other session each read, watch and change (polkit `allow_active` only; owner, 2026-10-26). It offers no D-Bus properties, and nothing is broadcast.

- **Connecting:** the app starts watching the state (`Watch()`) and reads all of it (`GetState()`). The plan fixes the order, so that no change falls between the two. Then the window shows:
  - the [service banner](#banners), when the bus name has no owner;
  - [not available in this session](#not-available-in-this-session), when the service refuses with `NotAuthorized`;
  - otherwise, the normal screens with that state.
- **Connecting again,** one call at a time and never polling: whenever the bus name gets a new owner (`NameOwnerChanged`), and whenever the window becomes active (`notify::is-active`). This also renews any watch the service dropped meanwhile, for example while this session was inactive.
- **Updates** come as `StateChanged` signals addressed to this app alone, and preview frames the same way after `WatchFrames()`, only while the window is mapped ([Preview column](#preview-column)).
- **State names:** `CoolerState`, `AudioConnected`, `UnavailableReasons`, `FanInputs`, `Gpus` and `ChosenGpu` in this document are the names in the Phase 7–9 contract, and `GpuInUse` is the Phase 10 plan's (Task 2). They come in `GetState()` and `StateChanged`, not as properties, and the Phase 10 plan sets their final shape.
  - The cooler's part of the window follows `CoolerState` (`connected`, `not-found`, `no-access`, `write-failed`; [Banners](#banners)). `CoolerConnected` stays in the state, but the app doesn't read it.
  - The GPU row reads `Gpus` (its kinds include `query-failed`), `ChosenGpu` (empty for Automatic) and `GpuInUse` ([Display](#display)).
- **Back from standby** (owner, 2026-10-26): no screen of its own, and nothing for the user to do. While the monitor is off or the screen is locked, the session stays active, so nothing changes for the app. After a sleep, the service takes up the cooler again by itself (PRD, Service model), and the preview follows the cooler as soon as frames come back. If the cooler or the sound takes a moment to return, the cooler banner and the sound status show it as usual.

## States

| State | What the app shows | Mockup |
|---|---|---|
| Normal, several GPUs | Display page; status "Cooler connected", "Service running"; fan and pump RPM. Configuration: the GPU on Automatic, and "Using Dedicated · Contoso Graphics R870." under it | [display.html](mockups/display.html), [configuration.html](mockups/configuration.html) |
| GPU list open | Automatic first; a GPU whose kind query failed as "Unknown type · …" with "driver couldn't be asked"; a GPU without a reading rule last, with "no readings"; the line under the dropdown names the integrated GPU, which Automatic reads here | [configuration-gpu-list.html](mockups/configuration-gpu-list.html) |
| A GPU added or removed since the last start | The choice is back on Automatic (the service cleared it), with no note; the line names the GPU in use | [configuration.html](mockups/configuration.html) |
| A Music mode | The status line adds "Sound connected" or "No sound: spectrum is silent" | [display-music.html](mockups/display-music.html) |
| A value of 100 or more, °F | The preview mirrors the scrolling strip; the temperature tiles show °F | [display-scrolling.html](mockups/display-scrolling.html) |
| Fan picker open | Every input with its chip, label and live RPM; None first | [configuration-picker-open.html](mockups/configuration-picker-open.html) |
| First start | Load · CPU, 0°, °C, no fan inputs ("Choose input"), peak dots on, Moderate / Slow, the GPU on Automatic | [first-start.html](mockups/first-start.html) |
| Cooler disconnected (`not-found`) | Cooler banner; status "Cooler not connected"; everything still changeable | [cooler-disconnected.html](mockups/cooler-disconnected.html) |
| Cooler found, no access (`no-access`) | Error banner with the fix; status "Cooler: no access"; caption "Not on the cooler: the service can't open it"; everything still changeable | [cooler-no-access.html](mockups/cooler-no-access.html) |
| Sending to the cooler fails, 2 s or more (`write-failed`) | Warning banner; status "Cooler: sending fails"; caption "Not on the cooler: sending it fails"; everything still changeable | [cooler-write-failed.html](mockups/cooler-write-failed.html) |
| Sending fails, under 2 s (one bad write, or the cooler being unplugged) | As connected: no banner, until the state changes or 2 s pass | (as display) |
| Service not running | Service banner with the command; pages replaced by a message; nothing changeable | [service-not-running.html](mockups/service-not-running.html) |
| Metric unavailable | Tiles disabled with "Unavailable"; each group lists its reasons once | [unavailable-metric.html](mockups/unavailable-metric.html) |
| The saved mode lost a reading | Tile still selected with "Reading lost"; dashes in the preview; reason under the preview | [reading-lost.html](mockups/reading-lost.html) |
| Not available in this session (remote or inactive) | Only one message: no values, no preview, no controls, no Refresh | [not-available.html](mockups/not-available.html) |
| One GPU | No GPU row at all | [one-gpu.html](mockups/one-gpu.html) |
| Saved fan or pump input missing | "— not found" on its picker, with a warning border and a hint; "Input not found" under the preview; no other input's RPM in its place. (A saved GPU can't be missing: the service clears the choice at start.) | [missing-input.html](mockups/missing-input.html) |
| No fan sensors | Both pickers insensitive with a note; the preview column says "No fan sensors found" | [no-fan-sensors.html](mockups/no-fan-sensors.html) |
| A GPU without a reading rule in use | GPU tiles disabled with "no reading for this GPU's driver"; a hint under the GPU dropdown | [configuration.html](mockups/configuration.html): choose "Unknown type · othergpu · 0000:07:00.0" |
| Another `CoolerState` (the dev copy's `released`, or a later value) | As `not-found` | (as cooler-disconnected) |
| Short read failure, first second | Nothing disabled; the preview shows the cooler's dashes | (as reading-lost, without the reason line) |
| A change refused | The control stays on the saved value; the overlay label says "Not changed." with the reason | (no mockup: Refresh's label, with a warning icon) |
| Back from standby | No screen of its own; the preview follows the cooler again when frames come back | — ([Talking to the service](#talking-to-the-service)) |
| Live theme switch | Colours change in place, with no restart and the same page and scroll position | every mockup: the theme buttons above the window |
| Default theme (no Omarchy theme) | The default dark palette | every mockup: "Default theme, dark" or "Default theme, light" |

## Theme

### How it's applied

- **One provider.**
  - A single `GtkCssProvider` is added with `gtk_style_context_add_provider_for_display` at `GTK_STYLE_PROVIDER_PRIORITY_APPLICATION` (600). Whether that function is deprecated in current GTK is **unverified**; the plan checks.
  - A user's own `~/.config/gtk-4.0/gtk.css` keeps the last word at USER priority, 800 (verified).
- **Generated CSS.**
  - The CSS is a string: a `:root { … }` block of custom properties, followed by the app's fixed stylesheet, which uses only `var(--…)`. It loads with `gtk_css_provider_load_from_string`.
  - GTK supports `:root`, custom properties and `var()` since 4.16 (verified, GTK CSS Properties). Arch's repository ships a newer GTK.
- **Colours with alpha** (`rgba(…)`) are computed in Rust and written into the `:root` block as literals, so the stylesheet needs no CSS colour functions.
- **Live reload (Omarchy).**
  - A `GFileMonitor` on `~/.local/state/omarchy/current/` reacts when `theme` is replaced (the PRD, Theme: themes are moved over `current/theme`).
  - After 150 ms of quiet, the app re-reads `colors.toml` and `shell.toml`, regenerates the string, reloads the same provider, and repaints the preview and tiles. There's no restart, and the page and scroll position are kept.
- **Fallback rules.**
  - When `colors.toml` is missing or unreadable, the whole default theme is used.
  - When one key is missing or invalid, that token comes from the default theme (dark, or light when `mode` says light), and the journal names the key.
- **No window opacity** and no blur, ever: Hyprland applies them (PRD, Theme). The window background is opaque `--bg`.
- **No motion:** the stylesheet sets `transition: none` and no CSS animations, overriding GTK's built-in theme. The global `gtk-enable-animations` setting is left alone, because it belongs to the user.
- **Light or dark base.** `colors.toml` `mode` also chooses GTK's dark or light base (`gtk-application-prefer-dark-theme`), so parts the stylesheet doesn't colour (scrollbars, popover arrows) match. Whether that property is still the right switch in GTK 4.20+ (for example a newer colour-scheme setting) is **unverified**.
- **Font:** the GTK system font (`gtk-font-name`). `shell.toml` `[font]` isn't read in v1.
- **Room for later:** a "theme source" setting after v1 only swaps where the tokens come from. The stylesheet and the token names stay. Nothing about it shows in v1.

### Tokens and where they come from

| Token | Omarchy source | Used for |
|---|---|---|
| `--bg` | `colors.toml` `background` | window, header bar, banner base |
| `--surface` | `lighter_background` | preview frame, configuration cards, popovers |
| `--fg` | `foreground` | text, lit preview dots (unlit dots: `--fg` at 0.12) |
| `--muted` | `muted` | hints, captions of unavailable tiles, group reason lines |
| `--accent` | `accent` | selected tile and toggle border, focus ring, switch on, scale fill, links |
| `--red`, `--yellow`, `--green` | `red`, `yellow`, `green` | status dots, banner icons and tints, warning text |
| `--selection` | `selection` | selected text in the selectable command label |
| `--ctl-normal-fill`, `--ctl-normal-border`, `--ctl-normal-border-width` | `shell.toml` `[controls]` `normal-color` + `normal-fill-alpha`, `normal-border` + `normal-border-alpha`, `normal-border-width` | idle buttons, toggles, dropdown buttons, tiles |
| `--ctl-hover-fill`, `--ctl-hover-border` | `hover-cursor-*` | `:hover` |
| `--ctl-focus-*` | `focus-*` | `:focus-visible` fill and border |
| `--ctl-selected-fill`, `--ctl-selected-border` | `selected-*` | `:checked` toggles, the selected tile, the selected list item |
| `--ctl-pressed-fill` | `pressed-fill-alpha` | `:active` |
| `--radius` | (none in `shell.toml`) | 0 px with an Omarchy theme, 6 px in the default theme |

- **Focus ring:** keyboard focus also gets a 2 px `--accent` outline, whatever the theme's focus alphas, so focus is always visible (WCAG 2.4.7).
- **Gradients:** `shell.toml` border keys may hold a Hyprland-style gradient (`"rgba(…) rgba(…) 45deg"`, per its comments). The app uses the gradient's first colour, since a gradient border colour in GTK CSS is **unverified**.

### Default theme

OpenDeepCool's own palette, a cool neutral grey with a teal accent. It is used wherever no Omarchy theme is found: dark in v1, and light when the Omarchy theme reports `mode = "light"` but a key is missing. Contrast ratios are WCAG 2 relative-luminance ratios, computed for these exact values.

| Token | Dark | Light |
|---|---|---|
| `--bg` | `#14171b` | `#f5f6f8` |
| `--surface` | `#1d2126` | `#ffffff` |
| `--fg` | `#e7eaee` | `#1a1d21` |
| `--muted` | `#9aa3ae` | `#5a626c` |
| `--accent` | `#4fc3b4` | `#0b7a6e` |
| `--red` | `#f08a80` | `#b42318` |
| `--yellow` | `#e6c15c` | `#8a5d00` |
| `--green` | `#7fd48f` | `#1d7a35` |
| control alphas (normal fill / normal border / hover fill / hover border / selected fill / pressed fill) | 0.04 / 0.40 / 0.08 / 0.30 / 0.18 / 0.22 | 0.04 / 0.50 / 0.07 / 0.35 / 0.14 / 0.20 |

| Check (need) | Dark | Light |
|---|---|---|
| text on background (4.5) | 14.9 | 15.6 |
| text on surface (4.5) | 13.4 | 16.9 |
| muted text on background / surface (4.5) | 7.0 / 6.3 | 5.7 / 6.2 |
| accent on background / surface (4.5, used as link text) | 8.4 / 7.5 | 4.8 / 5.2 |
| warning text (yellow) on background / surface (4.5) | 10.4 / 9.4 | 5.3 / 5.8 |
| red, green on background (3.0, status dots) | 7.4, 10.0 | 6.1, 5.0 |
| text on the selected fill (4.5) | 8.0 | 12.7 |
| control border on background (3.0, WCAG 1.4.11) | 3.4 | 3.2 |
| lit dot against unlit dot (3.0) | 10.9 | 12.3 |

Muted text on the selected fill is below 4.5:1 (dark 3.8), so nothing muted is drawn on a selected tile. The mockups' sample Omarchy theme is made up too; it passes the same checks (text 11.9, muted 5.4, accent 7.2 on its background).

## Keyboard

- **Tab order:** header switcher → Refresh → banner button (when present) → page content → preview column links. On the Display page: the unit toggles, then each tile group in turn. On the Configuration page: rotation, GPU, cooler fans, pump, peak dots, bar fall, peak fall.
- **Inside a tile group:** the arrow keys move focus (`GtkFlowBox` keyboard navigation), and Enter or Space chooses the tile, which applies it. Focus alone never applies.
- **Insensitive tiles can't take focus.** Their reasons are visible text under the group heading.
- **Toggle groups** (unit, rotation, switcher): Tab reaches them; Enter or Space toggles. Arrow keys between the buttons of a group are **unverified** for grouped `GtkToggleButton`s; if GTK doesn't do it, each button is a Tab stop.
- **Dropdowns:** Enter or Space opens; arrows move; Enter chooses; Escape closes.
- **Scales:** Left and Right move one stop; Home and End go to Slower and Faster.
- **Switch:** Space toggles.
- **Not available in this session:** nothing in the window takes focus but the window buttons, and only Ctrl+W and Ctrl+Q work; the page and Refresh actions are disabled.
- **Shortcuts** (`GtkApplication` accelerators):

  | Keys | Action |
  |---|---|
  | Ctrl+1 | Display page |
  | Ctrl+2 | Configuration page |
  | F5 or Ctrl+R | Refresh |
  | Ctrl+W or Ctrl+Q | close the window |

## Accessible names

The Phase 10 tests drive the app through AT-SPI, so every interactive widget gets a fixed English accessible label (`GTK_ACCESSIBLE_PROPERTY_LABEL`), and reasons go in `GTK_ACCESSIBLE_PROPERTY_DESCRIPTION`. Names never include live values, so tests find widgets by name and read values from state or description. Roles are GTK's defaults for each widget; the exact role per widget is **unverified** until checked with an accessibility inspector.

| Widget | GTK4 widget | Accessible name | State and description |
|---|---|---|---|
| Page switcher buttons | `GtkStackSwitcher` (or grouped `GtkToggleButton`) | `Display`, `Configuration` | selected or checked |
| Refresh | `GtkButton` | `Refresh display` | insensitive while the service isn't running; absent when not available in this session |
| Refresh and refusal label | `GtkLabel` in the `GtkOverlay` | `Change feedback` | its text ("Refreshed", or "Not changed." and the reason) |
| Service banner | `GtkBox` in `GtkRevealer` | `Service banner` | its text |
| Copy command | `GtkButton` | `Copy start command` | — |
| Not-available message | `GtkBox` holding two `GtkLabel`s | `Not available in this session` | its text |
| Cooler banner | `GtkBox` in `GtkRevealer` | `Cooler banner` | its text: not connected, no access or sending fails ([Banners](#banners)) |
| Preview | `GtkDrawingArea` | `Cooler display preview` | description: the mode caption, for example "Temperature · CPU + GPU" |
| Reason under the preview | `GtkLabel` | `Unavailable reading` | its text |
| Status items | `GtkLabel` | `Cooler status`, `Service status`, `Music sound status` | their text (`Cooler status`: "Cooler connected", "Cooler not connected", "Cooler: no access", "Cooler: sending fails" or "Cooler: unknown") |
| Fan and pump speed rows | `GtkLabel` | `Cooler fans speed`, `Pump speed` | their text (RPM, "Input not found", …) |
| Choose-input links | `GtkButton` | `Choose cooler fans input`, `Choose pump input` | — |
| Unit toggles | `GtkToggleButton` | `Celsius`, `Fahrenheit` | checked |
| Tile groups | `GtkFlowBox` | `Load options`, `Temperature options`, `Power options`, `Music options` | — |
| Group reason lines | `GtkLabel` | `Load unavailable reasons` (and so on per group) | their text |
| Tiles | `GtkFlowBoxChild` | `Load: CPU`, `Load: GPU`, `Load: RAM`, `Load: CPU + GPU`, `Load: CPU + RAM`, `Load: GPU + RAM`, `Temperature: CPU`, `Temperature: GPU`, `Temperature: CPU + GPU`, `Power: CPU`, `Power: GPU`, `Power: CPU + GPU`, `Music: Spectrum`, `Music: Emoji 1` … `Music: Emoji 4`, `Music: CPU temperature`, `Music: GPU temperature`, `Music: CPU usage`, `Music: GPU usage`, `Music: RAM usage`, `Music: Dancer` | selected; disabled; description `<metric>: <reason>` when unavailable, or "Reading lost: …" on the saved mode |
| Rotation | `GtkToggleButton` | `Rotation 0°`, `Rotation 90°`, `Rotation 180°`, `Rotation 270°` | checked |
| GPU | `GtkDropDown` | `GPU` | the selected item's label (`Automatic`, or the GPU's); description "Using <GPU label>" while Automatic is chosen, "no readings" when the GPU in use has no reading rule, "driver couldn't be asked" when a GPU whose kind query failed is chosen by hand, joined by "; " when several apply |
| GPU in use | `GtkLabel` under the GPU dropdown | `GPU in use` | its text ("Using Dedicated · Contoso Graphics R870."); present only while Automatic is chosen |
| Cooler fans input | `GtkDropDown` | `Cooler fans input` | the selected item's label; description "not found" |
| Pump input | `GtkDropDown` | `Pump input` | the same |
| Dropdown items | list item `GtkLabel`s | the item label without the RPM or note (`None`, `acme6000 · fan1`, `Automatic`, `Dedicated · Contoso Graphics R870`) | the RPM or the note ("no readings", "driver couldn't be asked") in the description |
| Peak dots | `GtkSwitch` | `Peak dots` | checked |
| Bar fall | `GtkScale` | `Bar fall` | value 0–4; value text the stop's name (`Moderate`) |
| Peak fall | `GtkScale` | `Peak fall` | the same (`Slow`) |

How to give a `GtkScale` a named value text (Slower … Faster) for AT-SPI is **unverified**; the plan checks `GTK_ACCESSIBLE_PROPERTY_VALUE_TEXT`.

## Text

- Short English sentences, with the mode names Load, Temperature, Power and Music, never "Guardian" or "… Mode".
- Metrics are named as the display shows them: CPU usage, GPU usage, RAM usage, CPU temperature, GPU temperature, CPU power, GPU power.
- Reasons come from the service and aren't rewritten. The app adds only the metric name in front.
- A banner that needs the user's action says what happened in bold, then the fix in one sentence. Details, such as device paths and error codes, go to the journal, and the banner says "The journal says more."
- Numbers use English digit grouping (`1,180 RPM`) and tabular figures.

## What the Phase 10 plan must add to the D-Bus contract

The design depends on these. The first two were agreed on 2026-10-25. The service offers no D-Bus properties (owner, 2026-10-26), so each item is part of the state that `GetState()` returns and `StateChanged` updates, or a signal addressed to one watcher ([Talking to the service](#talking-to-the-service)).

1. **The current frame:** a signal, addressed to each authorized watcher alone after `WatchFrames()` and never broadcast, carrying the frame exactly as written to the cooler, rotation included. It's sent only while the app watches, and only when the frame changes. Its encoding (the 28 pixel bytes or 196 booleans) is chosen in the plan.
   - While the cooler is away (`not-found`, `no-access`, `write-failed`), the design still needs the frame the engine draws on each tick, so the preview keeps showing the saved picture ([Banners](#banners)). The Phase 10 plan's "each written frame" (Task 2) has to cover those ticks too.
2. **Live RPM for every fan input:** RPM in the state's fan inputs (`FanInputs`), or a separate map, updated about once a second while the app watches.
3. **GPU names:**
   - either the service sends each GPU's PCI vendor and device ID, so the app can look up `/usr/share/hwdata/pci.ids`, or the service sends the name;
   - the current `Gpus` (`a(sss)`: address, driver, kind) has neither;
   - it also needs a "no reading rule" flag per GPU;
   - the Phase 10 plan makes it `a(sssqqb)` (Task 2). Its kinds already include `query-failed` (Phase 6), which the list notes.
4. **The GPU in use as well as the saved one:** needed for the line that names the GPU Automatic reads. `ChosenGpu` is only the saved choice, empty for Automatic (owner, 2026-11-04). The Phase 10 plan adds `GpuInUse` (s) (Task 2).
5. **Whether the saved fan and pump inputs are present:** for "not found". The app could infer it by matching `FanInputs`, but an explicit flag avoids two rules.
6. **How the app learns it's in a remote or inactive session:** `GetState()` refused with `NotAuthorized` ([Not available in this session](#not-available-in-this-session)). There's no `CanChangeSettings()` and no separate check: the refusal that protects the state is what tells the app.
7. **Watches that can be renewed:** calling `Watch()` or `WatchFrames()` again does no harm, because the app renews its watches each time it connects again, including on every window activation ([Talking to the service](#talking-to-the-service)).

## Coverage check

Every item from settings.md "What the settings app must offer", display.md, music.md and the brief, and where it's reached.

| Item | Reached at | Mockup |
|---|---|---|
| Modes 1–6 (Load) | Display page, Load tiles | display |
| Modes 7–9 (Temperature) | Display page, Temperature tiles | display |
| Modes 10–12 (Power) | Display page, Power tiles | display-scrolling |
| Mode 13 (Music, full-screen spectrum) | Music tile "Spectrum" | display |
| Modes 14–17 (emoji) | Music tiles "Emoji 1–4" | display |
| Modes 18–22 (value + spectrum) | Music tiles, each named by its metric (rule 11) | display-music |
| Mode 23 (Dancer) | Music tile "Dancer" | display |
| One choice → one mode | 23 tiles, 23 `SetMode` values | — |
| Temperature unit | Display page, top | display-scrolling |
| Rotation 0–3 | Configuration → Display | configuration |
| GPU dropdown only with more than one GPU, Automatic first and the default, then the spec's order, the choice kept | Configuration → Display | configuration, configuration-gpu-list, one-gpu |
| Automatic names the GPU it reads (`GpuInUse`) | The line under the GPU dropdown | configuration, configuration-gpu-list |
| A GPU added or removed: the choice goes back to Automatic, and a saved GPU is never missing | Configuration → GPU (no note; the journal has the reason) | configuration |
| A GPU whose kind query failed: listed with the unknown GPUs, with a short note, and choosable | Configuration → GPU, the open list | configuration-gpu-list |
| GPU without a reading rule → listed last, GPU modes disabled | Configuration → GPU, and tiles | configuration-gpu-list; configuration (choose it) |
| Cooler fans and pump inputs, None default, live RPM, no graphics card fans | Configuration → Cooler | configuration-picker-open |
| Saved input missing → not found | Configuration → Cooler; preview column | missing-input |
| No fan sensors → note | Configuration → Cooler; preview column | no-fan-sensors |
| Cooler fan and pump RPM shown, never on the cooler | Preview column, both pages | display |
| Peak dots on or off | Configuration → Music | configuration |
| Bar fall, peak fall, five stops, defaults | Configuration → Music | configuration |
| Configuration screen holds everything but mode and unit | Configuration page | configuration |
| Refresh, only for the user at the PC | Header bar; absent when not available in this session; a refusal shows as a refused change | display, not-available |
| Status: cooler connected | Status line; cooler banner | cooler-disconnected |
| Status: the cooler found but not usable (`no-access`), told apart from not connected, with the fix | Its own error banner; status line; caption | cooler-no-access |
| Status: sending to the cooler fails (`write-failed`), passing or lasting | Banner after 2 s, nothing before | cooler-write-failed |
| Status: service running | Status line; service banner | service-not-running |
| Status: Music sound (`AudioConnected`) | Status line, Music modes only | display-music |
| Availability: disabled with a one-line reason; device vs single metric | Tiles plus group reason lines | unavailable-metric |
| The saved mode losing a reading: stays selected, dashes, reason | Tile flag, preview, reason line | reading-lost |
| Only the user at the PC: a remote or inactive session gets one message and nothing else | The whole window ([Not available in this session](#not-available-in-this-session)) | not-available |
| Back from standby: no action and no new screen | The preview follows the frames when they return ([Talking to the service](#talking-to-the-service)) | — |
| Live preview with rotation, scrolling, dashes, full values to 9999, the spectrum settings | Preview column (mirrors the service's frame) | configuration (90°), display-scrolling, reading-lost |
| Numbers rounded as display.md Values: the nearest whole number, halves up; °F once from the exact °C | Preview (the service's frame); tiles (`odc_render`) | display-scrolling (°F tiles) |
| Instant apply, no Apply button | Every control | — |
| Omarchy theme followed live; opacity and blur untouched; `shell.toml` alphas on controls | Theme | every mockup's theme switch |
| Default theme, dark and light | Theme | every mockup's theme switch |
| First-start defaults | Opens with the service's settings | first-start |
| English only, no app-only settings | — | — |

**Out of scope, confirmed absent.** The design has no:
- fan or pump speed control (the RPM is read-only text);
- brightness, screen on/off or other hidden settings;
- lighting;
- other devices;
- CPU, GPU or RAM readouts outside display pictures (tiles use fixed sample values, and the preview shows only the cooler's picture);
- clocks, voltages or other fans (the pickers list fan inputs only to choose the cooler's);
- theme-source setting;
- Creative mode;
- static or scrolling option;
- language or start-at-login setting;
- button that changes the system, such as starting the service or applying the udev rule: the banners name the fix, and the user carries it out.

## Unverified points, answered (Phase 10 Task 1, 2026-11-06)

Each point the design left open, with its answer and source, or the fallback the app uses while it stays unverified. The phase's decisions log (`docs/prp/reports/phase-10-settings-app.decisions.md`, D6–D12) has the options behind each.

| Point | Answer | Source, or the fallback |
|---|---|---|
| `gtk_style_context_add_provider_for_display` | Not deprecated: it carries no deprecation notice and is declared in `gtkstyleprovider.h`, apart from the deprecated `GtkStyleContext`. gtk4-rs: `gtk::style_context_add_provider_for_display` | docs.gtk.org, `Gtk.StyleContext.add_provider_for_display` |
| Dark or light base (GTK 4.20 and later) | The app sets `GtkCssProvider:prefers-color-scheme` to `GTK_INTERFACE_COLOR_SCHEME_DARK` or `_LIGHT` on its own provider; `gtk-application-prefer-dark-theme` is deprecated since 4.20, and `GtkSettings:gtk-interface-color-scheme` only carries the system's preference. Every colour the app draws comes from the tokens, so the scheme touches only what GTK draws itself; whether GTK's built-in theme follows the app provider's scheme is **unverified**, and the layer 3 tests look at it | docs.gtk.org, `Gtk.CssProvider:prefers-color-scheme`, `Gtk.Settings:gtk-application-prefer-dark-theme`, `Gtk.Settings:gtk-interface-color-scheme` |
| Gradient border colours | `border-color` takes colours only; `border-image` takes an image, a gradient included. The app keeps a gradient's first colour, as Theme says, so no gradient is drawn | docs.gtk.org, GTK CSS Properties |
| Default accessible roles | `GtkFlowBoxChild` is `GRID_CELL` (its box `GRID`), `GtkLabel` `LABEL`. `GtkDrawingArea`'s default isn't documented, so the preview and every tile picture are built with the role `IMG` and their accessible label | docs.gtk.org, Accessibility (List of accessible roles) |
| Accessible names, descriptions, value text | `AccessibleExtManual::update_property` sets `Label` and `Description`; `ValueText` is ARIA's `aria-valuetext`. What AT-SPI shows for each is **unverified** until the layer 4 AT-SPI driver reads every name | gtk4-rs book, Accessibility; docs.gtk.org, Accessibility |
| `GtkScale` and whole stops | `GtkRange:round-digits` rounds the value "when it changes", drags included: with 0 digits over 0–4 the scale only holds whole stops, and the app applies each stop as it's reached (at most five calls a drag). If a test shows otherwise, the stop is applied on release | docs.gtk.org, `Gtk.Range:round-digits` |
| Arrow keys between grouped `GtkToggleButton`s | **Unverified.** Each unit and rotation toggle is a Tab stop as well, so the keyboard reaches each one either way; the tests record whether arrows move too | the fallback of Keyboard |
| Live RPM inside an open `GtkDropDown` | **Unverified.** Each row binds its RPM to its item's property, so it updates while open if GTK redraws bound rows, and in any case when the list reopens | the fallback of Cooler |
| Tooltips on insensitive widgets | **Unverified**, and not relied on: the reason line under each group says why a tile is unavailable | Display page, Unavailable modes |
| Hyprland and the window's minimum size | **Unverified**: it can be seen only in the owner's session, and no test opens a window there. The app sets its minimum size as designed; the optional attended tour looks at it | Window |
| `GtkWindow:is-active` after a session switch | **Unverified**, for the same reason. The app also follows logind's `Active` property of its own session (org.freedesktop.login1), a third trigger for connecting again, still without polling | Not available in this session |
| glib's one-shot timer name | **Unverified** until the app depends on gtk4 0.11's glib: `timeout_add_local_once` (glib 0.15 to 0.20, and gtk-rs-core's git docs, read 2026-11-06). Fallback: `timeout_add_local` with a closure that returns `ControlFlow::Break` | Banners, `write-failed` |
