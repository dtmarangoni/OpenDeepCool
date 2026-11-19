# Settings app design

The design of OpenDeepCool's GTK4 settings app (PRD, Phase 10): its structure, each screen's purpose, its GTK4 widgets, its states, its theme, keyboard use and accessible names, and the coverage check. The mockups are in [mockups/](mockups/); each one has a theme switch (a sample Omarchy theme and the default theme, dark and light). `odc-mockups` (`tools/mockups/`) writes them from the states they show and the style sheet and script in [mockups-src/](mockups-src/), and its tests check the palettes against the contrast tables below.

Sources: [settings.md](../spec/settings.md) "What the settings app must offer", [display.md](../spec/display.md), [music.md](../spec/music.md), [sensors.md](../spec/sensors.md), the PRD (Phase 10, Technical Approach → Theme, Decisions Log), the Phase 11 plan (Recommendation, "The Start at boot switch", and Task 3) and the designer brief (`.claude/agents/ui-ux-designer.md`). Every reading, chip name, GPU name and PCI address in this document and in the mockups is made up.

Widget names are GTK 4 classes. Anything not checked against docs.gtk.org is marked **unverified**; the Phase 10 plan checks those first.

## Goals

- **Feature parity, not interface parity.** Every LP360 feature is reachable. There are no DeepCreative tabs and no Combined/Separate split.
- **Few screens, few clicks.** There is one window with two pages, and every mode is one click away.
- **Fast.** It opens straight onto the current settings, with no animation in the app's own chrome. Idle CPU use is nil: the preview redraws only when the cooler's picture changes, and nothing polls.
- **Fits its content.** The window opens at the size of its content and floats where the compositor allows. On a screen too small for it, it has scrollbars instead of cutting anything off ([Size and scrolling](#size-and-scrolling)).
- **Only the cooler.** CPU, GPU and RAM values appear only inside display pictures.
- **Themed.** The app follows the current Omarchy theme live and uses its own default theme elsewhere. It never sets window opacity or blur.

## Decisions

The owner delegated the design choices (2026-10-25). These are final for v1.

| # | Question | Choice | Why |
|---|---|---|---|
| D1 | Window structure | One window: a persistent preview column on the left, and a `GtkStack` on the right with two pages, **Display** and **Configuration** | The preview stays in view while rotation, GPU or Music settings change. There is no second window to place |
| D2 | How the 23 modes are picked | A gallery of picture tiles in four groups, **Load**, **Temperature**, **Power** and **Music**. One tile is one mode | One click per mode, all of them visible, and each tile shows what the mode looks like. No Combined/Separate wording |
| D3 | Where the preview sits | The top of the left column, on both pages | Every setting's effect shows in place |
| D4 | Refresh and status | Refresh as an icon button at the right of the Display page's bar (D12; the header bar held it until Phase 11a); a status line of up to three items under the preview; a banner only for states that need attention | Status can be read at a glance, and the app only raises an alert when one is needed. Refresh sits with the page whose picture it syncs |
| D5 | How the configuration screen is reached | A gear toggle at the right of the header bar opens it, and a back icon at the start of its own page bar, Escape or Alt+Left closes it (D12). There is no page switcher and no Display tab | The Display page is home, so it needs no tab; one icon reaches Configuration, and the page says where you are |
| D6 | Apply model | **Instant apply.** Every choice goes to the service, and so to the cooler, when it's made. There is no Apply button and no unsaved state | Fewest clicks; the preview simply mirrors the cooler (settings.md, Changes apply at once) |
| D7 | Toolkit | Plain GTK4 through gtk4-rs, no libadwaita (PRD Decisions Log) | Every widget needed exists in GTK4, and libadwaita's look would mostly be overridden for Omarchy anyway |
| D8 | Theming | CSS custom properties in one `GtkCssProvider` at APPLICATION priority, generated from `colors.toml` and the `shell.toml` `[controls]` alphas. A `GFileMonitor` on `current/` triggers a live reload: Omarchy keeps `current/` and replaces only `current/theme` inside it (its `omarchy-theme-set`, read 2026-10-27; solution review P10-06) | [Theme](#theme) |
| D9 | Where the Start at boot switch sits (2026-11-11) | The last card of the Configuration page, **Startup**, and, while the service isn't running, the window's body, where the pages would be | Configuration already holds every setting but mode and unit, and the switch is flipped about once per PC, so it needn't take the header. While the service is down, Configuration can't be reached, and the body is where the user looks ([Start at boot](#start-at-boot)) |
| D10 | The window while the service isn't running (2026-11-11, revised the same day) | One screen, the **off screen**, with no banner: the heading "OpenDeepCool is off" (the display service doesn't start at boot) or "OpenDeepCool isn't running" (it does, but it's stopped, failed or starting), the Startup card with its note, and the terminal commands. The red service banner is gone | The switch now reads off whenever OpenDeepCool isn't ready, and a flip on starts a stopped service, so the switch is the fix in every case. A banner would say the same thing as the note, and its terminal command would come before the app's own control. The heading and the status dot ("Service off", hollow, or "Service not running", red) still tell a deliberate off from a fault |
| D11 | What the switch reads (2026-11-11, revised the same day: "on means ready to use", owner) | **On** only when the display service starts at boot and runs, and the sound starts at every login and runs in this session. Anything else reads **off**, with **one note** built from what's missing: at most one phrase for the display service and one for the sound ([The note](#the-note)) | A flip on always finishes the setup and starts what isn't running, so off-with-a-note is always one flip from ready. One composed note needs 7 phrases, against 15 combinations to word one by one, and still says exactly what's missing |
| D12 | The header, the page change, Refresh and the temperature unit (Phase 11a Task 7, 2026-11-19, from the owner's review of 2026-11-18) | The header bar holds the centred title and, at the right, two frameless icon buttons: a gear toggle (Configuration) and a close icon. Each page has its own **page bar**: Display has the title "Display" and the Refresh icon at the right; Configuration has a back icon at the start and the title "Configuration". The two pages are two children of one `GtkStack` with no transition. The temperature unit is a row of the Configuration page's Display card, after Rotation | The page change is one icon and one icon back, with no tab to explain. Every control stays a single click or key. The options weighed are in [Answers decided on 2026-11-19](#answers-decided-on-2026-11-19) |
| D13 | The preview column's line under the caption (Phase 11a Task 8, 2026-11-19, from the owner's review of 2026-11-18) | Gone in every state. What it said lives in the cooler banner, the cooler's status item and the preview's accessible description ([Preview column](#preview-column)). The column is the picture, its caption, the lost reading's reason when there is one, and the status items | In the one state where nothing is wrong it said "Shown on the cooler now", and in the three failures it repeated the banner above the window and the status item right below it. Without the line the column has the same layout in every cooler state, so nothing under the caption jumps when the cooler comes and goes |
| D14 | The Configuration page's rows (Task 8) | A row is its **label block** (the title, with its hint under it) and its **control**, side by side and left-aligned: one horizontal `GtkBox` per row, one `GtkSizeGroup` over every label block of the page so the controls start in one column, a 24 px gap, and nothing that expands ([Rows](#rows)) | Labels beside their controls, with the space between them cut to the gap (owner, 2026-11-18). Sizing the label column from one group keeps the controls' left edges straight, which a row-by-row layout wouldn't |
| D15 | The switches' size (Task 8) | 40 × 22 px for the two `GtkSwitch`es, from GTK's 48 × 24 ([The switches](#the-switches)) | "A little smaller" is a sixth narrower and a twelfth lower: seen at once, still a switch. 34 × 18, which Task 1 measured, is 29 % narrower and 25 % lower |
| D16 | The window's size, its scrolling and its floating (Phase 11a Task 9, 2026-11-19, from the owner's review of 2026-11-18) | No default and no minimum size, and not resizable (`resizable` false): the window opens at the natural size of its content, the largest page's in each direction, and is never larger than the smallest monitor less 48 px. One outer `GtkScrolledWindow` carries that cap and scrolls the whole content sideways on a narrow screen; the preview column and each page scroll down on their own, under the header bar and the page bar ([Size and scrolling](#size-and-scrolling)) | The window is as small as its content on any monitor (owner, 2026-11-18), and on a small one nothing is cut off. The picture stays in view while a page scrolls (D1, D3), and the cap, a banner included, is set in one place. The user can't resize or maximise it: the extra room would be empty, and the window already scrolls where room is short |

Answers to the pass-1 questions, decided under the owner's delegation (2026-10-25):

- **Tiles:** fixed made-up sample values, drawn locally.
- **Remote or inactive session** (owner, 2026-10-26, replacing pass 1's view-only session): the window shows one message and nothing else ([Not available in this session](#not-available-in-this-session)). No control is ever shown disabled for lack of rights.
- **Rotation:** four direct choices.
- **Temperature unit:** once, in the Display card of the Configuration page, after Rotation (owner, 2026-11-18; it was at the top of the Display page until then).
- **Service not running:** the app explains, shows the start command, and has no start button. Since 2026-11-10, the [Start at boot](#start-at-boot) switch on the [off screen](#the-off-screen) turns OpenDeepCool on, and since 2026-11-11 it also starts a service that's set to start but stopped; it's the only control that changes the system (owner, 2026-11-11: no separate start button).
- **Window background:** opaque. The `shell.toml` alphas apply to controls only.
- **Default theme:** dark and light variants. Outside Omarchy in v1, the dark one is used.
- **Preview dots:** lit dots in the foreground colour, unlit dots in a low-alpha foreground.
- **Language and app settings:** English only, with no app-only settings.
- **GPU label:** `<kind> · <product name>`.
- **Preview source:** the preview mirrors the service's current frame.

Answers decided on 2026-11-06, under the same delegation, before Phase 10's code (Phase 10 plan, Task 1):

- **Automatic in the GPU dropdown** (owner, 2026-11-04): the entry and the button read "Automatic". While it's chosen, a muted line under the dropdown names the GPU it reads, from `GpuInUse`: "Using Dedicated · Contoso Graphics R870." ([Display](#display)).
- **A GPU added or removed:** the app says nothing about why the choice went back to Automatic. The line under the dropdown names the GPU in use, and the journal has the reason.
- **Cooler found, but the service can't open it** (`CoolerState` `no-access`): an error banner of its own with the fix in words (restart the PC, then reinstall OpenDeepCool). It has no command and no button, and the status says "Cooler: no access" ([Banners](#banners)).
- **Sending to the cooler fails** (`write-failed`): a warning banner, shown only once the state has lasted 2 s. Before that, the window looks as if the cooler were connected, so unplugging the cooler doesn't flash a second banner.
- **A GPU whose kind query failed** (`query-failed`): listed as "Unknown type · <name>", with the muted note "driver couldn't be asked" in the open list.

Answers decided on 2026-11-11, under the same delegation, for the Start at boot switch (Phase 11 plan, Task 3; [Start at boot](#start-at-boot)). Each choice, with the options weighed:

- **Where it sits (D9):**
  - (a) the header bar, next to the app name or Refresh: always in view, but it gives a once-per-PC, password-gated action the window's most visible place; next to the name it reads as a display on/off, a hidden setting that is out of scope; and at the 720 px minimum the header has no room for a labelled switch;
  - (b) the preview column, under the service status: always in view too, but it mixes a control into read-only status, and the 226 px column can't hold the terminal commands of a failure;
  - (c) **chosen:** a Startup card on the Configuration page, plus the window's body while the service is down. One switch is visible at a time, with its hint and its notes beside it.
- **The window while the service is down (D10, revised after the owner's "on means ready to use", 2026-11-11):**
  - (a) the red banner in every case, with the switch added to the body: a new install opens on an error;
  - (b) the first pass's split: an off screen while the display service doesn't start at boot, and the red banner while it does but isn't running. The switch can now start that service itself, so the banner would repeat the note and put its terminal command ahead of the app's own control;
  - (c) a banner only for a failed service: a third rule for one case that the note already names, with the journal;
  - (d) **chosen:** the off screen in every case, with no banner. Its heading tells "is off" from "isn't running", the status dot tells a fault (red) from a choice (hollow), the note names what's wrong, and the terminal commands, `enable --now` first, stay under the card.
- **What the switch reads (D11, revised the same day):** on only when all four facts hold (owner). For what's missing:
  - (a) one note per combination of the four facts: 15 texts to word and test;
  - (b) one fixed "Not fully on" note: it doesn't say what's missing, which the owner asked for;
  - (c) **chosen:** one note built from at most two phrases, one for the display service (4: runs but doesn't start at boot, stopped, failed, starting) and one for the sound (3: off, runs here but doesn't start at login, not running in this session), with one of two endings.
  - A partly-on setup can't be turned off in one flip: from off, a flip always turns it on (owner: a flip on finishes everything). Turning it off then takes a second flip, after it's on.
- **The display service starting:** (a) a sensitive switch: a flip asks for the password to start a service that's starting anyway; (b) **chosen:** insensitive, with "The display service is starting." It lasts until the service is ready or fails, then the switch follows. A failed start ends in `failed` or a restart, which the switch shows.
- **While a flip runs:** (a) a `GtkSpinner` beside the switch: it moves for as long as the password dialog stays open, against the no-motion rule; (b) **chosen:** GtkSwitch's delayed state (the handle at the new position, the trough at systemd's) and one line of text, with the switch insensitive.
- **Where a failure shows:** (a) a banner at the top: far from the switch on the Configuration page, and it would stack with the cooler banners; (b) the change-feedback label: it lasts 5 s, too short for copying commands; (c) **chosen:** a note under the switch, with the commands and Copy, kept until the next flip or until systemd shows the change made.
- **State unknown:** (a) the switch sensitive at off, a flip running `on`: safe, since `on` repeats nothing harmful, but the switch's position would say nothing true; (b) **chosen:** insensitive at off, with the reason and the commands.
- **When the state is read again:** (a) systemd's `UnitFilesChanged` signal after `Subscribe()`: live for the display service, but `Subscribe()` makes the system manager send its signals on the bus while the app runs, and a `--global` change (made on the files, `systemctl --global`) sends nothing at all; (b) **chosen:** at start, after each flip, whenever the window becomes active, and when the service's bus name gets or loses an owner. Someone who types the commands in a terminal comes back to the window, which turns it active. No polling.

### Answers decided on 2026-11-19

Decided under the same delegation, for the owner's review of 2026-11-18 (Phase 11a Task 7: no Display tab, the title in the middle, a gear and a close icon, a back icon, Refresh as an icon on its page, the unit on the Configuration page). Each choice, with the options weighed:

- **The gear while the Configuration page shows:**
  - (a) hidden: the header's shape changes between the two pages, and the only "you are here" signal is the page title;
  - (b) insensitive: a disabled control for the page you're on says nothing about why, and a state that is current reads as unavailable;
  - (c) enabled and doing nothing: a control that does nothing is worse than none;
  - (d) **chosen:** a toggle, pressed on the Configuration page, that goes back to the Display page when it's pressed again. The header is the same on both pages, the pressed state (a fill and the accent on the icon) is the "you are here" signal, and a gear that opens a panel and closes it again is the common idiom. Assistive technology reads "Configuration, toggle button, pressed".
- **Where the back icon sits:**
  - (a) the header bar's start slot, as `AdwHeaderBar` puts the back button of a navigation view: it would sit above the preview column, far from the page it leaves, and the owner asked for it "at the top of the configuration page";
  - (b) the end side, beside the gear: against GNOME's rule that back is at the top-left in left-to-right text, and KDE's guidelines name no other side;
  - (c) **chosen:** the start of the Configuration page's own page bar, left of the page title in left-to-right text, so GTK mirrors it by itself for right-to-left text. GNOME's HIG, Browsing: "In left-to-right locales, the standard position for the back button is the top-left" (developer.gnome.org/hig, read 2026-11-19), and its header bar page puts the main actions, back among them, "at the start of the header bar". KDE's HIG puts "back/forward buttons in the toolbar" of Kirigami's page stack (develop.kde.org/hig, Layout and navigation, read 2026-11-19) and states no side, so GNOME's rule decides.
- **Page bars or none:**
  - (a) none: Refresh floats over the first group's heading row and the back icon over the first card. They scroll away on a small screen (the window now scrolls where the monitor is small) and a long reason line under "Load" could run under the icon;
  - (b) **chosen:** one page bar on each page, outside the scrolled area, 32 px high. It costs about the height of the unit row it replaces on the Display page, and it gives both pages a title.
- **Refresh while the service isn't running:** it belongs to the Display page, which the off screen replaces, so it isn't drawn then. The gear stays visible but insensitive, as the page switcher was (Configuration can't be reached while the service is down, and Start at boot is on the off screen, D10). In a session that can't use OpenDeepCool only the title and the close icon show.
- **The temperature unit's control:** (a) a `GtkDropDown`: two choices behind a click; (b) a `GtkSwitch` "Fahrenheit": a switch is for on and off, and °C isn't "off"; (c) **chosen:** the two linked toggles that Rotation has. Both choices show, it's one click, the widgets and their names are the ones the Display page had, and "each control is one D-Bus call, shown as the service confirms it" stays as it was.
- **The shortcuts:** Ctrl+1 and Ctrl+2 went with the tabs. GNOME's standard list gives Preferences Ctrl+, and Back Alt+← (developer.gnome.org/hig, Keyboard, read 2026-11-19), and libadwaita's navigation view goes back with Escape and Alt+← (its `AdwNavigationView` page, read 2026-11-19), so the app binds Ctrl+, (open Configuration), and Escape and Alt+Left (back), the last two only while the Configuration page shows, so Escape is free everywhere else. F5 and Ctrl+R stay for Refresh. The mouse's back button isn't bound: nobody asked.
- **Where the "Refreshed" and refusal label sits:** it was at the top right of the content, which is where Refresh is now. It moves to the bottom centre of the content, as the mockups' toast always drew it, so it never hides a control.

### Answers decided on 2026-11-19, Task 8

Decided under the same delegation, for the owner's four items of 2026-11-18 that are about structure and sizes: the preview column's line, the Start at boot hint, the Configuration page's rows and the switches. Each choice, with the options weighed:

- **The preview column's "Not on the cooler: …" lines** (the owner removed "Shown on the cooler now"; the plan left these three to the design):
  - (a) keep them as one line that shows only while the cooler can't show the picture: the facts stay visible, but the line says again what the banner above the window and the status item right below it say, and it adds a row to the column that comes and goes with the cooler, so the status items under it jump when a cooler is plugged in;
  - (b) fold them into the status items by lengthening their words ("Cooler not connected: not shown"): the items get longer than the 226 px column's lines, and the information is still the banner's;
  - (c) **chosen:** the line goes in every state, and its sentence becomes the second sentence of the preview's accessible description. The banner has the full text and the fix, the status item the state in two or three words, and the picture's own description, which is what a screen reader reads when it reaches the picture, says that the picture isn't on the cooler. Nothing is lost, the column keeps one layout, and a sighted user is told twice, not three times.
  - The service-down line, "The service draws the picture", goes with it: the off screen says "The cooler shows nothing until the display service runs." in the window's body, and the column's caption says "No picture".
- **The Start at boot hint:** the second sentence, "Each change asks for the administrator's password.", is removed (owner, 2026-11-18). The password is said where it matters: the line "Turning on… Your desktop asks for the administrator's password first." shows the moment a flip starts, and the desktop's own dialog is the authority. The hint keeps its first sentence.
- **How a Configuration row is laid out:**
  - (a) keep the row as it is, label block expanding and the control at the right edge: the owner's complaint;
  - (b) a `GtkGrid` per card, label block in column 0 and control in column 1: right, but a hidden row (the GPU row on a PC with one GPU) must hide two cells, and whether the grid then drops that row's spacing is not in the documentation (**unverified**). The page would also need one grid per card, with three columns of controls that don't align between cards;
  - (c) **chosen:** one horizontal `GtkBox` per row, the label block and the control, with one `GtkSizeGroup` over the page's label blocks. A row stays one widget (the GPU row hides with one `set_visible`, as it does today), the controls of all the cards start at one x, and the existing `row()` helper keeps its shape.
  - **Where the hint goes**, with the title in the block:
    - (c1) **chosen:** under the title, in the label block, wrapping at 28 characters. The title and its explanation stay together, as the rows had them, and the block is about 28 characters wide, not the 45 of the longest hint;
    - (c2) under the control, in the control's column, as a form layout does: the label column would be only as wide as the longest title, the tightest layout, but the explanations move away from their titles, a switch gets a caption under it, and the GPU's static hint would join its two dynamic lines under the dropdown. If the owner still finds the gap wide after this review, (c2) is the next step, and a change to the same two helpers.
  - **Stacking in a narrow window** (KDE's Kirigami `FormLayout` puts a label above its control when there's no room): no. The window isn't resizable by the user (Task 9), so a page is narrow only on a small monitor, where the content scrolls sideways ([Rows](#rows), Narrow windows), and a `GtkBox` can't wrap a control under its label anyway.
- **The switches' size:**
  - (a) 34 × 18, the size Task 1 measured: 29 % narrower and 25 % lower than GTK's 48 × 24, which is more than "a little bit smaller" (owner, 2026-11-18), with a 14 px knob and a pointer target 6 px under WCAG 2.2's 24 px minimum;
  - (b) 36 × 20: 25 % narrower and 17 % lower, still a clear step down, with a 4 px shortfall on the target;
  - (c) **chosen:** 40 × 22. The width falls by a sixth and the height by a twelfth, so the change is plain at a glance and the pill keeps a ratio (1.8 : 1) near GTK's (2 : 1). The height is 2 px under WCAG 2.2's 24 px target minimum, so the spacing exception is the rule that holds ([The switches](#the-switches)).

### Answers decided on 2026-11-19, Task 9

Decided under the same delegation, for the owner's request of 2026-11-18: the window opens outside tiling mode, at the smallest size that fits its content in width and height, so its size depends on the monitor, and has scrollbars where the content would overflow the screen. Each choice, with the options weighed:

- **What sets the size:**
  - (a) a fixed default and minimum, as Phases 10 and 11 had (1040 × 720 and 720 × 560): the same window on every monitor, with a gap under the tiles on a large one and a cut-off page on a small one: the owner's complaint;
  - (b) the natural size of the page that shows, so the window resizes at each page change: about 575 px high on Display and about 765 on Configuration. A floating window that changes height at every click on the gear, with its bottom edge and the preview column moving under the pointer;
  - (c) **chosen** (the plan, with Task 1's measurements): the natural size of the largest page in each direction, the stack homogeneous. The window never changes size when the page changes or the service stops, and the off screen fits it. The price is empty space under the Display page's tiles, about 240 px on a large monitor: the price of a window that stays where it is.
- **Which `GtkScrolledWindow` wraps what:**
  - (a) one scrolled window around the whole body: the simplest tree, and the cap is exact, but on a 768 px screen the preview column and the page bars scroll away with the page, so the picture leaves the view while the user changes rotation or the Music settings (D1, D3);
  - (b) a scrolled window per page only (the plan's first wording), the column outside, the cap as `max-content-*` on each page's scroller: the cap would have to subtract the header bar, the page bar and the banner, whose height changes; the column, about 440 px high, would be cut off on a screen under about 540 px; and a narrow screen would scroll only the page, so a 640 px screen would show 330 px of a page that needs 588;
  - (c) **chosen:** one outer scrolled window around the content, which carries the cap and scrolls sideways, and inside it a scrolled window for the column and one for each page, which scroll down. The cap is set on one widget and counts the banner for free; the picture and the page bar stay put while a page scrolls down; and below 850 px the whole content scrolls sideways, so the page can come fully into view.
- **The cap's margin:**
  - (a) none: the window may fill the monitor and sit under a panel on a compositor that sends no work area;
  - (b) **chosen:** 48 px in each direction, room for one panel or dock. It's a judgment, not a measurement: panels are typically 30 to 50 px (**unverified**), and where the compositor sends its own bounds, which GTK also respects ([Unverified points, answered (Phase 11a Task 9)](#unverified-points-answered-phase-11a-task-9-2026-11-19)), they set the exact limit;
  - (c) 96 px, a panel on each edge: 48 px lost on every screen, and on a 1080 px screen it decides whether the Configuration page scrolls once Task 11's Theme card is in.
- **Several monitors** (settled by Task 1's brief):
  - (a) the monitor the window opens on: GDK can't say before the window has a surface (`gdk_display_get_monitor_at_surface` needs one), and on Wayland the compositor picks the output;
  - (b) **chosen:** the narrowest width and the shortest height among all monitors, which may be two different ones. The window fits wherever it opens, and a large monitor beside a small laptop panel gets the laptop's cap.
- **The monitors change while the app runs:**
  - (a) listen to the monitor list's `items-changed` and recompute the cap: the window would resize itself while the user works, on every hot-plug or resolution change;
  - (b) **chosen:** the cap is read once, before the window is mapped, and the window doesn't resize itself afterwards. What the user sees is in [Size and scrolling](#size-and-scrolling).
- **The not-available screen's size:**
  - (a) the same as the main screen: a window about 1000 × 810 px around two lines of text;
  - (b) **chosen:** its own. `screens` isn't homogeneous, so the window is as small as the message, about 440 × 170 px, and grows to its full size when the service answers.
- **A banner's room:**
  - (a) reserve it: 44 to 64 px of empty window under the header while no banner shows;
  - (b) **chosen:** the banner is part of the content. Shown, it adds its height to the window, or takes it from the pages' viewports where the cap bites.
- **Floating:** `resizable` false and nothing else. GTK has no request for "floating"; Task 1 measured a non-resizable window floated and a plain one, or one with no parent, tiled. No compositor protocol, rule or name enters the app (rule 14).

## Window

```
┌ GtkApplicationWindow ──────────────────────────────────────────────────────┐
│ GtkHeaderBar                    OpenDeepCool                    [⚙] [✕]   │
├────────────────────────────────────────────────────────────────────────────┤
│ Banner (GtkRevealer > GtkBox): only when needed                            │
├──────────────────────┬─────────────────────────────────────────────────────┤
│ Preview column       │ GtkStack "pages" (transition: none)                 │
│  GtkDrawingArea      │  "display": page bar  [Display ............ ⟳]     │
│  caption, status     │             tile gallery (scrolls)                  │
│                      │  "config":  page bar  [← Configuration]             │
│                      │             Display / Music settings / Startup      │
│                      │             (scrolls)                               │
│                      │  "off":     the off screen (scrolls)                │
└──────────────────────┴─────────────────────────────────────────────────────┘
```

- **Size:** the size of its content, which depends on the monitor; not resizable by the user (`resizable` false, which also asks a compositor that floats fixed-size windows to float it); never larger than the smallest monitor less 48 px, and where the content is larger than that, it has scrollbars. The default size of 1040 × 720 px and the minimum of 720 × 560 px of Phases 10 and 11 are gone. The scrolled windows aren't drawn above: [Size and scrolling](#size-and-scrolling) has the one widget tree.
- **Header bar** (`GtkHeaderBar`, `show-title-buttons` false: the desktop's window buttons are off, so no desktop draws a second close button; Phase 11a Task 1 measured it). Its layout:
  - **Title,** centred: a `GtkLabel` "OpenDeepCool" (class `app-name`, not focusable) as the bar's title widget (`set_title_widget`). GTK's docs say the title "will be centered with respect to the width of the box, even if the children at either side take up different amounts of space" (docs.gtk.org, HeaderBar, read 2026-11-19), so the empty start side doesn't push it. The window's own title, for the window manager, stays "OpenDeepCool" on every page.
  - **Nothing at the start.**
  - **At the end:** one horizontal `GtkBox` (4 px between its children), packed with `pack_end`, holding the gear and then the close icon, so the close icon is the window's last control at the right. Packing one box keeps the order from depending on `pack_end`'s.
    - **Gear:** a `GtkToggleButton` with `icon-name` `emblem-system-symbolic`, the class `icon-button`, accessible name "Configuration", tooltip "Configuration". Pressed while the Configuration page shows. It has no action name: its `toggled` handler goes to the page (guarded while the app sets it itself, as the other toggles are), and Ctrl+, calls the same code.
    - **Close:** a `GtkButton` with `icon-name` `window-close-symbolic`, the class `icon-button`, action `win.close`, accessible name "Close", tooltip "Close".
  - **The icons** are Adwaita's symbolic ones: `emblem-system-symbolic` (the gear, in the `legacy` folder of `adwaita-icon-theme`), `window-close-symbolic`, `go-previous-symbolic` and `view-refresh-symbolic`, which Phase 11a Task 1 found in the user's theme. Should an icon theme lack one, the tooltip and the accessible name still say what the button does.
- **Icon buttons** (gear, close, back, Refresh) have no frame in any state (owner, 2026-11-18). Each is 32 × 32 px with a 16 px icon, so the target is above WCAG 2.2's 24 × 24 minimum (SC 2.5.8). The stylesheet draws them with tokens only (the test that no colour literal appears outside `:root` forbids `transparent`, so the resting fill is the surface they sit on, as `button.link` does):

  | State | Look |
  |---|---|
  | rest | fill and text colour `--bg` and `--fg`, no border (`border-style: none`) |
  | `:hover` | fill `--ctl-hover-fill` |
  | `:focus-visible` | fill `--ctl-focus-fill`, and the 2 px `--accent` outline every control has |
  | `:active` | fill `--ctl-pressed-fill` |
  | `:checked` (the gear on the Configuration page) | fill `--ctl-selected-fill`, icon `--accent` |
  | `:disabled` | fill `--bg`, icon `--muted`, no `opacity` |

  The rules read `button.icon-button`, `button.icon-button:hover` and so on, which outrank the plain `button` and `button:checked` rules the controls share, and the border stays off in every state. The icon on the selected fill needs 3.0:1 (WCAG 1.4.11); Phase 11a Task 11's contrast table, which replaces the palette, includes that row.
- **The header, by state:**

  | State | Title | Gear | Close |
  |---|---|---|---|
  | Normal, Display page | shown | shown, sensitive, not pressed | shown, sensitive |
  | Normal, Configuration page | shown | shown, sensitive, **pressed** | shown, sensitive |
  | Service not running (the [off screen](#the-off-screen)) | shown | shown, **insensitive**, not pressed | shown, sensitive |
  | [Not available in this session](#not-available-in-this-session) | shown | **hidden** (`visible` false) | shown, sensitive |

  - While the service is down the gear keeps its place, insensitive: Configuration can't be reached then, because Start at boot is on the off screen (D10), and the header keeps its shape.
  - In a session that can't use OpenDeepCool the gear is hidden, not just insensitive: nothing may suggest a control that could work (owner, 2026-10-26). Only the title and the close icon show.
  - If the service goes down while the Configuration page shows, the off screen replaces it and the gear lets go; when the service is back, the window returns to the page last shown and the gear is pressed again.
- **The pages.**
  - **One `GtkStack`**, `pages`, with three children: `display`, `config` and `off` (the off screen). It is `GTK_STACK_TRANSITION_TYPE_NONE` and homogeneous in both directions, so it is as wide as its widest page and as tall as its tallest, and the window doesn't change size when the page changes or when the service stops. The not-available message is in the `screens` stack around it, which is not homogeneous, so the window takes that message's own size there ([Size and scrolling](#size-and-scrolling)).
  - **A page** is a vertical `GtkBox` of its page bar (below) and a `GtkScrolledWindow` holding its content, so the bar never scrolls away. Each page keeps its own scroll position while the other shows. The off screen has no page bar; its `GtkScrolledWindow` is the page. The page's scrolled window scrolls down only: sideways, the window's outer scrolled window does it ([Size and scrolling](#size-and-scrolling)).
  - **The page changes only by the user's action:** the gear, Ctrl+,, the back icon, Escape and Alt+Left (or by the service going down and coming back, as above). No motion.
  - **Focus.** Opening the Configuration page moves the focus to its back icon, the page's first control, so Tab goes on into Rotation. Closing it moves the focus to the gear, the control that opened it, whichever way the page was left (back icon, Escape, Alt+Left); when the gear itself closed it, the focus is there already. If the gear can't take it (the service went down), the off screen's own rule applies ([The off screen](#the-off-screen)). A page change by the mouse moves the focus too: GTK draws no focus ring after a pointer click, so nothing shows.
  - **Screen readers** hear the page's title (`gtk_accessible_announce`, since GTK 4.14, verified) when the user changes the page, not when the window returns to a page after the service did.
- **Page bar.** A horizontal `GtkBox` (class `page-bar`, 8 px between its children, 18 px at its start and end, 12 px above, none below) outside the scrolled area:
  - **Display:** the title "Display" (a `GtkLabel`, class `page-title`, `hexpand`, `xalign` 0) and the Refresh icon at the end ([Refresh](#refresh));
  - **Configuration:** the back icon at the start, then the title "Configuration" (`GtkLabel`, class `page-title`):
    - the back icon is a `GtkButton` with `icon-name` `go-previous-symbolic`, the class `icon-button`, action `win.back`, accessible name "Back", tooltip "Back to Display";
    - `page-title` is one step above the card headings: weight 600, and a size from Phase 11a Task 11's type scale;
    - the content's top margin under a page bar is 6 px, so the first row sits close under the bar. (With no bar the page keeps its 18 px margin all round: the off screen.)
- **No motion in the chrome.** `GtkStack` uses `GTK_STACK_TRANSITION_TYPE_NONE`, and the app's stylesheet sets `transition: none` on everything. The preview is the only thing that moves.
- **Remote or inactive session:** the header bar keeps only the title and the close icon, and the body holds only one message ([Not available in this session](#not-available-in-this-session)).

### Size and scrolling

**What the owner asked** (2026-11-18): the window opens outside tiling mode, at the smallest size that fits its content in width and height, so its size depends on the monitor; where the content would overflow the screen, it has scrollbars. Phase 11a Task 1 measured what GTK does with these properties ([Unverified points, answered (Phase 11a Task 1)](#unverified-points-answered-phase-11a-task-1-2026-11-18)); this is what the app builds. The pixel sizes are **estimates** from this design's own numbers, not measurements: the real ones depend on the user's font, so the tests compare the window with its own content's natural size and pin no pixel count.

- **The size is the content's.** The window has no default size (`default-width` and `default-height` unset), no minimum size, and no `set_size_request` on it or on anything that sets its size (the scrolled windows, the stacks and the boxes in the tree below). The requests that remain belong to the content: the preview column's 262 px, the GPU box's and the scales' 320 px. GTK then takes the window's natural size from its child: in GTK's source, `gtk_window_compute_default_size` uses the natural size when no default size is set, and clamps it to the compositor's bounds ([Unverified points, answered (Phase 11a Task 9)](#unverified-points-answered-phase-11a-task-9-2026-11-19)).
- **The content** is the header bar, the banner while one shows, and the body: the preview column beside the pages. The pages stack is homogeneous in both directions, so the body is as wide as its widest page and as tall as its tallest, whichever page shows. The Display page is the widest (the Music group's six tiles) and the Configuration page the tallest; the off screen is smaller than both. So the window doesn't change size when the user opens Configuration, nor when the service stops and the off screen replaces a page. One thing would break that: the Startup card leaves the Configuration page for the off screen, which would make that page, the tallest, about 150 px shorter. An empty box stands in its place on the page, as tall as the card was there (remembered when the card leaves, its natural height before that), so the window keeps its height to within a wrapped line of hint text (found by Task 9's tests). The cost is empty space under the Display page's tiles, about 240 px on a large monitor ([Answers decided on 2026-11-19, Task 9](#answers-decided-on-2026-11-19-task-9)).
- **The tree,** the one the app builds. Only the scrolled windows' properties and the stacks' homogeneity are new; everything inside a page is as its own section designs it.

  ```
  GtkApplicationWindow       resizable false; no default size; no size request; titlebar: the GtkHeaderBar
  └─ GtkOverlay              its overlay child is the "Refreshed" / "Not changed" label, outside every scroller
     └─ GtkScrolledWindow "window scroller"     the cap, and the sideways scroll (table below)
        └─ GtkBox (vertical) "content"
           ├─ GtkRevealer                        the cooler banner, transition none
           └─ GtkStack "screens"                 hhomogeneous false, vhomogeneous false, transition none
              ├─ "main"  GtkBox (horizontal)
              │  ├─ GtkScrolledWindow "column"   class `side`
              │  │  └─ GtkBox                    the preview column, 262 px wide
              │  └─ GtkStack "pages"             hhomogeneous true, vhomogeneous true, transition none, hexpand, vexpand
              │     ├─ "display"  GtkBox (vertical): the page bar, then GtkScrolledWindow "page" holding the tiles
              │     ├─ "config"   GtkBox (vertical): the page bar, then GtkScrolledWindow "page" holding the cards
              │     └─ "off"      GtkScrolledWindow "page" holding the off screen
              └─ "not-available"  GtkBox (vertical), centred, 24 px of margin all round
  ```

  | Scrolled window | `hscrollbar-policy` | `vscrollbar-policy` | `propagate-natural-width` / `-height` | `max-content-width` / `-height` | Also |
  |---|---|---|---|---|---|
  | window scroller | automatic | automatic | true / true | the cap's width / its height less 48 (below); unset when no monitor is known | `hexpand`, `vexpand`, `focusable` false |
  | column | never | automatic | true / true | unset | class `side`, `vexpand`, `focusable` false |
  | page (`display`, `config`, `off`) | never | automatic | true / true | unset | `hexpand`, `vexpand`, `focusable` false |

  - `min-content-width` and `-height` stay unset (−1) on all of them, so every scroller can shrink below its content.
  - The window scroller carries the cap and nothing else. Its child's minimum height is small, because the column and the pages can shrink, so in practice it never scrolls down: the column and the pages take the squeeze. It scrolls sideways, below the content's minimum width.
  - The column and the pages never scroll sideways themselves (`never`): their minimum width, the Configuration page's 588 px and the column's 262, then counts in the content's minimum, which is what makes the window scroller scroll. With `propagate-natural-width` set, the `never` policy changes nothing (a scrolled window that can't scroll sideways already asks for its child's width); it is set so the intent reads in the code.
  - The `side` class moves to the column's scrolled window: it draws the 1 px divider on its right, so the divider runs the window's full height, as the column's box did. The box inside keeps the 18 px padding.
  - The feedback label stays in the `GtkOverlay` around the window scroller, so it keeps its place at the window's bottom edge while the content scrolls.

- **Expected size, on a normal monitor** (estimates; the font is the system's, about 14.7 px):

  | Part | Width (px) | Height (px) |
  |---|---|---|
  | Header bar | the window's | 48 (about 47) |
  | Preview column | 262 | about 380, up to 440 while a reason line shows |
  | Display page | about 754: 36 of margin, six Music tiles of about 113 (the caption "GPU temperature", the tile's border and its padding) and five 8 px gaps | about 575: the 44 px page bar, then 6, 122, 18, 122, 18, 226 and 18 |
  | Configuration page | about 588: 36 of margin, 32 of card padding, a 520 px row | about 766 with the GPU row (709 without): the 44 px page bar, then 6, the Display card 240 (169), 18, the Music card 275, 18, the Startup card 147 and 18 |
  | Off screen | about 560 | about 430 |
  | **Window** | **about 1016** (262 + 754; 930 with the mockups' 98 px tiles) | **about 814** (48 + 766; 757 with one GPU) |

  The Configuration page is the tallest by about 190 px, so a Display page full of reason lines can't overtake it; Task 11's Theme card adds about 170 px to it, and the window to about 984. The Display page is the widest by about 166 px, so nothing on the Configuration page, a note and the terminal commands included, can widen the window.

- **The cap.** The window is never larger than the smallest monitor less 48 px in each direction. It is computed once, in the window's constructor, before the window is mapped:
  1. `WidgetExt::display(&window).monitors()` (`gdk_display_get_monitors`: the monitors attached to the display, a `GListModel` of `GdkMonitor`; gtk4-rs `DisplayExt::monitors`; verified).
  2. For each monitor, `geometry()` (`gdk_monitor_get_geometry`; `MonitorExt::geometry`; verified). It is in application pixels, "not in device pixels" (docs.gtk.org, read 2026-11-19), the units widgets are measured in, so a fractional scale needs no conversion. Take the smallest width and the smallest height over all monitors: they may be two different monitors.
  3. `cap_w = max(smallest_width − 48, 320)` and `cap_h = max(smallest_height − 48, 240)`. 48 px is the margin (below), and the floor keeps a bogus monitor report from making an unusable window.
  4. `window_scroller.set_max_content_width(cap_w)` and `set_max_content_height(cap_h − 48)`. The cap is for the whole window and `max-content-*` for what's inside the header bar, so the second 48 is the header bar's height (about 47, taken as a constant, not measured: the window can differ from the cap by a pixel or two, which the margin absorbs and the test allows). Task 1: `max-content-height` caps the window at its value plus the window's own chrome.
  5. With no monitor listed, both stay unset. The window then takes its natural size, limited only by the compositor's bounds, where it sends them. A made-up cap would be a guess that puts scrollbars on a large screen, and the list is empty only when there is no screen to size for.
  6. The rule is a pure function of the monitors' sizes, `cap(&[(width, height)]) -> Option<(cap_w, cap_h)>`, so its unit test needs no display.

  | Screen | Cap | Window (estimate) | What scrolls |
  |---|---|---|---|
  | 2560 × 1440 or 1920 × 1080 | 2512 × 1392, 1872 × 1032 | about 1016 × 814 | nothing (with the Theme card, about 984 still fits 1032) |
  | 1440 × 900 | 1392 × 852 | about 1016 × 814 | nothing |
  | 1280 × 800 | 1232 × 752 | about 1016 × 752 | the Configuration page, about 60 px |
  | 1366 × 768 | 1318 × 720 | about 1016 × 720 | the Configuration page, about 95 px |
  | 1024 × 768 | 976 × 720 | 976 × 720 | the Configuration page down; the Music tiles wrap to five a line |
  | 640 × 480 | 592 × 432 | 592 × 432 | every page down, and the whole content sideways (850 px needed) |

- **Scrolling.**
  - **Down:** a page scrolls inside its scrolled window, under its page bar. The header bar, the banner, the page bar and the preview column stay where they are, so the picture stays in view while the user changes rotation, the GPU or the Music settings. The column scrolls on its own only when the window's body is shorter than it (about 440 px with a reason line), on a screen under about 540 px high: the column, the header bar and the margin. Each page keeps its scroll position while the other shows, and a theme change keeps it.
  - **Sideways:** below the content's minimum width (the column's 262 plus the Configuration page's 588, about 850 px) the whole content scrolls sideways under the header bar, the preview column with it. On a 640 px screen a page that fits fully once the column has scrolled away is better than a page cut to 330 px beside a column that stays (Task 8's Rows text, which had the page alone scroll sideways, is replaced by this). A GTK viewport scrolls in a direction only when it is allocated less than its child's minimum size in that direction (docs.gtk.org, `GtkViewport`, read 2026-11-19), which is why the threshold is the content's minimum, not its natural width.
  - **Between** the natural width and 850 px the Display page's flow boxes wrap their tiles into more lines (`min-children-per-line` 1), so a window a little narrower than its content grows taller, not wider, before it scrolls sideways.
  - **Scrollbars** are GTK's own. With overlay scrolling (GTK's default, the user's `gtk-overlay-scrolling`), they take no room. Whether a classic scrollbar, which takes room, counts toward the natural size is **unverified**.
  - **The keyboard and the pointer:** a focused control scrolls into view by itself ([Keyboard](#keyboard)); the wheel scrolls the page under the pointer.

- **What changes the window's size while the app runs.** The window follows its content's natural size, since it has no default size to hold it ([Unverified points, answered (Phase 11a Task 9)](#unverified-points-answered-phase-11a-task-9-2026-11-19)), so each of these resizes it unless the cap holds it. There is no animation.

  | Part | Effect on the window |
  |---|---|
  | Cooler banner | Height only: it adds its own height while it shows, about 44 px (about 64 with two lines), and gives it back when it goes. Where the window is at its cap, the pages' viewports give up that height instead. Never width: its text is capped ([below](#size-and-scrolling)) |
  | Reason line under a group heading (Display page), reason line under the preview | About 17 px a line on the page, 17 to 60 under the picture: never enough to make the Display page or the column the tallest part |
  | Start at boot note and terminal commands (Configuration page) | Height only, about 90 to 130 px on the tallest page; the width stays under the Display page's |
  | The off screen | Nothing: a child of the homogeneous `pages` stack, smaller than the other pages. The window keeps its size when the service stops and when it comes back, and the preview column stays |
  | The not-available screen | It's in `screens`, a different stack and not homogeneous: the window takes the message's own size, about 440 × 170 px, and grows to its full size when the service answers (a few milliseconds after the first map in a session that is remote or inactive: no animation) |
  | The "Refreshed" and refusal label | Nothing: an overlay child takes no part in the window's size |
  | Opening Configuration, or the service stopping | Nothing: the stack is homogeneous |

- **Not resizable, and floating.**
  - The window is built with `resizable` false ("If true, users can resize the window", docs.gtk.org, `GtkWindow:resizable`). GTK has no request for "floating"; a window with a fixed size is the hint a compositor can read, and Task 1 measured one such compositor floating it ([Unverified points, answered (Phase 11a Task 1)](#unverified-points-answered-phase-11a-task-1-2026-11-18); system evidence, not a test).
  - **What the user can no longer do:** drag an edge to resize the window, or maximise it. The header bar has no maximise button either way (the desktop's window buttons are off). For a settings window of fixed content that costs nothing: the extra room would be empty, since the pages' rows and the tiles stay at their sizes, and where room is short the window already scrolls.
  - **A compositor that tiles it anyway:** the window is a tile, and GTK lays the content out in the size the compositor gives it. A bigger tile leaves the extra room empty (the pages expand, the rows and tiles don't, and everything stays at the top left); a smaller one scrolls exactly as on a small monitor. Nothing breaks. This is **unverified** on any compositor but the one Task 1 ran.
  - The app doesn't detect the compositor, doesn't ask any compositor for floating, and ships no window rule (AGENTS.md rule 14). Whether the owner adds a rule of their own is theirs to decide (rule 7).

- **The monitors change while the app runs** (a monitor plugged in or out, a new resolution). The cap is read once, so the window doesn't resize itself.
  - **To a smaller screen:** the window keeps its size. Where the compositor re-fits the window to the new output, the pages scroll as on any small screen (**unverified**: the compositor decides); where it doesn't, part of the window may lie off the screen, until the user moves it with the compositor's own means or closes the app and opens it again, which reads the monitors afresh and fits.
  - **To a larger screen:** the window keeps its size, and a window that was capped keeps its scrollbars until the app is opened again.
  - A window that resizes itself at every hot-plug, while the user is working, is the worse choice for a settings window that is opened for a minute ([Answers decided on 2026-11-19, Task 9](#answers-decided-on-2026-11-19-task-9)).

- **Text never sets the window's width.** A wrapping `GtkLabel`'s natural width is its text on one line unless `max-width-chars` says otherwise (docs.gtk.org, `GtkLabel`, Text layout), so a long sentence would widen the window. Every wrapping label that spans a container asks for a measure instead; once the window is wider, the label rewraps to the room it gets.

  | Text | `max-width-chars` | About |
  |---|---|---|
  | Banner text | 100 | 740 px, under the window's width |
  | A hint or note under a card's rows (the Music hint, the Startup hint, the Start at boot note) | 72 | the rows' 520 px |
  | The off screen's heading and text | 48 | centred |
  | The not-available message | 40 | the window's width less its 48 px of margin |
  | The preview column's lines, the label blocks' hints, the GPU box's lines | 28, 28, 40 | as built |

  The terminal commands set the Startup card's width up to about 540 px (the longest command, and Copy beside it), which is under the Display page's, so they never change the window.

- **What the tests check** (the app's window tests, which need a realised window: `--features weston-tests`; `Weston::start` takes the output's size). No test pins a pixel count, because the font differs between machines; each compares the window with its own measurement:
  - **Natural size.** On an output larger than anything the window needs (2560 × 1600), the window is exactly its natural size (`measure` of the window), and no page, the column nor the window scroller shows a scrollbar (`upper` ≤ `page_size` on each adjustment).
  - **No change of size on a page change.** The window's size is the same on Display, Configuration and the off screen, and after the service stops and comes back.
  - **A screen that just fits.** On an output of the natural size plus 48 px in each direction, the window is its natural size and nothing scrolls.
  - **A smaller screen.** On an output 120 px lower than that, the window's height is the output's less 48 px (±2 px), its width is unchanged, the Configuration page has a vertical scrollbar, the preview column has none, the preview's position doesn't change when the page is scrolled to its end, and the page bar stays.
  - **640 × 480.** The window is at most 592 × 432 (so never wider than the output); the Display and Configuration pages each have a vertical scrollbar; the window scroller has a horizontal one; no page and not the column has a horizontal scrollbar.
  - **Not resizable.** `resizable` is false, `default_size` is (0, 0), and `size_request` of the window is (−1, −1).
  - **The cap's rule,** as a unit test with no display: no monitor gives `None`; one gives its size less 48; several give the smallest width and the smallest height, which may be two monitors'; a tiny one gives the floor (320 × 240).
  - **Not available.** The window is at most 480 × 220 px, shows the title and the close icon, and grows to the natural size when the service answers.
  - **A banner.** After a `not-found` state, the window is taller by the banner's height, or, when GTK doesn't resize a mapped window, the Display page's viewport is shorter by it; the test records which, and neither lets the banner or the window go beyond the cap.

## Preview column

**Purpose:** what the cooler shows right now, plus the short status (PRD Phase 10: preview, status).

- **Preview:** a `GtkDrawingArea` of 210 × 210 px, 15 px per dot, in a 226 × 226 px frame with the surface colour and 8 px of padding, as the mockups draw it (decisions log D51).
  - It draws 14 × 14 round dots: lit dots in `--fg`, unlit dots in `--fg` at 0.12 alpha.
  - It draws the frame the service sends the app over D-Bus, in a signal addressed to it alone ([Talking to the service](#talking-to-the-service)). It's the same frame written to the cooler, so rotation, scrolling, dashes, rounding, the 9999 cap, the spectrum with its peak and fall settings, the emoji loops and the Dancer are exactly the cooler's.
  - `queue_draw()` runs only when a new frame differs from the last one. A static picture costs nothing; a moving one redraws at the cooler's 100 ms tick.
  - It also redraws once on a theme change.
  - The app watches frames (`WatchFrames()`) only while the window is mapped, and stops watching when it's hidden or minimised.
- **Caption:** the current mode, `<group> · <option>` (for example "Temperature · CPU + GPU"), or "No picture" while the service isn't running. A `GtkLabel`, class `caption`, wrapping at 28 characters. **Nothing stands under it:** the column has no line of its own for the cooler's state (owner, 2026-11-18; D13). The status item says it in a few words, and the banner has the full sentence and the fix ([Banners](#banners)).
- **Reason line**, when the saved mode has lost a reading: a warning icon and `<metric>: <reason>. The cooler shows dashes until it's back.` The reason text comes from the service (`UnavailableReasons`). It's the one line that can stand between the caption and the status.
- **Status line:** a `GtkBox` of up to three items. Each is a coloured dot plus words, never colour alone:
  - cooler, from `CoolerState`: "Cooler connected" (green), "Cooler not connected" (yellow), "Cooler: no access" (red) or "Cooler: sending fails" (yellow). `write-failed` reads "Cooler connected" for its first 2 s and "Cooler: sending fails" after (the same rule as the banner). While the service is down it says "Cooler: unknown" (hollow dot);
  - service: "Service running" (green); while it's down, on the [off screen](#the-off-screen), "Service off" (hollow dot: it doesn't start at boot, off on purpose) or "Service not running" (red: it's set to start but stopped, failed or starting);
  - Music sound, **only while a Music mode is chosen** (`AudioConnected`): "Sound connected" or "No sound: spectrum is silent".
- **The picture isn't on the cooler:** the preview keeps drawing the service's picture while the cooler is away (the banners' text says so), so the picture alone can't tell. Three places do, and none of them is a line under the caption:
  - the **banner** above the window, with the sentence and the fix;
  - the **cooler's status item**, yellow or red, with its words;
  - the **preview's accessible description**, which a screen reader reads when it reaches the picture: the caption, a full stop and the words of the cooler's state, in the table below.
- **What the column holds, by state.** The cooler's state changes the words and the dot of its status item and the picture's description, never the number of lines, so nothing in the column moves when the cooler comes and goes:

  | State | Caption | Cooler status item | Preview's description (`Cooler display preview`) |
  |---|---|---|---|
  | `connected`, or `write-failed` for under 2 s | the mode's | "Cooler connected", green | the caption alone: "Temperature · CPU + GPU" |
  | `not-found` (and any value the app doesn't know, such as the dev copy's `released`) | the mode's | "Cooler not connected", yellow | "Temperature · CPU + GPU. Not on the cooler: it isn't connected." |
  | `no-access` | the mode's | "Cooler: no access", red | "Temperature · CPU + GPU. Not on the cooler: the service can't open it." |
  | `write-failed` for 2 s or more | the mode's | "Cooler: sending fails", yellow | "Temperature · CPU + GPU. Not on the cooler: sending it fails." |
  | the saved mode lost a reading | the mode's, with the reason line under it | as the cooler's state | as the cooler's state |
  | the service isn't running | "No picture" | "Cooler: unknown", hollow | "No picture" |
  | not available in this session | the column isn't there ([Not available in this session](#not-available-in-this-session)) | | |

- **Widgets:** the column is a vertical `GtkBox` (10 px between its children, 18 px of padding, 262 px wide with the divider) of the frame, the caption, the reason box (hidden unless a reading is lost) and the status box, inside its own `GtkScrolledWindow` (the column scroller, class `side`, which draws the 1 px divider so it runs the window's full height; [Size and scrolling](#size-and-scrolling)). The column stays in place while a page scrolls down, and scrolls by itself only where the window is shorter than it. Its lines wrap at 28 characters, so no line widens it. The description is set on the `GtkDrawingArea` by the app each time the caption or the cooler's state changes (`names.describe`), and the caption label is only a label.

## Display page

**Purpose:** choose the mode (settings.md: the only setting outside the configuration screen). The page is home: the window opens on it, and no tab leads to it.

- **Page bar** at the top, outside the scrolled area ([Window](#window)): the title "Display" at the start, and the Refresh icon at the end ([Refresh](#refresh)). The icon sits at the right end of the bar, above the Load group and level with the title; the preview column is on its left and isn't involved.
- **Size:** this is the widest page. Its natural width is the Music group's six tiles (a flow box asks for room for `max-children-per-line` children, docs.gtk.org, `GtkFlowBox`; a tile in a homogeneous flow box is as wide as the widest caption in its group, "GPU temperature" in Music), about 754 px. Its natural height, about 575 px, is the shortest of the pages, and stays under the Configuration page's even with a reason line under each of the four group headings. Where the page gets less than its natural width, the flow boxes wrap their tiles, so the page grows taller instead of wider ([Size and scrolling](#size-and-scrolling)). That the natural width is the six tiles', and not another number, is **unverified**: the docs give no formula, and the layout test measures it.
- **The unit** isn't here any more. The temperature tiles show the unit set on the Configuration page, and redraw when the service confirms a change ([The unit](#display)).
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
    - tiles are drawn upright (no rotation) in the current unit, with peak dots as currently set. They redraw only when the unit, the peak-dots setting or the theme changes;
    - lit dots are opaque `--fg`, unlit ones `--fg` at 0.12 over the tile's own fill; an unavailable tile draws its lit dots in `--muted` (decisions log D56). The sample spectrum is the mockups' (`SAMPLE_SPECTRUM`), drawn by the service's own code from a spectrum standing at those levels (`odc_render::Spectrum::at`, decisions log D54).
  - **Selection:**
    - each `GtkFlowBox` activates on a single click (`activate-on-single-click`) and selects nothing itself (`GTK_SELECTION_NONE`), since GTK's own selection would follow keyboard focus. The app marks the saved mode's tile, its look and its accessible selected state, so the four boxes act as one radio group and a tile shows as chosen only once the service confirms it (decisions log D55);
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

**Purpose:** every setting but the mode, on one screen (settings.md, A configuration screen; the temperature unit joined it on 2026-11-18). The gear opens it, and the back icon, Escape or Alt+Left leave it ([Window](#window)). It is a [page bar](#window) with the back icon and the title "Configuration", and under it, in the scrolled area, three sections, Display, Music and Startup, each a surface-coloured `GtkBox` card with a heading and rows. A row is a label block (the title, with its hint under it) and its control, side by side at the left of the card ([Rows](#rows)). The third, Startup, holds no setting of the service: its switch shows and changes systemd's state for OpenDeepCool's units (settings.md, Start at boot). Phase 11a Task 11 adds the Theme card in front of Display, with rows of the same kind.

### Rows

**What the owner asked** (2026-11-18): the labels sit close to their controls, left-aligned, and no longer fill the space between them with the control pushed to the right edge. Each card is a vertical `GtkBox` (14 px between its children, 14 px above and below and 16 px at the sides: the stylesheet's `.card`), as before.

```
┌ card ───────────────────────────────────────────────────────────────────────┐
│ Display                                                                     │
│ Rotation                    [ 0° ][ 90° ][ 180° ][ 270° ]                   │
│ Turns the picture clockwise                                                 │
│ Temperature unit            [ °C ][ °F ]                                    │
│ Used wherever the display                                                   │
│ shows a temperature                                                         │
│ GPU                         [ Automatic                         v ]         │
│ Where the display's GPU     Using Dedicated · Contoso Graphics R870.        │
│ values come from                                                            │
└─────────────────────────────────────────────────────────────────────────────┘
  |<- label block: 28 characters ->|<24>|<- control ->|  the rest stays empty
```

- **A row** is a horizontal `GtkBox` of two children, 24 px apart (`spacing` 24): the label block, then the control. Nothing in it has `hexpand`, so the row is as wide as the two and the rest of the card stays empty at the right. That is the layout the owner asked for: left-sided.
- **The label block** is a vertical `GtkBox` (`spacing` 2, `hexpand` false, `halign` start, `valign` start) of:
  - the **title**, a `GtkLabel` (`xalign` 0);
  - the **hint**, when the setting has one, a `GtkLabel` with the classes `muted` and `small`, `xalign` 0, `wrap` true, `wrap-mode` word, `width-chars` and `max-width-chars` both **28** (as built, so the block's minimum is the same 28 characters).
  - **How a hint wraps.** docs.gtk.org's GtkLabel page: "For wrapping labels, width-chars is used as the minimum width, if specified, and max-width-chars is used as the natural width", and "wrapping labels will be rewrapped to use all of the available width" (read 2026-11-19). So a hint asks for about 28 characters, as its minimum and as its natural width, and breaks at word boundaries there: "Used wherever the display shows a temperature" takes two lines, "Turns the picture clockwise" one. A hint never makes its block wider than 28 characters, which is what keeps the controls close, and it takes the full width of the block once the size group (below) widens it.
  - The hint's size is the class `small`, which `app.css` has no rule for (nothing in `crates/odc-app/src` styles it, 2026-11-19; whether GTK's own theme gives it a size is **unverified**). Phase 11a Task 11's type scale defines it; the 28 characters don't depend on it. The mockups draw it at 12 px.
- **One column of controls.** Every label block of the Configuration page is in one `GtkSizeGroup` (`mode` horizontal). GTK's docs: "the size requested for each widget in a GtkSizeGroup is the maximum of the sizes that would have been requested" (class page, read 2026-11-19). So the group's width is the widest block's, which is the 28-character hint (about 176 px in the mockups' type), and all the page's controls start at the same x, across the Display, Music and Startup cards. Task 11's Theme card joins the same group.
  - The Startup card leaves the group while it stands on the [off screen](#the-off-screen), where it is alone and needs only its own width; it joins again when it moves back into the Configuration page (`StartupCard::place`).
  - Whether a hidden member still counts toward the group's width is **unverified** (the class page doesn't say). It changes nothing here: the one row that hides, GPU, has a block no wider than the others' (the same 28 characters). Fallback: remove the block from the group while its row is hidden.
- **The control** has `hexpand` false and `halign` start. Its `valign` says which row it is:

  | Row | Control | `valign` of the control | Width |
  |---|---|---|---|
  | Rotation, Temperature unit | the linked toggles (`GtkBox`, class `linked`) | center | natural |
  | Peak dots, Start at boot | a `GtkSwitch` ([The switches](#the-switches)) | center | 40 px |
  | GPU | a vertical `GtkBox`: the dropdown, the "Using …" line, the "No reading …" hint | start | 320 px (`width-request`) |
  | Bar fall, Peak fall | a `GtkScale`, marks below | start | 320 px (`width-request`) |

  The toggles and the switches are shorter than a block of a title and a hint, so they centre against it. The dropdown's box and the scales hang from the top, level with the title, since a scale's marks and the dropdown's lines hang below them.
- **The lines under a control** (the GPU's "Using …" and "No reading …", with `max-width-chars` 40) and the lines that span a card (the Music hint, the Startup note, the terminal commands, the Startup hint) are not in the row: they are children of the card or of the control's box, and wrap at their own width. They have no part in the group. A line that spans a card asks for 72 characters at most (`max-width-chars`, about the rows' 520 px), so a long sentence never makes the page wider than its rows ([Size and scrolling](#size-and-scrolling), Text never sets the window's width).
- **Hidden rows.** The GPU row hides with one `set_visible(false)`, as it does today. The card's 14 px spacing skips hidden children (GtkBox), so a PC with one GPU has no gap there.
- **Narrow windows.**
  - The window opens at the size of its content and isn't resizable by the user, so a page is narrower than its natural width only where the monitor is: the content is capped to the screen ([Size and scrolling](#size-and-scrolling)).
  - A row needs its label block, 24 px and its control: about 176 + 24 + 320 = 520 px, 588 px with the card's padding and the page's margins, 850 px with the preview column beside it. A row doesn't narrow below that: the label block keeps its 28 characters (its minimum, `width-chars`) and the control its 320 px. Below 850 px the pages' own scrolled windows don't scroll sideways (`hscrollbar-policy` never); the window's outer scrolled window does, with the preview column.
  - No row stacks its control under its label, and the GPU box and the scales keep their 320 px (`width-request`) and don't shrink. (Whether GTK could give a scale a smaller minimum than its natural width isn't in the documentation: **unverified**, and not needed, since the content scrolls.)
- **Right-to-left text** mirrors the rows by itself (a `GtkBox` follows the text direction): the block at the right, the control at its left.
- **What the tests check** (the app's layout tests, which need a realised window: `--features weston-tests`):
  - in every row, the control's left edge is 24 px (±1) right of its block's right edge, and the block's width is the group's;
  - every control of the page starts at the same x (±1), and that x doesn't change when the GPU row hides;
  - the Temperature unit hint has wrapped to two lines: the group's width is less than that hint's one-line width;
  - the Startup card on the off screen has its block at its own width, and on the Configuration page it has the group's.

### Display

- **Rotation:** "Turns the picture clockwise". Four grouped `GtkToggleButton`s, `0°`, `90°`, `180°`, `270°`, which call `SetRotation`. The preview turns with the next frame.
- **Temperature unit,** the row after Rotation: "Temperature unit", with the hint "Used wherever the display shows a temperature". The same control as Rotation's: two grouped `GtkToggleButton`s, `°C` and `°F`, linked, with the accessible names "Celsius" and "Fahrenheit" (`gtk_toggle_button_set_group`, verified).
  - A toggle calls `SetTemperatureUnit` and goes back to the saved unit until the service confirms the change (`StateChanged`), as every control does ([Refused changes](#refused-changes)).
  - In a mode without a temperature, the unit is only saved and the cooler doesn't change (display.md, Mode switches). Nothing in the app needs to say so, because the preview stays the same.
  - The temperature tiles on the Display page redraw in the confirmed unit.
- **GPU:** "Where the display's GPU values come from".
  - A `GtkDropDown`, shown only when the PC has more than one GPU. It and the lines under it stand in one vertical `GtkBox` 320 px wide (`width-request`), `halign` start, `valign` start ([Rows](#rows)); the dropdown fills it, and the open list opens at least as wide as the button, aligned with its left edge. Its `factory` draws the button, with the label alone. Its `list-factory` draws the popup's entries: a check mark, the label, and a muted note at the right (GtkDropDown:list-factory, "the factory for populating list items in the popup", docs.gtk.org, read 2026-11-06).
  - **Automatic** is the first entry and the default (owner, 2026-11-04). It reads "Automatic", in the list and on the button, and choosing it calls `ChooseGpu("")`. Its entry has no note.
  - **The GPU in use:** while Automatic is chosen, a muted `GtkLabel` under the dropdown names the GPU it reads: "Using Dedicated · Contoso Graphics R870."
    - The name comes from `GpuInUse`, with the same label as that GPU's entry, and the line follows `GpuInUse` when it changes.
    - It names the GPU instead of saying "the first in the list". The list moves the GPUs without a reading rule last, while the service's order (`Gpus`) doesn't, so the first in the list isn't always the one Automatic reads.
    - A GPU chosen by hand is named on the button, so the line is hidden then.
  - **The GPUs:** each entry is `<kind> · <product name>`, for example "Dedicated · Contoso Graphics R870". The product name comes from the system's PCI ID list (`hwdata`): the last bracketed part of the device's name there when it ends with one ("Chip 12 [Contoso Graphics R870]"), else the whole name. Without an entry there, the label is `<kind> · <driver> · <PCI address>`. Two GPUs whose labels would be the same each get ` · <PCI address>` after it (decisions log D36).
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

### Music

- **Peak dots:** a `GtkSwitch` ([The switches](#the-switches)), on by default. It calls `SetMusicPeaks`.
- **Bar fall** and **Peak fall:** two horizontal `GtkScale`s on a 0–4 adjustment (step 1, no value drawn), with five labelled marks: Slower, Slow, Moderate, Fast, Faster (`gtk_scale_add_mark`, verified). Each is 320 px wide (`width-request`), so a scale and the GPU dropdown end at the same x, `halign` start, `valign` start, so the title beside it lines up with the trough and the marks hang below ([Rows](#rows)).
  - Bar fall defaults to Moderate, Peak fall to Slow.
  - A change calls `SetMusicBarFall` or `SetMusicPeakFall` with the stop's name when the value lands on a new stop.
  - Snapping to whole stops during a drag (with `round-digits` 0 and a `change-value` handler) is **unverified**.
- **Hint:** "For every Music option except Dancer. Changes show on the next frame." In a mode without a spectrum, the change is only saved (music.md, point 7).

### Startup

- **Start at boot:** "For the whole PC". A `GtkSwitch` ([The switches](#the-switches)), on only when OpenDeepCool is ready to use, with a note under the row when one applies, and the card's hint: "When it's on, the display service starts at every boot, and the sound for the Music options at every login." (One sentence: the password is said by the "Turning on…" and "Turning off…" lines at the moment a flip asks for it, owner, 2026-11-18.)
- It's the last card, because it's flipped about once per PC. Its states, texts and flows are in [Start at boot](#start-at-boot).

### The switches

The two `GtkSwitch`es, Peak dots and Start at boot, are **40 × 22 px** (owner, 2026-11-18: "a little bit smaller"; GTK's own is 48 × 24, which Phase 11a Task 1 measured). Only they change: the rotation and unit toggle buttons keep their size, and the scales' knobs stay GTK's own.

- **The look.** A switch keeps its fill, border and states (the trough at the `--accent` fill while `:checked`, a `--muted` slider while insensitive, no `opacity`: decisions log D43). Only its size changes. The knob is the trough's height less 4 px, 18 px, the same relation as in Task 1's measured recipe (a 14 px slider in an 18 px switch), and the handle travels 18 px.
- **The stylesheet** gets two sizes in `app.css`, in the rules the switch already has. Colours stay tokens; the test that no colour literal or `opacity` appears outside `:root` still passes, because a length isn't either:

  ```css
  switch {
    min-width: 40px;
    min-height: 22px;
    background-color: var(--ctl-normal-fill);
    border: 1px solid var(--ctl-normal-border);
  }

  switch > slider {
    background-color: var(--fg);
    min-width: 18px;
    min-height: 18px;
  }
  ```

  The rules for `switch:checked`, `switch:disabled`, `switch:disabled:checked` and `switch:disabled > slider` stay as they are.
- **The size is the design, the numbers are the means.** docs.gtk.org's CSS Properties page lists `min-width` and `min-height` and doesn't say which box they measure, and Task 1 didn't record whether its 34 × 18 had the border of the app's stylesheet (**unverified**). The test pins the outcome: with the app's stylesheet loaded, the switch's minimum and natural size, in both directions, is 40 × 22 (`measure()` of the realised widget), so the implementer adjusts the four lengths until it is. Whether GTK lets the slider leave a gap inside the trough, or fills its height as GTK's own theme may do, is also **unverified**: the mockups draw the gap (1 px beside the 1 px border), and the app takes what GTK draws at an 18 px slider.
- **Why 40 × 22:**
  - GTK's default is 2 : 1. 40 × 22 is 1.8 : 1, so it reads as the same control, a sixth narrower and a twelfth lower: "a little smaller" is a change the eye sees at once without making a switch that looks like a checkbox.
  - 34 × 18 (Task 1's) is about a quarter off in both directions, and 18 px high is a quarter under the 24 px pointer target; 36 × 20 is a smaller step but still leaves a 4 px shortfall.
- **The pointer target** (WCAG 2.2 SC 2.5.8, Target Size (Minimum), AA). The criterion asks for 24 × 24 CSS px "except when" the target satisfies one of its exceptions; the **spacing** one reads: "Undersized targets (those less than 24 by 24 CSS pixels) are positioned so that if a 24 CSS pixel diameter circle is centered on the bounding box of each, the circles do not intersect another target or the circle for another undersized target" (w3.org, Understanding SC 2.5.8, read 2026-11-19).
  - The switch is 2 px short of 24 in height. A 24 px circle centred on its box stays inside its 40 px width and reaches 1 px above it and 1 px below it.
  - Nothing that takes a pointer lies within 1 px of it: the rows are 14 px apart (the card's spacing), a label beside it is no target, and the scale below the Peak dots switch hangs 14 px lower. So the spacing exception applies, and the AA level holds with no change to the hit area.
  - The icon buttons are 32 × 32, the toggle buttons and the dropdown are taller than 24, and a scale is GTK's own, so no other control relies on an exception.
- **Placement:** `halign` start in its row (Start at boot's was end), `valign` center ([Rows](#rows)).
- **The mockups** draw both switches at 40 × 22 with an 18 px knob, in every state: off, on, the handle ahead of the trough while a flip runs, and insensitive.

## Banners

A banner is a `GtkRevealer` (transition none) holding a `GtkBox`: an icon, a `GtkLabel`, and an optional button. GtkInfoBar is deprecated since GTK 4.10, and GTK's docs suggest exactly this construction (verified). Banners stack in this order:

| Banner | When | Text | Effect on the window |
|---|---|---|---|
| Cooler not connected (warning) | `CoolerState` is `not-found` | "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back." | Nothing is disabled. The preview keeps mirroring the service's picture, and its description adds "Not on the cooler: it isn't connected." ([Preview column](#preview-column)). Status "Cooler not connected" |
| Cooler, no access (error) | `CoolerState` is `no-access`: the service finds the cooler but can't open its device node, a permissions problem such as a missing udev rule (settings.md, Status; solution review P5-02) | "**The cooler is plugged in, but the service can't open it.** The udev rule that gives the service access is missing or not applied yet. Restart the PC; if that doesn't help, reinstall OpenDeepCool. The journal says more." | Nothing is disabled, and changes are saved. The preview keeps mirroring the service's picture, and its description adds "Not on the cooler: the service can't open it." Status "Cooler: no access", with a red dot. There's no button and no command, because both fixes are the user's own and the app can't do either |
| Cooler, sending fails (warning) | `CoolerState` has been `write-failed` for 2 s | "**The cooler is connected, but sending it the picture fails.** The service keeps trying by itself, and changes are still saved. The journal says more." | Nothing is disabled. The preview's description adds "Not on the cooler: sending it fails." Status "Cooler: sending fails", with a yellow dot |

- **No service banner** (since 2026-11-11, D10). While the bus name `io.github.dtmarangoni.OpenDeepCool1` has no owner, the window shows the [off screen](#the-off-screen): the gear is insensitive, the pages (and so Refresh) aren't shown, the preview is all unlit, and the status says "Cooler: unknown" and "Service off" or "Service not running". Its note and its terminal commands, `sudo systemctl enable --now opendeepcool.service` first, say what the banner said. The app watches `NameOwnerChanged` and connects by itself once the name has an owner ([Talking to the service](#talking-to-the-service)); it doesn't poll.
- **Size:** the banner is part of the window's content, above the screens and inside the window scroller ([Size and scrolling](#size-and-scrolling)). While it shows, it adds its own height to the window, about 44 px for one line and about 64 for two, and gives it back when it goes; where the window is at its cap, the pages' viewports give up that height instead, so the page scrolls a little more and the banner is never cut off. It never widens the window: its text is a wrapping label with `max-width-chars` 100 (about 740 px), and it rewraps to the room the window gives it. The banner doesn't scroll away with a page.
- **One cooler banner at a time,** chosen by `CoolerState` (dbus.md, State). The app reads `CoolerState`, never `CoolerConnected`, which stays in the state for other clients. While the service is down, there's no cooler banner, because the cooler's state is unknown then.
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

- **The control:** a frameless icon button at the right end of the Display page's [page bar](#window), at the top right of the page: a `GtkButton` with `icon-name` `view-refresh-symbolic`, the class `icon-button`, action `win.refresh`, accessible name "Refresh display", tooltip "Refresh the display: fresh readings, and the picture starts again". It is above the Load group and doesn't scroll away.
- It calls `Refresh()`: fresh readings, and the picture restarts from home (display.md, Mode switches).
- **F5 and Ctrl+R** do the same from either page while the service runs. The icon is on the Display page only, and the action is enabled in every state where the window shows its pages.
- A small confirmation label, "Refreshed", shows in the `GtkOverlay` around the window's content for about 1.5 s, without animation, at the bottom centre (`halign` center, `valign` end, 16 px from the bottom), so it never covers a control. A static picture often looks the same after a refresh, so the user needs this confirmation.
- Refresh shows only where OpenDeepCool can be used. It's on the Display page, so it isn't drawn when the app is [not available in this session](#not-available-in-this-session), nor on the [off screen](#the-off-screen) (the previous header kept it insensitive; the page has no place for a control that can't work), and its action is disabled in both states, so F5 and Ctrl+R do nothing there. If the service refuses it anyway (`NotAuthorized`, for example under an administrator's own polkit rule that allows reading but refuses changes), that's shown like any [refused change](#refused-changes).

## Refused changes

- A control shows a new value only once the service confirms it (`StateChanged`), so the app never shows a change the service refused.
- When the service refuses a change or a Refresh, the control stays on the saved value. The overlay label that says "Refreshed" says instead, with a warning icon, "Not changed." or "Not refreshed." followed by the reason, for about 5 s, without animation.
  - The reason is the service's error message. For `NotAuthorized` it's the app's own text: "This session may not change settings."
  - Screen readers get the same text (`gtk_accessible_announce`, since GTK 4.14, verified).
- There's no separate state for it. An administrator's own polkit rule that allows reading but refuses changes simply ends every change this way.

## Start at boot

**Purpose:** turn OpenDeepCool on or off for the whole PC (settings.md, Start at boot; owner, 2026-11-10). The package turns nothing on at install, as Arch's packages don't. The switch isn't a setting of the service: it shows and changes systemd's own state for two units, the display service `opendeepcool.service` and the sound helper `opendeepcool-audio.service`, which runs in each user's session. It's on only when OpenDeepCool is ready to use (owner, 2026-11-11), and a flip on always finishes the setup and starts what isn't running.

### Where it sits

- On the Configuration page, in the last card, **Startup** (D9).
- While the service isn't running, on the [off screen](#the-off-screen), where the pages would be (D10). Configuration can't be reached then, and a new install is exactly that state.
- Never when the app is [not available in this session](#not-available-in-this-session). While the service is down in a session that logind reports as remote or inactive, the off screen has no Startup card ([Who may flip it](#who-may-flip-it)).
- Exactly one Start at boot switch is visible at a time. One row widget moved between the card and the body keeps one name for AT-SPI, which is simpler for the tests than two bound to one model; the plan chooses.

### Where its state comes from

- **Four facts** (owner, 2026-11-11: on means ready to use), read from systemd without a password: "Read access is generally granted to all clients" (org.freedesktop.systemd1(5), read 2026-11-11). None goes through the OpenDeepCool service, so the switch works while it's down.

  | Fact | Read with | Holds when |
  |---|---|---|
  | The display service starts at boot | `GetUnitFileState("opendeepcool.service")` on the system manager (system bus) | `enabled` |
  | The display service runs | the unit's `ActiveState` (and `SubState`) on the system manager, through `LoadUnit`: `GetUnit` fails for a unit systemd hasn't loaded, and `LoadUnit` "will load the unit from disk if possible" (org.freedesktop.systemd1(5)) | `active` or `reloading`; or `activating` while its bus name already has an owner (below) |
  | The sound starts at every login | `GetUnitFileState("opendeepcool-audio.service")` on the user's own manager (session bus), which reports a `--global` link as `enabled` | `enabled` |
  | The sound runs in this session | the unit's `ActiveState` on the user's own manager | `active` or `reloading` |

- **On** when all four hold. **Off** otherwise, with a [note](#the-note) saying what's missing (D11).
- **Unknown** when a read fails or doesn't answer, or when a unit-file state is neither `enabled` nor `disabled` (`masked`, `enabled-runtime`, `linked`, `bad` and so on; systemctl(1), is-enabled). A unit that isn't installed fails the read ("No such file or directory").
- **Only these two units count.** The one-shot helpers the display service starts (`Wants=`: the GPU ranking and the sensor detection) run and exit, so they don't count (settings.md, Start at boot).
- **How "not running" reads,** for the display service's note, from its `ActiveState` (values: org.freedesktop.systemd1(5), Table 1) and `SubState` (strings: systemd's `src/basic/unit-def.c`):
  - **stopped:** `inactive`, `deactivating` or `maintenance`;
  - **failed:** `failed`, or `activating` with `SubState` `auto-restart` or `auto-restart-queued`. That's a service waiting to restart after a failure, which systemd reports as `activating` (`src/core/service.c`, its state table; read 2026-11-11). With `Restart=` and no start limit (the unit's `StartLimitIntervalSec=0`), a crashing service cycles through it and never stays `failed`;
  - **starting:** `activating` with any other `SubState`;
  - `refreshing` is a kind of `active`, and counts as running.
- **Activating, with its name already owned:** the service takes its bus name just before it reports ready (`READY=1`; `crates/odc-service/src/dbus.rs`). So a read made right after the name's change can find it `activating` for a moment. It counts as running then; otherwise the switch would stay off, reading "starting", until the next trigger.
- **When it's read:** at start, together with the first connection to the service; after each flip; whenever the window becomes active (`notify::is-active`); and whenever the service's bus name gets or loses an owner (`NameOwnerChanged`). A service that stops shows through its name's change.
  - After the app starts or stops the sound in this session, it reads again once systemd's job has ended. `StartUnit` and `StopUnit` return a job, whose end is the `JobRemoved` signal ("sent out each time a new job is queued or dequeued"). A manager sends its signals only once some client has called `Subscribe()` (org.freedesktop.systemd1(5)). The plan chooses how the app waits; `Subscribe()` on the user's own manager, for that job alone, is the documented way.
  - There's no polling, and no `Subscribe()` on the system manager ([Decisions](#decisions)). A change made in a terminal shows once the user is back in the window. A sound helper that stops or restarts by itself in this session shows at the next trigger.
- **At start,** while the service is down, the off screen's heading and note wait for the four reads, so neither flashes. A read that hasn't answered within 1 s counts as failed.
- The user's own manager reports a `--global` link (in `/etc/systemd/user/`) as `enabled`: checked on the development PC, 2026-11-11, with the dev copy's sound helper, whose link is in `/etc/systemd/user/default.target.wants/` (`GetUnitFileState` on the user bus answered `enabled`). The package's layout, with the unit file under `/usr/lib/systemd/user/`, is checked again on the owner's PC in Phase 11 Task 7. Had it not, the app would need another read of the global state, and the design would stay the same.

### The switch

- A `GtkSwitch` with GTK's delayed state, as the peak dots switch has (decisions log D63). Its `active` property is the handle, the user's latest flip; its `state` property is the trough's colour, systemd's state. GTK's docs: "the slider position indicates the user's recent change", "the trough color indicates the present underlying state" (docs.gtk.org, GtkSwitch, read 2026-11-11).
  - GTK's source (`gtk/gtkswitch.c`, read 2026-11-11): `gtk_switch_set_state` sets or clears the `:checked` state, which the app's stylesheet draws with the `--accent` trough; `gtk_switch_set_active` emits `state-set` and sets the accessible checked state, so a screen reader reads the handle.
  - The `state-set` handler starts the flip and returns `TRUE` (stop), so the trough keeps systemd's state ("The signal handler should return TRUE to prevent the default handler from running", docs.gtk.org, GtkSwitch::state-set). When the flip ends, the app sets `state` and `active` from what systemd reports. Setting `active` from the app emits `state-set` again, so the app ignores the signal while it sets the switch itself, as it does for the peak dots switch.
- **The trough** (`state`) is on only when all four facts hold; **the handle** (`active`) is the latest flip. A flip from off always goes to on, and from on to off.
- **Insensitive** while a flip runs, while the state is unknown, and while the display service is starting. The stylesheet draws an insensitive switch with the normal fill and border and a `--muted` slider, without `opacity` (decisions log D43); a checked one keeps the `--accent` trough, so a running flip's "off / on" (the trough at systemd's state) still shows (Phase 11 Task 3, as built).
- The handle's slide is GTK's own and follows the user's `gtk-enable-animations` setting, as the peak dots switch does.
- **Row:** a label block, the title "Start at boot" with the muted subtitle "For the whole PC" under it, and the switch beside it, left-aligned, 24 px from the block ([Rows](#rows): `halign` start, `valign` center). The switch is 40 × 22 ([The switches](#the-switches)). Under the row, the note, when one applies. At the card's foot, the hint: "When it's on, the display service starts at every boot, and the sound for the Music options at every login." It no longer says that each change asks for the administrator's password (owner, 2026-11-18). The "Turning on…" and "Turning off…" lines say so at the moment a flip asks, and the desktop's own dialog is the authority.

### The note

When the switch reads off and something is set up, one note under the row says what's missing (D11). It's built from at most two phrases, one for each unit:

| The display service | Phrase |
|---|---|
| starts at boot and runs | none |
| neither starts at boot nor runs | none, and no note at all: OpenDeepCool is off, which the off screen's heading says |
| runs, but doesn't start at boot | "the display service runs now, but doesn't start at boot" |
| starts at boot, stopped | "the display service is stopped" |
| starts at boot, failed or waiting to restart | "the display service failed" |
| starts at boot, starting | none: the note is "The display service is starting." alone, in the information style, and the switch is insensitive until it has started or failed |

| The sound | Phrase |
|---|---|
| starts at every login and runs in this session | none |
| neither | "the sound for Music is off" |
| runs in this session, but doesn't start at login | "the sound for Music runs here, but doesn't start at login" |
| starts at every login, but isn't running in this session | "the sound for Music isn't running in this session" |

- **The sentence:** "Not fully on: ‹phrase›." or "Not fully on: ‹display phrase›, and ‹sound phrase›.", then one ending:
  - "Turn it on to start it, with no password." when the only phrase is "the sound for Music isn't running in this session": the flip then only starts it ([A flip, step by step](#a-flip-step-by-step));
  - "Turn it on to finish." otherwise.
  - After "the display service failed", one more sentence: "The journal says why the service failed."
- **Examples:**
  - after only `sudo systemctl start opendeepcool.service`: "Not fully on: the display service runs now, but doesn't start at boot, and the sound for Music is off. Turn it on to finish.";
  - a crashing service, set up in full: "Not fully on: the display service failed. Turn it on to finish. The journal says why the service failed.";
  - after only the two `sudo` commands, until the next login: "Not fully on: the sound for Music isn't running in this session. Turn it on to start it, with no password."
- **When the display service is off** and only the sound is set up (someone ran just the `--global` command), there's no note: the off screen says OpenDeepCool is off, and a flip sets everything up.
- **In the warning style** (`--yellow`, the warning icon), as the other "something's missing" hints, except "The display service is starting.", which is information (`--muted`, the information icon).

### States and their text

| State | Handle / trough / sensitive | Note under the row | Terminal commands |
|---|---|---|---|
| On | on / on / yes | none | none |
| Off, nothing set up (a new install, or turned off) | off / off / yes | none: the off screen's heading says "OpenDeepCool is off" | on the off screen, the on commands always under the card |
| Not fully on | off / off / yes | warning: [the note](#the-note) | none in the card (on the off screen, always under it) |
| The display service starting | off / off / no | information: "The display service is starting." | none |
| Unknown: a read failed | off / off / no | information: "Can't tell whether OpenDeepCool is on: reading it from systemd failed." | "To turn it on from a terminal:", the on commands |
| Unknown: another unit state | off / off / no | information: "Can't change it here: systemd reports opendeepcool.service as “masked”." (the unit and its state as systemd gives them) | none: `enable` fails on a masked unit, and an administrator who set such a state knows its command |
| Turning on | on / off / no | "Turning on… Your desktop asks for the administrator's password first." (foreground, no icon) | none |
| Turning on, only the sound in this session missing | on / off / no | "Turning on… Starting the sound for Music in this session." | none |
| Turning off (only from on) | off / on / no | "Turning off… Your desktop asks for the administrator's password first." | none |
| Not changed: dialog dismissed (pkexec 126) | systemd's state / sensitive | warning: "Not changed: the password dialog was closed." | "From a terminal instead:", the commands for the flip's direction |
| Not changed: not authorized, or no polkit agent (pkexec 127) | systemd's state / sensitive | warning: "Not changed: the password wasn't accepted, or no password dialog could open in this session." | the same |
| Didn't finish: a step failed (the program's exit 1; any exit code but 0, 126 and 127 alike) | systemd's state / sensitive | warning: "Didn't finish: ‹the program's message›. The switch shows what's set now; the journal says more." | the same |
| Didn't finish: the sound didn't start in this session (the app's own step, after `on` or as the flip without a password) | off (the sound isn't running here) / sensitive | warning: "Didn't finish: the sound for Music didn't start in this session. It starts at your next login; the journal says more." | the on commands |
| Turned off, but the sound didn't stop in this session | off / sensitive | warning: "Off, but the sound for Music didn't stop in this session. It stops when you log out." | none: the change for the whole PC was made |

- **Which note shows,** when more than one applies: a running flip's line, then a failure's note, then unknown, then "starting", then the missing parts. A failure's note replaces the missing-parts note, which comes back when the failure's note goes.

- **The commands:**
  - to turn on (settings.md): `sudo systemctl enable --now opendeepcool.service`, `sudo systemctl --global enable opendeepcool-audio.service`, and `systemctl --user start opendeepcool-audio.service`, which starts the sound for Music in this session at once, without a password (owner, 2026-11-11: the third line, so Music isn't silent until the next login);
  - to turn off: `sudo systemctl disable --now opendeepcool.service` and `sudo systemctl --global disable opendeepcool-audio.service` (owner, 2026-11-11: the commands for the flip's direction, since the on commands would undo what the user wanted).
- **How long a note stays:** a failure's note stays until the next flip, or until a read shows the state the failed flip wanted (the user typed the commands), and then goes. The missing-parts, starting and unknown notes follow the state.
- **pkexec's codes** (pkexec(1), read 2026-11-11): 126 is only "the user dismissed the authentication dialog"; 127 is "not authorized, or an authorization could not be obtained through authentication, or an error occurred", so a refused password, no polkit agent and any other pkexec error all end with 127. The 127 sentence covers every one of those causes, so no text depends on pkexec's messages, whose wording is **unverified**. Its stderr goes to the app's log only.
- **The program's codes** (Phase 11 plan, Task 3): `opendeepcool-boot` exits 2 when it refuses its arguments or its caller, and 1 when a `systemctl` step fails, naming the step in one line on stderr. The app shows that line after "Didn't finish:", without the program's name in front and without a trailing period. The program's commit set the wording: "step 3 of 3, `systemctl start opendeepcool.service`, failed: exit status: 1", or, when one of systemd's two checks can't run, the check's path and the error. An exit 2 can come only from a fault in the app, and shows the same way.
- **No toast on success:** the switch and the window are the confirmation. Screen readers get "Start at boot is on." or "Start at boot is off." at the end of a flip, and a note's text when one appears (`gtk_accessible_announce`, since GTK 4.14, verified).

### The terminal commands

- A `GtkBox` (group "Terminal commands") with a muted `GtkLabel` ("From a terminal instead:" or "To turn it on from a terminal:"), a selectable monospace `GtkLabel` holding the commands, one a line (three to turn on, two to turn off), on the `--code-bg` fill, which wraps in a narrow window, and a "Copy" `GtkButton` that copies every line (`clipboard().set_text`, as the Phase 10 service banner's Copy did).
- On the Configuration page it shows under a failure's note and with the unknown note. On the off screen it's always under the card, with the on commands and "From a terminal instead:", so a note there shows only its sentence.

### A flip, step by step

1. The user flips the switch (a click, or Space). The `state-set` handler makes the switch insensitive, leaves the trough at systemd's state, and shows the line for the flip ([States and their text](#states-and-their-text)). The app holds itself (`g_application_hold`, docs.gtk.org, read 2026-11-11) until the flip ends, so a closed window doesn't cut it short.
2. **On** (from off, whatever is missing):
   - when the only part missing is the sound running in this session, the app starts it through the user's own manager (`StartUnit`), with no `pkexec` and no password, since nothing changes for the whole PC (owner, 2026-11-11);
   - otherwise it runs `pkexec --disable-internal-agent /usr/lib/opendeepcool/opendeepcool-boot on` as a subprocess, without blocking the main loop. The program enables both units and starts the display service, which does nothing to a part already set (Phase 11 plan). The desktop's polkit agent shows its own password dialog, which the app doesn't draw; the app sets no time limit on it. When pkexec ends with 0, the app starts the sound in this session.

   **Off** (only from on): `pkexec --disable-internal-agent /usr/lib/opendeepcool/opendeepcool-boot off`, which disables both units and stops the display service; when it ends with 0, the app stops the sound in this session.
3. When pkexec ends with any code but 0, the failure's note appears, and the app's own step is skipped.
4. The app reads the four facts again, once the app's own step's job has ended, and the switch takes what systemd reports, sensitive again.
5. Focus goes back to the switch when it had it, or, when the window changed screens meanwhile, to the switch on the new screen.

- **`--disable-internal-agent`** (owner, 2026-11-11): pkexec(1) says that with no agent, pkexec otherwise "will register its own textual authentication agent". That agent would ask on the terminal the app was started from, if it has one, where nobody looks, and the switch would wait there; that terminal behaviour is **unverified**, and the option avoids it either way. With it, a missing agent ends at once with 127. The fake `pkexec` of the layer 4 tests expects the argument.

### The window around a flip

- **While the service isn't running,** the window shows the off screen. Its heading is "OpenDeepCool is off" while the display service doesn't start at boot or a flip toward off runs, and "OpenDeepCool isn't running" otherwise. So:
  - **turning on from the off screen:** the off screen stays, with the switch changing, while the dialog is open and the program runs. The unit is `Type=notify`, so the program's `systemctl start` returns once the service says it's ready. The service's bus name gets an owner, the app connects by itself, and the normal window appears on the page last shown, the Display page after a new install. After a successful `on`, the flip counts as running until the bus name has an owner or 5 s have passed, so the heading and the note can't flash to "isn't running" in between;
  - **turning off from the Configuration page:** the service stops while the program runs. The window turns to the off screen at once, headed "OpenDeepCool is off", and the switch finishes changing there;
  - **a service that stops or crashes** while the window shows the pages: its name loses its owner, the window turns to the off screen headed "OpenDeepCool isn't running", and the note says "stopped" or "failed". A crashing service comes back by itself (`Restart=on-failure`), and the window with it.
- While the service is down the gear stays visible but insensitive, and Refresh isn't drawn (it belongs to the Display page, which the off screen replaces). Their shortcuts do nothing.

### The off screen

```
┌ GtkApplicationWindow ──────────────────────────────────────────────────────┐
│ GtkHeaderBar                    OpenDeepCool             [⚙ dimmed] [✕]   │
├──────────────────────┬─────────────────────────────────────────────────────┤
│ Preview, all unlit   │      OpenDeepCool is off  (or: … isn't running)     │
│ No picture           │   The cooler shows nothing until the display        │
│ ○ Cooler: unknown    │   service runs.                                     │
│ ○ Service off        │  ┌───────────────────────────────────────────────┐  │
│   (● Service not     │  │ Start at boot                       ( ○    )  │  │
│    running, red)     │  │ For the whole PC                              │  │
│                      │  │ ⚠ Not fully on: the display service failed. … │  │
│                      │  │ When it's on, the display service starts …    │  │
│                      │  └───────────────────────────────────────────────┘  │
│                      │  From a terminal instead:                           │
│                      │  sudo systemctl enable --now opendeepcool.service   │
│                      │  sudo systemctl --global enable opendeepcool-aud…   │
│                      │  systemctl --user start opendeepcool-audio.service  │
│                      │                                          [Copy]     │
└──────────────────────┴─────────────────────────────────────────────────────┘
```

- **When:** the service's bus name has no owner, in every case (D10). A new install opens on it.
- **Heading,** one of two:
  - "OpenDeepCool is off" while the display service doesn't start at boot, or a flip toward off runs. The status reads "Service off", with a hollow dot: off on purpose, not a fault;
  - "OpenDeepCool isn't running" otherwise: it's set to start at boot but is stopped, failed or starting, or the state is unknown. The status reads "Service not running", with a red dot.
- **Text** under the heading, in `--muted`, the same for both: "The cooler shows nothing until the display service runs."
- **Then** the Startup card without its heading, with [the note](#the-note) when one applies (a new install has none), and the terminal commands to turn it on, "From a terminal instead:". Its row is the same label block and switch as on the Configuration page, left-aligned in the card; the block is at its own width here, since the card leaves the page's size group on the off screen ([Rows](#rows)).
- **No banner.** The note says what's wrong, and the switch is the fix.
- **Widgets:** a vertical `GtkBox` centred in the content area, holding a `GtkBox` (group "Service message") around the heading and text `GtkLabel`s (wrapping, centred), then the card, then the terminal commands. The off screen is the `off` child of the `pages` stack, in its own `GtkScrolledWindow` (a page scroller: [Size and scrolling](#size-and-scrolling)) and without a page bar. **Its size:** it is smaller than the other pages and the stack is homogeneous, so it doesn't change the window: the window keeps the size it had with the pages, the off screen stands centred in it, and when the service comes back the page appears in the same window with nothing moving. Its heading and text wrap at 48 characters (`max-width-chars`) and its card and commands set a width of about 560 px, under the Display page's. The header bar shows the title, the insensitive gear and the close icon ([the header, by state](#window)); the preview column is as before while the service is down.
- **Focus:** the switch takes the first focus when it's sensitive, so Space turns OpenDeepCool on. The gear is insensitive here, so a page that was closed by the service going down gives its focus to this switch, not to the gear.

### Who may flip it

- Only the user of the active local session, as with every change: the polkit action `io.github.dtmarangoni.OpenDeepCool1.start-at-boot` has `allow_active` `auth_admin` (the administrator's password at every change, no keep) and `allow_any` and `allow_inactive` `no` (Phase 11 plan). It's the only password in the app.
- When the app is [not available in this session](#not-available-in-this-session), the window shows the message alone, as before.
- While the service is down, the app can't ask it about the session, but logind can tell: its session's `Active` and `Remote` properties (org.freedesktop.login1(5), read 2026-11-11). The app already follows `Active` to connect again ([Unverified points](#unverified-points-answered-phase-10-task-1-2026-11-06)). In a session that isn't both active and local, the off screen has no Startup card, since polkit would refuse the flip there: only its heading, its text and the terminal commands.

## Not available in this session

**When:** the service refuses `GetState()` with `NotAuthorized`, because the app runs in a remote or inactive session (owner, 2026-10-26). The service refuses such a session every read, change and preview frame (polkit `allow_active` only), so the app has nothing to show. This state replaces the view-only state, which no longer exists.

```
┌ GtkApplicationWindow ───────────────────────────┐
│ GtkHeaderBar     OpenDeepCool              [✕]  │
├─────────────────────────────────────────────────┤
│                                                 │
│       OpenDeepCool can be used only from        │
│              the session at the PC              │
│   This session is remote or not the active one. │
│                                                 │
└─────────────────────────────────────────────────┘
```

- **Size:** the window is as small as its message, about 440 × 170 px, not the size of the full window. The message is in the `screens` stack, which isn't homogeneous, so the window takes the size of whichever screen shows ([Size and scrolling](#size-and-scrolling)). The app presents the window before the service has answered, with the main screen, so in a session that can't use OpenDeepCool the window is first mapped at its full size and shrinks to the message when the service refuses, a few milliseconds later, with no animation. (Should that flash be visible, presenting the window after the first answer is the fallback; it isn't needed in the normal case, where the same size serves the off screen and the pages.) When the session becomes usable, the window grows to its full size. It stays not resizable by the user, and capped to the screen.
- **Text:** the heading "OpenDeepCool can be used only from the session at the PC", and under it, in `--muted`, "This session is remote or not the active one."
- **Widgets**, all plain GTK4 (no libadwaita, so no `AdwStatusPage`):
  - the same `GtkHeaderBar`, with the title and the close icon only. The gear is hidden (`visible` false), not just insensitive; Refresh is on the Display page, which isn't shown. The close icon is the app's own: the desktop's window buttons are off ([Window](#window));
  - the body is a vertical `GtkBox`, centred in the window (`halign` and `valign` center) with 24 px of margin all round, holding two `GtkLabel`s, the heading and the text. Both wrap (`wrap` true, `justify` center) at 40 characters (`max-width-chars`), which is what sets the window's width here. They look like the off screen's heading and text;
  - nothing else: no banner, no preview column, no pages, no status, nothing that could show a value.
- **Keyboard and screen readers:** nothing in the content takes focus, and in the window only the close icon does. Ctrl+W and Ctrl+Q still close the window; the Configuration, back and Refresh shortcuts do nothing, because their actions are disabled. On entering the state, the app announces the heading and text once (`gtk_accessible_announce`, since GTK 4.14, verified).
- **Trying again,** without polling: when the window becomes active again (`notify::is-active` on `GtkWindow:is-active`, verified) and when the service's bus name gets a new owner (`NameOwnerChanged`). If `GetState()` succeeds then, the normal screens appear with the current state.
- **The service not running comes first.** Without the service, the app can't tell whether this session may use OpenDeepCool, so it shows the service state, which holds no settings and no picture. In a session logind reports as remote or inactive, that state, the off screen, has no Start at boot switch ([Who may flip it](#who-may-flip-it)).
- **An administrator's own polkit rule** might allow reading but refuse changes. That's no separate state: the app shows the normal screens, and each change fails with `NotAuthorized`, shown like any [refused change](#refused-changes).

## Talking to the service

The D-Bus system bus (settings.md, Status). Only the user of the active local session can use OpenDeepCool: the service refuses every other session each read, watch and change (polkit `allow_active` only; owner, 2026-10-26). It offers no D-Bus properties, and nothing is broadcast.

- **Connecting:** the app starts watching the state (`Watch()`) and reads all of it (`GetState()`). The plan fixes the order, so that no change falls between the two. Then the window shows:
  - the [off screen](#the-off-screen), when the bus name has no owner;
  - [not available in this session](#not-available-in-this-session), when the service refuses with `NotAuthorized`;
  - otherwise, the normal screens with that state.
- **Connecting again,** one call at a time and never polling: whenever the bus name gets a new owner (`NameOwnerChanged`), and whenever the window becomes active (`notify::is-active`). This also renews any watch the service dropped meanwhile, for example while this session was inactive.
- **Updates** come as `StateChanged` signals addressed to this app alone, and preview frames the same way after `WatchFrames()`, only while the window is mapped ([Preview column](#preview-column)).
- **State names:** `CoolerState`, `AudioConnected`, `UnavailableReasons`, `Gpus` and `ChosenGpu` in this document are the names in the Phase 7–9 contract, and `GpuInUse` is the Phase 10 plan's (Task 2). They come in `GetState()` and `StateChanged`, not as properties, and the Phase 10 plan sets their final shape.
  - The cooler's part of the window follows `CoolerState` (`connected`, `not-found`, `no-access`, `write-failed`; [Banners](#banners)). `CoolerConnected` stays in the state, but the app doesn't read it.
  - The GPU row reads `Gpus` (its kinds include `query-failed`), `ChosenGpu` (empty for Automatic) and `GpuInUse` ([Display](#display)).
- **Start at boot** reads systemd, not the service, so it works while the service is down ([Where its state comes from](#where-its-state-comes-from)).
- **Back from standby** (owner, 2026-10-26): no screen of its own, and nothing for the user to do. While the monitor is off or the screen is locked, the session stays active, so nothing changes for the app. After a sleep, the service takes up the cooler again by itself (PRD, Service model), and the preview follows the cooler as soon as frames come back. If the cooler or the sound takes a moment to return, the cooler banner and the sound status show it as usual.

## States

| State | What the app shows | Mockup |
|---|---|---|
| Normal, several GPUs | Header: the title in the middle, the gear and the close icon at the right. Display page: its page bar ("Display", the Refresh icon at the right) and the tile gallery. The preview column: the picture, the caption ("Temperature · CPU + GPU") and the status items "Cooler connected" and "Service running", with no line between the caption and the status. Configuration (the gear pressed): its page bar (the back icon, "Configuration"), then Rotation, Temperature unit and the GPU on Automatic with "Using Dedicated · Contoso Graphics R870." under it, each label beside its control and the controls in one column, and the Peak dots and Start at boot switches at 40 × 22 | [display.html](mockups/display.html), [configuration.html](mockups/configuration.html) |
| The Configuration page is open | The gear is pressed; the back icon, Escape or Alt+Left, or the gear again, return to the Display page, and focus goes to the gear | [configuration.html](mockups/configuration.html): click the back icon, the gear, or press Escape |
| The temperature unit | A row of the Display card after Rotation; the temperature tiles on the Display page follow it | [configuration.html](mockups/configuration.html), [display-scrolling.html](mockups/display-scrolling.html) (the tiles show °F) |
| GPU list open | Automatic first; a GPU whose kind query failed as "Unknown type · …" with "driver couldn't be asked"; a GPU without a reading rule last, with "no readings"; the line under the dropdown names the integrated GPU, which Automatic reads here | [configuration-gpu-list.html](mockups/configuration-gpu-list.html) |
| A GPU added or removed since the last start | The choice is back on Automatic (the service cleared it), with no note; the line names the GPU in use | [configuration.html](mockups/configuration.html) |
| A Music mode | The status line adds "Sound connected" or "No sound: spectrum is silent" | [display-music.html](mockups/display-music.html) |
| A value of 100 or more, °F | The preview mirrors the scrolling strip; the temperature tiles show °F | [display-scrolling.html](mockups/display-scrolling.html) |
| First start | Load · CPU, 0°, °C, peak dots on, Moderate / Slow, the GPU on Automatic | [first-start.html](mockups/first-start.html) |
| Cooler disconnected (`not-found`) | Cooler banner; status "Cooler not connected"; the preview's description adds "Not on the cooler: it isn't connected."; the column has the same lines as when connected; everything still changeable | [cooler-disconnected.html](mockups/cooler-disconnected.html) |
| Cooler found, no access (`no-access`) | Error banner with the fix; status "Cooler: no access"; the preview's description adds "Not on the cooler: the service can't open it."; everything still changeable | [cooler-no-access.html](mockups/cooler-no-access.html) |
| Sending to the cooler fails, 2 s or more (`write-failed`) | Warning banner; status "Cooler: sending fails"; the preview's description adds "Not on the cooler: sending it fails."; everything still changeable | [cooler-write-failed.html](mockups/cooler-write-failed.html) |
| Sending fails, under 2 s (one bad write, or the cooler being unplugged) | As connected: no banner, until the state changes or 2 s pass | (as display) |
| Service not running, set to start at boot (stopped, failed or restarting) | The off screen headed "OpenDeepCool isn't running", status "Service not running" (red); no banner; the switch off, with "Not fully on: the display service failed. Turn it on to finish. The journal says why the service failed."; the terminal commands under the card; the header keeps the title and the close icon, with the gear visible but insensitive, and there's no Refresh (it belongs to the Display page); nothing else changeable | [service-not-running.html](mockups/service-not-running.html) |
| The same, just stopped | As above, with "the display service is stopped" and no journal sentence | (as service-not-running) |
| The display service starting (at boot or login, before it's ready) | As above, with "The display service is starting." and the switch insensitive | (as service-not-running) |
| OpenDeepCool is off (a new install, or turned off) | The off screen headed "OpenDeepCool is off": no banner, "Service off", the switch off with no note, the terminal commands under it. Flip it in the mockup to see the window turn on | [boot-off.html](mockups/boot-off.html) |
| Turning on (the password dialog is open, or the program runs) | The handle on, the trough off, the switch insensitive; "Turning on… Your desktop asks for the administrator's password first." | [boot-changing.html](mockups/boot-changing.html) |
| Turning on, only the sound in this session missing | As boot-changing, in the Configuration card, with "Turning on… Starting the sound for Music in this session."; no password | (as boot-changing) |
| Turning off | The same, the other way; the window turns to the off screen as the service stops | (as boot-changing) |
| Not turned on: password not accepted, or no polkit agent (127) | The switch back at off; the reason under it; the commands below | [boot-not-changed.html](mockups/boot-not-changed.html) |
| Not changed: dialog closed (126) | As 127, with "Not changed: the password dialog was closed." | (as boot-not-changed) |
| A step failed (exit 1) | The switch at systemd's state; "Didn't finish: …" under it; the commands for the flip's direction | (as boot-not-changed, or in the Configuration card with the commands under the note) |
| Not fully on, the service running (the Configuration page) | The switch off, with the note naming what's missing; one flip finishes it | [boot-partly.html](mockups/boot-partly.html) |
| Not fully on: only the sound in this session | The note ends "Turn it on to start it, with no password." | (as boot-partly) |
| Start at boot unknown | The switch insensitive at off; the reason; the on commands | [boot-unknown.html](mockups/boot-unknown.html) |
| Start at boot on, normal window | The Startup card, last on the Configuration page | [configuration.html](mockups/configuration.html) |
| Service down in a remote or inactive session | The off screen without the Startup card: its heading, text and the terminal commands | (as service-not-running, without the card) |
| Metric unavailable | Tiles disabled with "Unavailable"; each group lists its reasons once | [unavailable-metric.html](mockups/unavailable-metric.html) |
| The saved mode lost a reading | Tile still selected with "Reading lost"; dashes in the preview; reason under the preview | [reading-lost.html](mockups/reading-lost.html) |
| Not available in this session (remote or inactive) | Only one message: no values, no preview, no controls, no gear and no Refresh. The header keeps the title and the close icon | [not-available.html](mockups/not-available.html) |
| One GPU | No GPU row at all | [one-gpu.html](mockups/one-gpu.html) |
| A GPU without a reading rule in use | GPU tiles disabled with "no reading for this GPU's driver"; a hint under the GPU dropdown | [configuration.html](mockups/configuration.html): choose "Unknown type · othergpu · 0000:07:00.0" |
| Another `CoolerState` (the dev copy's `released`, or a later value) | As `not-found` | (as cooler-disconnected) |
| Short read failure, first second | Nothing disabled; the preview shows the cooler's dashes | (as reading-lost, without the reason line) |
| A change refused | The control stays on the saved value; the overlay label, at the bottom centre, says "Not changed." with the reason | (no mockup: Refresh's label, with a warning icon) |
| Back from standby | No screen of its own; the preview follows the cooler again when frames come back | — ([Talking to the service](#talking-to-the-service)) |
| A large screen | The window opens at the natural size of its content (estimated about 1016 × 814 px: the widest page beside the 262 px column, under the tallest page), with no scrollbar; it doesn't change size when the page changes or the service stops. Empty space stands under the Display page's tiles | every mockup: drawn at the natural size (930 px wide with the mockups' smaller tiles, and about 813 high) |
| A small screen | The window is never larger than the smallest monitor less 48 px in each direction. The pages scroll down under their page bars, with the preview column and the banner in place; the whole content scrolls sideways below about 850 px; the column scrolls by itself on a screen under about 540 px high | every mockup: a browser window shorter than the mockup scrolls the page inside it (only the page scrolls sideways there, not the whole content) |
| Not resizable | `resizable` false: no resizing or maximising by the user, and a compositor that floats fixed-size windows floats it; on one that tiles it anyway, the window is a tile that scrolls inside, and nothing breaks | — |
| The monitors change while the app runs | The window keeps its size; the cap is read once. On a smaller screen the compositor decides whether to re-fit it; reopening the app fits it | — |
| A banner, a note or the terminal commands appear | The window grows by their height, or the page scrolls a little more where the window is at its cap; the width never changes | cooler-disconnected, cooler-no-access, boot-partly |
| Not available, drawn small | The window is as small as its message (about 440 × 170 px), and grows when the service answers | [not-available.html](mockups/not-available.html) |
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
  - A `GFileMonitor` on `~/.local/state/omarchy/current/` reacts when `theme` is replaced (the PRD, Theme: themes are moved over `current/theme`; Omarchy stages the new one in `current/next-theme`, removes `current/theme` and moves it in, its `omarchy-theme-set`, read 2026-11-06). A second one on `current/theme/` catches an edit in place, and while a folder of that path is missing, the nearest existing folder above it is watched, so a theme that appears later is found (decisions log D65).
  - After 150 ms of quiet, the app re-reads `colors.toml` and `shell.toml` and, when the tokens changed, regenerates the string, reloads the same provider, and repaints the preview and tiles. There's no restart, and the page and scroll position are kept. The source at start and each reload are logged (`theme: the Omarchy theme`, `theme: the default theme`, `theme: reloaded, …`), with each missing or rejected key.
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
| `--bg` | `colors.toml` `background` | window, header bar, banner base, the resting fill of the frameless icon buttons ([Window](#window)) |
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

- **Computed tokens**, from the fixed alphas of the mockups: `--line` (`--fg` at 0.12: header, column and banner rules), `--code-bg` (`--fg` at 0.08: the start command), `--unlit` (`--fg` at 0.12: unlit preview dots), `--banner-error` and `--banner-warn` (`--red` at 0.16 and `--yellow` at 0.14 over `--bg`, opaque).
- **Values the tables leave open** (decisions log D30–D32, D39, D43):
  - a border width is CSS's four sides, as `shell.toml`'s comment allows (`N`, `"Y X"`, `"T X B"`, `"T R B L"`, with `-top`, `-right`, `-bottom` and `-left` keys over it), each from 0 to 16 px;
  - a missing or rejected `shell.toml` colour key takes the theme's own `--fg`, as Omarchy's template writes them, and a missing alpha the default theme's; `--ctl-pressed-fill` uses `normal-color`;
  - the default theme's focus fill and border are its hover ones, its selected border is `--fg` opaque, its border widths 1 px, and its `--selection` is `--accent` at 0.35 over `--bg`;
  - a `colors.toml` colour's own alpha is dropped, so the window stays opaque; a `shell.toml` colour's own alpha is multiplied by its `*-alpha` key;
  - a disabled control has muted text on the normal fill and border, since the CSS has no `opacity`.
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

- **Tab order:** the gear → the close icon → the banner button (when present) → the page bar's icon (Refresh on the Display page, the back icon on the Configuration page) → page content. On the Display page: each tile group in turn. On the Configuration page: rotation, temperature unit, GPU, peak dots, bar fall, peak fall, Start at boot, then Copy of the terminal commands when they show. While the service is down, on the off screen: the gear is insensitive and takes no focus, so the order is the close icon, then Start at boot, which takes the first focus when it's sensitive, then the commands' Copy.
- **Where focus lands on a page change:** see [Window](#window), The pages. Opening the Configuration page puts it on the back icon; closing the page puts it on the gear. Nothing else moves it.
- **The scrolled windows take no focus** (`focusable` false on the window scroller, the column and every page, [Size and scrolling](#size-and-scrolling)), so they add no Tab stop and the order above stands. Whether `GtkScrolledWindow` is focusable by default isn't in the docs (**unverified**), so the app sets it. Scrolling by keyboard goes through focus: a viewport scrolls to the focused control by itself (`GtkViewport:scroll-to-focus`, "whether to scroll when the focus changes", docs.gtk.org; its default and its behaviour in nested viewports are **unverified**, and the window test focuses the last control of the Configuration page on a small output and checks it came into view). The preview column has nothing to focus, so on a screen too short for it the column scrolls only by wheel or touch; every control stays reachable, and the column's words are also in the preview's description and the status items' names. Fallback if a nested viewport doesn't scroll to focus: the app moves the page's vertical adjustment itself when focus enters a control that lies outside the viewport.
- **Inside a tile group:** the arrow keys move focus (`GtkFlowBox` keyboard navigation), and Enter or Space chooses the tile, which applies it. Focus alone never applies.
- **Insensitive tiles can't take focus.** Their reasons are visible text under the group heading.
- **Toggle groups** (rotation and the unit): Tab reaches them; Enter or Space toggles. Arrow keys between the buttons of a group are **unverified** for grouped `GtkToggleButton`s; if GTK doesn't do it, each button is a Tab stop. The gear is one toggle on its own, not a group.
- **Dropdowns:** Enter or Space opens; arrows move; Enter chooses; Escape closes the list. While the list is open Escape closes only the list: the back action doesn't take it (**unverified**: the list is a popover with its own surface and grab, which should get the key first; if a test shows the page going back as well, `win.back` is disabled while the GPU list is open, or Escape is dropped and Alt+Left and the back icon remain).
- **Scales:** Left and Right move one stop; Home and End go to Slower and Faster.
- **Switches:** Space toggles. Start at boot is insensitive while a flip runs, so it can't be flipped twice; focus comes back to it when the flip ends. It's also insensitive while its state is unknown and while the display service is starting; Tab then goes straight to Copy.
- **Not available in this session:** nothing in the window takes focus but the close icon, and only Ctrl+W and Ctrl+Q work; the Configuration, back and Refresh actions are disabled.
- **Shortcuts** (`GtkApplication` accelerators, in GTK's accelerator strings: `<Control>comma`, `Escape`, `<Alt>Left`, `F5`, `<Control>r`, `<Control>w`, `<Control>q`; a key is written by its lowercase name, as `gtk_accelerator_parse` asks, docs.gtk.org, read 2026-11-19). One set, the same wherever the design mentions a route:

  | Keys | Action | When it works |
  |---|---|---|
  | Ctrl+, | open the Configuration page (`win.configuration`) | the window shows its pages; on the Configuration page it does nothing |
  | Escape or Alt+Left | back to the Display page (`win.back`) | only while the Configuration page shows: the action is disabled elsewhere, so Escape stays free for menus, popovers and everything else |
  | F5 or Ctrl+R | Refresh (`win.refresh`) | from either page, while the service runs |
  | Ctrl+W or Ctrl+Q | close the window (`win.close`) | always |

  Ctrl+, is GNOME's standard for Preferences and Alt+← for Back (developer.gnome.org/hig, Keyboard); Escape also goes back in libadwaita's navigation view.

## Accessible names

The Phase 10 tests drive the app through AT-SPI, so every interactive widget gets a fixed English accessible label (`GTK_ACCESSIBLE_PROPERTY_LABEL`), and reasons go in `GTK_ACCESSIBLE_PROPERTY_DESCRIPTION`. Names never include live values, so tests find widgets by name and read values from state or description. The roles are the ones GTK gives each widget, as AT-SPI reads them (`GetRoleName`; the layer 4 run, Phase 10 Task 8, 2026-11-07).

What AT-SPI showed, and what the app does about it (decisions log D67–D72):

- **A button's name:** a button with a text label is labelled by that label, which wins over the name the app gives it, so the app drops that relation when it names a widget: the `°C` toggle reads "Celsius" (D68).
- **A label's name:** GTK names a label by its own text, again at every change, so a label that needs a fixed name sits in a group of that name, and its text is the value: "Cooler status", then "Cooler connected" (D72).
- **Actions:** a tile and an open dropdown row get no AT-SPI action from GTK that chooses them, so the app gives each one: `tile.activate` and `row.activate`, which do what Enter or a click does. An insensitive tile offers none (D67, D69).
- **Insensitive:** GTK marks only the widget itself disabled for assistive technology, not its children, so the app makes each control insensitive itself, never only its box (D70).

| Widget | GTK4 widget (role) | Accessible name | State and description |
|---|---|---|---|
| Gear | `GtkToggleButton` (toggle button), icon only, class `icon-button`, tooltip "Configuration" | `Configuration` | pressed on the Configuration page; insensitive while the service isn't running; hidden when not available in this session |
| Close | `GtkButton` (button), icon only, class `icon-button`, action `win.close`, tooltip "Close" | `Close` | always present and sensitive |
| Back | `GtkButton` (button), icon only, class `icon-button`, action `win.back`, tooltip "Back to Display" | `Back` | on the Configuration page's bar only |
| Page titles | `GtkLabel` (label), class `page-title` | their text: `Display`, `Configuration` | not named by the app: a label is named by its own text. A test tells the `Configuration` title from the gear by their roles (label, toggle button) |
| Refresh | `GtkButton` (button), icon only, class `icon-button`, action `win.refresh`, tooltip "Refresh the display: fresh readings, and the picture starts again" | `Refresh display` | on the Display page's bar only, so absent on the off screen and when not available in this session |
| Refresh and refusal label | `GtkBox` (group) in the `GtkOverlay`, around the `GtkLabel` | `Change feedback` | its label's text ("Refreshed", or "Not changed." and the reason) |
| Off screen message | `GtkBox` (group) holding the heading and text `GtkLabel`s | `Service message` | description: its text ("OpenDeepCool is off" or "OpenDeepCool isn't running", then the text). It replaces Phase 10's `Service banner` and its `Copy start command` (D10) |
| Not-available message | `GtkBox` (group) holding two `GtkLabel`s | `Not available in this session` | description: its text |
| Cooler banner | `GtkBox` (group) in `GtkRevealer` | `Cooler banner` | description: its text: not connected, no access or sending fails ([Banners](#banners)) |
| Preview | `GtkDrawingArea` (image) | `Cooler display preview` | description: the mode caption, for example "Temperature · CPU + GPU"; while the cooler isn't showing the picture, the caption, a full stop and the words of the cooler's state: "Temperature · CPU + GPU. Not on the cooler: it isn't connected." (`not-found`), "… Not on the cooler: the service can't open it." (`no-access`) or "… Not on the cooler: sending it fails." (`write-failed` for 2 s or more); while the service isn't running, "No picture". It changes with the caption and the cooler's state, and nothing else in the column carries these words ([Preview column](#preview-column)) |
| Reason under the preview | `GtkBox` (group) around the icon and the `GtkLabel` | `Unavailable reading` | its label's text |
| Status items | `GtkBox` (group) around the dot and the `GtkLabel` | `Cooler status`, `Service status`, `Music sound status` | their label's text (`Cooler status`: "Cooler connected", "Cooler not connected", "Cooler: no access", "Cooler: sending fails" or "Cooler: unknown"; `Service status`: "Service running", "Service not running" or "Service off") |
| Tile groups | `GtkFlowBox` (grid) | `Load options`, `Temperature options`, `Power options`, `Music options` | — |
| Group reason lines | `GtkBox` (group) around the `GtkLabel` | `Load unavailable reasons` (and so on per group) | its label's text |
| Tiles | `GtkFlowBoxChild` (grid cell), its picture an image of the same name | `Load: CPU`, `Load: GPU`, `Load: RAM`, `Load: CPU + GPU`, `Load: CPU + RAM`, `Load: GPU + RAM`, `Temperature: CPU`, `Temperature: GPU`, `Temperature: CPU + GPU`, `Power: CPU`, `Power: GPU`, `Power: CPU + GPU`, `Music: Spectrum`, `Music: Emoji 1` … `Music: Emoji 4`, `Music: CPU temperature`, `Music: GPU temperature`, `Music: CPU usage`, `Music: GPU usage`, `Music: RAM usage`, `Music: Dancer` | selected; disabled; description `<metric>: <reason>` when unavailable, or "Reading lost: …" on the saved mode; action `tile.activate` while sensitive |
| Configuration cards | `GtkBox` (group) | `Display settings`, `Music settings`, `Startup settings`, as the mockups label their sections (decisions log D62). The Startup card keeps its name in the window's body while the service is down | — |
| Rotation | `GtkToggleButton` (toggle button) | `Rotation 0°`, `Rotation 90°`, `Rotation 180°`, `Rotation 270°` | pressed |
| Unit toggles | `GtkToggleButton` (toggle button), in the Display card after Rotation | `Celsius`, `Fahrenheit` | pressed |
| GPU | `GtkDropDown` (combo box) | `GPU` | the selected item's label (`Automatic`, or the GPU's); description "Using <GPU label>" while Automatic is chosen, "no readings" when the GPU in use has no reading rule, "driver couldn't be asked" when a GPU whose kind query failed is chosen by hand, joined by "; " when several apply |
| GPU in use | `GtkBox` (group) around the `GtkLabel` under the GPU dropdown | `GPU in use` | its label's text ("Using Dedicated · Contoso Graphics R870."); present only while Automatic is chosen |
| Dropdown items | the open list's rows (list item) | the item label without its note (`Automatic`, `Dedicated · Contoso Graphics R870`) | the note ("no readings", "driver couldn't be asked") in the description; action `row.activate` on the row's content |
| Peak dots | `GtkSwitch` (switch) | `Peak dots` | checked: the switch's handle, which moves at the press (decisions log D63, D79) |
| Bar fall | `GtkScale` (slider) | `Bar fall` | value 0–4; value text the stop's name (`Moderate`) |
| Peak fall | `GtkScale` (slider) | `Peak fall` | the same (`Slow`) |
| Start at boot | `GtkSwitch` (switch) | `Start at boot` | checked: the handle (`active`; GTK sets the accessible checked state from it, `gtk/gtkswitch.c`); disabled while a flip runs, while the state is unknown and while the display service is starting; description: the note's text when one shows ("Turning on… Your desktop asks for the administrator's password first.", "Not fully on: …", "Not changed: …") |
| Start at boot note | `GtkBox` (group) around the icon and the `GtkLabel` under the row | `Start at boot status` | its label's text; present only while a note applies |
| Terminal commands | `GtkBox` (group) around the muted label, the selectable command label and Copy | `Terminal commands` | its labels' text: the lead-in and the commands |
| Copy terminal commands | `GtkButton` (button) | `Copy terminal commands` | — (it copies every line) |

A scale's value text reaches AT-SPI as its Value interface's text, the stop's name (`GTK_ACCESSIBLE_PROPERTY_VALUE_TEXT`; the layer 4 run reads "Moderate").

A Configuration row, its label block and its hint (Task 8, [Rows](#rows)) are a plain `GtkBox` and `GtkLabel`s with no name of their own: a title and a hint read as the labels they are, in the page's order, and each control keeps the fixed name above. The two switches' size (40 × 22) changes no name, role or state. The preview column has no line under the caption to name (a label is named by its own text, D72): the layer 4 scenarios that look for the cooler's words read the preview's description instead.

The window's size and its scrolled windows (Task 9, [Size and scrolling](#size-and-scrolling)) add no name, role or state the app sets: the window scroller, the column and the page scrollers are unnamed containers with the role GTK gives a scrolled window, and each control below them keeps its fixed name and its place in the tree order. A control that has scrolled out of the viewport keeps its name, state and actions, and the layer 4 scenarios act through AT-SPI actions, not the pointer, so they shouldn't need to scroll first (**unverified**: the first run of the scenarios on a capped window shows it). The scenarios that read a widget's position on screen start the fake Wayland output at a size that doesn't cap the window (`Weston::start` takes the size).

## Text

- Short English sentences, with the mode names Load, Temperature, Power and Music, never "Guardian" or "… Mode".
- Metrics are named as the display shows them: CPU usage, GPU usage, RAM usage, CPU temperature, GPU temperature, CPU power, GPU power.
- Reasons come from the service and aren't rewritten. The service puts the metric in front by the spec's name ("Gpu usage: …", "Memory Load: …"); the app writes that name as the display does ("GPU usage: …", "RAM usage: …") and keeps the rest (decisions log D35).
- A banner that needs the user's action says what happened in bold, then the fix in one sentence. Details, such as device paths and error codes, go to the journal, and the banner says "The journal says more."
- Terminal commands are shown whole, in a selectable monospace label with a Copy button, never inside a sentence. Users read "the sound for Music", never "sound helper" or a unit name, except where systemd's own answer names a unit.

## What the Phase 10 plan must add to the D-Bus contract

The design depends on these. The first was agreed on 2026-10-25. The service offers no D-Bus properties (owner, 2026-10-26), so each item is part of the state that `GetState()` returns and `StateChanged` updates, or a signal addressed to one watcher ([Talking to the service](#talking-to-the-service)).

1. **The current frame:** a signal, addressed to each authorized watcher alone after `WatchFrames()` and never broadcast, carrying the frame exactly as written to the cooler, rotation included. It's sent only while the app watches, and only when the frame changes. Its encoding (the 28 pixel bytes or 196 booleans) is chosen in the plan.
   - While the cooler is away (`not-found`, `no-access`, `write-failed`), the design still needs the frame the engine draws on each tick, so the preview keeps showing the saved picture ([Banners](#banners)). The Phase 10 plan's "each written frame" (Task 2) has to cover those ticks too.
2. **GPU names:**
   - either the service sends each GPU's PCI vendor and device ID, so the app can look up `/usr/share/hwdata/pci.ids`, or the service sends the name;
   - the current `Gpus` (`a(sss)`: address, driver, kind) has neither;
   - it also needs a "no reading rule" flag per GPU;
   - the Phase 10 plan makes it `a(sssqqb)` (Task 2). Its kinds already include `query-failed` (Phase 6), which the list notes.
3. **The GPU in use as well as the saved one:** needed for the line that names the GPU Automatic reads. `ChosenGpu` is only the saved choice, empty for Automatic (owner, 2026-11-04). The Phase 10 plan adds `GpuInUse` (s) (Task 2).
4. **How the app learns it's in a remote or inactive session:** `GetState()` refused with `NotAuthorized` ([Not available in this session](#not-available-in-this-session)). There's no `CanChangeSettings()` and no separate check: the refusal that protects the state is what tells the app.
5. **Watches that can be renewed:** calling `Watch()` or `WatchFrames()` again does no harm, because the app renews its watches each time it connects again, including on every window activation ([Talking to the service](#talking-to-the-service)).

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
| Temperature unit | Configuration → Display, the row after Rotation (the temperature tiles follow it) | configuration, display-scrolling (°F tiles) |
| Rotation 0–3 | Configuration → Display | configuration |
| GPU dropdown only with more than one GPU, Automatic first and the default, then the spec's order, the choice kept | Configuration → Display | configuration, configuration-gpu-list, one-gpu |
| Automatic names the GPU it reads (`GpuInUse`) | The line under the GPU dropdown | configuration, configuration-gpu-list |
| A GPU added or removed: the choice goes back to Automatic, and a saved GPU is never missing | Configuration → GPU (no note; the journal has the reason) | configuration |
| A GPU whose kind query failed: listed with the unknown GPUs, with a short note, and choosable | Configuration → GPU, the open list | configuration-gpu-list |
| GPU without a reading rule → listed last, GPU modes disabled | Configuration → GPU, and tiles | configuration-gpu-list; configuration (choose it) |
| Peak dots on or off | Configuration → Music | configuration |
| Bar fall, peak fall, five stops, defaults | Configuration → Music | configuration |
| Configuration screen holds everything but the mode (owner, 2026-11-18: the unit too) | Configuration page | configuration |
| Labels beside their controls, left-aligned, with the space between cut to a 24 px gap (owner, 2026-11-18) | Every row of the Configuration page and of the Startup card on the off screen: [Rows](#rows) | configuration, one-gpu, boot-partly, boot-off |
| The two switches (Peak dots, Start at boot) a little smaller (owner, 2026-11-18); the rotation and unit toggles unchanged | 40 × 22 px: [The switches](#the-switches) | configuration, boot-off |
| The preview column holds the picture, its caption, the lost reading's reason and the status; no "Shown on the cooler now", and no other line under the caption (owner, 2026-11-18) | [Preview column](#preview-column) | every mockup with the column |
| Both pages reachable with no tab: the gear opens Configuration; the back icon, the gear again, Escape and Alt+Left return to Display | Header bar; the Configuration page's bar; [Keyboard](#keyboard) | display, configuration |
| The app closes from its own window | The close icon at the right of the header bar (the desktop's window buttons are off), Ctrl+W, Ctrl+Q | every mockup |
| The window opens outside tiling mode (owner, 2026-11-18) | `resizable` false; a compositor that floats fixed-size windows floats it, and none is named or asked for anything in the app ([Size and scrolling](#size-and-scrolling), Not resizable, and floating) | — |
| The window is the smallest size that fits its content in width and height, so it depends on the monitor | No default or minimum size: the natural size of the largest page beside the preview column, capped at the smallest monitor less 48 px ([Size and scrolling](#size-and-scrolling)) | every mockup (drawn at the natural size) |
| Scrollbars where the content would overflow the screen | Each page and the preview column scroll down; the whole content scrolls sideways below about 850 px | every mockup (vertical, in a short browser window) |
| Refresh, only for the user at the PC | The icon at the top right of the Display page; not drawn when not available in this session or while the service isn't running (the gear is insensitive then); F5 and Ctrl+R from either page while the service runs; a refusal shows as a refused change | display, service-not-running, not-available |
| Status: cooler connected | Status line; cooler banner | cooler-disconnected |
| Status: the cooler found but not usable (`no-access`), told apart from not connected, with the fix | Its own error banner; status line; the preview's description | cooler-no-access |
| The picture isn't on the cooler (`not-found`, `no-access`, `write-failed` for 2 s or more): still said, without a line under the caption | The banner; the cooler's status item; the preview's accessible description ([Preview column](#preview-column)) | cooler-disconnected, cooler-no-access, cooler-write-failed |
| Status: sending to the cooler fails (`write-failed`), passing or lasting | Banner after 2 s, nothing before | cooler-write-failed |
| Status: service running | Status line; the off screen's heading and status dot | service-not-running, boot-off |
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
| Start at boot: one switch turns OpenDeepCool on or off for the whole PC, both units | Configuration → Startup; the off screen | configuration, boot-off, service-not-running |
| On only when ready to use: the display service enabled and running, the sound enabled for every user and running in this session; the one-shot helpers don't count | [Where its state comes from](#where-its-state-comes-from): four facts | configuration (on), boot-partly, service-not-running |
| Anything else reads off, with a note saying what's missing, a stopped or failed service included | [The note](#the-note) | boot-partly, service-not-running |
| A flip on finishes the setup and starts what isn't running; no separate start button | [A flip, step by step](#a-flip-step-by-step); the off screen has no banner and no start button (D10) | boot-off (flip it), service-not-running |
| Only the sound missing in this session: a flip on starts it, without a password | A flip, step by step; the note's ending | (text: boot-partly's state with only the sound not running) |
| Off: neither starts any more, and both stop | A flip, step by step | configuration (flip it) |
| Every change asks for the administrator's password, and nothing else in the app does | The desktop's polkit dialog, and the changing line ("Turning on… Your desktop asks for the administrator's password first."). The card's hint doesn't repeat it (owner, 2026-11-18) | boot-changing |
| The switch shows systemd's state, a terminal change and a stopped service included | Read at start, after a flip, on window activation and on the bus name's changes | boot-partly, service-not-running |
| It works while the service isn't running | The off screen holds the switch | boot-off, service-not-running |
| A change that doesn't happen: the switch goes back to systemd's state, and the commands for the flip's direction show (three lines to turn on, two to turn off) | The note under the switch: dialog dismissed (126), not authorized or no agent (127, `--disable-internal-agent`), a step failed (the program's code) | boot-not-changed |
| Unknown | Insensitive at off, with the reason and the commands | boot-unknown |
| The service-not-running command is `sudo systemctl enable --now opendeepcool.service` | The off screen's terminal commands, first line | boot-off, service-not-running |
| Only the user at the PC may flip it | polkit `allow_active` only; no switch when not available, nor on the off screen in a remote or inactive session | not-available |
| English only, no app-only settings | — | — |

**Out of scope, confirmed absent.** The design has no:
- brightness, screen on/off or other hidden settings;
- lighting;
- other devices;
- CPU, GPU or RAM readouts outside display pictures (tiles use fixed sample values, and the preview shows only the cooler's picture);
- clocks or voltages;
- theme-source setting;
- Creative mode;
- static or scrolling option;
- language or start-at-login setting for the app itself (Start at boot is systemd's state for OpenDeepCool's units, not an app setting);
- control that changes the system other than the Start at boot switch (owner, 2026-11-10): no separate start button (owner, 2026-11-11: the switch does it) and no udev fix. The banners name that fix, and the user carries it out.

## Unverified points, answered (Phase 10 Task 1, 2026-11-06)

Each point the design left open, with its answer and source, or the fallback the app uses while it stays unverified. The phase's decisions log (`docs/prp/reports/phase-10-settings-app.decisions.md`, D6–D12) has the options behind each.

| Point | Answer | Source, or the fallback |
|---|---|---|
| `gtk_style_context_add_provider_for_display` | Not deprecated: it carries no deprecation notice and is declared in `gtkstyleprovider.h`, apart from the deprecated `GtkStyleContext`. gtk4-rs: `gtk::style_context_add_provider_for_display` | docs.gtk.org, `Gtk.StyleContext.add_provider_for_display` |
| Dark or light base (GTK 4.20 and later) | The app sets `GtkCssProvider:prefers-color-scheme` to `GTK_INTERFACE_COLOR_SCHEME_DARK` or `_LIGHT` on its own provider; `gtk-application-prefer-dark-theme` is deprecated since 4.20, and `GtkSettings:gtk-interface-color-scheme` only carries the system's preference. Every colour the app draws comes from the tokens, so the scheme touches only what GTK draws itself; whether GTK's built-in theme follows the app provider's scheme is **unverified**, and the layer 3 tests look at it | docs.gtk.org, `Gtk.CssProvider:prefers-color-scheme`, `Gtk.Settings:gtk-application-prefer-dark-theme`, `Gtk.Settings:gtk-interface-color-scheme` |
| Gradient border colours | `border-color` takes colours only; `border-image` takes an image, a gradient included. The app keeps a gradient's first colour, as Theme says, so no gradient is drawn | docs.gtk.org, GTK CSS Properties |
| Default accessible roles | `GtkFlowBoxChild` is `GRID_CELL` (its box `GRID`), `GtkLabel` `LABEL`. `GtkDrawingArea`'s default isn't documented, so the preview and every tile picture are built with the role `IMG` and their accessible label. A named `GtkBox` (the banners, the not-available message, and every group around a label of a fixed name) is built as `GROUP`, since its default, generic, may carry no name in ARIA (decisions log D48, D72). Through AT-SPI, every role is as the [names table](#accessible-names) gives it (Phase 10 Task 8) | docs.gtk.org, Accessibility (List of accessible roles); the layer 4 run |
| Accessible names, descriptions, value text | `AccessibleExtManual::update_property` sets `Label` and `Description`; `ValueText` is ARIA's `aria-valuetext`. Read through AT-SPI (Phase 10 Task 8): descriptions and value text arrive as set; a name arrives as set once the app drops a button's labelled-by relation (decisions log D68), and on a group around a label, since a label is always named by its text (D72) | gtk4-rs book, Accessibility; docs.gtk.org, Accessibility; GTK's source (`gtkbutton.c`, `gtklabel.c`); the layer 4 run |
| `GtkScale` and whole stops | `GtkRange:round-digits` rounds the value "when it changes", drags included: with 0 digits over 0–4 the scale only holds whole stops, and the app applies each stop as it's reached (at most five calls a drag). If a test shows otherwise, the stop is applied on release | docs.gtk.org, `Gtk.Range:round-digits` |
| Arrow keys between grouped `GtkToggleButton`s | **Unverified.** Each unit and rotation toggle is a Tab stop as well, so the keyboard reaches each one either way. AT-SPI's key events don't reach a Wayland app (Phase 10 Task 8, decisions log D71), so no test types keys; the shortcuts' keys are checked in-process | the fallback of Keyboard |
| Tooltips on insensitive widgets | **Unverified**, and not relied on: the reason line under each group says why a tile is unavailable | Display page, Unavailable modes |
| A compositor and the window's minimum size | Superseded (Phase 11a Task 9, 2026-11-19): the window has no minimum size any more, and is not resizable ([Size and scrolling](#size-and-scrolling)). What Task 1 measured in a real session is in [Unverified points, answered (Phase 11a Task 1)](#unverified-points-answered-phase-11a-task-1-2026-11-18) | Window |
| `GtkWindow:is-active` after a session switch | **Unverified** under Hyprland, for the same reason. The app also follows logind's `Active` property of its own session (org.freedesktop.login1), a third trigger for connecting again, still without polling. The reconnect on activation itself is checked in headless weston, where another window takes the focus and closes (Phase 10 Task 8, decisions log D78) | Not available in this session |
| glib's one-shot timer name | `glib::timeout_add_local_once`, in glib 0.22, gtk4 0.11's (its source, `src/source.rs`, read 2026-11-06) | Banners, `write-failed` |

The Start at boot design's own points (2026-11-11):

| Point | Answer | Source, or the fallback |
|---|---|---|
| `GtkSwitch`'s delayed state | Verified: `active` is the handle, `state` the trough; `state-set` returning `TRUE` stops the default handler; `gtk_switch_set_state` sets `:checked`; the accessible checked state follows `active` | docs.gtk.org, GtkSwitch and GtkSwitch::state-set; GTK's source, `gtk/gtkswitch.c` |
| pkexec's exit codes | Verified: 126 is only a dismissed dialog; 127 is every other refusal or error, a missing agent included | pkexec(1) |
| pkexec's messages on stderr | **Unverified**, and not relied on: one sentence covers every 127 | [States and their text](#states-and-their-text) |
| pkexec's text agent on the app's terminal | **Unverified**; `--disable-internal-agent` avoids it either way (owner, 2026-11-11) | pkexec(1) documents the option |
| The user's manager reading a `--global` link as `enabled` | Verified with the dev copy's layout (2026-11-11): `GetUnitFileState` on the user bus answered `enabled`; the package's layout is checked again in Phase 11 Task 7 | [Where its state comes from](#where-its-state-comes-from) |
| `GetUnitFileState` for a unit that isn't installed | Verified (2026-11-11, system and user bus): the call fails ("No such file or directory"), so the switch reads unknown | `busctl call … GetUnitFileState` on the development PC |
| `systemctl start` returning only once the service owns its bus name | Verified from the code: the unit is `Type=notify`, and the service sends `READY=1` only once it owns its bus name (`crates/odc-service/src/dbus.rs`, the D-Bus side's start), so `systemctl start` returns after the name has an owner. The 5 s wait after a successful `on` stays, for the app to see the name's new owner | `packaging/systemd/opendeepcool.service`; [The window around a flip](#the-window-around-a-flip) |
| logind's `Active` and `Remote` | Verified: both are properties of the session object | org.freedesktop.login1(5) |
| `ActiveState`'s values | Verified: `active`, `reloading`, `inactive`, `failed`, `activating`, `deactivating`, `maintenance`, `refreshing`; `failed` covers "too many restarts" too | org.freedesktop.systemd1(5), Table 1 |
| A service waiting to restart | Verified: `SubState` `auto-restart` and `auto-restart-queued` map to `activating` | systemd's source, `src/core/service.c` (its state tables) and `src/basic/unit-def.c` (the strings), read 2026-11-11 |
| Reading a unit's `ActiveState` | Verified: `GetUnit` fails for a unit not loaded; `LoadUnit` loads it from disk; `StartUnit` returns a job whose end is `JobRemoved`, sent only once some client has called `Subscribe()` | org.freedesktop.systemd1(5) |
| `Restart=` of the display service | The unit says `Restart=on-failure` (`packaging/systemd/opendeepcool.service`, read 2026-11-11), with `StartLimitIntervalSec=0`. The design reads `failed` and the restart wait alike, so it holds for `Restart=always` too | the unit file |
| `g_application_hold` | Verified: "Increases the use count of application", to say it "has a reason to continue to run" | docs.gtk.org, GApplication.hold |

## Unverified points, answered (Phase 11a Task 1, 2026-11-18)

The points the Phase 11a plan left open, each measured with a throwaway gtk4-rs program (GTK 4.24, never committed) in a headless weston with a private session bus, and, where the plan said so, once in a real Hyprland session. Whatever a point says is unverified stays marked, with the fallback the design uses.

| Point | Answer | Source, or the fallback |
|---|---|---|
| The system's dark or light preference | **`GtkSettings:gtk-interface-color-scheme` is enough, and the app needs no bus code of its own.** GDK fills it from the XDG portal's `org.freedesktop.appearance` `color-scheme` (it calls the portal's `ReadAll` and follows `SettingChanged`), with no environment variable. A fake portal gave `Default` for 0, `Dark` for 1 and `Light` for 2, and flipping it from 1 to 2 while the program ran sent a `notify::gtk-interface-color-scheme` with `Light`. With no portal on the bus it reads `Unsupported`. GSettings plays no part: the same program read `Dark` with the memory backend, and `Unsupported` with no session bus | Measured. The docs say only "the system-wide preference" and give `UNSUPPORTED` as the default (docs.gtk.org, `Gtk.Settings:gtk-interface-color-scheme`, read 2026-11-18). **The rule for "found":** `Unsupported` is "no system theme found" (the warning); `Default`, `Dark` and `Light` are found, and `Default`, which says no preference, shows OpenDeepCool's dark theme |
| A fake portal in the tests | GDK's first step is `ListActivatableNames`: it uses the portal only when `org.freedesktop.portal.Desktop` is **activatable**. On a bus with no service directories (the testkit's `PrivateBus`), a fake that owns the name is not enough and GTK reads `Unsupported`. With a service file for that name in a directory the bus's configuration lists (its `Exec` can be `/bin/true`), and the fake started first, the readings above held | Measured, on a bus with the testkit's configuration plus one `<servicedir>`. Task 11 adds that directory to `PrivateBus` for the app's tests; no test flag is needed |
| A window that fits its content | On each page's `GtkScrolledWindow`, **`propagate-natural-width` and `propagate-natural-height` make the window the size of its content**: a plain `GtkScrolledWindow` opened at 168 × 85, with both properties at 460 × 915 wanted and 460 × 800 given on a 1280 × 800 output and 460 × 480 on a 640 × 480 one, the page scrolling inside. `max-content-height` and `-width` cap it at their value plus the window's own chrome (300 gave 339). A `GtkStack`'s pages are homogeneous by default, so the window takes the larger page's size and doesn't change size when the page changes | Measured; docs.gtk.org's `propagate-natural-height` says the child's natural height "should be calculated and propagated" (read 2026-11-18). The design caps both at the smallest monitor's geometry from `GdkDisplay::monitors` minus a margin, so the size doesn't depend on the compositor's own clamping |
| A floating window | On the Hyprland session read with `hyprctl clients -j`: a plain `GtkApplicationWindow` is **tiled**, a window with `resizable` false is **floating**, and a non-modal window with no parent is tiled. A fixed-size window of its natural size is clamped to the output and still scrolls (above). So the design takes `resizable` false, which also matches "the size of its content" | `system:` evidence for Hyprland, 2026-11-18, not a test (AGENTS.md rule 14). **Unverified** for any other compositor: where one ignores the hint, the window is tiled and the owner decides |
| Our own close icon | `GtkHeaderBar:show-title-buttons` false removes the window buttons the desktop's layout would draw, so the app's own gear, back and close buttons are the only ones. The icons exist: `adwaita-icon-theme`, a dependency of Arch's `gtk4`, provides `window-close-symbolic`, `go-previous-symbolic`, `view-refresh-symbolic` and `emblem-system-symbolic` (the gear, in its `legacy` folder); a probe found all four in the user's theme, which falls back to Adwaita. Every icon-only button also carries an accessible name and a tooltip | `pacman -Qi gtk4` and `pacman -Ql adwaita-icon-theme`, read 2026-11-18; `Gtk.IconTheme.has_icon` in the probe. Fallback: if a theme lacks an icon, the tooltip and accessible name still say what the button does |
| Smaller switches | CSS on the `switch` node shrinks a `GtkSwitch`: the default is 48 × 24, and `switch { min-width: 34px; min-height: 18px; }` with `switch slider { min-width: 14px; min-height: 14px; }` gave 34 × 18. The design picks the exact size when the stylesheet changes (Task 8); it is "a little smaller", not these numbers | Measured. A switch stretched to its row's width reads wide (166 px in the probe), so its `halign` is start, as it is now |
| The cost of 23 live tiles, and what stops them | 23 areas of 14 × 14 dots redrawn at 10 Hz used 570 ms of CPU in 4 s (about 0.14 of a core) in a debug build, drawn by a software renderer in headless weston; a release build on a GPU compositor will use less, and Task 14 sets its ceiling at about sixty times what it measures (owner, 2026-11-17). **A frame-clock tick does not stop for a hidden window:** hiding the window left 37 of 40 steps in 4 s, and a timeout kept all 40. **It does stop when the compositor draws nothing:** with the monitor off, the real session gave the frame clock 0 steps in 12 s and the same program with a timeout 119. Whether it stops for a window on another workspace with the screen on is **unverified**: the monitor was off during the test and the owner's screen wasn't woken | Measured. **The design: one tick from the frame clock, counted to 100 ms, stopped explicitly when the window is unmapped, hidden or minimised (`notify::visible` or unmap), and relied on to stop for occlusion.** The check on another workspace is repeated at Task 17's sign-off if the screen is on, and recorded in the report |
| The remaining sensor drivers without lm-sensors | By their module alias tables (`modinfo`, read 2026-11-18): `coretemp` has a CPU-feature alias, `k10temp` has PCI aliases, and `intel_rapl_msr` has CPU-feature aliases, with `intel_rapl_common` as its dependency, so udev loads them by modalias when the CPU or the PCI device appears. On the development PC, `modprobe -R` on the CPU's modalias (`/sys/devices/system/cpu/modalias`) resolves to the CPU-temperature and powercap drivers that fit its CPU, and the powercap driver was loaded at boot with no configuration naming it (read 2026-11-19; which drivers: `docs/local/`). The CPU-temperature driver's load there proves nothing on its own, because another unit loads it too, and the PCI-matched `k10temp` was not exercised there: **unverified**. (The first text of this row said all three drivers were loaded on the development PC. That was wrong, since a PC loads only the drivers its own CPU and devices match, and it named the PC's drivers against AGENTS.md rule 10; corrected 2026-11-19.) The udev step itself is standard and was **not** re-tested here | Measured (aliases); the failure mode is already specified: a metric with no source disables its modes with the reason (`settings.md`, Availability), and the README names the driver to load |

## Unverified points, answered (Phase 11a Task 7, 2026-11-19)

The points the header and page design depends on. Read in the official documents on 2026-11-19; nothing here was run, since the designer has no shell, so what only a window can show is marked **unverified**, with the fallback, and the implementer's tests check it.

| Point | Answer | Source, or the fallback |
|---|---|---|
| The title stays centred with nothing at the start of the bar and two buttons at the end | The documents say so: "The title will be centered with respect to the width of the box, even if the children at either side take up different amounts of space." The measured allocation is **unverified**: the window test asserts the title label's centre is within 1 px of the header bar's | docs.gtk.org, `Gtk.HeaderBar`. Fallback: an empty `GtkBox`, as wide as the end box, packed at the start |
| An icon-only `GtkButton` | `GtkButton:icon-name` is "the name of the icon used to automatically populate the button". The accessible name and the tooltip are set by the app, since an icon has no text for GTK to name the button from | docs.gtk.org, `Gtk.Button:icon-name` |
| Accelerator strings | `<Control>comma`, `<Alt>Left`, `Escape`, `F5`: "for character keys, use the lowercase key name rather than the symbol", and `<Control>a` and `<Shift><Alt>F1` are the documented forms | docs.gtk.org, `Gtk.accelerator_parse` |
| Back with Escape and Alt+Left; Preferences with Ctrl+, | Both are the documented conventions: GNOME's HIG lists Ctrl+, "Opens the preferences window" and Alt+← "Navigates to the previous location", and libadwaita's `AdwNavigationView` pops a page with Escape, Alt+← and the back mouse button | developer.gnome.org/hig, Keyboard; libadwaita 1.x docs, `AdwNavigationView`, read 2026-11-19 |
| Escape while the GPU list is open | **Unverified:** it should close only the list, whose popover has its own grab and gets the key first. Fallback in [Keyboard](#keyboard) | The window test opens the list and sends Escape in-process |
| Where the back button sits | GNOME's HIG: "In left-to-right locales, the standard position for the back button is the top-left." KDE's HIG names back and forward buttons "in the toolbar" and no side, so GNOME's rule decides | developer.gnome.org/hig, Browsing; develop.kde.org/hig, Layout and navigation |
| `pack_end`'s order | The documents say only "packed with reference to the end", so the design packs one box holding the gear then the close icon, whose order is plain | docs.gtk.org, `Gtk.HeaderBar.pack_end` |
| The gear icon's name | `emblem-system-symbolic`, found by Phase 11a Task 1. A theme without it falls back to Adwaita's; if a desktop's own theme draws it as something else, the tooltip still says "Configuration" | The Task 1 row, "Our own close icon" |

## Unverified points, answered (Phase 11a Task 8, 2026-11-19)

The points the Configuration rows, the switches and the preview column depend on. Read in the official documents on 2026-11-19; nothing was run, since the designer has no shell. What only a window can show is marked **unverified**, with the fallback, and the implementer's layout tests check it.

| Point | Answer | Source, or the fallback |
|---|---|---|
| How a wrapping `GtkLabel`'s width is negotiated | Verified: "For wrapping labels, width-chars is used as the minimum width, if specified, and max-width-chars is used as the natural width", and "Even if max-width-chars specified, wrapping labels will be rewrapped to use all of the available width". So a hint with `max-width-chars` 28 asks for 28 characters, and fills the size group's width once the group is wider | docs.gtk.org, `Gtk.Label`, Text layout |
| `GtkSizeGroup` and hidden members | **Unverified.** The class page gives the rule for members, "the size requested for each widget in a GtkSizeGroup is the maximum of the sizes that would have been requested", and says nothing of hidden ones. The only hidden row (GPU) has a block no wider than the others, so it changes nothing. Fallback: take the block out of the group while the row is hidden | docs.gtk.org, `Gtk.SizeGroup` |
| `GtkGrid` and rows with no visible child | **Unverified:** the class page says nothing of such rows or their spacing. Not relied on: the rows are `GtkBox`es ([Answers decided on 2026-11-19, Task 8](#answers-decided-on-2026-11-19-task-8)) | docs.gtk.org, `Gtk.Grid` |
| What `min-width` and `min-height` measure | **Unverified:** the CSS Properties page lists them and names no box. Task 1's 34 × 18 doesn't say whether the app's 1 px border was in it. The test pins the switch's outer size, 40 × 22, not the numbers ([The switches](#the-switches)) | docs.gtk.org, GTK CSS Properties; the Task 1 row, "Smaller switches" |
| The slider's height inside the trough | **Unverified:** `GtkSwitch`'s page lists the nodes `switch`, `image` and `slider` and no sizes. The mockups draw an 18 px knob with 1 px beside the border; the app takes what GTK draws | docs.gtk.org, `Gtk.Switch`, CSS nodes |
| The pointer target of a 40 × 22 switch | Verified from the criterion's text: SC 2.5.8's spacing exception holds when no other target, nor another undersized target's 24 px circle, lies within the circle centred on the switch's box (1 px beyond it, above and below). The rows are 14 px apart | w3.org, Understanding SC 2.5.8 (Target Size (Minimum)) |
| Where a form puts a label in a narrow window | KDE's Kirigami `FormLayout` stacks a label above its control when there is no room. The app doesn't: its window isn't resizable (Task 9), and a `GtkBox` can't wrap a control under its label, so the content scrolls sideways | develop.kde.org, Kirigami form layouts (read 2026-11-19; as the search summary gave it, the page itself was not opened) |
| The `small` style class | **Unverified** whether GTK's own theme defines it; the app's stylesheet doesn't (no match in `crates/odc-app/src`, 2026-11-19). The design never relies on its size: Task 11's type scale gives it one | grep of the tree |
| A description on an image-role `GtkDrawingArea` | `Names::describe` sets `GTK_ACCESSIBLE_PROPERTY_DESCRIPTION` on it today (the preview's description is the caption); the rows above change only its text. AT-SPI reads it as the Phase 10 layer 4 run found for every description | the Accessible names notes; the Phase 10 Task 8 run |

## Unverified points, answered (Phase 11a Task 9, 2026-11-19)

The points the window's size and scrolling depend on. Read in the official documents and, for the window's sizing, in GTK's source on 2026-11-19; nothing was run, since the designer has no shell. What only a window can show is marked **unverified**, with the fallback, and the implementer's window tests check it. The pixel sizes in [Size and scrolling](#size-and-scrolling) are estimates from this design's numbers (and assume GTK's flow-box child has the 3 px of padding its default theme gives it, **unverified**); no test pins them.

| Point | Answer | Source, or the fallback |
|---|---|---|
| How GTK sizes a window that has no default size | `gtk_window_compute_default_size` has no branch on `resizable`: unless a default size was set, it takes the natural size, and clamps it between the minimum and the bounds the toplevel gives (`gdk_toplevel_size_get_bounds`, which on Wayland come from the compositor). `should_remember_size` is false when the window isn't resizable, so its size isn't kept as its default. The docs say only "If true, users can resize the window". It matches what Task 1 measured in weston (915 px wanted, 800 given on an 800 px output). Read from the source, not run | `gtk/gtkwindow.c` (GNOME/gtk, main), read 2026-11-19 through a fetch tool that summarised the file's first 200,000 characters, not by eye; docs.gtk.org, `Gtk.Window:resizable` |
| A mapped, non-resizable window follows its content's natural size (the banner appearing) | **Unverified.** The source read showed no code that stores such a window's size, so the window should follow; the path from a child's size change to the toplevel wasn't in the part read. The design holds either way: if it doesn't follow, the banner takes its height from the pages' viewports instead. The window test records which happens | The banner test in [Size and scrolling](#size-and-scrolling) |
| Nested scrolled windows: natural size, and the cap on the outer one | **Unverified.** Task 1 measured one scrolled window. The design relies on a viewport scrolling only when it's allocated less than its child's minimum, and on the column and the pages having a small minimum height so that they take the squeeze. The window tests check: the window is its natural size; capped, it is the cap; the pages shrink, and the window scroller never scrolls down. **Fallback, the flat tree:** no window scroller; `max-content-width` and `-height` on the column and each page, the height as the cap less the header bar (48), the page bar (44) and the banner's height, which the app then measures (`Widget::measure`) and sets again whenever the banner's state changes; `hscrollbar-policy` automatic on the pages, so a page scrolls sideways beside the column | docs.gtk.org, `Gtk.Viewport`: it "starts scrolling" when allocated less than the child's minimum in that orientation (read 2026-11-19) |
| The monitors' list and sizes | Verified: `gdk_display_get_monitors` "lists the monitors attached to the display" as a `GListModel` of `GdkMonitor`, the same list on every call, with `items-changed` for additions and removals (not used); `gdk_monitor_get_geometry` is "in application pixels, not in device pixels". gtk4-rs has `DisplayExt::monitors() -> ListModel` and `MonitorExt::geometry() -> Rectangle` | docs.gtk.org, `Gdk.Display.get_monitors` and `Gdk.Monitor.get_geometry`; docs.rs, `gdk4` 0.x `DisplayExt` and `MonitorExt`, read 2026-11-19 |
| The Display page's natural width | **Unverified** that a flow box's natural width is `max-children-per-line` children and the gaps: the class page says "The maximum amount of children to request space for consecutively in the given orientation" and gives no formula. The window test reads the page's natural width and only requires it to be at least the six tiles' width | docs.gtk.org, `Gtk.FlowBox:max-children-per-line` |
| A non-homogeneous `screens` stack gives the window the not-available message's size | **Unverified.** Task 1 measured the default (homogeneous) stack. If a hidden child still counted, the app sets it invisible (`set_visible(false)`) while the other shows | The not-available window test |
| The header bar's height, the 48 px reserved for it | **Unverified.** About 47 from the design's own numbers: the 32 px buttons, 7 px of padding above and below, and the 1 px rule. A constant, not a measure, so a capped window can differ from the cap by a pixel or two, which the margin absorbs and the test allows (2 px) | The mockups' `.hb` rule; the window test |
| The margin of 48 px | A judgment, not a measurement: panels are typically 30 to 50 px (**unverified**), and a compositor that sends bounds sets the exact limit, which GTK takes as the smaller of the two | [Answers decided on 2026-11-19, Task 9](#answers-decided-on-2026-11-19-task-9) |
| Whether a scrolled window, and a viewport inside it, scroll a focused child into view by default | **Unverified**: the docs describe `GtkViewport:scroll-to-focus` as "whether to scroll when the focus changes" and give no default; `GtkScrolledWindow`'s `focusable` default isn't on its page. The app sets `focusable` false, and the test checks that focusing the last control of the Configuration page on a small output brings it into view; fallback in [Keyboard](#keyboard) | docs.gtk.org, `Gtk.Viewport`, `Gtk.ScrolledWindow` |
| Classic (non-overlay) scrollbars and the natural size | **Unverified** whether a scrollbar that takes room counts in the natural width or height. With GTK's default overlay scrolling nothing is added | docs.gtk.org, `Gtk.ScrolledWindow:overlay-scrolling` ("whether overlay scrolling is enabled") |
| A compositor that tiles the window anyway; what a compositor does with a window larger than a new, smaller output | **Unverified** for any compositor but the one in Task 1's session: GTK lays the content out in whatever size it is given, and a smaller size scrolls; whether a compositor re-fits a window after a monitor change is the compositor's own matter | [Size and scrolling](#size-and-scrolling), Not resizable, and floating; The monitors change |
| The first map in a remote or inactive session | The window is presented before the service answers, so it is drawn at the full size for a few milliseconds before the not-available message shrinks it. **Unverified** whether the flash is visible. Fallback: present the window after the first answer | `crates/odc-app/src/main.rs`, `connect_activate`, read 2026-11-19 |
