# Settings app design

The design of OpenDeepCool's GTK4 settings app (PRD, Phase 10): its structure, each screen's purpose, its GTK4 widgets, its states, its theme, keyboard use and accessible names, and the coverage check. The mockups are in [mockups/](mockups/); each one has a theme switch (a sample Omarchy theme and the default theme, dark and light). `odc-mockups` (`tools/mockups/`) writes them from the states they show and the style sheet and script in [mockups-src/](mockups-src/), and its tests check the palettes against the contrast tables below.

Sources: [settings.md](../spec/settings.md) "What the settings app must offer", [display.md](../spec/display.md), [music.md](../spec/music.md), [sensors.md](../spec/sensors.md), the PRD (Phase 10, Technical Approach → Theme, Decisions Log), the Phase 11 plan (Recommendation, "The Start at boot switch", and Task 3) and the designer brief (`.claude/agents/ui-ux-designer.md`). Every reading, chip name, GPU name and PCI address in this document and in the mockups is made up.

Widget names are GTK 4 classes. Anything not checked against docs.gtk.org is marked **unverified**; the Phase 10 plan checks those first.

## Goals

- **Feature parity, not interface parity.** Every LP360 feature is reachable. There are no DeepCreative tabs and no Combined/Separate split.
- **Few screens, few clicks.** There is one window with two pages, and every mode is one click away.
- **Fast.** It opens straight onto the current settings, with no animation in the app's own chrome. Idle CPU use is nil: the preview redraws only when the cooler's picture changes, and nothing polls.
- **Only the cooler.** CPU, GPU and RAM values appear only inside display pictures.
- **Themed.** The app follows the current Omarchy theme live and uses its own default theme elsewhere. It never sets window opacity or blur.

## Decisions

The owner delegated the design choices (2026-10-25). These are final for v1.

| # | Question | Choice | Why |
|---|---|---|---|
| D1 | Window structure | One window: a persistent preview column on the left, and a `GtkStack` on the right with two pages, **Display** and **Configuration** | The preview stays in view while rotation, GPU or Music settings change. There is no second window to tile in Hyprland |
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

- **Size:** default 1040 × 720 px, minimum 720 × 560 px. The tile rows reflow (`GtkFlowBox`), and each page scrolls vertically. Whether Hyprland honours a GTK window's minimum size when it tiles is **unverified**.
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
  - **One `GtkStack`**, `pages`, with three children: `display`, `config` and `off` (the off screen). It is `GTK_STACK_TRANSITION_TYPE_NONE` and homogeneous, so the window doesn't change size when the page changes. The not-available message is in the `screens` stack around it, as before.
  - **A page** is a vertical `GtkBox` of its page bar (below) and a `GtkScrolledWindow` holding its content, so the bar never scrolls away. Each page keeps its own scroll position while the other shows. The off screen has no page bar.
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

## Preview column

**Purpose:** what the cooler shows right now, plus the short status (PRD Phase 10: preview, status).

