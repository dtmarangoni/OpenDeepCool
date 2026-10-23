# Settings that change what the cooler shows

Every DeepCreative 1.2.14 setting that affects the LP360: its values, its default on first start, and what comes back after a restart. What each setting does to the picture is in [display.md](display.md) and [music.md](music.md); evidence labels are defined in [README.md](README.md).

## Settings

| Setting | Where in DeepCreative | Values | First-start default | Restored after a restart | Effect |
|---|---|---|---|---|---|
| Mode | LP page: the mode family and its option | modes 1–22, 100 and 100 + n | **1** (Load Mode → Separate → CPU) | yes | [display.md](display.md#modes), [music.md](music.md#modes) |
| Rotate Screen | LP page: one button | 0–3. Each press adds a quarter turn clockwise, and 3 wraps to 0 | **0** | yes | [rotation](display.md#rotation) |
| Temperature Display | app Settings page, for all devices | Celsius (0), Fahrenheit (1) | **Celsius** | yes | modes 7–9, 18 and 19 ([values](display.md#values)) |
| CPU Fan I/O, Pump Fan I/O | LP page | one of the board's fan inputs | none chosen | unverified: under Wine the list is empty | the RPM on DeepCreative's own device card, never the cooler ([sensors.md](sensors.md#fan-and-pump-speed)) |
| Saved Creative images | Creative Mode editor | a list of `{index, data}`, each a 14 × 14 grid | none | yes | [Creative Mode](display.md#creative-mode) |

Evidence:
- **Defaults:** capture and code: DeepCreative started with its data folder moved aside, 2026-10-22.
  - It drew mode 1 at once, unrotated, with the real CPU usage.
  - Its stores held mode 1, rotation 0, no fan inputs, and the unit Celsius.
  - The data folder was restored afterwards.
- **Restore:** capture: mode 9, Fahrenheit and rotation 2 were set, DeepCreative was quit and restarted, and every steady frame was the restored picture (render check). Owner: 2026-10-22, `32°F` twice, upside down and scrolling, on the cooler after the restart. A previous session restored mode 4 the same way.
- **Saved images:** the image the owner drew in the morning was still shown as mode 101 after the evening's reboot and restarts (capture: matrix sweep, 2026-10-22).
- **Rotate Screen** is one button that steps the value: code, LP page.
- **Unit:** the Celsius/Fahrenheit choice is one global setting. The device options also hold a field with the same name, but the cooler doesn't follow it: with that field at 1 and the global setting at Celsius, mode 7 drew `57°C` (capture, 2026-10-22). It sits with the hidden settings and wasn't researched further (AGENTS.md rule 8).

## Start-up

- DeepCreative's main process drives the cooler from the saved settings as soon as it starts, even while its window shows another page. Capture: restart of 2026-10-22, window on the home page.
- Sequence: the start-up command `03 08` ([protocol.md](protocol.md#start-up-command)), then frames of the saved picture.
  - A static mode appears with the first frame (first start: mode 1).
  - After a restart into a scrolling mode, DeepCreative sent blank frames for 3 s, then the text scrolled in from the right edge. Why it waited 3 s is unverified.
- **OpenDeepCool:** sends `03 08`, then the saved mode's frames straight away. A scrolling mode starts with its text entering from the right edge, as after a [mode switch](display.md#mode-switches), without the 3 s wait.

## Where the settings live

- DeepCreative keeps them in its own data folder, under the Wine user's application data. Each change on the LP page is saved at once through its main process (code: LP page).
- Mode, rotation and unit are stored in the app's database. The per-device file kept its date while they changed, so it isn't where they live; it is probably the encrypted store of saved images. Both points are inferred from file dates and strings, unverified.
- **OpenDeepCool:** keeps one settings file for the whole PC, in the display service's state directory, the same for every user (PRD, Service model; owner, 2026-10-23). It never reads DeepCreative's files.

## OpenDeepCool's defaults

The same as DeepCreative's first start: mode 1, rotation 0, Celsius, no fan inputs chosen, no saved images.

GPU: the dedicated GPU when the PC has one, otherwise the integrated GPU ([sensors.md](sensors.md#gpu-choice)).

## DeepCreative interface quirks, not reproduced

- **Separate options hidden:** Load Mode's Separate options (modes 1–3) sit below the visible part of the options box, and under Wine there's no scrollbar, so only scrolling reveals them. Code: LP page; owner: found by scrolling, 2026-10-22.
- **Wrong tab on start:** when the saved mode is 7 (Temperature Guardian → CPU), DeepCreative's window opens on the Load Mode tab, because its check treats modes up to 7 as Load Mode. The cooler still shows mode 7. Code: LP page.
- **OpenDeepCool's settings app** has its own design, not DeepCreative's pages and tabs (owner, 2026-10-22; PRD, Phase 11). These quirks don't carry over.

## Not covered

- Hidden display settings: after v1 (PRD, What We're NOT Building).
- App settings that don't touch the cooler (start at login, language): the settings app's own concern (Phase 11).
