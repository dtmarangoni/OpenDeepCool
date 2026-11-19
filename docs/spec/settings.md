# Settings that change what the cooler shows

Every DeepCreative 1.2.14 setting that affects the LP360: its values, its default on first start, and what comes back after a restart. What each setting does to the picture is in [display.md](display.md) and [music.md](music.md); evidence labels are defined in [README.md](README.md).

## Settings

| Setting | Where in DeepCreative | Values | First-start default | Restored after a restart | Effect |
|---|---|---|---|---|---|
| Mode | LP page: the mode family and its option | modes 1–22; OpenDeepCool adds 23 (Dancer) | **1** (Load Mode → Separate → CPU) | yes | [display.md](display.md#modes), [music.md](music.md#modes) |
| Rotate Screen | LP page: one button | 0–3. Each press adds a quarter turn clockwise, and 3 wraps to 0 | **0** | yes | [rotation](display.md#rotation) |
| Temperature Display | app Settings page, for all devices | Celsius (0), Fahrenheit (1) | **Celsius** | yes | modes 7–9, 18 and 19 ([values](display.md#values)) |

Evidence:
- **Defaults:** capture and code: DeepCreative started with its data folder moved aside, 2026-10-22.
  - It drew mode 1 at once, unrotated, with the real CPU usage.
  - Its stores held mode 1, rotation 0 and the unit Celsius.
  - The data folder was restored afterwards.
- **Restore:** capture: mode 9, Fahrenheit and rotation 2 were set, DeepCreative was quit and restarted, and every steady frame was the restored picture (render check). Owner: 2026-10-22, `32°F` twice, upside down and scrolling, on the cooler after the restart. A previous session restored mode 4 the same way.
- **Rotate Screen** is one button that steps the value: code, LP page.
- **Unit:** the Celsius/Fahrenheit choice is one global setting. The device options also hold a field with the same name, but the cooler doesn't follow it: with that field at 1 and the global setting at Celsius, mode 7 drew `57°C` (capture, 2026-10-22). It sits with the hidden settings and wasn't researched further (AGENTS.md rule 8).

## Start-up

- DeepCreative's main process drives the cooler from the saved settings as soon as it starts, even while its window shows another page. Capture: restart of 2026-10-22, window on the home page.
- Sequence: the start-up command `03 08` ([protocol.md](protocol.md#start-up-command)), then frames of the saved picture.
  - A static mode appears with the first frame (first start: mode 1).
  - After a restart into a scrolling mode, DeepCreative sent blank frames for 3 s, then the text scrolled in from the right edge. Why it waited 3 s is unverified.
  - Checked again on 2026-10-23, restarting into a scrolling °F mode rotated 180°: the first frame came about 1 s after `03 08`, then 30 blank frames over 3.0 s, then the text scrolled in. On the cooler, the owner saw the text start quickly ("like 2 seconds"); blank frames look the same as a dark display. With the screen rotated 180°, the text enters from the right edge of the unrotated picture, so on the cooler it moves left to right, upside down.
  - DeepCreative sends no frames until its sensor helper delivers values: with the helper started 105 s late, only `03 08` went out until then (research rig, 2026-10-23).
- **OpenDeepCool:** sends `03 08`, then the saved mode's picture at its home position straight away, without the 3 s wait. If a value is 100 or more, the text starts moving from home, as after a [mode switch](display.md#mode-switches).

## Where the settings live

- DeepCreative keeps them in its own data folder, under the Wine user's application data. Each change on the LP page is saved at once through its main process (code: LP page).
- Mode, rotation and unit are stored in the app's database. The per-device file kept its date while they changed, so it isn't where they live. This is inferred from file dates and strings, unverified.
- **OpenDeepCool:** keeps one settings file for the whole PC, in the display service's state directory, the same for every user (PRD, Service model; owner, 2026-10-23). It never reads DeepCreative's files.
  - A setting that a later build added, and that this build doesn't know, is kept and written back unchanged, so moving to an older build and back loses nothing (owner, 2026-10-27; solution review P7-12).

## OpenDeepCool's defaults

The same as DeepCreative's first start: mode 1, rotation 0, Celsius.

OpenDeepCool's own settings: Music peak dots on, bar fall moderate, peak fall slow (Winamp's defaults) ([music.md](music.md#opendeepcools-spectrum-precisely); owner, 2026-10-25); and the theme: use the system's theme on, OpenDeepCool's own theme dark (owner, 2026-11-18). Both theme settings are the service's, kept in its one settings file for the whole PC, so "no app-only settings in v1" stays true; the service stores them and never interprets a theme ([dbus.md](dbus.md): `SetUseSystemTheme`, `SetDarkTheme`). A file written before them loads with both on.

GPU: the dedicated GPU when the PC has one, otherwise the integrated GPU ([sensors.md](sensors.md#gpu-choice)).

## What the settings app must offer

The input for the settings app's design (PRD, Phase 10). The `ui-ux-designer` agent designs from this list; the app has its own design, not DeepCreative's ([below](#deepcreative-interface-quirks-not-reproduced)).

**UI review** (owner, 2026-11-18; decided, not yet built): the changes in the [Phase 11a plan](../prp/plans/phase-11a-settings-app-ui-review.plan.md) supersede the lines below where they disagree. The app shows no fans or pump (removed on 2026-11-18, kept in [fans-and-pump.md](fans-and-pump.md)), and the lines below no longer mention them. The temperature unit sits on the configuration screen, after Rotation (built in Task 7). A Theme card, first on the configuration screen, holds two settings the service keeps for the whole PC (use the system theme; dark or light), so "No app-only settings in v1" stays true, and OpenDeepCool's own theme is redesigned in the same review (owner, 2026-11-18); both are built (Task 11). The pairs of values are centred on the cooler as one block (built in Task 12), and the tiles on the mode screen are drawn live, each as the cooler would draw its mode from the service's readings (Tasks 13 and 14, below). The owner's second list of 2026-11-19 (sixteen small changes, Tasks 17–32 of the plan) removes the app's terminal commands, several hint lines and the dismissed-dialog note, moves the theme line, and makes the switches, the slider knobs and some gaps smaller; each task updates the lines here that it changes. Each task changes these lines when it is built.

**Only the cooler** (owner, 2026-10-23): the app covers the display's settings, nothing else. It shows no hardware information beyond what the display needs, and no controls for any hardware. Users use dedicated tools for those, as OpenRGB handles lighting. Keep the app as small as that allows.

- **Every setting above:** mode, rotation, temperature unit.
- **Changes apply at once** (2026-10-25): every choice in the app goes to the service, and so to the cooler, when it's made. There's no Apply button and no unsaved state. The preview shows what the cooler shows.
- **No app-only settings in v1** (2026-10-25): the app offers no settings of its own (start at login, language); it's in English.
- **A configuration screen** (owner, 2026-10-25): every OpenDeepCool setting sits on one configuration screen, except choosing the mode, which stays where the mode is chosen. So the configuration screen holds the rotation, the temperature unit (after the rotation; owner, 2026-11-18, moved from the mode screen), the GPU choice and the Music settings below.
- **Music settings** (owner, 2026-10-25): peak dots on or off (on by default); a *bar fall* slider and a *peak fall* slider, each with Winamp's five stops, slower, slow, moderate, fast and faster (defaults: moderate for bars, slow for peaks, as in Winamp). The preview draws them the same way ([music.md](music.md#opendeepcools-spectrum-precisely)).
- **Mode names** (owner, 2026-10-24): Load, Temperature, Power and Music. Never DeepCreative's "Temperature Guardian", "Power Guardian" or "… Mode" ([display.md](display.md#modes)).
- **Refresh button** (owner, 2026-10-24): syncs the display. The service takes fresh readings at once and redraws the picture from its home position, so a scrolling value starts its cycle again (and stops at home if it now fits). Allowed to the same users as settings changes.
- **No layout setting** (owner, 2026-10-24): the app has no static or scrolling option. Every mode is static while its values fit (99 or less) and scrolls on its own while one is 100 or more ([display.md](display.md#modes)).
- **Full values** (owner, 2026-10-24): the cooler and the preview show every value in full, with no cap below 9999. Every value is capped at 9999 everywhere: a reading of 10 000 or more shows `9999` on the cooler, in the preview and anywhere else the app shows it (owner, 2026-10-25). A value that doesn't fit the static layout scrolls on its own until it fits again; in Music Mode only the value scrolls, never the spectrum ([display.md](display.md#values)).
- **GPU choice** (owner, 2026-10-23; rules in [sensors.md](sensors.md#gpu-choice)):
  - shown only when the PC has more than one GPU. With one GPU, dedicated or integrated, there's no GPU control at all;
  - a dropdown with **Automatic** first, the default, then the PC's GPUs, dedicated before integrated. Automatic reads the first GPU in that order, and choosing it again clears the user's choice (owner, 2026-11-04). Each GPU is named by its kind and its product name from the system's PCI ID list (`/usr/share/hwdata/pci.ids`, package `hwdata`), or by its kind, driver and PCI address when the list has no entry (2026-10-25); two GPUs that would read the same also show their PCI addresses (Phase 10 decisions log D36, pending the owner's review);
  - the user's choice is kept across reboots, until a GPU is added or removed: the service then clears it at start, back to Automatic, and the user picks again if they want (owner, 2026-11-04);
  - it only chooses which GPU the display's GPU metrics come from. The app shows no other GPU information.
- **Start at boot** (owner, 2026-11-10; Phase 11 plan, Task 3): the package turns nothing on when it's installed, as Arch's packages don't. One switch in the app turns OpenDeepCool on or off for the whole PC. It isn't an app setting: it shows and changes systemd's own state for OpenDeepCool's units.
  - **On:** the display service starts at every boot and starts now, and the sound helper starts in every user's session at login, and now in this one. Turning it on always starts at once whatever isn't running yet, besides setting it to start at boot (owner, 2026-11-11). **Off:** neither starts any more, and both stop.
  - Every change asks for the administrator's password, every time (polkit `auth_admin`). Nothing else in the app asks for one.
  - **The switch is on only when OpenDeepCool is fully on and ready to use** (owner, 2026-11-11): the display service is enabled at boot and running, and the sound helper is enabled for every user and running in this session. In any other case it's off, a service that stopped included, with a note saying what's missing, and flipping it on finishes the setup and starts what isn't running. There's no separate start button: the switch does it (owner, 2026-11-11). When the only part missing is the sound helper in this session, flipping on just starts it, without a password, since nothing changes for the whole PC (owner, 2026-11-11).
    - What counts: only these two units. The display service starts its one-shot helper itself (the GPU ranking), which runs and exits, so it doesn't count. The metrics aren't services: the display service reads them from the kernel's files. The sound helper only waits in the session, and takes sound only while a Music mode is shown (owner, 2026-10-26).
    - The switch shows systemd's state, read without a password, so a change made in a terminal, or a service that stops, shows too.
  - It works while the service isn't running, which is how a new install is turned on.
  - When the change doesn't happen (the password is refused, no polkit agent runs in the session, or a step fails), the switch goes back to systemd's state, with a note saying what happened. A closed password dialog changed nothing and has no note (owner, 2026-11-19).
  - **No terminal commands in the app** (owner, 2026-11-19; they were shown from 2026-11-11 to 2026-11-19). The commands that do the same from a terminal are in the README, the manual: to turn on, `sudo systemctl enable --now opendeepcool.service`, `sudo systemctl --global enable opendeepcool-audio.service` and `systemctl --user start opendeepcool-audio.service` (which starts the sound for the Music modes in this session at once, without a password, instead of at the next login); to turn off, `sudo systemctl disable --now opendeepcool.service` and `sudo systemctl --global disable opendeepcool-audio.service`.
  - A missing polkit agent ends the change at once: the app runs `pkexec` with `--disable-internal-agent`, so `pkexec` never falls back to asking for the password in a terminal where nobody looks (pkexec(1); owner, 2026-11-11).
  - Only the user at the PC may flip it, as with every other change (polkit `allow_active` only).
- **Shared settings:** one set for the whole PC, the same for every user. The user at the PC (the active local session) changes them without a password (PRD, Settings app ↔ service).
- **Only the user at the PC** (owner, 2026-10-26, replacing the read-only view of 2026-10-23): a remote or inactive session can't use OpenDeepCool at all. The service refuses it every read, every change and the preview's frames, so the app there shows only that OpenDeepCool is used from the session at the PC. No other session receives the settings or the cooler's picture.
- **Back from standby** (owner, 2026-10-26): the monitor may turn off for inactivity, the screen may lock and the PC may sleep. None of it needs anything from the user. While the PC is idle the cooler keeps showing live values, and after it wakes OpenDeepCool is back on its own: the cooler shows the saved mode again ([protocol.md](protocol.md#standby)), the preview follows it, and Music's sound returns.
- **Availability** (owner, 2026-10-24):
  - A mode can't be chosen when a metric it shows is **persistently unavailable**: no driver rule, no device, no permission, a GPU driver updated since boot, a driver's library that can't be loaded or started, or a reading the rules exclude (for example the power of integrated AMD graphics). The app shows it disabled, with the one-line reason.
  - When a whole device is missing (for example no usable GPU), every mode that shows any of its metrics is disabled: Load, Temperature and Power options, combined or separate, and Music variants with that metric.
  - When the device works but one metric doesn't (for example GPU power can't be read), only the modes that show that metric are disabled. On Intel GPUs, Gpu usage has no documented reading, so every mode showing Gpu usage is disabled there ([sensors.md](sensors.md#driver-rules)).
  - This applies to every metric, not only the GPU's.
  - A short read failure, or the first second after the service starts, doesn't disable anything; the cooler shows dashes meanwhile ([display.md](display.md#unavailable-values)).
  - If the saved mode loses a reading after it was chosen, it stays the saved mode: the cooler shows dashes and the app shows the reason.
- **GPU list order** (owner, 2026-10-24): dedicated GPUs first, then integrated, then GPUs whose driver doesn't say, each group by PCI address. A GPU whose driver has no reading rule is still listed, last; choosing it disables the GPU modes with the reason "no reading for this GPU's driver" ([sensors.md](sensors.md#gpu-choice)). A GPU whose kind couldn't be asked, because the query failed, is listed with the unknown ones, with a short note saying so (owner, 2026-10-27; solution review P6-07).
- **Status**, kept short:
  - whether the cooler is connected and whether the service is running. A cooler that's plugged in but that the service can't open (a permissions problem, such as a missing udev rule) is told apart from one that isn't connected, with what to do about it (owner, 2026-10-27; solution review P5-02);
  - in a Music mode, whether sound is coming in from the session at the PC (the service's `AudioConnected`; without it the spectrum is silent);
  - the settings app talks to the service over the D-Bus system bus; polkit lets the user of the active local session read and change settings without a password, and refuses every other session (PRD, Settings app ↔ service);
  - when a value the chosen mode needs is unavailable, a one-line reason, for example "no reading for this GPU's driver";
- **What the service gives the app for this** (Phase 10; [dbus.md](dbus.md)): the preview's frames, every frame the engine draws, rotation included, whether or not the cooler got it, sent only to the app that watches them (`WatchFrames`, `Frame`); the GPU the readings come from, so Automatic can name it (`GpuInUse`); and each GPU's PCI vendor and device ID, for its product name, and whether its driver has a reading rule (`Gpus`).
- **What the service gives the app for the live tiles** (Phase 11a Task 13; [dbus.md](dbus.md)): the readings, each metric's exact fraction, the one with no value left out (`Readings`, in the state, sent to the app that watches frames), so the app draws the same pictures the cooler would from the very values it would, with `odc-render`; and, while a Music mode is shown and the session's sound is attached, the spectrum's 14 levels once per tick (`Levels`), so the Music tiles move with the sound. A metric with no value is drawn as dashes, as on the cooler, and with no `Levels` the Music tiles show their still sample. The app reads no sensor and no sound itself.

  Details, such as which source each value comes from, and the first readings at each start, exact (`first readings: CPU Temperature 45000/1000, CPU Usage not-yet, …`; owner, 2026-11-04), go to the service's journal, not the app.
- **Theme** (owner, 2026-10-23; PRD, Theme; the Theme card, owner, 2026-11-18, D6): a card first on the Configuration page, with two settings the service keeps for the whole PC ([dbus.md](dbus.md): `UseSystemTheme`, `DarkTheme`; defaults on, dark; the app writes no file).
  - **Use the system's theme** (on): the app follows an Omarchy theme live when it finds one (`colors.toml` that parses as TOML), else the system's dark or light preference applied to OpenDeepCool's own theme (no preference shows the dark one). The dark or light control is dimmed and shows what the system gives.
  - **No system theme found** (no Omarchy theme and an unsupported preference): the switch is off and dimmed with a warning that OpenDeepCool can't find the system's theme and is using its own, and the dark or light control is usable.
  - **Off**: the dark or light control picks OpenDeepCool's own theme, dark or light, whatever the system says. Choosing another desktop's theme comes after v1.
  - **OpenDeepCool's own theme** is designed in the owner's UI review (Phase 11a Task 11; the design's "OpenDeepCool's own theme"): dark and light, in the system font, with the contrast the design's table pins.
  - **How the system's preference is known** (read 2026-11-17, measured 2026-11-18): the XDG desktop portal's Settings interface has a standardized key, `org.freedesktop.appearance` `color-scheme`: 0 no preference, 1 prefer dark, 2 prefer light, unknown values meaning 0, with a `SettingChanged` signal for a live change. GTK 4.20 and later exposes the system's choice as `GtkSettings:gtk-interface-color-scheme` (`UNSUPPORTED` where the system has none, then `DEFAULT`, `DARK` or `LIGHT`), which GDK fills from the portal and keeps live when the portal's name is activatable; the app reads that property and needs no bus code of its own. It is a standard, but not every system sets it: a bare window manager may not answer, which is the "no system theme found" state.
- **Opacity and blur** (owner, 2026-10-23: the app takes the theme's opacity, blur and so on): on Omarchy, Hyprland applies window opacity and blur to every window by its own rules, so the app must not override them. Its own surfaces use the alpha values of the current theme's `shell.toml`: background, and the hover, focus, selected and pressed fills (PRD, Theme).
- **Music Mode audio** comes from the session of the user at the PC. Before anyone logs in, the spectrum shows silence. Sound is taken only while a Music mode is shown (owner, 2026-10-26).

## DeepCreative interface quirks, not reproduced

- **Separate options hidden:** Load Mode's Separate options (modes 1–3) sit below the visible part of the options box, and under Wine there's no scrollbar, so only scrolling reveals them. Code: LP page; owner: found by scrolling, 2026-10-22.
- **Wrong tab on start:** when the saved mode is 7 (Temperature Guardian → CPU), DeepCreative's window opens on the Load Mode tab, because its check treats modes up to 7 as Load Mode. The cooler still shows mode 7. Code: LP page.
- **OpenDeepCool's settings app** has its own design, not DeepCreative's pages and tabs (owner, 2026-10-22; PRD, Phase 10). These quirks don't carry over.

## Not covered

- Hidden display settings: after v1 (PRD, What We're NOT Building).
- App settings that don't touch the cooler (start at login, language): the settings app's own concern (Phase 10).