- **Preview:** a `GtkDrawingArea` of 210 × 210 px, 15 px per dot, in a 226 × 226 px frame with the surface colour and 8 px of padding, as the mockups draw it (decisions log D51).
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
  - service: "Service running" (green); while it's down, on the [off screen](#the-off-screen), "Service off" (hollow dot: it doesn't start at boot, off on purpose) or "Service not running" (red: it's set to start but stopped, failed or starting);
  - Music sound, **only while a Music mode is chosen** (`AudioConnected`): "Sound connected" or "No sound: spectrum is silent".

## Display page

**Purpose:** choose the mode (settings.md: the only setting outside the configuration screen). The page is home: the window opens on it, and no tab leads to it.

- **Page bar** at the top, outside the scrolled area ([Window](#window)): the title "Display" at the start, and the Refresh icon at the end ([Refresh](#refresh)). The icon sits at the right end of the bar, above the Load group and level with the title; the preview column is on its left and isn't involved.
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

**Purpose:** every setting but the mode, on one screen (settings.md, A configuration screen; the temperature unit joined it on 2026-11-18). The gear opens it, and the back icon, Escape or Alt+Left leave it ([Window](#window)). It is a [page bar](#window) with the back icon and the title "Configuration", and under it, in the scrolled area, three sections, Display, Music and Startup, each a surface-coloured `GtkBox` card with a heading and rows. A row is a label (and an optional hint) on the left and the control on the right. The third, Startup, holds no setting of the service: its switch shows and changes systemd's state for OpenDeepCool's units (settings.md, Start at boot). Phase 11a Task 11 adds the Theme card in front of Display.

### Display

- **Rotation:** "Turns the picture clockwise". Four grouped `GtkToggleButton`s, `0°`, `90°`, `180°`, `270°`, which call `SetRotation`. The preview turns with the next frame.
- **Temperature unit,** the row after Rotation: "Temperature unit", with the hint "Used wherever the display shows a temperature". The same control as Rotation's: two grouped `GtkToggleButton`s, `°C` and `°F`, linked, with the accessible names "Celsius" and "Fahrenheit" (`gtk_toggle_button_set_group`, verified).
  - A toggle calls `SetTemperatureUnit` and goes back to the saved unit until the service confirms the change (`StateChanged`), as every control does ([Refused changes](#refused-changes)).
  - In a mode without a temperature, the unit is only saved and the cooler doesn't change (display.md, Mode switches). Nothing in the app needs to say so, because the preview stays the same.
  - The temperature tiles on the Display page redraw in the confirmed unit.
- **GPU:** "Where the display's GPU values come from".
  - A `GtkDropDown`, shown only when the PC has more than one GPU. Its `factory` draws the button, with the label alone. Its `list-factory` draws the popup's entries: a check mark, the label, and a muted note at the right (GtkDropDown:list-factory, "the factory for populating list items in the popup", docs.gtk.org, read 2026-11-06).
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

- **Peak dots:** a `GtkSwitch`, on by default. It calls `SetMusicPeaks`.
- **Bar fall** and **Peak fall:** two horizontal `GtkScale`s on a 0–4 adjustment (step 1, no value drawn), with five labelled marks: Slower, Slow, Moderate, Fast, Faster (`gtk_scale_add_mark`, verified).
  - Bar fall defaults to Moderate, Peak fall to Slow.
  - A change calls `SetMusicBarFall` or `SetMusicPeakFall` with the stop's name when the value lands on a new stop.
  - Snapping to whole stops during a drag (with `round-digits` 0 and a `change-value` handler) is **unverified**.
- **Hint:** "For every Music option except Dancer. Changes show on the next frame." In a mode without a spectrum, the change is only saved (music.md, point 7).

### Startup

- **Start at boot:** "For the whole PC". A `GtkSwitch`, on only when OpenDeepCool is ready to use, with a note under the row when one applies, and the card's hint: "When it's on, the display service starts at every boot, and the sound for the Music options at every login. Each change asks for the administrator's password."
- It's the last card, because it's flipped about once per PC. Its states, texts and flows are in [Start at boot](#start-at-boot).

## Banners

A banner is a `GtkRevealer` (transition none) holding a `GtkBox`: an icon, a `GtkLabel`, and an optional button. GtkInfoBar is deprecated since GTK 4.10, and GTK's docs suggest exactly this construction (verified). Banners stack in this order:

| Banner | When | Text | Effect on the window |
|---|---|---|---|
| Cooler not connected (warning) | `CoolerState` is `not-found` | "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back." | Nothing is disabled. The preview keeps mirroring the service's picture, captioned "Not on the cooler: it isn't connected". Status "Cooler not connected" |
| Cooler, no access (error) | `CoolerState` is `no-access`: the service finds the cooler but can't open its device node, a permissions problem such as a missing udev rule (settings.md, Status; solution review P5-02) | "**The cooler is plugged in, but the service can't open it.** The udev rule that gives the service access is missing or not applied yet. Restart the PC; if that doesn't help, reinstall OpenDeepCool. The journal says more." | Nothing is disabled, and changes are saved. The preview keeps mirroring the service's picture, captioned "Not on the cooler: the service can't open it". Status "Cooler: no access", with a red dot. There's no button and no command, because both fixes are the user's own and the app can't do either |
| Cooler, sending fails (warning) | `CoolerState` has been `write-failed` for 2 s | "**The cooler is connected, but sending it the picture fails.** The service keeps trying by itself, and changes are still saved. The journal says more." | Nothing is disabled. Caption "Not on the cooler: sending it fails". Status "Cooler: sending fails", with a yellow dot |

- **No service banner** (since 2026-11-11, D10). While the bus name `io.github.dtmarangoni.OpenDeepCool1` has no owner, the window shows the [off screen](#the-off-screen): the gear is insensitive, the pages (and so Refresh) aren't shown, the preview is all unlit, and the status says "Cooler: unknown" and "Service off" or "Service not running". Its note and its terminal commands, `sudo systemctl enable --now opendeepcool.service` first, say what the banner said. The app watches `NameOwnerChanged` and connects by itself once the name has an owner ([Talking to the service](#talking-to-the-service)); it doesn't poll.
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
- **Row:** the label "Start at boot", the muted subtitle "For the whole PC", and the switch at the right (`valign` center). Under the row, the note, when one applies. At the card's foot, the hint: "When it's on, the display service starts at every boot, and the sound for the Music options at every login. Each change asks for the administrator's password."

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
- **Then** the Startup card without its heading, with [the note](#the-note) when one applies (a new install has none), and the terminal commands to turn it on, "From a terminal instead:".
- **No banner.** The note says what's wrong, and the switch is the fix.
- **Widgets:** a vertical `GtkBox` centred in the content area, holding a `GtkBox` (group "Service message") around the heading and text `GtkLabel`s (wrapping, centred), then the card, then the terminal commands. The off screen is the `off` child of the `pages` stack, in its own `GtkScrolledWindow` and without a page bar. The header bar shows the title, the insensitive gear and the close icon ([the header, by state](#window)); the preview column is as before while the service is down.
- **Focus:** the switch takes the first focus when it's sensitive, so Space turns OpenDeepCool on. The gear is insensitive here, so a page that was closed by the service going down gives its focus to this switch, not to the gear.

### Who may flip it

- Only the user of the active local session, as with every change: the polkit action `io.github.dtmarangoni.OpenDeepCool1.start-at-boot` has `allow_active` `auth_admin` (the administrator's password at every change, no keep) and `allow_any` and `allow_inactive` `no` (Phase 11 plan). It's the only password in the app.
- When the app is [not available in this session](#not-available-in-this-session), the window shows the message alone, as before.
- While the service is down, the app can't ask it about the session, but logind can tell: its session's `Active` and `Remote` properties (org.freedesktop.login1(5), read 2026-11-11). The app already follows `Active` to connect again ([Unverified points](#unverified-points-answered-phase-10-task-1-2026-11-06)). In a session that isn't both active and local, the off screen has no Startup card, since polkit would refuse the flip there: only its heading, its text and the terminal commands.

## Not available in this session

**When:** the service refuses `GetState()` with `NotAuthorized`, because the app runs in a remote or inactive session (owner, 2026-10-26). The service refuses such a session every read, change and preview frame (polkit `allow_active` only), so the app has nothing to show. This state replaces the view-only state, which no longer exists.

```
┌ GtkApplicationWindow ──────────────────────────────────────────────────────┐
│ GtkHeaderBar                    OpenDeepCool                          [✕]  │
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
  - the same `GtkHeaderBar`, with the title and the close icon only. The gear is hidden (`visible` false), not just insensitive; Refresh is on the Display page, which isn't shown. The close icon is the app's own: the desktop's window buttons are off ([Window](#window));
  - the body is a vertical `GtkBox`, centred in the window (`halign` and `valign` center), holding two `GtkLabel`s, the heading and the text. Both wrap (`wrap` true, `justify` center), so a narrow window wraps them instead of cutting them. They look like the off screen's heading and text;
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
| Normal, several GPUs | Header: the title in the middle, the gear and the close icon at the right. Display page: its page bar ("Display", the Refresh icon at the right) and the tile gallery; status "Cooler connected", "Service running". Configuration (the gear pressed): its page bar (the back icon, "Configuration"), then Rotation, Temperature unit and the GPU on Automatic with "Using Dedicated · Contoso Graphics R870." under it | [display.html](mockups/display.html), [configuration.html](mockups/configuration.html) |
| The Configuration page is open | The gear is pressed; the back icon, Escape or Alt+Left, or the gear again, return to the Display page, and focus goes to the gear | [configuration.html](mockups/configuration.html): click the back icon, the gear, or press Escape |
| The temperature unit | A row of the Display card after Rotation; the temperature tiles on the Display page follow it | [configuration.html](mockups/configuration.html), [display-scrolling.html](mockups/display-scrolling.html) (the tiles show °F) |
| GPU list open | Automatic first; a GPU whose kind query failed as "Unknown type · …" with "driver couldn't be asked"; a GPU without a reading rule last, with "no readings"; the line under the dropdown names the integrated GPU, which Automatic reads here | [configuration-gpu-list.html](mockups/configuration-gpu-list.html) |
| A GPU added or removed since the last start | The choice is back on Automatic (the service cleared it), with no note; the line names the GPU in use | [configuration.html](mockups/configuration.html) |
| A Music mode | The status line adds "Sound connected" or "No sound: spectrum is silent" | [display-music.html](mockups/display-music.html) |
| A value of 100 or more, °F | The preview mirrors the scrolling strip; the temperature tiles show °F | [display-scrolling.html](mockups/display-scrolling.html) |
| First start | Load · CPU, 0°, °C, peak dots on, Moderate / Slow, the GPU on Automatic | [first-start.html](mockups/first-start.html) |
| Cooler disconnected (`not-found`) | Cooler banner; status "Cooler not connected"; everything still changeable | [cooler-disconnected.html](mockups/cooler-disconnected.html) |
| Cooler found, no access (`no-access`) | Error banner with the fix; status "Cooler: no access"; caption "Not on the cooler: the service can't open it"; everything still changeable | [cooler-no-access.html](mockups/cooler-no-access.html) |
| Sending to the cooler fails, 2 s or more (`write-failed`) | Warning banner; status "Cooler: sending fails"; caption "Not on the cooler: sending it fails"; everything still changeable | [cooler-write-failed.html](mockups/cooler-write-failed.html) |
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
| Preview | `GtkDrawingArea` (image) | `Cooler display preview` | description: the mode caption, for example "Temperature · CPU + GPU" |
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
| Both pages reachable with no tab: the gear opens Configuration; the back icon, the gear again, Escape and Alt+Left return to Display | Header bar; the Configuration page's bar; [Keyboard](#keyboard) | display, configuration |
| The app closes from its own window | The close icon at the right of the header bar (the desktop's window buttons are off), Ctrl+W, Ctrl+Q | every mockup |
| Refresh, only for the user at the PC | The icon at the top right of the Display page; not drawn when not available in this session or while the service isn't running (the gear is insensitive then); F5 and Ctrl+R from either page while the service runs; a refusal shows as a refused change | display, service-not-running, not-available |
| Status: cooler connected | Status line; cooler banner | cooler-disconnected |
| Status: the cooler found but not usable (`no-access`), told apart from not connected, with the fix | Its own error banner; status line; caption | cooler-no-access |
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
| Every change asks for the administrator's password, and nothing else in the app does | The card's hint; the changing line | boot-changing |
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
| Hyprland and the window's minimum size | **Unverified**: it can be seen only in the owner's session, and no test opens a window there. The app sets its minimum size as designed; the optional attended tour looks at it | Window |
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
