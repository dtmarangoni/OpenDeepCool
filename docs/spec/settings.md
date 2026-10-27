# Settings that change what the cooler shows

Every DeepCreative 1.2.14 setting that affects the LP360: its values, its default on first start, and what comes back after a restart. What each setting does to the picture is in [display.md](display.md) and [music.md](music.md); evidence labels are defined in [README.md](README.md).

## Settings

| Setting | Where in DeepCreative | Values | First-start default | Restored after a restart | Effect |
|---|---|---|---|---|---|
| Mode | LP page: the mode family and its option | modes 1–22; OpenDeepCool adds 23 (Dancer) | **1** (Load Mode → Separate → CPU) | yes | [display.md](display.md#modes), [music.md](music.md#modes) |
| Rotate Screen | LP page: one button | 0–3. Each press adds a quarter turn clockwise, and 3 wraps to 0 | **0** | yes | [rotation](display.md#rotation) |
| Temperature Display | app Settings page, for all devices | Celsius (0), Fahrenheit (1) | **Celsius** | yes | modes 7–9, 18 and 19 ([values](display.md#values)) |
| CPU Fan I/O, Pump Fan I/O | LP page | one of the board's fan inputs | none chosen | unverified: under Wine the list is empty | the RPM on DeepCreative's own device card, never the cooler ([sensors.md](sensors.md#fan-and-pump-speed)) |

Evidence:
- **Defaults:** capture and code: DeepCreative started with its data folder moved aside, 2026-10-22.
  - It drew mode 1 at once, unrotated, with the real CPU usage.
  - Its stores held mode 1, rotation 0, no fan inputs, and the unit Celsius.
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

The same as DeepCreative's first start: mode 1, rotation 0, Celsius, no fan inputs chosen.

OpenDeepCool's own settings: Music peak dots on, bar fall moderate, peak fall slow (Winamp's defaults) ([music.md](music.md#opendeepcools-spectrum-precisely); owner, 2026-10-25).

GPU: the dedicated GPU when the PC has one, otherwise the integrated GPU ([sensors.md](sensors.md#gpu-choice)).

## What the settings app must offer

The input for the settings app's design (PRD, Phase 10). The `ui-ux-designer` agent designs from this list; the app has its own design, not DeepCreative's ([below](#deepcreative-interface-quirks-not-reproduced)).

**Only the cooler** (owner, 2026-10-23): the app covers the display's settings and the cooler's fans and pump, nothing else. It shows no other hardware information and no controls for the CPU, GPU, other fans or power. Users use dedicated tools for those, as OpenRGB handles lighting. Keep the app as small as that allows.

- **Every setting above:** mode, rotation, temperature unit, the cooler's fan and pump inputs.
- **Changes apply at once** (2026-10-25): every choice in the app goes to the service, and so to the cooler, when it's made. There's no Apply button and no unsaved state. The preview shows what the cooler shows.
- **No app-only settings in v1** (2026-10-25): the app offers no settings of its own (start at login, language); it's in English.
- **A configuration screen** (owner, 2026-10-25): every OpenDeepCool setting sits on one configuration screen, except choosing the mode and the temperature unit, which stay where the mode is chosen. So the configuration screen holds the rotation, the GPU choice, the cooler's fan and pump inputs, and the Music settings below.
- **Music settings** (owner, 2026-10-25): peak dots on or off (on by default); a *bar fall* slider and a *peak fall* slider, each with Winamp's five stops, slower, slow, moderate, fast and faster (defaults: moderate for bars, slow for peaks, as in Winamp). The preview draws them the same way ([music.md](music.md#opendeepcools-spectrum-precisely)).
- **Mode names** (owner, 2026-10-24): Load, Temperature, Power and Music. Never DeepCreative's "Temperature Guardian", "Power Guardian" or "… Mode" ([display.md](display.md#modes)).
- **Refresh button** (owner, 2026-10-24): syncs the display. The service takes fresh readings at once and redraws the picture from its home position, so a scrolling value starts its cycle again (and stops at home if it now fits). Allowed to the same users as settings changes.
- **No layout setting** (owner, 2026-10-24): the app has no static or scrolling option. Every mode is static while its values fit (99 or less) and scrolls on its own while one is 100 or more ([display.md](display.md#modes)).
- **Full values** (owner, 2026-10-24): the cooler and the preview show every value in full, with no cap below 9999. Every value is capped at 9999 everywhere: a reading of 10 000 or more shows `9999` on the cooler, in the preview and anywhere else the app shows it (owner, 2026-10-25). A value that doesn't fit the static layout scrolls on its own until it fits again; in Music Mode only the value scrolls, never the spectrum ([display.md](display.md#values)).
- **GPU choice** (owner, 2026-10-23; rules in [sensors.md](sensors.md#gpu-choice)):
  - shown only when the PC has more than one GPU. With one GPU, dedicated or integrated, there's no GPU control at all;
  - a dropdown of the PC's GPUs, dedicated before integrated, set to the dedicated GPU on the first run. Each GPU is named by its kind and its product name from the system's PCI ID list (`/usr/share/hwdata/pci.ids`, package `hwdata`), or by its kind, driver and PCI address when the list has no entry (2026-10-25);
  - the user's choice is kept across reboots;
  - it only chooses which GPU the display's GPU metrics come from. The app shows no other GPU information.
- **Fan inputs** ([sensors.md](sensors.md#fan-and-pump-speed)):
  - one picker for the cooler's fans (DeepCreative's CPU Fan I/O) and one for its pump. Each lists every fan input in the PC except a graphics card's own fans, with its chip's name, its label when the driver has one, and its live RPM;
  - "none" is the default;
  - a saved input that's missing shows as not found;
  - the app shows the cooler's fan and pump RPM from the chosen inputs, capped at 9,999 RPM like every other value: a faster reading shows `9,999 RPM` (owner, 2026-10-25). These speeds never reach the cooler's display.
- **Shared settings:** one set for the whole PC, the same for every user. The user at the PC (the active local session) changes them without a password (PRD, Settings app ↔ service).
- **Only the user at the PC** (owner, 2026-10-26, replacing the read-only view of 2026-10-23): a remote or inactive session can't use OpenDeepCool at all. The service refuses it every read, every change and the preview's frames, so the app there shows only that OpenDeepCool is used from the session at the PC. No other session receives the settings or the cooler's picture.
- **Back from standby** (owner, 2026-10-26): the monitor may turn off for inactivity, the screen may lock and the PC may sleep. None of it needs anything from the user. While the PC is idle the cooler keeps showing live values, and after it wakes OpenDeepCool is back on its own: the cooler shows the saved mode again ([protocol.md](protocol.md#standby)), the preview follows it, and Music's sound returns.
- **Availability** (owner, 2026-10-24):
  - A mode can't be chosen when a metric it shows is **persistently unavailable**: no driver rule, no device, no permission, or a reading the rules exclude (for example the power of integrated AMD graphics). The app shows it disabled, with the one-line reason.
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
  - in the fan and pump pickers, a note when no fan sensors were found.

  Details, such as which source each value comes from, go to the service's journal, not the app.
- **Theme** (owner, 2026-10-23; PRD, Theme): the app follows the current Omarchy theme live, and uses OpenDeepCool's own default theme when it finds none. There's no theme setting in v1; choosing another desktop's theme comes after v1.
- **Opacity and blur** (owner, 2026-10-23: the app takes the theme's opacity, blur and so on): on Omarchy, Hyprland applies window opacity and blur to every window by its own rules, so the app must not override them. Its own surfaces use the alpha values of the current theme's `shell.toml`: background, and the hover, focus, selected and pressed fills (PRD, Theme).
- **Music Mode audio** comes from the session of the user at the PC. Before anyone logs in, the spectrum shows silence. Sound is taken only while a Music mode is shown (owner, 2026-10-26).

## DeepCreative interface quirks, not reproduced

- **Separate options hidden:** Load Mode's Separate options (modes 1–3) sit below the visible part of the options box, and under Wine there's no scrollbar, so only scrolling reveals them. Code: LP page; owner: found by scrolling, 2026-10-22.
- **Wrong tab on start:** when the saved mode is 7 (Temperature Guardian → CPU), DeepCreative's window opens on the Load Mode tab, because its check treats modes up to 7 as Load Mode. The cooler still shows mode 7. Code: LP page.
- **OpenDeepCool's settings app** has its own design, not DeepCreative's pages and tabs (owner, 2026-10-22; PRD, Phase 10). These quirks don't carry over.

## Not covered

- Hidden display settings: after v1 (PRD, What We're NOT Building).
- App settings that don't touch the cooler (start at login, language): the settings app's own concern (Phase 10).
