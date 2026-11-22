# Phase 11a: the settings app's UI review: fans and pump leave the product, the window and its pages are rebuilt, the tiles go live, and pairs of values are centred on the cooler

**Plan ID:** `phase-11a-settings-app-ui-review`
**Source PRD:** `docs/prp/prds/opendeepcool-lp360.prd.md` (repository-relative: an absolute path would name the owner's home folder, AGENTS.md rule 10)
**PRD Phase:** `11a — Settings app UI review`
**Source Issue:** `None`
**Plan Publication:** `None`

> **Numbering.** The PRD (Phase 12 scope; Decisions Log, "When the owner reviews the settings app") put the owner's review of the settings app after Phase 11 and before Phase 12. This phase is that review's work, written on 2026-11-18 after the owner's review. It is 11a so that Releases stays Phase 12: renumbering would change about sixty references in the completed phases' plans, reports and reviews, which stay as written.
>
> **Evidence read** on 2026-11-18 at `main` = `1a6a4e9` (Phase 11 complete). The owner's decisions D1–D4 below were given the same day, in answer to four questions about the owner's draft review, and D5–D8 later the same day, in answer to the four questions of [the solution review](../solution-reviews/phase-11a-settings-app-ui-review.review.md). The draft itself is not part of the repository.

## Outcome

**Problem:** The owner reviewed the Phase 10 app as built and found:
- it measures, lists and shows the cooler's fans and pump, which the owner doesn't want in OpenDeepCool at all;
- its window tiles like any window and keeps a 1040 × 720 default, its header carries a page switcher and a bordered Refresh, the temperature unit sits apart from the other settings, and its switches are large;
- its 23 tiles are still pictures drawn from fixed sample values, though the service already reads every metric once a second;
- on the cooler, the two-value modes are right-sided;
- there is no way to choose between the system's theme and a dark or light theme of our own.

**Affected user:** The owner, and any user with an LP360 on Arch or an Arch-based distribution.

**User outcome:**
- The product is the display and nothing else: no fan or pump reading, service or screen.
- `opendeepcool` opens at the size of its content, with scrollbars where the content wouldn't fit the screen, and asks the window manager for a floating window where that is possible.
- The header has the title in the middle, and a gear and a close icon at the right. The Configuration page has a back icon, the temperature unit after Rotation, labels beside their controls, and a Theme card first.
- Refresh is an icon on the display page.
- Every mode tile shows its mode drawn with the live readings, and all tiles move together. The Music tiles and the Dancer's follow the sound when the cooler is already in a Music mode.
- On the cooler, a pair of values is centred as one block. The Dancer stays as it is.

**Invariant:**
- **Only the cooler's display** (owner, 2026-11-18): outside the kept spec file `docs/spec/fans-and-pump.md` and the completed phases' own records, no live file mentions reading, showing or choosing the cooler's fans or pump, and nothing in the service, the package or the tests reads them.
- **Every mode and setting stays reachable**, each control still one D-Bus call, each tile still one spec mode, and every tile shows the metric it names (rule 11).
- **The preview is the cooler:** it shows only frames the service publishes.
- **Cooler frames change only where the owner decided:** modes 4, 5, 6, 9 and 12. The Dancer is unchanged (D3). The expected frames of every other mode stay byte for byte, and the oracle's regeneration shows no other difference.
- **Sound is captured as now:** only while a Music mode is shown. The live tiles add no capture.
- **Colours come from the theme** or from the default theme, never hard-coded in a widget, and the app never sets window opacity or blur.
- **Nothing assumes Omarchy or a window manager** except the Omarchy theme the app follows when it finds one (rule 14).
- **Uninstalling leaves the system as it was** (rule 7). The app writes no file of its own: the theme choice is the service's setting, saved with the others in its state directory, which the package's removal already deletes (Task 10).
- **The service's access rules don't widen:** the new data goes only to a caller that watches frames and passes the same one-second check, and a client that doesn't know it ignores it.

**Success signal:** The owner reviews the new build (Task 53) and signs the phase off. Behind that, every acceptance criterion below passes unattended: the app's AT-SPI run on every scenario, `--only display` on every mode, `--only service`, `--only package`, and the full parity run on the installed package.

**Approach:**
- **Remove first** (the owner's order): keep the fans-and-pump facts in one spec file, then take the feature out of the app, the service with its contract and the sensors, the package (the sensor detection exists only for the fan chip), and the documents.
- **Rebuild the app's structure** (header, pages, window size, theme card), design first (the `ui-ux-designer` agent), then code.
- **Centre the two-value pictures** with one rule change in `odc-render` and in the oracle. The Dancer stays as it is.
- **Make the tiles live by sending the app two more things and drawing every tile with the code the service already uses:** the readings, as a state key, and the sound's 14 column levels, as a signal, both only to frame watchers. `odc-render` stays the only drawing code, in the service and in the app (`crates/odc-render/src/lib.rs:1-5`).
- **Close with the owner's own steps:** a whole-design review by the designer, a review of all the app's code, a new build, and the owner's review.

### The owner's decisions of 2026-11-18

D1–D4 answer four questions about the owner's draft review, D5–D8 the solution review's four. Each is in the PRD's Decisions Log or the Phase 11a details, and in a "decided, not yet built" note in the spec where the spec is affected.

| # | Decision | Choice |
|---|---|---|
| D1 | The completed phases' plans, reports and solution reviews that mention fans and pump | **Keep history, strip the rest.** Fans and pump go from the PRD body, `AGENTS.md`, the README, the spec, the design, the code and the packaging. Their facts move to `docs/spec/fans-and-pump.md`. The completed phases' own records stay as written, with one note in the PRD |
| D2 | Two-value modes (4, 5, 6, 9, 12) on the cooler | **Centred as one block:** the wider value by the one-value rule (the extra spare column on the left), the shorter value's last digit under the wider one's. One-value modes were already centred in both directions |
| D3 | The Dancer | **No change** (owner, 2026-11-18, after seeing the pictures proposed for rows 2–13: "keep it as today ... no changes for the dancer, except that it should be centred horizontally"). Its rows (3–14), its pictures, its jump and its squat stay. Measured against `POSES` the same day, it is already centred sideways by the owner's odd-column rule: the standing figure's head is on columns 7–9, and seven of the eleven poses are centred by their widest columns; the two sways and the two points lean to one side on purpose. So nothing needs to change |
| D4 | The Music and Dancer tiles when the tiles go live | **Real sound only if already captured:** they show the live spectrum while the cooler is in a Music mode, and a still sample otherwise. When sound is captured doesn't change |
| D5 | The tiles' motion | **Real time, no pause control, and only while the window is on screen:** a minimised window, or one on another workspace, costs nothing. This knowingly leaves WCAG 2.2 SC 2.2.2 (Level A) to the owner's decision: there is no mechanism to pause the tiles (review P11A-06) |
| D6 | Where the theme choice lives, and OpenDeepCool's own theme | **The service keeps the choice** for the whole PC, with no manual interaction. **The designer creates OpenDeepCool's own theme**, dark and light, following the UI/UX standards and Anthropic's Frontend Design guidelines |
| D7 | System steps on the development PC | **Approved to ask:** remove the old package before Task 4, `odc-dev setup` after each service change, install the new package at Task 53. Each still asks for the password when it comes |
| D8 | The Dancer's pictures | **Shown, then withdrawn:** drawn on rows 2–13 for the owner on 2026-11-18, who answered that the Dancer stays as today. Nothing is left to approve |
| D9 | The owner's second list, 2026-11-19, and how it is done | **Sixteen small changes (C11–C26), each its own task and its own commit** (Tasks 17–32), before the build for review; **the full app run once, at the end** (Task 53 now; it was Task 33, then Task 42), and the owner is called only when everything is done (owner, 2026-11-19: "don't disturb me"). Each task updates the design and the mockups before the code (AGENTS.md) and runs only the tests it touches; the guard still runs its checks at every commit. The package installed on 2026-11-19 (before this list) is replaced by a new build at Task 53 |
| D10 | The owner's third list, 2026-11-22, and how it is done | **Nine small changes (C27–C35), each its own task and its own commit** (Tasks 33–41), before the build for review, which moved to Task 42 (and to Task 53 with D11) (owner, 2026-11-22: "Start working on it without asking me... only call me at the end"). The method is D9's: the design and the mockups first, then the code and the test that measures the change, then one commit with the guard's checks. Numbers and wording the owner didn't give were taken without asking, and each is stated in its task so the review can change it: the gap under a page's title 24 px (from 14), the gap between the cards 6 px (from 10), the notice 48 px above the panel's foot (3 × 16), the knob of a scale in the colour of an on switch's knob. No other scope was added |
| D11 | The owner's fourth list, 2026-11-22, and how it is done | **Eleven changes (C36–C46), each its own task and its own commit** (Tasks 42–52), before the build for review, which is now Task 53. The method is D9's, with the owner's new rules (2026-11-22: "only run the necessary tests when you have finished everything", "dont bother me at the end, not before. Only ask me something if you have any doubts"): each task is committed after the design, mockups, code and its test are written, the suites run once after Task 52, and the owner hears nothing until it is all done. The choices taken without asking, each stated in its task: the caption-to-status gap 36 px (from 24); the notice 64 px above the panel's foot (the owner's "one more space": one more 16 px step after the 3 × 16); the three words and two words of the Default Theme options are Omarchy's own (`Window Opacity`: opaque, transparent, glassy; `Window Corners`: sharp, rounded; read in Omarchy's menu and `omarchy-hyprland-window-transparency`, `omarchy-hyprland-corners`, 2026-11-22); the defaults are opaque and rounded, as the app is today; the system theme's window opacity is read from the toggle files those two programs write, never by running them; the alpha numbers and the new palettes are the designer's; the emoji gallery shows twenty animations and changes nothing on the cooler until the owner picks |

## Recommendation

**One data path for the live tiles, and no second engine.**
- The app already links `odc-render`, and the tiles already draw with it (`crates/odc-app/src/ui/tiles.rs:56-76`). What it lacks is the data: today it gets the current mode's frame only.
- `Values::of(&Snapshot)` (`crates/odc-render/src/lib.rs:67`) and the state machines that the engine owns (`Marquee`, `EmojiLoop`, `Dancer`, `Spectrum`: `crates/odc-service/src/engine.rs:140-143,348-364`) are pure and have no clock, so the app can step one of each per tile on its own tick.
- The invariant "a tile is the cooler's picture for that mode" then holds by construction, and a test proves it: a tile's home frame equals the frame the service writes for the same readings.
- The alternative, the service drawing 23 frames a tick, would need 23 engines' state and 23 frames a tick on the bus.

**Fans and pump sit behind one seam, so removal is deletion.**
- In `odc-sensors` they are `fans.rs`, the two `Fan` fields of `Snapshot` (`model.rs:175-198`) and `Registry::choose_fans` (`registry.rs:125-136,167-170`).
- In the service they are two saved keys (`settings.rs:130-132`), `fan_rpms` in the sensor thread (`runtime.rs:422,471-474`), and 2 methods and 7 state keys in the contract (`docs/spec/dbus.md`; `crates/odc-ipc/src/state.rs:23-29`).
- The sensor detection (`crates/odc-sensors-detect`, two root units, its record, the install script's undo, a package dependency on `lm_sensors`) exists, by the spec's own account, only to load the driver of the chip that measures them (`docs/spec/sensors.md:64-75`); the CPU, GPU and memory rules read drivers the kernel loads by itself. It goes whole.

**The two centring changes are one rule each.**
- The one place that decides a picture's home column is `crates/odc-render/src/picture.rs:102-113`, mirrored by the oracle at `tools/oracle/src/rules.rs:203-211`. Today one value is centred and two values end at column 14. The owner's decision is that both are centred, and the spare column of an odd split goes on the left.

### Evidence

- `crates/odc-app/src/ui/tiles.rs:28-76` — the tiles are drawn from `sample_values()` and a fixed `SAMPLE_BARS`; they redraw only on unit, peak or theme changes. The live version replaces this input, not the drawing.
- `crates/odc-render/src/marquee.rs:22,37,59`, `emoji.rs:167,190`, `dancer.rs:287`, `spectrum.rs:170,184,201,240` — the pure state machines and `Spectrum::at`, which the app already uses.
- `crates/odc-service/src/dbus.rs:1409-1473` — `tell_watchers` sends `StateChanged` and withholds `FanInputs` from anyone who doesn't watch frames; `tell_frame_watchers` sends `Frame` to each frame watcher alone after the one-second check. The new data copies this pattern, and takes over the withheld key's slot.
- `crates/odc-service/src/settings.rs:141-143` — a `#[serde(flatten)] later: toml::Table` keeps fields this build doesn't know and writes them back. Left alone, the two retired fan keys would stay in the file for good, so they are dropped at load (Task 4).
- `packaging/arch/opendeepcool.install:92-137,160-162` and `crates/odc-pkg-test/src/run.rs:30-33,185-265,582-587` — the package undoes the detection on removal, from its root-only record, and the package test proves it. Removing the old package on the development PC therefore undoes the detection under rule 7 with code that is already tested, so the new package carries no migration code.
- `docs/spec/settings.md:108-110` — the owner decided on 2026-11-17 that the default theme follows the system's dark or light preference and that the app gets a dark/light option, and left two points to this review: which interface the app reads (the XDG portal's `color-scheme`, or GTK 4.20's `gtk-interface-color-scheme`, which `crates/odc-app/Cargo.toml:22` already enables with `v4_20`), and what "no preference" shows.
- `crates/odc-testkit/src/weston.rs:36,93-94` — the headless output is 1280 × 800 and weston takes `--width` and `--height`, so a small screen can be tested.
- The owner's review items, with the codes this plan uses for them, are mapped to tasks under [Scope](#scope).

### Alternatives considered

- **Service-drawn tiles (23 frames a tick):** loses on state and bus cost (above).
- **A per-user theme file:** rejected by the owner (2026-11-18, "service ... no manual interaction"): pacman can't remove a file in a home folder, so uninstall would need a manual step, and the service already keeps every other setting.
- **Renumbering Releases to Phase 13:** about sixty edits to completed phases' records. Not worth it.
- **Migration code in the package for the detection's leftovers:** the only package that ever ran the detection is the development PC's; `pacman -R` of that package already undoes it. Code that runs for one installation is not worth shipping.

## Visuals

### The app, before and after

| Surface | Before | After | User impact |
|---|---|---|---|
| Header | Title at the left, a Display / Configuration switcher in the middle, a bordered "Refresh" at the right | Title centred; a gear and a close icon at the right, no borders | Fewer controls; the page change is one icon |
| Configuration page | A tab; four cards (Display, Cooler, Music settings, Startup); a label at the far left and its control at the far right | Reached by the gear, with a back icon; cards Theme, Display (with the unit after Rotation), Music settings, Startup; each label next to its control | Settings are together and readable |
| Display page | Unit toggles, then the four tile groups | The tile groups, and Refresh as an icon at the top right; every tile live | The page shows the cooler's whole range at once |
| Preview column | Picture, caption, "Shown on the cooler now", status, fan and pump RPM | Picture, caption, status | No fans, no repeated line |
| Window | 1040 × 720 default, 720 × 560 minimum, tiled | The size of its content, scrolling on a small screen, floating where the window manager allows | Fits any monitor |

### The live data's path (new or changed in bold)

```mermaid
flowchart LR
  S[Sensor thread: one Snapshot a second] --> E[Engine: draws the shown mode, 10 Hz]
  A[Sound window] --> E
  E --> D["D-Bus: Frame (as now), **Readings** state key, **Levels** signal<br/>only to frame watchers, one-second check"]
  S --> D
  D --> C[App client]
  C --> T["**Live tile driver**: one 100 ms tick while the window is shown<br/>a Marquee, EmojiLoop, Dancer or shared Spectrum per tile, from odc_render"]
  C --> P[Preview: the frame, as now]
  T --> W[23 tile drawing areas, redrawn only when their frame changes]
```

The service stays the only reader of sensors and sound, and the only authority on who may watch. The app adds no polling of the bus: the tick drives only the tiles' own animation.

## Implementation Context

### Mandatory reading

| File | Why it matters |
|---|---|
| `AGENTS.md` (rules 1–5, 7, 8, 10, 13, 14; Commands) | The rules every task obeys: one implementation, one commit; tests with every step; refusal tests at every entry point; no machine data; Rust everywhere; Arch, not Omarchy |
| `docs/design/settings-app.md` | The current design the designer changes: Window, Preview column, Display page, Configuration page, Banners, Refresh, Start at boot, States, Theme, Keyboard, Accessible names, Coverage check |
| `docs/spec/display.md` (Static layout, "OpenDeepCool: alignment"; Scrolling layout) | The rule Task 12 changes |
| `docs/spec/dbus.md` (Methods, State, Watchers) and `docs/prp/solution-reviews/project-overview.review.md` ("Entry points after the cross-check") | The contract Task 13 adds to, and the refusal tests it needs |
| `docs/prp/solution-reviews/phase-10-settings-app.review.md` (P10-01) | Why `Frame` goes to each watcher alone, after a one-second check: the new signal follows it |
| `crates/odc-app/src/ui/window.rs:35-36,197-262,331-460,520-530,739-768` | The sizes, the header, the page stack and actions, and the rules for when the switcher and Refresh show |
| `crates/odc-app/src/model.rs` (fans: 159-203, 272-335, 369-414, 748-800; caption: 505) | The pure view model, which most of Task 3 deletes from |
| `crates/odc-app/src/ui/config_page.rs`, `display_page.rs`, `tiles.rs`, `startup.rs`, `theme.rs`; `crates/odc-app/src/theme.rs:174-203` | The pages, the tiles and the theme the later tasks change |
| `crates/odc-service/src/dbus.rs:1409-1500`, `engine.rs:140-143,348-364`, `status.rs:71-74`, `runtime.rs:417-474,559-584`, `settings.rs:104-200` | Where fans leave, and where the live data comes from |
| `crates/odc-render/src/picture.rs:74-121`, `tools/oracle/src/rules.rs:188-215`, `crates/odc-render/src/dancer.rs:11-60`, `tools/oracle/src/dancer.rs:12-45,283` | Where the centring rules live |
| `packaging/` and `crates/odc-pkg-test/src/run.rs` | The package, its install script and the test that proves install and uninstall |

### Existing patterns and primitives

- **Watcher signals addressed to one caller:** `tell_frame_watchers` and `signal_frame` (`crates/odc-service/src/dbus.rs:1452-1482`), with the watchers' cap and check in `crates/odc-service/src/watchers.rs`. `Readings` and `Levels` use them; there is no new method and no new cap.
- **Retiring a saved key:** none exists yet. The flatten table at `settings.rs:141-143` is where a retired key lands, so the loader removes the two keys from it, and a test loads a file that still has them.
- **A new saved setting:** the Music settings were added as optional fields with serde defaults, so a file written before them loads with Winamp's defaults (`crates/odc-service/src/settings.rs:134-140`). The two theme keys follow that precedent.
- **Headless display tests:** `odc_testkit::weston` (`crates/odc-testkit/src/weston.rs`), the fake service (`odc-testkit`) and the AT-SPI driver (`crates/odc-ui-e2e`), whose scenarios are listed at `scenarios.rs:485-515`.
- **Generated mockups:** `cargo run -p odc-mockups -- docs/design/mockups` from the states in `tools/mockups/src/lib.rs` (Rust) and the sources in `docs/design/mockups-src/` (`app.js`, `app.css`); `cargo test -p odc-mockups` fails when a committed mockup differs.

### Integration points

- `crates/odc-ipc/src/state.rs:23-29` and `STATE_KEYS` — a state key is added here and in `docs/spec/dbus.md` together; the contract tests (`crates/odc-ipc/tests/contract.rs`) check the two agree.
- `crates/odc-testkit/src/fake_service.rs` — the app's fake service must emit everything the real one does, or the app's tests prove nothing.
- `tools/oracle/src/scenarios.rs`, `docs/spec/frames/` — regenerated by `cargo run -p odc-oracle -- docs/spec/frames`; the committed files are the unit tests' expected frames.

## Scope

### In scope

- The owner's review: every item below, the decisions D1–D4, and the owner's closing steps (a design review, a code review, a new build and the owner's review).
- The documents that change with them: PRD, spec, design and mockups, README, `AGENTS.md`, the designer's brief, the package's footprint.

| Code | The owner's item (2026-11-18) | Task |
|---|---|---|
| R1 | Fans and pump leave the product: no reading, no service, no display, no mention; their facts stay in a spec file (D1) | 2–6 |
| R2 | Remove the "Shown on the cooler now" line under the preview | 8 |
| R3 | Remove the Display tab | 7 |
| R4 | Remove the second sentence of the Start at boot hint, "Each change asks for the administrator's password." | 8 |
| C1 | The window launches outside tiling mode, at the smallest size that fits its content; scrollbars on a small screen | 9 |
| C2 | The app title is centred in the top bar | 7 |
| C3 | Configuration becomes a gear icon at the right, beside a close icon; a back icon on the Configuration page; no borders on the gear, the X or the back icon | 7 |
| C4 | Labels sit next to their controls on the Configuration page, left-aligned | 8 |
| C5 | Refresh is a borderless icon at the top right of the display page | 7 |
| C6 | The temperature unit moves to Configuration, Display section, after Rotation | 7 |
| C7 | The switches are a little smaller | 8 |
| C8 | Every mode tile shows live values, all animating together (D4: Music tiles live only with sound already captured; D5) | 13, 14 |
| C9 | Modes 1–12 centred on the cooler, the Music modes unchanged; the odd spare column goes on the left (D2) | 12 |
| C10 | The Dancer centred horizontally and vertically: no change, it stays as today and is already centred sideways (D3) | - |
| A1 | A Theme card, first in Configuration: "use system theme", and a dark / light toggle; OpenDeepCool's own theme designed for it (D6) | 10, 11 |
| C11 | The password prompt of the Start at boot switch says what it does: "Automatically start the cooler service at boot." and "Disable automatic cooler service start at boot." (owner, 2026-11-19) | 17 |
| C12 | No "Not changed: the password dialog was closed." note, and no note section for it: the switch shows the state | 18 |
| C13 | No terminal commands in the app, on the first start or in Configuration; they stay in the README, the manual | 19 |
| C14 | Start at boot: no "For the whole PC" label | 20 |
| C15 | Startup: no "When it's on, the display service starts …" text | 21 |
| C16 | Music: no "For every Music option, except Dancer …" text | 22 |
| C17 | Display: no "Turns the picture clockwise" text | 23 |
| C18 | Display: no "Used wherever the display shows a temperature" text | 24 |
| C19 | Theme: "Using the Omarchy theme." moves under "Use system theme" | 25 |
| C20 | The switches in Configuration are smaller | 26 |
| C21 | The sliders' knobs in Configuration are a little smaller, not the bars | 27 |
| C22 | The vertical gap between the cards in Configuration is smaller, not the gaps to the window, the bar or the preview | 28 |
| C23 | The "Refreshed" notice is centred in the right panel, not in the window | 29 |
| C24 | Left panel: more space between the picture's caption and the status items | 30 |
| C25 | Left panel: the caption and the status items are centred in it | 31 |
| C26 | More space between a page's title and its content, on Display and on Configuration | 32 |
| C27 | Start at boot: no phrase under the switch saying it is turning on or off | 33 |
| C28 | The switches are top-aligned with their labels: Use system theme, Peak dots, Start at boot | 34 |
| C29 | The sliders' knobs are the colour of the switches' knobs | 35 |
| C30 | The Display title is centred in the right panel | 36 |
| C31 | The Configuration title is centred in the right panel | 37 |
| C32 | More space between the Display and Configuration titles and their pages' content (again) | 38 |
| C33 | A smaller gap between the Configuration cards, Theme, Display, Music and Startup only (again) | 39 |
| C34 | The "Refreshed" notice stands three times as far from the right panel's foot | 40 |
| C35 | The Refresh button's tooltip is "Refresh the display readings" | 41 |
| C36 | Left panel: more space between the picture's caption and "Cooler connected" (again) | 42 |
| C37 | The "Refreshed" notice stands one more step above the right panel's foot | 43 |
| C38 | OpenDeepCool's own Dark and Light themes are redesigned: not just white and black, with the Anthropic Frontend Design guidance | 44 |
| C39 | With the system theme, the window follows Omarchy's window opacity: opaque, transparent, glassy | 45 |
| C40 | Configuration, Theme: the line under "Use system theme" is as close to it as a hint is to "Peak dots" | 46 |
| C41 | Configuration: the label "Dark or light" becomes "Appearance" | 47 |
| C42 | Configuration: the space under the last card equals the space from the page's top to the first card | 48 |
| C43 | Window Opacity (opaque, transparent, glassy) and Window Corners (sharp, rounded) are settings of OpenDeepCool's own theme, kept by the service | 49 |
| C44 | A "Default Theme" subcategory in the Theme card holds Appearance, Window Opacity and Window Corners, all disabled (its label too) while the system theme is used | 50 |
| C45 | OpenDeepCool's own theme applies the window opacity and the window corners | 51 |
| C46 | Twenty original emoji animations on an HTML page for the owner to choose from, instead of DeepCreative's four | 52 |

### Not building

- **Fan or pump readings or control** in any form, here or later in v1. Their facts stay in `docs/spec/fans-and-pump.md`.
- **Real sound captured while the window is open** outside a Music mode (owner, 2026-11-18, D4).
- **Following other desktops' themes beyond their dark or light preference** (rule 8): "use system theme" means the Omarchy theme or the system's dark/light preference, nothing else.
- **A Hyprland window rule shipped by the package** (rule 14). A rule in the owner's own configuration would be a separate change the owner approves (rule 7).
- **Creative Mode, ARGB, hidden display settings** (rule 8): unchanged.

## Delivery Considerations

| Concern | Decision and owned work |
|---|---|
| Compatibility / migration | No release exists yet, so the D-Bus contract (frozen at `v1.0.0`) and the settings file can still change. A settings file that still holds the two fan keys loads, and its keys are dropped at the first save (Task 4, with a test). The only package that ever ran the sensor detection is the development PC's: the owner removes it with `pacman -R` before this phase's service tests, and that removal undoes the detection from its record (rule 7; the package test proves it), so the new package carries no migration code (Tasks 4 and 5). A settings file written before the two theme keys loads with their defaults (Task 10) |
| Rollout / reversibility | Everything lands on a phase branch, one commit per task, and moves to `main` at sign-off. Each commit builds and passes its tests, so a task can be reverted alone. The development PC moves from the package to the dev copy for the service tests and back to the new package at the end (Task 33); each step is a system change the owner approves first |
| Discoverability | The README's screenshots and text name no fans or pump, and its "Installed on your system" list changes (Tasks 5, 6) |
| Documentation / communication | The design, the mockups, the spec, the PRD, `AGENTS.md` (crate list, layout, commands) and the designer's brief change in the task that changes their subject. Task 6 sweeps what is left |
| Observability | `opendeepcool --report` (spec `docs/spec/report.md`) loses its fan lines in Task 3 |

## Implementation

Each task is one implementation and **one commit** (rule 4), after its own tests pass, with its documents changed in the same commit. A UI task goes in this order: the `ui-ux-designer` agent first updates `docs/design/settings-app.md` and the sources in `docs/design/mockups-src/` and reads the current app code and design again; then the implementer encodes the mockup states in `tools/mockups/src/lib.rs`, regenerates the mockups, and writes the code and tests. The designer has no shell and writes no Rust, so it never builds, tests or commits. Each task ends by ticking its row in the table at the end of the phase's report.

Real runs that need the changed service use the dev copy (Tasks 4, 10 and 12 to 14); see Delivery Considerations, and ask the owner before each system step.

### 1. Settle the open points before they harden

**Files and integration points**
- `docs/design/settings-app.md` — UPDATE — a section "Unverified points, answered (Phase 11a Task 1)", as Phase 10's Task 1 did.
- A throwaway gtk4-rs program under the scratchpad (never committed) for the checks that need a window.

**Implementation**
- Answer each point against docs.gtk.org (Context7 or Firecrawl, then the page itself), then with the minimal program in headless weston, and give each a fallback that changes a widget and never the behaviour:
  1. **The system's dark/light preference:** what `GtkSettings:gtk-interface-color-scheme` reads under headless weston with and without a portal, whether GTK fills it from the XDG portal's `color-scheme` itself, and whether the app can test its `UNSUPPORTED`, `DEFAULT`, `DARK` and `LIGHT` with a fake `org.freedesktop.portal.Desktop` on the tests' private session bus, with no test flag. Recommendation to confirm: read the GTK property, `DEFAULT` and `UNSUPPORTED` show the dark default theme, and "system theme found" means an Omarchy theme or a property that isn't `UNSUPPORTED`.
  2. **A window that fits its content:** how a `GtkScrolledWindow`'s natural size propagates to the window (`propagate-natural-width/height`, `max-content-width/height`), and how to cap it to the monitor before the window is mapped on Wayland.
  3. **A floating window:** whether a window with a fixed size (`resizable` false), or with another hint GTK can give, is floated by Hyprland, read once from the owner's session with `hyprctl clients -j` and recorded as `system:` evidence in the report; never a test, because rule 14 forbids tests that assume Omarchy. If nothing GTK can say floats it, the design says so, and the owner decides (Risks).
  4. **A header with our own close icon:** `GtkHeaderBar::show-title-buttons` false with our own gear, back and close buttons, so desktops that draw window buttons don't show two.
  5. **Smaller switches:** which CSS on `switch` and `slider` shrinks a `GtkSwitch` under the app's stylesheet.
  6. **The cost of 23 live tiles, and what stops them:** a program drawing 23 `GtkDrawingArea`s of 14 × 14 dots at 10 Hz under weston, measured with `/proc/<pid>/stat`, once driven by a `glib` timeout and once by a `Widget::add_tick_callback` counted to 100 ms, with the window shown, hidden and unmapped. The tiles must move only while the window is on screen (owner, 2026-11-18): a window that is minimised or on another workspace must cost nothing. The frame clock is expected to stop for a surface the compositor doesn't draw, which a timeout doesn't (**unverified**); if neither stops on the owner's compositor, the result goes to the owner before Task 14.
  7. **The remaining sources' drivers without lm-sensors:** whether the kernel loads, by itself, the drivers the CPU temperature, CPU power and memory rules read (`coretemp`, `k10temp`, the powercap RAPL driver), from the kernel's module alias tables and the Arch wiki, since the detection also loaded such drivers on a PC where they weren't. Where one doesn't autoload, the modes that need it are disabled with their reason, as the spec already says, and the README names the driver to load; the product loads none (**unverified**).
- The first task also compares this plan with what Phases 5–11 built: the names, files and commands above were read from the tree on 2026-11-18.

**Tests**
- None in the repository; the answers are evidence in the design.

**Validation**
- `cargo test -p odc-mockups` — still passes (the design text changed, the states didn't).

### 2. Keep the facts about fans and pump

**Files and integration points**
- `docs/spec/fans-and-pump.md` — CREATE — everything the project learned about measuring and showing the cooler's fan and pump speeds, in one document, so that a later version can bring them back.
- `docs/spec/README.md` — UPDATE — its index row.

**Implementation**
- Copy, with their evidence labels and dates, from: `docs/spec/sensors.md` ("Fan and pump speed", and the "Board sensor driver" detection with its sandbox, record and evidence), `docs/spec/settings.md` (the fan inputs, their RPM, the two pickers' rules), `docs/spec/dbus.md` (`FanInputs`, the two input keys and their present flags, the two RPM keys, the two setter methods), `docs/design/settings-app.md` (the Cooler card and the RPM rows, as a description of what the app showed), `docs/spec/parity-checklist.md` (S9, T1's fan part), the PRD's decisions about fan inputs, the detection and fan control, and `AGENTS.md` rule 8's `pwm*` note.
- Machine-specific facts (which input is which on the owner's board) stay in `docs/local/` and are not copied (rule 10).
- A banner at the top: removed from OpenDeepCool on 2026-11-18 (owner); kept for a later version; nothing here is built; the sources are `docs/spec` at commit `1a6a4e9`.

**Tests**
- None; `cargo run -p odc-guard -- history` checks the commit like any other.

**Validation**
- Every fact in the old sections is in the new file or is named as dropped: a diff of the sections against the file, read by the implementer, with the list in the commit message's body.

### 3. The settings app stops showing and choosing fans and pump

**Files and integration points**
- `docs/design/settings-app.md`, `docs/design/mockups-src/app.js`, `tools/mockups/src/lib.rs` — UPDATE (designer, then implementer) — the preview column's RPM rows, the Cooler card, the "missing input" and "no fan sensors" states, the coverage check and the accessible names go. Two mockups (`missing-input`, `no-fan-sensors`) go; the others lose the rows.
- `crates/odc-app/src/model.rs:159-203,272-335,369-414,748-800`, `ui/window.rs` (`SpeedRow`), `ui/config_page.rs`, `ui/mod.rs:30-32`, `client.rs:107,246-252` and `ui/dropdowns.rs` (its two test-only row helpers) — UPDATE — the view model, the rows, the Cooler card and the two requests go. (The bug report's only mentions are the detection's units and program, which go in Task 5.)
- `crates/odc-app/src/model/tests.rs`, `tests/config_page.rs`, `tests/common/mod.rs`, `tests/window.rs`, `crates/odc-ui-e2e/src/scenarios.rs` (`inputs`, `missing_input`, `no_fan_sensors`, and the `names` list), `crates/odc-ui-e2e/src/fake.rs` — UPDATE — the tests that exercised them go with them.

**Implementation**
- The app stops calling `SetCoolerFanInput` and `SetPumpInput` and ignores the fan keys; the service still publishes them until Task 4, which keeps the build green at this commit. The preview column keeps its picture, caption and status items.

**Tests**
- The view model's tests no longer mention fans; the preview column's words are asserted without the RPM rows; `--only app` runs the remaining scenarios.

**Validation**
- `cargo test -p odc-app` and `cargo test -p odc-mockups` — pass.
- `cargo run -p odc-e2e -- --only app --require weston` — every remaining scenario passes.

### 4. Fans and pump leave the service, the contract and the sensors

**Files and integration points**
- `crates/odc-sensors/src/fans.rs` (DELETE), `model.rs:175-198`, `registry.rs:8,61-62,90-91,125-136,167-170`, `lib.rs:7,19,24`, `tests/fans.rs` (DELETE), `tests/model.rs` — UPDATE — `Fan`, the two `Snapshot` fields, `choose_fans`, `fan_inputs` and `fan_rpm` go.
- `crates/odc-ipc/src/lib.rs:12,87-90`, `state.rs:23-29,120-172`, `tests/contract.rs` — UPDATE — `SetCoolerFanInput`, `SetPumpInput`, the seven keys and the types `FanInput`, `FanSpeed` and `Input` go.
- `crates/odc-service/src/settings.rs:107-132,141-143,190-198,227-260`, `runtime.rs:417-474,559-584`, `status.rs:71-74`, `service.rs:35-37,175,208`, `main.rs:13,85-99,146-166`, `engine.rs:67,80-100,154,203-208`, `dbus.rs:317-330,616-685,1435-1438`, and `tests/dbus.rs`, `tests/settings.rs` — UPDATE.
- `crates/odc-testkit/src/{fake_service,fixtures,hardware}.rs`, `tests/fake_service.rs`, `tests/hardware_probe.rs` — UPDATE.
- `crates/odc-e2e/src/sensors.rs`, `src/lib.rs:35`, `tests/flags_and_fakes.rs` — UPDATE — the `board-fans` capability, the `fans` check and the fan part of the timing check (`sensors.rs:867-900`) go; `sensors-tool` stays, because the CPU check uses `sensors -j`.
- `docs/spec/dbus.md`, `docs/spec/settings.md`, `docs/spec/sensors.md`, and the entry-points table in `docs/prp/solution-reviews/project-overview.review.md` ("The service's D-Bus methods": the two setters leave its list) — UPDATE.
- `docs/spec/parity-checklist.md` — UPDATE — in this commit, because `crates/odc-testkit/tests/parity_checklist.rs` checks that every test a row links still exists: row S9 links four fan tests (`crates/odc-sensors/tests/fans.rs`, and `fan_speeds_reach_only_frame_watchers_and_follow_the_readings` in `crates/odc-service/tests/dbus.rs`), which this task deletes. Row S9 is deleted (its text is in `docs/spec/fans-and-pump.md` from Task 2), and row T1 loses "no fan inputs".

**Implementation**
- The settings loader removes `cooler_fan_input` and `pump_input` from the flatten table, so an old file loads and loses the keys at its next save; `check_input` and `check_device_path`, used only for them, go (`check_pci` stays: the GPU choice uses it). `VERSION` stays 1: the file's form didn't change, only two optional keys left.
- With the package installed the service tests can't run (`odc-dev setup` refuses), so the owner removes the package first (`sudo pacman -R opendeepcool`), which undoes the detection from its record, then runs `cargo run -p odc-dev -- setup`. Both are system changes: ask first (rule 7).

**Tests**
- A settings file with both keys loads, the service uses the defaults for them, and the next save has no trace of them. An unknown method or key from an older client is refused as before. The contract test shows the state keys equal the spec's table. Property tests on the sensors' snapshot are unchanged.

**Validation**
- `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check` — pass.
- `cargo run -p odc-e2e -- --only sensors --require <this PC's list from docs/local/, minus board-fans>` — passes.
- `cargo run -p odc-e2e -- --only service --require dev-service` — passes, after `odc-dev setup`.

### 5. The sensor detection leaves the package

**Files and integration points**
- `crates/odc-sensors-detect/` (DELETE, whole crate), `packaging/systemd/opendeepcool-sensors-detect.service` and `…-detect-modules.service` (DELETE), `packaging/systemd/opendeepcool.service:4,7-8` — UPDATE — `Wants=` and `After=` drop the detection and `lm_sensors.service`.
- `packaging/arch/PKGBUILD:18,28,59,79-90`, `.SRCINFO`, `opendeepcool.install:16-17,46,73-78,82-137,160-162`, `packaging/FILES.md`, `crates/odc-service/tests/packaging.rs` — UPDATE — the `lm_sensors` dependency, the program and the two units, the install message and the undo of the detection go.
- `crates/odc-app/src/report.rs` and `docs/spec/report.md` — UPDATE — the detection's two units and its program leave the bug report's lists.
- `crates/odc-pkg-test/src/{main,run}.rs`, `Cargo.toml` — UPDATE — the lm-sensors cases, their constants and the detection record helper go; the install, upgrade and uninstall cases stay, and the unit score no longer reads the two units.
- `AGENTS.md` (the crate table), `README.md` ("Installed on your system"), the PRD's System Footprint (the detection rows and the lm-sensors list), and the entry-points table in `docs/prp/solution-reviews/project-overview.review.md` (the row "The detection record") — UPDATE.

**Implementation**
- The new package never creates `/etc/conf.d/lm_sensors`, a modprobe file, a root-only record or an enabled `lm_sensors.service`, so its uninstall has nothing to undo, and nothing in it runs as root at every boot (the PRD's "service model" no longer has that exception).
- On the development PC the owner removes the old package before Task 4's service runs (above); this task needs nothing more there.

**Tests**
- The package test proves the system is left exactly as before, installed and removed, both ways. `packaging.rs` proves `FILES.md` lists every file under `packaging/` and every unit's `ExecStart=` names an installed program.

**Validation**
- `cargo test --workspace`, `cargo clippy …`, `cargo fmt --all --check` — pass.
- `cargo run -p odc-e2e -- --only package --require container` — passes, after the commit and before the push (about ten minutes; it refuses an uncommitted tree).

### 6. The documents drop fans and pump

**Files and integration points**
- `docs/prp/prds/opendeepcool-lp360.prd.md` — UPDATE — the body and its tables (problem and scope text, What We're NOT Building, Core Capabilities, Technical Approach, Risks, the Phase 6, 10 and 11 descriptions, System Footprint) lose fans, pump and the detection. The Decisions Log rows about fan inputs, fan control, the detection and the product scope are replaced by the one row of 2026-11-18 (D1), plus one dated note that the completed phases' plans, reports and reviews still describe them.
- `AGENTS.md` (the opening scope line and rule 8), `README.md`, `.claude/agents/ui-ux-designer.md` (the earlier briefs' fan lines), `docs/spec/{settings,sensors,dbus,display}.md`, `docs/hardware/deepcool-lp360.md` — UPDATE — whatever still says OpenDeepCool reads, shows or chooses fans or the pump. (The parity checklist's rows S9 and T1 went in Task 4, with the tests they link; the "100% of parity items" metric counts only the rows that remain.)

**Implementation**
- Rule 8's ban on writing `pwm*` stays as one sentence, as a safety rule: the product writes no fan or power setting (Risks).
- The hardware notes about the LP360's own cables stay: they describe the cooler, and say nothing of OpenDeepCool reading them.
- Completed phases' plans, reports and reviews are not edited (D1).

**Tests**
- A check, run by the implementer and in the report: `grep -rniE "\bfans?\b|\bpump\b|fan_input|pump_input|sensors-detect|lm_sensors|lm-sensors" .` over live files (not `docs/prp/{plans,reports,solution-reviews}`, not `docs/spec/fans-and-pump.md`, not `docs/local`, not `target`) finds only `AGENTS.md` rule 8's one sentence, the PRD's note, and `docs/hardware/deepcool-lp360.md`'s own cable facts.

**Validation**
- The grep above; `cargo test -p odc-service --test packaging`, `cargo run -p odc-guard -- history` — pass.

### 7. Header and pages: no Display tab, title in the middle, gear and close icons, back icon, Refresh and the unit moved

**Files and integration points**
- `docs/design/settings-app.md` (Window, Display page, Configuration page, Refresh, Keyboard, Accessible names, States, Coverage check), `docs/design/mockups-src/*`, `tools/mockups/src/lib.rs` — UPDATE (designer, then implementer).
- `crates/odc-app/src/ui/window.rs:69-86,197-262,331-460,520-530,739-768` — UPDATE — the header, the page stack, the actions and the visibility rules.
- `crates/odc-app/src/ui/display_page.rs:61-250` (the unit toggles, `unit_request`) — UPDATE — the unit leaves; the page keeps the tile groups and holds the Refresh icon at its top right.
- `crates/odc-app/src/ui/config_page.rs` — UPDATE — the Display card gains the unit after Rotation, and a back icon heads the page.
- `crates/odc-app/tests/{window,display_page,config_page}.rs`, `crates/odc-ui-e2e/src/scenarios.rs` (`names`, `unit`, `refresh`, `shortcuts`, `service_down`, `first_start`, `not_available`) — UPDATE.

**Implementation**
- R3, C2, C3, C5 and C6 together, because they share the header and the stack.
- The header shows the title in the middle (`set_title_widget`), and a gear and a close icon at the right, both without frames and with fixed accessible names; `show-title-buttons` is off if Task 1's check says so, so no desktop draws a second close button. A back icon heads the Configuration page without a frame; the designer picks its side by the GNOME and KDE human-interface guidelines.
- Refresh is an icon without a frame at the top right of the display page. It keeps its accessible name, its tooltip, `F5` and `Ctrl+R`, and its rules from `window.rs:739-768`: absent when the app isn't available in this session, and not drawn while the service isn't running, since the off screen replaces the Display page it sits on (designer, 2026-11-19; its action is disabled in both).
- The gear is visible but insensitive on the service-off screen, as the switcher is today (D10: Start at boot lives on the off screen itself, and Configuration can't be reached while the service is down). Corrected 2026-11-19: the plan first said the gear stays usable there, which contradicted "as the switcher is today" and the design.
- The unit's request and the mode tiles' follow-the-unit redraw (`tiles.rs` `follows_unit`) keep their behaviour, now from the Configuration page.

**Tests**
- The view model and window tests assert each state of the header (normal, service off, not available), that the gear opens Configuration and the back icon returns, that the unit sits after Rotation and still sends `SetTemperatureUnit`, and that Refresh sends `Refresh`.
- The AT-SPI scenarios drive the same, with the new names.

**Validation**
- `cargo test -p odc-app`, `cargo test -p odc-app --features weston-tests`, `cargo test -p odc-mockups` — pass.
- `cargo run -p odc-e2e -- --only app --require weston` — passes.

### 8. Smaller changes: no caption line, no second sentence, labels beside controls, smaller switches

**Files and integration points**
- Design, mockups and tools as in Task 7.
- `crates/odc-app/src/model.rs:505` (the caption line's "Shown on the cooler now") and its test (`model/tests.rs:322`) — UPDATE.
- `crates/odc-app/src/boot.rs:243-244` and `boot/tests.rs:705`, `tools/mockups/src/lib.rs:324`, `docs/design/mockups-src/app.js:359` — UPDATE — the hint's second sentence goes; the "Turning on…" lines keep theirs.
- `crates/odc-app/src/ui/config_page.rs`, `ui/startup.rs:70`, `crates/odc-app/src/app.css` (included at `src/theme.rs:15`), `docs/design/mockups-src/app.css` — UPDATE — rows lay out label and control side by side, left-aligned; the switches are a little smaller.

**Implementation**
- R2: the line goes. The warning lines that say the cooler isn't reachable stay, since they are not "shown on the cooler now" and nothing else says them in the column (the designer may fold them into the status items; the design records which).
- R4, C4, C7 as the owner wrote them. "Switches" means the two `GtkSwitch`es (Start at boot, Peak dots); the unit and rotation buttons keep their size.

**Tests**
- The preview column's words in every cooler state; the Startup card's hint; each row's label and control in one box, left-aligned; the stylesheet test that no value is hard-coded still passes.

**Validation**
- As Task 7.

### 9. The window opens at the size of its content, and asks to float

**Files and integration points**
- `docs/design/settings-app.md` (Window), `crates/odc-app/src/ui/window.rs:35-36,205-214` — UPDATE — `DEFAULT_SIZE` and `MIN_SIZE` go. The pages' `scrolled()` helper (`window.rs:188-195`) gets Task 1's natural-size settings.
- `crates/odc-testkit/src/weston.rs` — UPDATE — `Weston::start` takes the output's size, so a test can start a small screen; the default stays 1280 × 800.
- `crates/odc-app/tests/window.rs`, `crates/odc-ui-e2e` — UPDATE.

**Implementation**
- The window takes the natural size of its content (`propagate-natural-width` and `-height` on each page's scrolled window, the stack homogeneous so it doesn't resize on a page change), capped at the smallest monitor's geometry minus a margin, and the page scrolls inside it (C1). It is not resizable (`resizable` false), which Task 1 measured makes Hyprland float it; for a compositor that ignores the hint the window is tiled and the design says so.
- No compositor rule, no Hyprland call and no Omarchy check in the app (rule 14).

**Tests**
- On a 1280 × 800 output the window opens at its natural size, with no scrollbar; on a small output (such as 640 × 480) it fits the output and each page has a vertical scrollbar. A window never opens wider than the output.

**Validation**
- `cargo test -p odc-app --features weston-tests`, `cargo run -p odc-e2e -- --only app --require weston` — pass.
- The owner's session check from Task 1 is repeated on the built window and recorded in the report.

### 10. The service keeps the theme choice (D6)

**Files and integration points**
- `docs/spec/settings.md` (the settings table, OpenDeepCool's defaults, and the "UI review" paragraph), `docs/spec/dbus.md` (two setters, two state keys) and the entry-points table in `docs/prp/solution-reviews/project-overview.review.md` — UPDATE first. The members' exact names and types are fixed in `dbus.md` before the code.
- `crates/odc-service/src/settings.rs:134-160,170-200` — UPDATE — two booleans, each with a serde default: use the system theme on, dark theme on. A wrong type already makes the file invalid, and it is moved aside with the defaults written; `VERSION` stays 1.
- `crates/odc-service/src/dbus.rs`, `crates/odc-ipc/src/{lib,state}.rs`, `crates/odc-testkit/src/fake_service.rs` — UPDATE — two setters under `change-settings`, two state keys and the proxy's methods.
- `crates/odc-service/tests/{dbus,settings}.rs`, `crates/odc-ipc/tests/contract.rs`, `crates/odc-testkit/tests/fake_service.rs`, `crates/odc-e2e/src/` (the service checks) — UPDATE.

**Implementation**
- The choice is the whole PC's and lives in `settings.toml` with every other setting (owner, 2026-11-18: "service ... no manual interaction"). Uninstalling already removes the service's state directory, so the footprint grows by nothing and the app writes no file.
- A setter that sets what is already set succeeds and changes nothing (`dbus.md`); neither touches the cooler or restarts the picture.
- The service stores two booleans and never interprets a theme. What "the system theme" is belongs to the app, in each session (Task 11).

**Tests**
- A file written before the keys loads with both on; each setter saves, signals `StateChanged` and survives a restart of the service; the same value again changes nothing; a caller polkit refuses gets `NotAuthorized`; a file with a wrong type for either key is moved aside and the defaults used. The contract test shows the state keys equal the spec's table.

**Validation**
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — pass.
- `cargo run -p odc-e2e -- --only service --require dev-service` — passes, after `odc-dev setup` (ask first).

### 11. A Theme card, and OpenDeepCool's own theme (D6)

**Files and integration points**
- `docs/design/settings-app.md` (Configuration page, Theme, Default theme and its contrast table, Accessible names, States), `docs/spec/settings.md` (the Theme bullets and "No app-only settings in v1", which stays true: both settings are the service's), `docs/design/mockups-src/*`, `tools/mockups/src/lib.rs` — UPDATE (designer, then implementer).
- `crates/odc-app/src/theme.rs:91-115,174-203` (the `DARK` and `LIGHT` palettes, and the theme's resolution), `ui/theme.rs:37-151`, `ui/config_page.rs`, `client.rs`, `model.rs`, `main.rs:35-60` — UPDATE — the resolved theme is one of: the Omarchy theme as today; OpenDeepCool's theme, dark or light, from the system's preference; OpenDeepCool's theme, dark or light, as chosen.
- `crates/odc-testkit` — UPDATE — a fake `org.freedesktop.portal.Desktop` on the tests' private session bus, with a service file for that name in a directory `PrivateBus`'s configuration lists: Task 1 measured that GDK uses the portal only when the name is activatable, and then reads `Default`, `Dark` and `Light` from it, live. No test flag is needed.

**Implementation**
- **OpenDeepCool's own theme** (owner, 2026-11-18: "It's us that defines the default OpenDeepCool theme. You should create it using UI/UX and Anthropic Frontend Guidelines"). The `ui-ux-designer` agent designs it, dark and light, as one deliberate whole: a core palette of named hex values, a type scale, hierarchy and spacing, restraint, and the quality floor (contrast, visible focus), following the UI/UX standards and Anthropic's Frontend Design guidelines, and avoiding the generic palettes those guidelines name. It replaces the Phase 10 default (a cool grey with a teal accent). The design's contrast table is recomputed for the new values, and `cargo test -p odc-mockups` and the theme tests check it. The Phase 10 design uses the system's font: the designer may propose a type scale on it, and a font that needs a new package dependency goes to the owner first (rule 7). No motion in the chrome.
- **Use system theme** on (the default): an Omarchy theme if one is found, else the system's dark/light preference applied to OpenDeepCool's theme (Task 1's interface). It disables the dark/light toggle, which shows what the system gives. The live Omarchy monitors (`ui/theme.rs`) and the property's notify keep it live.
- **No system theme found** (no Omarchy theme and an unsupported preference): the toggle is disabled and off, a warning says OpenDeepCool can't find the system's theme and is using its own, and the dark/light toggle is enabled.
- **Use system theme** off: the dark/light toggle is enabled and picks OpenDeepCool's theme dark or light, whatever Omarchy says.
- Each toggle sends `SetUseSystemTheme` or `SetDarkTheme` and shows the state the service confirms, like every setting. The app writes no file and changes no setting itself.

**Tests**
- Pure: every combination of (Omarchy found, preference, use system theme, dark theme) gives the expected theme. The contrast checks of both palettes (`theme.rs` tests) pass for the new values.
- Widgets: the card's states (system found; not found with the warning; toggle off), the dark/light toggle's sensitivity, a change confirmed by the service, and the theme changing live.
- AT-SPI: the `theme` scenario drives the card against the fake service and the fake portal, and a live Omarchy theme switch still reloads.

**Validation**
- `cargo test -p odc-app`, `… --features weston-tests`, `cargo test -p odc-mockups`, `cargo run -p odc-e2e -- --only app --require weston` — pass.

### 12. Pairs of values are centred as one block on the cooler (D2)

**Files and integration points**
- `docs/spec/display.md` (Static layout, "OpenDeepCool: alignment", the sentence on `°C` moving one column, and Scrolling layout's home) — UPDATE — the rule becomes: one value or two, the picture is placed by its width, with the extra spare column of an odd split on the left; two values end at the same column, so a pair is centred as the wider value would be, and the shorter keeps its last digit under the wider one's.
- `crates/odc-render/src/picture.rs:102-113` and `tools/oracle/src/rules.rs:203-211` — UPDATE — the `bands.len() == 1` branch goes; `left = spare - spare / 2` for every picture that fits.
- `docs/spec/frames/` — REGENERATE — `cargo run -p odc-oracle -- docs/spec/frames`. Only the frames of modes 4, 5, 6, 9 and 12 and their scenarios change.
- `crates/odc-render/tests/differential_{load,temperature,power,music}.rs` (`differs()`), `docs/spec/parity-checklist.md` — UPDATE — the places where a pair now differs from DeepCreative's capture are listed as differing by decision, with the reason, as the existing rows are; a row about a pair's alignment takes the `test: …; ours: ID` kind that `parity_checklist.rs` already checks, and its links must still name existing test functions.

**Implementation**
- Examples, for the spec and the tests: `42%` over `7%` doesn't move (the wider value is 13 columns); `5%` over `7%` (9 columns) moves from columns 6–14 to columns 4–12; a `°C` pair of two-digit values (12 columns) takes columns 2–13.
- A stopped picture is re-centred at its home when its width changes, which the movement rules already say (`display.md`, Movement, precisely): they use `home()`, so they follow.
- The rows of both bands, the one-value modes and the Music modes (`layout_at` with its own tops) don't change.
- One rule, one implementation, one commit, though five modes' frames change (Risks).

**Tests**
- The unit matrices of modes 4, 5, 6, 9 and 12 (`tests/matrix_*.rs`, `scenarios_*.rs`) compare each case with the regenerated frames; a property test says a pair's block is centred as the rule says for every pair of widths; `cargo test -p odc-oracle` fails if a committed frame differs from the oracle; every other mode's file is unchanged (the commit's diff shows it).
- Layer 2 (local): `cargo test -p odc-render --test differential_load --test differential_temperature --test differential_power --test differential_music -- --ignored --nocapture`.

**Validation**
- `cargo test -p odc-render -p odc-oracle`, `cargo run -p odc-oracle -- docs/spec/frames` (no diff after the commit) — pass.
- `cargo run -p odc-e2e -- --only display --modes 4-6,9,12 --require lp360`, `--only service --modes 4-6,9,12 --require dev-service` — pass: the real cooler shows the new frames.

### 13. The service sends the app the readings and the sound's levels

**Files and integration points**
- `docs/spec/dbus.md` (State: a `Readings` key, in the withheld key's place; Methods and signals: `Levels`; the watchers' text), and the entry-points table in `docs/prp/solution-reviews/project-overview.review.md` — UPDATE first.
- `crates/odc-ipc/src/state.rs`, `lib.rs`, `tests/contract.rs` — UPDATE — the key, its type and the signal in the proxy.
- `crates/odc-service/src/dbus.rs:1409-1500` (the key goes only to frame watchers, as `FanInputs` did; `tell_level_watchers`, modelled on `tell_frame_watchers`), `engine.rs:348-364` (the engine keeps the latest levels, as it keeps the latest frame in `shared.frames`), `status.rs` — UPDATE.
- `crates/odc-testkit/src/fake_service.rs`, `tests/fake_service.rs` — UPDATE — the fake emits both.
- `crates/odc-e2e/src/` (the service checks), `docs/spec/settings.md` — UPDATE.

**Implementation**
- **`Readings`:** one entry per metric that has a value, in the spec's metric names (as `Diagnostics` writes them), each as the exact fraction the display rounds from (`Exact`'s numerator and denominator), never floating point (`display.md`, Values). The exact D-Bus type is fixed in `dbus.md` before the code. It changes at most once a second, with the sensor thread's snapshot, and goes only to a watcher of frames, which `StateChanged` already withholds `FanInputs` from.
- **`Levels`:** the 14 column levels the engine analysed this tick, as `ad`, addressed to each frame watcher alone, only while a Music mode is shown and sound is attached, and only after the same one-second check. No new method and no new watcher cap. NaN or infinite values are never sent (the analysis already bounds them; a test proves it).
- A frame watcher already may see the Music modes' spectrum through `Frame`; `Levels` shows nothing more than that.

**Tests**
- Contract: the keys equal the spec's table; `Readings` round-trips every metric's exact value, including a metric with no value.
- Refusals (rule 5): a caller polkit refuses gets neither; a caller that doesn't watch frames gets neither; a watcher that fails its one-second check gets nothing until it passes; no `Levels` with no sound attached or outside a Music mode; a watcher that left the bus is dropped.
- Integration with the fake clock and audio: `Readings` equals the snapshot the engine drew; `Levels` equal `analyse` of the sound window.

**Validation**
- `cargo test --workspace`, `cargo clippy …`, `cargo fmt --all --check` — pass.
- `cargo run -p odc-e2e -- --only service --require dev-service` — passes, after `odc-dev setup`; it also checks that, for the shown mode, `odc_render::picture` over the service's `Readings` has the home frame the service wrote.

### 14. The tiles go live

**Files and integration points**
- `docs/design/settings-app.md` (Display page: Tile content, Selection; Window: "No motion in the chrome"; Preview column), `docs/design/mockups-src/*`, `tools/mockups/src/lib.rs` — UPDATE (designer, then implementer): tiles show live values; the static mockups show one moment; a still sample stands for the Music tiles without sound; the design's "tiles redraw only when…" and "the preview is the only thing that moves" rules change to the new ones.
- `crates/odc-app/src/live.rs` — CREATE — the pure part: given the readings, the unit, the music settings, the levels and a tick, every mode's frame, with a `Marquee` for each tile whose picture is wider than the screen or must scroll, an `EmojiLoop` for modes 14–17, a `Dancer` stepped by the levels, and one `Spectrum` shared by the Music tiles.
- `crates/odc-app/src/ui/tiles.rs:28-76` (`sample_values`, `picture`), `ui/display_page.rs`, `client.rs`, `model.rs` — UPDATE — the tile takes its frame from the pure part; the sample values stay as the picture before the first readings, and while the service is down.
- `crates/odc-ui-e2e/src/{scenarios,fake}.rs` — UPDATE — a `tiles_live` scenario and a `tile_cost` check.

**Implementation**
- One tick of 100 ms steps every tile's state machine together, so all tiles move in real time and in step, with no pause control (D5). The tick runs only while the window is on screen (D5). It comes from the window's frame clock (`add_tick_callback`, counted to 100 ms), which Task 1 measured stopping by itself when the compositor draws nothing, and it is stopped explicitly when the window is unmapped, hidden or minimised, because Task 1 also measured it still ticking for a hidden window. Whether it stops for a window on another workspace with the screen on is unverified: the sign-off (Task 53) repeats that check. A tile queues a redraw only when its frame differs from the last.
- Value tiles draw with the live `Readings` in the current unit. A reading the service has no value for shows dashes, as the cooler does (`display.md`, Unavailable values), with the tile's existing "Unavailable" and "Reading lost" flags.
- **Music and Dancer tiles** (D4): while `AudioConnected` is true and a Music mode is shown, the shared `Spectrum` is updated with each `Levels` and the Dancer stepped by it; otherwise the tiles show the still sample, as today. The emoji tiles play their loops always. Nothing asks the service for sound.
- The app's idle cost is measured in Task 1 and checked here: with the window shown and nothing changing, the tick is the only work.

**Tests**
- Pure: for every mode, unit, peak setting and readings, a tile's home frame equals `odc_render::picture`'s; the matrix is complete, not sampled (rule 5), each metric with its own distinct non-zero value; scrolling tiles scroll in step; emoji loops start together; a reading that changes keeps a scrolling tile's position; without sound the Music tiles equal today's sample; with levels the Dancer follows `Dancer::tick`.
- Widgets: no redraw without a changed frame; the timer stops when unmapped and restarts at home when shown; a dash tile.
- AT-SPI: against the fake service, a changing reading changes its tile and no other; against the dev service, the preview and its own tile agree.
- `tile_cost`: the app's CPU over ten seconds with the window shown, under a ceiling about sixty times what is measured here, so slower PCs pass (owner, 2026-11-17).

**Validation**
- `cargo test -p odc-app`, `… --features weston-tests`, `cargo test -p odc-mockups` — pass.
- `cargo run -p odc-e2e -- --only app --require weston`, then `--require dev-service,weston` — pass.

### 15. The designer reviews the whole design

**Files and integration points**
- `docs/design/settings-app.md`, `docs/design/mockups/`, `docs/design/mockups-src/` — UPDATE — whatever the review finds.

**Implementation**
- The `ui-ux-designer` agent reads the current design and the mockups as a whole and checks them against modern, minimal and clean: hierarchy, spacing, alignment, density, contrast, the empty and error states, keyboard and accessible names, and consistency between the Display page and the Configuration page. Anthropic's Frontend Design guidelines are the reference, with the UI/UX standards, for OpenDeepCool's own theme (Task 11) and for this review: hierarchy, one memorable element and restraint elsewhere, plain sentence-case copy, a visible quality floor. Where the brief fixes something (the system font, no motion in the chrome), the brief wins, as the guidelines themselves say a fixed direction does. The agent can't run the app, so the mockups and the design are its surface; the owner's review of the build (Task 33) is the visual check of the real window.
- Each change the review finds is applied by the designer in the design and mockups, and by the implementer in the code, each its own commit if it changes code.

**Tests**
- `cargo test -p odc-mockups` — the palettes still meet the design's contrast table; `--only app` — still passes.

**Validation**
- The designer's written verdict, with each finding and what was done, is in the report.

### 16. Review all the app's code

**Files and integration points**
- `crates/odc-app/`, `crates/odc-ui-e2e/`, `crates/odc-testkit/` (the fakes), `tools/mockups/`, `docs/design/`, `docs/spec/`, `README.md` — UPDATE — what the review finds.

**Implementation**
- Check, as the owner listed: **no leftovers** of the removals and changes (fans, pump, RPM, `sensors-detect`, the switcher, the unit toggles on the display page, the sample-only tile code, dead styles, dead test helpers, flags no test uses); **documents** that need an update (the app's and the project's); **comments** and function documents that say what the code no longer does (module headers such as the one at the top of `ui/tiles.rs` and `ui/window.rs` name the old behaviour); **clean code**: modularity, reuse, maintainability, performance, as the project's rules ask (`window.rs` is 980 lines; split it if the header's new code makes it worse).
- Run the repository's review (`/code-review`, or `/prp-core:prp-review` on the phase's diff) and a grep sweep of the patterns above.

**Tests**
- Whatever it finds gets its test; the sweep leaves `cargo test --workspace` green.

**Validation**
- `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check` — pass; the findings and their fixes are in the report.

### 17. The Start at boot password prompt says what it does (C11)

**Files and integration points**
- `packaging/polkit/io.github.dtmarangoni.OpenDeepCool1.policy` — UPDATE — the one action `start-at-boot` becomes two, `start-at-boot-on` and `start-at-boot-off`, both with `org.freedesktop.policykit.exec.path` `/usr/lib/opendeepcool/opendeepcool-boot` and `org.freedesktop.policykit.exec.argv1` `on` or `off` (pkexec(1), read 2026-11-19: "If the org.freedesktop.policykit.exec.argv1 annotation is present, the action will only be picked if the first argument to the program matches"), the same defaults as now (`auth_admin` for the active session, nothing else). Messages, the owner's words: "Automatically start the cooler service at boot." and "Disable automatic cooler service start at boot."; the descriptions say the same shortly.
- `crates/odc-service/tests/packaging.rs`, `crates/odc-pkg-test/`, `packaging/FILES.md`, `README.md` ("Installed on your system"), the PRD's System Footprint — UPDATE — wherever the one action's id is named or counted.

**Tests**
- The policy parses to two actions for the program, with `on` and `off`, the same defaults, the two messages and a final dot, and no third; the lists of the README, FILES.md and the PRD name both ids. At the end (Task 53), on the installed package: `pkaction --action-id … --verbose` shows both.

### 18. No note for a closed password dialog (C12)

**Files and integration points**
- `crates/odc-app/src/boot.rs` (`Failure::Dismissed`), `boot/tests.rs`, `crates/odc-app/tests/window.rs`, `crates/odc-ui-e2e/src/scenarios.rs`, `docs/design/settings-app.md`, the mockups if one shows it — UPDATE.

**Implementation**
- A closed dialog changed nothing, and the switch shows the saved state: no note, no note row. The other refusals and failures keep their notes.

**Tests**
- A dismissed dialog leaves the card exactly as it was, with no note; every other failure still says what happened.

### 19. No terminal commands in the app (C13)

**Files and integration points**
- `crates/odc-app/src/boot.rs` (`Commands`, `commands()`, `FROM_A_TERMINAL`), `ui/startup.rs`, `ui/window.rs` (the off screen), their tests, `crates/odc-ui-e2e/src/scenarios.rs`, `docs/design/settings-app.md`, `docs/spec/settings.md`, `docs/design/mockups-src/*` and `tools/mockups/src/lib.rs` (the pages that show commands and their Copy button), the regenerated mockups — UPDATE.

**Implementation**
- The terminal commands go from the first start (the off screen) and from the Startup card, with their lead-in and their Copy button. They stay in the README, which is the manual: "Installing turns nothing on …" lists the commands to turn the service on and off.

**Tests**
- No text of the commands, no lead-in and no Copy button in any screen; the switch's flows are as before.

### 20. Start at boot without "For the whole PC" (C14)

- `boot.rs` (`SUBTITLE`) and its use in `ui/startup.rs`, the tests, the design and mockups — UPDATE: the label under the switch's title goes. Test: no subtitle row, and the title and switch still sit as the design says.

### 21. Startup without "When it's on, the display service starts …" (C15)

- `boot.rs` (`HINT`) and its use, the tests, `scenarios.rs`, the design and mockups — UPDATE: the whole hint goes. Test: the card holds the title and the switch, and the notes the flows need.

### 22. Music without "For every Music option, except Dancer …" (C16)

- `ui/config_page.rs`, the design and mockups — UPDATE: the closing sentence of the Music card goes. Test: the card ends with its last control.

### 23. Display without "Turns the picture clockwise" (C17)

- `ui/config_page.rs`, the design and mockups — UPDATE: the Rotation row's hint goes; the row is its label and its control. Test: the row has no hint, and the control keeps its accessible name.

### 24. Display without "Used wherever the display shows a temperature" (C18)

- `ui/config_page.rs`, the design and mockups — UPDATE: the Temperature unit row's hint goes. Test: the same.

### 25. "Using the Omarchy theme." under "Use system theme" (C19)

- `ui/config_page.rs` (the Theme card), `model.rs` if the line's place is in the view model, `tests/theme_choice.rs`, `scenarios.rs`, the design and mockups (`lib.rs`) — UPDATE: the line that says which theme is used sits directly under the "Use system theme" row, before "Dark or light", not at the card's foot. Test: the line is under that row in every state of the Theme card.

### 26. Smaller switches (C20)

- `crates/odc-app/src/app.css`, `docs/design/settings-app.md` ("The switches"), `docs/design/mockups-src/app.css`, `crates/odc-app/tests/config_page.rs` (the size the test pins) — UPDATE: the switches are smaller than the 40 × 22 of Task 7; the designer's sizes in the design come first, with the knob, the border and the contrast kept as they are. Test: the measured size in the realised widget.

### 27. Smaller slider knobs (C21)

- `app.css`, the design, the mockups' stylesheet — UPDATE: the knob of the Bar fall and Peak fall sliders is a little smaller; the trough and the marks stay. Test: the knob's measured size.

### 28. A smaller gap between the cards (C22)

- `ui/config_page.rs`, `app.css`, the design and the mockups' stylesheet — UPDATE: the vertical gap between the Configuration page's cards is smaller than 18 px; the gaps to the window, the page bar and the preview are as before. Test: the measured gap.

### 29. The "Refreshed" notice in the right panel (C23)

- `ui/window.rs` (the feedback box and its overlay), `crates/odc-app/tests/window.rs`, the design and mockups — UPDATE: the notice is centred over the right panel, not over the window. Test: its centre is the right panel's centre.

### 30. More space under the picture's caption (C24)

- `ui/window.rs` (the preview column), the design and mockups — UPDATE: more space between the caption ("Temperature · CPU + GPU") and the first status item. Test: the measured gap.

### 31. The left panel's caption and status items centred (C25)

- `ui/window.rs`, `app.css`, the design and mockups — UPDATE: the caption and the status items are centred in the left panel. Test: their centres are the panel's centre.

### 32. More space between a page's title and its content (C26)

- `ui/page.rs` (the page bar), `ui/display_page.rs`, `ui/config_page.rs`, `app.css`, the design and mockups — UPDATE: more space between "Display" and its content and between "Configuration" and its content. Test: the measured gap on both pages.

### 33. Start at boot without a "Turning on…" or "Turning off…" line (C27)

- `crates/odc-app/src/boot.rs` (`TURNING_ON`, `TURNING_ON_SOUND`, `TURNING_OFF` and their use in `switch()`), `boot/tests.rs`, `crates/odc-app/tests/window.rs`, `crates/odc-ui-e2e/src/scenarios.rs`, `docs/design/settings-app.md` (the states table, the mockup table, the accessibility rows), `docs/design/mockups-src/app.js`, `tools/mockups/src/lib.rs` and the regenerated mockups (`boot-changing`) — UPDATE: the three lines go, the one that names the password dialog included. While a flip runs the card shows **no note at all**: the handle has moved, the trough keeps systemd's state and the switch is insensitive, which is what the card says of a running flip; a failure, "Not fully on" and the other notes keep their rules and show after the flip ends, as before. The screen reader's "Start at boot is on." and "…is off." at a flip's end stay. Test: in every state a flip can start from, in both directions and for the sound-only flip, the card has no note while the flip runs and the same notes as before when it ends.

### 34. The switches top-aligned with their labels (C28)

- `crates/odc-app/src/ui/config_page.rs` (`row()`: the `centred` argument), `ui/startup.rs` (the switch's `valign`), `tests/config_page.rs`, `tests/theme_choice.rs` if it measures a row, `docs/design/settings-app.md` (Rows), `docs/design/mockups-src/app.css` and `tools/mockups/src/lib.rs` — UPDATE: in every row the control's top edge is the label block's top edge, as it already is for the dropdown and the scales. Today the three switches (Use system theme, Peak dots, Start at boot) are centred on their block, so Peak dots, whose label has a hint under it, sits half a hint lower than its title. The toggle-button groups (Dark or light, Rotation, Temperature unit) are as tall as their one-line labels, so they don't move; the rule is one for all rows (the argument goes). Test: the measured top of each switch equals the top of its label block, with Peak dots' hint present.

### 35. The sliders' knobs in the colour of the switches' knobs (C29)

- `crates/odc-app/src/app.css`, `docs/design/settings-app.md` (D21, "The scales' knobs", "The switches"), `docs/design/mockups-src/app.css` — UPDATE: the knob of the Bar fall and Peak fall scales has the colour of a switch's knob. A switch's knob is `--bg` on an `--accent` trough when the switch is on and `--muted` when insensitive (`app.css`); the scale's was `--fg`. **Choice taken without asking:** the on switch's `--bg`, since the three switches are on in the owner's session and the slider's bar is `--accent` to the knob's left, like the switch's trough; the knob's size and the bar stay. A `--bg` disc against the card's `--surface` is about 1.2:1, below WCAG 2.2 SC 1.4.11's 3:1 for a control's parts, so the design checks the render first and keeps the knob visible with a ring only if it needs one (the Task's note in the report says which). Test: the knob's colour is the switch's, read from the realised widgets' styles, in the sensitive and insensitive states; the test of no colour literal outside `:root` still passes.

### 36. The Display title centred in the right panel (C30)

- `crates/odc-app/src/ui/page.rs` (`page_bar`), `ui/display_page.rs`, `tests/display_page.rs`, `app.css`, `docs/design/settings-app.md` (Window, Page bar), `docs/design/mockups-src/app.css` and its script, the mockups — UPDATE: "Display" is centred over the right panel's whole width (a `GtkCenterBox` with the title in the middle, Refresh at the end), not left of the Refresh icon. Test: the title's centre is the panel's centre, within a pixel. The Configuration page keeps its old bar until Task 37, because the two share one function: Task 36 gives it a flag, Task 37 turns it on for Configuration and removes the flag.

### 37. The Configuration title centred in the right panel (C31)

- The same files for the Configuration page (`ui/config_page.rs` or `ui/window.rs`, whichever builds its bar), `tests/config_page.rs` — UPDATE: "Configuration" is centred over the right panel's whole width, with the back icon at the start. Test: the title's centre is the panel's centre, with the back icon present. The flag of Task 36 goes.

### 38. More room between a page's title and its content, again (C32)

- `ui/page.rs` (the bar's bottom margin, now 8 px; the content's own 6 px stay), `app.css` if the number lives there, the design (decision D26's number and the height figures that hold it), the mockups' style sheet, `tests/display_page.rs`, `tests/config_page.rs` or wherever Task 32's measurement is — UPDATE: **24 px** from the title to the content on both pages, from 14. **Choice taken without asking:** 24 px is the step the status gap and the rows' spacing use. Test: the measured gap on both pages is 24 px.

### 39. A smaller gap between the Configuration cards, again (C33)

- `ui/config_page.rs` (the page box's spacing, 10 px), the design (D22's number and the figures that sum the gaps), the mockups' style sheet, the Task 28 check `the_cards_are_10_px_apart` (renamed and re-measured) — UPDATE: **6 px** between Theme, Display, Music and Startup, from 10; the page's margins, the gap to the page bar and the gap to the left panel are as they are. **Choice taken without asking.** Test: the measured gap between each pair of neighbouring cards is 6 px, and the margins are as before.

### 40. The "Refreshed" notice three times as far from the panel's foot (C34)

- `ui/window.rs` (the feedback label's bottom margin, 16 px), `tests/window.rs`, the design (D23's number and the toast's place), the mockups' style sheet — UPDATE: the notice stands **48 px** above the right panel's foot, from 16 (the owner: "3x more space"). Test: the measured distance from the label's foot to the panel's foot is 48 px.

### 41. The Refresh button's tooltip (C35)

- `ui/display_page.rs` (the tooltip text), `tests/display_page.rs`, the design (the Refresh row and the accessible-names table), `tools/mockups/src/lib.rs` or the mockups' script if the tooltip is drawn — UPDATE: the tooltip reads **"Refresh the display readings"** (the owner's words, no full stop); the accessible name "Refresh display" is as it was. Test: the realised button's tooltip is exactly that text.

### 42. More room between the picture's caption and the status items, again (C36)

- `ui/window.rs` (`STATUS_GAP`, now 14 px of margin above the status box), `tests/window.rs` (`the_status_items_stand_24_px_under_the_caption`), the design (D24's number and the column's height figures), the mockups' style sheet — UPDATE: the first status item stands **36 px** under the caption, from 24. **Choice taken without asking:** the owner gave no number. Test: the measured gap is 36 px.

### 43. The "Refreshed" notice one more step above the panel's foot (C37)

- `ui/window.rs` (the feedback box's bottom margin, 48 px), `tests/window.rs`, the design (D34's number), the mockups' toast — UPDATE: **64 px**, from 48 (the owner: "add one more space"; the 3 × 16 of Task 40 plus one more 16). Test: the measured distance from the label's foot to the panel's foot is 64 px.

### 44. OpenDeepCool's own Dark and Light themes, redesigned (C38)

- Designer first (`ui-ux-designer` agent, then the implementer): `docs/design/settings-app.md` (OpenDeepCool's own theme, the Default theme palette tables, the contrast table, The look of the switches and scales), `docs/design/mockups-src/app.css` (the palettes), `tools/mockups/src/lib.rs`; then `crates/odc-app/src/theme.rs` (`DARK` and `LIGHT` palettes, `SELECTION_ALPHA`, the tests that pin the contrast figures) — UPDATE. The owner: "Its ugly right now, just white and black. You can do much better. Use the Anthropic Frontend Design to create a much better Dark and Light themes." The designer works from Anthropic's Frontend Design guidance (a distinctive, cohesive identity with a dominant colour and sharp accents, atmosphere from tone and depth, no generic default palette), keeps the app's rules (the system font, no new package, every colour a token, no opacity and no motion in the chrome, the cooler's lit dot colour), and keeps the contrast the design states (4.5:1 text, 3:1 controls and the knob's ring), with every figure recomputed. Dark and light are one identity in two lights, not two designs.
- **Test:** the test that computes the design's contrast table passes with the new palettes (`the_default_theme_has_the_contrast_the_design_states`), the mockups test passes, and a rendered picture of the Configuration page in each theme is read for the report.

### 45. With the system theme, the window follows Omarchy's window opacity (C39)

- `crates/odc-app/src/` (a new pure module for the window's style: the mode, its alpha and blur; the code that reads the toggle files and watches them as the theme is watched), `src/theme.rs` and `src/app.css` (a window and card background with alpha), `docs/design/settings-app.md` (Window background: it was opaque and said "the `shell.toml` alphas apply to controls only"), `docs/spec/settings.md`, `tests/` — UPDATE. **What Omarchy does** (read in `/usr/share/omarchy` on 2026-11-22): Appearance > Windows > Window Opacity is `opaque`, `transparent` (the default) or `glassy`; `omarchy-hyprland-window-transparency` saves it as the toggle file `~/.local/state/omarchy/toggles/hypr/window-opaque.lua` or `window-glassy.lua`, none for transparent; glassy blurs what is behind a see-through window (Hyprland's blur) and gives terminals a background alpha of 0.65; a GTK app gets the compositor's blur only where its CSS has `backdrop-filter: blur(...)` (Omarchy's `default/gtk/glass.css` has `window { backdrop-filter: blur(20px); }`); Omarchy's own see-through windows have the opacity 0.985 (active) and 0.96 (inactive). **The app follows it by drawing its window background, and the cards on it, with an alpha** (it can't be a Hyprland window rule: the plan does not ship one, rule 14): opaque 1, transparent and glassy with the designer's numbers, glassy with the blur request. The mode is read from the two toggle files, never by running Omarchy's programs, and follows a change live as the Omarchy theme does. With no Omarchy theme the window follows nothing and uses its own theme's setting (Task 51). The alpha applies to the window's and the cards' background only, never to text or the cooler's picture.
- **Test:** the pure module's table (every mode, both files present, one present, none; a file that is a directory or unreadable counts as absent); a rendered picture of the window in each mode, sampled: the window's pixel alpha is the mode's and the preview's lit dot is opaque; a change of the toggle files while the app runs changes the window.
- **To verify, not assume:** that GTK's `backdrop-filter` is accepted by this GTK (no CSS parse problem) and that the app's window can be see-through at all under weston; the report says what was measured.

### 46. "Using the Omarchy theme." as close to "Use system theme" as a hint is to its title (C40)

- `ui/config_page.rs` (the Theme card: the status group under the "Use system theme" row), `tests/config_page.rs` (`the_theme_line_sits_under_use_system_theme_in_every_state`), the design (D-decision of Task 25 and the Theme card drawing), the mockups' script and style sheet — UPDATE: the line under "Use system theme" is **2 px** below the row's title, the gap a hint has under its title in a label block ("Peak dots" and "A dot above each bar…"), not the card's 14 px. It stays one line in every state, so the card keeps its height. Test: the measured gap between the title's foot and the line's top is the Peak dots hint's gap (2 px), in every state of the card.

### 47. "Appearance" instead of "Dark or light" (C41)

- `ui/config_page.rs` (the row's title and the buttons' group name), `crates/odc-ui-e2e/src/scenarios.rs` and the tests that find the row by name, the design (the Theme card, the accessible-names table), the mockups' script, `docs/spec/settings.md` — UPDATE: the row is titled **"Appearance"**; its buttons stay "Dark" and "Light" and their accessible names. Test: the row's title is "Appearance" and "Dark or light" appears nowhere on the page.

### 48. The same room under the last card as above the first (C42)

- `ui/config_page.rs` (the page box's bottom margin, now 18 px), `tests/window.rs` or `config_page.rs`, the design (the Configuration page's height figures and the margins text), the mockups' style sheet — UPDATE: the space under the Startup card is the space from the panel's top to the first card: the page bar's 12 above, 32, 18 below and the content's 6, **68 px**. Test: the measured distance from the Startup card's foot to the page's foot equals the distance from the page's top to the Theme card's top.

### 49. Window Opacity and Window Corners are settings of the service (C43)

- Spec first: `docs/spec/dbus.md` (state keys `WindowOpacity` and `WindowCorners`, `s` each, and the methods `SetWindowOpacity(s)` and `SetWindowCorners(s)` under `change-settings`), `docs/spec/settings.md` (the keys, the defaults `opaque` and `rounded`, the accepted words), the entry-points table of `docs/prp/solution-reviews/project-overview.review.md` (what each refuses). Code, as Task 10 did for the theme choice: `crates/odc-ipc` (`state.rs`, `lib.rs`, the proxy, `STATE_KEYS` 18 to 20), `crates/odc-service` (`settings.rs`: two enums with serde defaults, an unknown word makes the file invalid as every wrong value does; `dbus.rs`: the two setters, refused with the same polkit rule as every setter, an unknown word refused with `InvalidArgs`), `crates/odc-testkit/src/fake_service.rs`, `crates/odc-ui-e2e/src/fake.rs`, `crates/odc-e2e/src/service.rs` (the scenarios) — UPDATE. The words are Omarchy's (D11). Test: the contract round trip, the settings file (a file from before them loads with the defaults and writes them at its next save; a wrong word is invalid and moved aside), the D-Bus setters (accepted, refused for a caller polkit refuses, refused for an unknown word, a repeated value changes nothing and tells nobody), the watchers' `StateChanged`.

### 50. The Default Theme subcategory of the Theme card (C44)

- Designer first: `docs/design/settings-app.md` (Theme card, Rows, accessible names), the mockups; then `ui/config_page.rs`, `src/choice.rs` (what enables the group), `src/model.rs` (the view model's state for the card), `ui/window.rs` if it reads the card, `crates/odc-app/tests/`, `crates/odc-ui-e2e/src/scenarios.rs`, `tools/mockups/src/lib.rs` — UPDATE. The Theme card holds "Use system theme" and, as a subcategory of it, **"Default Theme"**: a heading label, and under it the rows **Appearance** (the Dark and Light buttons), **Window Opacity** (Opaque, Transparent, Glassy) and **Window Corners** (Sharp, Rounded). The heading and the three rows are disabled while the system theme is used (the rule that disables Dark and Light today: the window follows the system's theme), and each control asks for its call and shows the service's saved value, as every setting does. "Default Theme" is a child of the Theme card as "Use system theme" is. The card keeps its height in every state. Test: the controls' names, their calls with the right argument, the group disabled (heading included) while the system theme is followed and enabled when it isn't, and the saved value shown until the service confirms.

### 51. OpenDeepCool's own theme applies Window Opacity and Window Corners (C45)

- `src/theme.rs`, `src/app.css`, `ui/window.rs` (the one style provider and the window's class), the window-style module of Task 45, `docs/design/settings-app.md`, `tests/theme.rs`, `tests/theme_choice.rs` — UPDATE: with the own theme, the window's alpha and blur are the service's `WindowOpacity` (the same alpha numbers as Omarchy's modes in Task 45), and the corners are the service's `WindowCorners`: **sharp** gives the window, the cards and the controls radius 0 and **rounded** the designer's radii (today 4 px for a control and 10 px for what holds controls; the window's own corners 12 px with GTK's client-side decoration). Following the system theme, neither setting is applied. Test: sampled pixels of the window in each opacity and the radii of each corner choice; the settings change the window live; none applies while the system theme is followed.

### 52. Twenty original emoji animations for the owner to choose from (C46)

- `docs/design/emoji-options/index.html` (CREATE: one self-contained page, its own script and style, no network) — the owner: "lets redraw emoji pictures to have our own instead of copying DeepCreative ... create 20 variations, create a html page to me see all of them, I will choose the ones I want". The Music emoji modes 14 to 17 draw a picture in rows 1–7 of the 14 × 14 display and the spectrum below ([music.md](../../spec/music.md), Emoji animations); each animation is a loop on the 100 ms tick. The page shows **twenty** new animations on that grid (14 columns, 7 rows) with a spectrum moving under each, each with a number and a short name, a way to see one large, and the frame count and loop length, so the owner can pick four (or more). The designer draws them from pixel-art animation guidelines in trusted sources (frame economy, anticipation and follow-through, readable at 14 × 7, one idea each) and from Anthropic's Frontend Design guidance for the page. Nothing on the cooler changes until the owner chooses: the spec's four DeepCreative pictures stay until then. A check that the page's frames are 14 × 7 and every loop is whole 100 ms ticks is part of the page's own script, and the report names the file.

### 53. A new build, the owner's review, and the sign-off

**Files and integration points**
- `docs/prp/reports/phase-11a-settings-app-ui-review.report.md` — CREATE.
- `docs/prp/prds/opendeepcool-lp360.prd.md` (the phase row), `README.md` (Status), `AGENTS.md` (counts and commands that changed: the mockups, the crates, the `--require` names), `docs/design/settings-app.md` — UPDATE.
- `.github/workflows/` — UPDATE only if CI's pinned actions have newer releases (sign-off rule).

**Implementation**
- With the owner's OK for each system step: build the release package from the committed HEAD (`cargo run -p odc-pkg-test -- --repo . --work target/pkg-work --export <dir>`), remove the dev copy (`cargo run -p odc-dev -- teardown`), install the package (`pacman -U`), set the test access again (`odc-dev setup --test-access`), and run the full parity run on the installed package, as Phase 11's Task 9 did.
- The owner reviews the build and the mockups and says what is left. Anything they list goes into this plan as a new task or a note first.
- At sign-off: push the branch, wait for CI, fast-forward `main` to it and push; the PRD row becomes `complete`, with `-` in the PR column (AGENTS.md, How to work).

**Tests**
- Nothing new: the validation gates below are the proof.

**Validation**
- Every gate in the table below passes on the installed package; the report lists each with its result.

## Acceptance

1. **AC1 — Only the display:** the grep in Task 6 finds nothing outside the allowed places; the service, the contract, the sensors, the package and the tests have no fan, pump or sensor-detection code; the PRD's Decisions Log has the 2026-11-18 row.
2. **AC2 — The package:** it installs and uninstalls leaving the system exactly as it was, both ways, with no detection program, units, dependency or record; the footprint adds nothing, since the theme choice lives in the service's settings file.
3. **AC3 — The app's structure:** title centred; gear and close icons without frames at the right; a back icon on the Configuration page; no Display tab; no "Shown on the cooler now" line; Refresh an icon at the top right of the display page; the unit after Rotation on the Configuration page; no second sentence in Start at boot's hint; labels beside their controls; smaller switches. Every scenario of the AT-SPI run passes.
4. **AC4 — The window:** it opens at the size of its content, scrolls where the screen is smaller, and its floating behaviour is as Task 1 proved and the report records.
5. **AC5 — The theme card:** first on the Configuration page; "use system theme" follows an Omarchy theme or the system's preference live and disables the dark/light toggle; with no system theme found it is disabled with a warning; the toggle off lets dark or light be chosen; the choice is the service's setting, kept for the whole PC, so it survives a restart of the app and of the service, and the app writes no file.
6. **AC6 — The cooler:** the frames of modes 4, 5, 6, 9 and 12 follow the new rule on the real LP360 (one example each) and in every in-process case; every other mode's frames, the Dancer's included, are byte for byte as before; the Music spectrum stays at the bottom.
7. **AC7 — Live tiles:** every tile shows its mode with the live readings in the current unit and all move together; the Music and Dancer tiles follow the sound only while the cooler is in a Music mode, and are still samples otherwise; sound is captured only while a Music mode is shown; the tiles move in real time, with no pause control (owner, 2026-11-18), and only while the window is on screen, not minimised and not on another workspace; the app's cost stays under its ceiling.
8. **AC8 — The contract:** `Readings` and `Levels` go only to frame watchers that pass the one-second check; each refusal in Task 13 is tested, and the two theme setters are refused for a caller polkit refuses, as every setting's are.
9. **AC9 — The documents agree:** the design, the mockups, the spec, the PRD, `AGENTS.md`, the README and the designer's brief say what is built, with no fan or pump text and no "decided, not yet built" note left.
10. **AC10 — The owner's review:** the owner reviews the new build and signs the phase off.

## Validation

| Gate | Command or procedure | Proves |
|---|---|---|
| Build, tests, lint, format | `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check` | AC2, AC8, AC9; run before every commit (the guard runs them) |
| The app's display tests | `cargo test -p odc-app --features weston-tests` | AC3–AC5, AC7 in-process |
| Expected frames | `cargo run -p odc-oracle -- docs/spec/frames` (no diff after the commit) and `cargo test -p odc-oracle` | AC6, and that no other mode changed |
| Mockups | `cargo run -p odc-mockups -- docs/design/mockups` and `cargo test -p odc-mockups` | AC9 |
| Layer 2, local | `cargo test -p odc-render --test differential_load --test differential_temperature --test differential_power --test differential_music -- --ignored --nocapture` | AC6: every difference from DeepCreative is a listed one |
| Real cooler | `cargo run -p odc-e2e -- --only display --require lp360` (and `--modes` for a task's modes) | AC6 |
| Sensors | `cargo run -p odc-e2e -- --only sensors --require <the PC's list, minus board-fans>` | AC1: the remaining metrics are untouched |
| The service | `cargo run -p odc-e2e -- --only service --require dev-service` | AC6, AC8 |
| Sound | `cargo run -p odc-e2e -- --only music --require dev-service,pipewire` | AC7: sound capture is as before |
| The app, headless | `cargo run -p odc-e2e -- --only app --require weston`, and with `dev-service,weston` | AC3–AC5, AC7 |
| The package | `cargo run -p odc-e2e -- --only package --require container` | AC2 |
| The full parity run | `cargo run -p odc-e2e -- --dev-interface --require <the PC's list, minus board-fans>` on the installed package | Every capability still passes after the phase |
| Baseline | `cargo run -p odc-e2e -- --dev-interface --only baseline --require dev-service` | The service's cost is not worse than Phase 11's |
| Leftovers | The grep of Task 6 and Task 16's sweep | AC1, AC9 |
| The owner's review | The owner runs the new build | AC10 |

## Risks and Decisions

| Decision or risk | Recommendation | Evidence / mitigation | Consequence if different |
|---|---|---|---|
| **Who does what.** The draft makes the `ui-ux-designer` agent do every task | The designer owns the design, mockups and design reviews; the implementer owns code, tests and commits | The agent's definition says it writes no Rust and has no shell (`.claude/agents/ui-ux-designer.md`); AGENTS.md rule 4 needs builds and tests before each commit | Giving the agent a shell would change its definition and let a designer commit code |
| **A floating window** can't be ordered from GTK | Try the window's own hints (Task 1), record what the owner's session does, ship no compositor rule | Rule 14; Hyprland's handling of a fixed-size window is **unverified** | If nothing floats it, the owner decides between a rule in their own config (a system change) and a tiled window of its content's size |
| **Where the theme choice lives** (decided, owner, 2026-11-18) | The service keeps "use system theme" and "dark theme" in its settings file, with two setters under `change-settings`; the app writes no file | "No manual interaction": uninstall already removes the service's state; the Music settings are the precedent for a new key | A choice for the whole PC: one user's change shows in every session of that PC |
| **Which interface reports the system's preference, and what "no preference" shows** (left to this review by `settings.md:110`) | Read `GtkSettings:gtk-interface-color-scheme`; `DEFAULT` and `UNSUPPORTED` show our dark default theme | The app already enables GTK 4.20; Task 1 confirms against docs.gtk.org and a program | The portal read directly: one more bus dependency in the app |
| **Rule 8's ban on writing `pwm*`** | Keep one sentence as a safety rule | The ban protects hardware and costs nothing; the draft asks for "everything related to fans" gone | Removing it leaves no written rule against a fan write |
| **One commit for five modes** (rule 4) | One commit for the centring rule | It is one implementation: one rule in two places, and five modes' frames follow; per-mode commits would each leave the other four inconsistent with the spec | Five commits, each with the rule applied to one mode: contrived, and the spec would be wrong in between |
| **The detection's leftovers on the development PC** | The owner removes the old package (`pacman -R`), which undoes the detection from its record | The package test proves that undo; no release has the detection | Migration code in the new package: code for one installation |
| **The development PC's package** | Remove it for the dev-copy runs (Tasks 4–14) and install the new one at the end (Task 53), each step with the owner's OK (approved to ask, 2026-11-18; each still asks for the password) | AGENTS.md: `odc-dev setup` refuses while the package is installed; `--dev-interface` drives the packaged service, which wouldn't have the new code | Running the new service tests needs a new package per task: ten minutes each |
| **`docs/local/machine.md`'s `--require` list** names `board-fans` | The owner edits it by hand | It is local and not committed (rule 10) | `odc-e2e --require board-fans` would fail as an unknown capability |
| **Phase numbering** | 11a | Sixty references to Phase 12 in history stay valid | Renumbering touches every completed phase's record |

## Related Plans

- **Depends on:** `docs/prp/plans/phase-10-settings-app.plan.md` (the app), `docs/prp/plans/phase-11-packaging-final-validation.plan.md` (the package and its test).
- **Followed by:** Phase 12, the releases, whose plan is written after this phase signs off.

## Agent Notes

- The owner's draft review is deliberately not in the repository; its items are the tasks above, with R1–R4 the removals, C1–C10 the changes and A1 the addition. D1–D4 are the owner's four answers of 2026-11-18, recorded in the PRD's Decisions Log and in the spec notes, each marked "decided, not yet built" until its task lands.
- Chat sessions that continue this plan should re-read Task N before starting it, and tell the owner which task they are on (AGENTS.md, Start of every session).
