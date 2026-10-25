# Parity checklist

One row for each mode, option, setting and behaviour in the spec. Each row links to its spec section, says what evidence backs it, and records the owner's check. Evidence labels are defined in [README.md](README.md). Phase 3 ends when the owner signs off the whole list ([sign-off](#sign-off)); Phase 11 runs it again against OpenDeepCool.

- **Mode:** DeepCreative's mode number, or `-` when the row isn't about one mode.
- **Owner check:** `ok <date>`, with what the cooler showed, or `pending`. A row about something the cooler can't show (bytes on the wire, sensor files) is checked by the owner reviewing its evidence: `reviewed <date>`.
- Where DeepCreative does something OpenDeepCool deliberately doesn't, the row says so; the owner checks DeepCreative's behaviour, and the spec section describes ours.

## Protocol

| ID | Mode | Item | Spec | Evidence | Owner check |
|---|---|---|---|---|---|
| P1 | - | The device is USB `3633:000c` with one vendor-defined HID interface, found by its USB ID, never by its hidraw number | [protocol](protocol.md#device) | capture: lsusb and sysfs, 2026-10-21; code: `report_descriptor` | reviewed 2026-10-23: evidence re-run on all 8 captures |
| P2 | - | DeepCreative uses output report 16 only; reports 2–5 and input report 16 are never sent | [protocol](protocol.md#device) | capture: all sessions, every message a write of report 16 | reviewed 2026-10-23: evidence re-run on all 8 captures |
| P3 | - | Every message is 64 bytes: ID, fixed header, length, payload, checksum, terminator, zero padding | [protocol](protocol.md#message-layout) | capture: all 8 app starts, 106,075 messages, 0 errors | reviewed 2026-10-23: evidence re-run on all 8 captures |
| P4 | - | Start-up command `03 08`, sent once before the first frame; its meaning is unverified | [protocol](protocol.md#start-up-command) | capture: all 8 app starts, each one's first message, 2026-10-22 | reviewed 2026-10-23: evidence re-run on all 8 captures |
| P5 | - | A frame is `01` plus 28 pixel bytes in the row-pair bit layout | [protocol](protocol.md#frame) | capture: all sessions; owner: decoded `3%` matched the cooler | ok 2026-10-22: `3%` as decoded |
| P6 | - | Frames are sent continuously, one per 100 ms tick (single gaps 96 or 104 ms), even when the picture doesn't change | [protocol](protocol.md#timing) | capture: long-run mean 99.98–100.06 ms in all 14 continuous runs | reviewed 2026-10-23: evidence re-run on all 8 captures; corrected 2026-10-24 from "about every 104 ms" (the median of single gaps), owner OK |
| P7 | - | At exit DeepCreative sends no special message | [protocol](protocol.md#session-end-and-the-display-without-frames) | capture: clean quit, 2026-10-22 | reviewed 2026-10-23: evidence re-run on all 8 captures |
| P8 | - | Without frames the cooler keeps the last picture for about 10 s, then goes blank | [protocol](protocol.md#session-end-and-the-display-without-frames) | owner: 2026-10-22 | ok 2026-10-22: last picture, then blank after about 10 s |
| P9 | - | When the cooler disappears, DeepCreative retries 3 times in about 1.8 s, then drops it; the cooler returns under another hidraw number. Whether DeepCreative sends `03 08` after a reconnect is unverified; OpenDeepCool keeps looking and sends `03 08` before the first frame after every reconnect | [protocol](protocol.md#disconnect-and-reconnect) | capture: reconnect test, 2026-10-24, three cycles | reviewed 2026-10-24: owner chose to send `03 08` after every reconnect |

## Display: grid, values and glyphs

| ID | Mode | Item | Spec | Evidence | Owner check |
|---|---|---|---|---|---|
| D1 | - | 14 × 14 grid, row 1 at the top, column 1 at the left | [display](display.md#grid) | owner: digits legible the right way up | ok 2026-10-22 |
| D2 | - | Digits 0–9 are 3 × 5; `%`, `°C`, `°F` and `W` as drawn in the spec | [display](display.md#glyphs) | capture: sweeps, every digit at a known reading | ok 2026-10-23: `99%` in mode 4 and `999W` in mode 12 |
| D3 | - | CPU and GPU usage round up, at most 99 | [display](display.md#values) | capture: sweeps (0.4 → 1, 100 → 99) | ok 2026-10-23: mode 4, CPU 0.4 and GPU 100 drew `1%` over `99%` |
| D4 | - | Memory Load rounds to the nearest whole number, at most 99 | [display](display.md#values) | capture: sweeps (0.4 → 0, 0.5 → 1) | ok 2026-10-23: 0.4 drew `0%` (mode 5), 0.5 drew `1%` (mode 6) |
| D5 | - | °C temperatures round up, at most 99 | [display](display.md#values) | capture: sweeps (57.4 → 58, 120 → 99) | ok 2026-10-23: mode 9, CPU 57.4 and GPU 120 drew `58°C` over `99°C` |
| D6 | - | °F: round the °C reading up, convert, drop the fraction; from 1000 °F only the last two digits are drawn | [display](display.md#values) | capture: sweeps (57 → 134, 120 → 248); limit steps of 2026-10-24 (537 °C → 998, 538 °C → 00) | ok 2026-10-23: mode 9 in °F, CPU 120 and GPU 37.8 scrolled `248°F` over `100°F` |
| D7 | - | Power rounds up; from 1000 W only the last two digits are drawn | [display](display.md#values) | capture: sweeps (999 → 999, 1234 → 34); limit step of 2026-10-24 (10000 → 00) | ok 2026-10-23: mode 12, CPU 999 and GPU 1234 scrolled `999W` over `34W` |
| D8 | - | One value in rows 6–10; two values in rows 2–6 and 9–13 | [display](display.md#bands) | capture: sweeps | ok 2026-10-22: seen in D11–D25 |
| D9 | - | Static layout: tens in columns 2–4 (blank below 10), units in 6–8, symbol from column 10 | [display](display.md#static-layout-usage-and-temperature-in-c) | capture: sweeps | ok 2026-10-22: `7%` in the units place |
| D10 | - | Scrolling: 21-column cycle, right-aligned 17-column field, one column left with every frame (the 100 ms tick); two values move together | [display](display.md#scrolling-layout-temperature-in-f-and-power) | capture: sweeps | ok 2026-10-22: `65W` over `250W`, right ends aligned |

## Display: modes 1–12

| ID | Mode | Item | Spec | Evidence | Owner check |
|---|---|---|---|---|---|
| D11 | 1 | Load Mode → Separate → CPU: CPU Usage, static | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `42%` |
| D12 | 2 | Load Mode → Separate → GPU: Gpu usage, static | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `7%` |
| D13 | 3 | Load Mode → Separate → RAM: Memory Load, static | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `63%` |
| D14 | 4 | Load Mode → Combined → CPU / GPU, CPU on top | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `42%` over `7%` |
| D15 | 5 | Load Mode → Combined → CPU / RAM | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `42%` over `63%` |
| D16 | 6 | Load Mode → Combined → GPU / RAM | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `7%` over `63%` |
| D17 | 7 | Temperature Guardian → Separate → CPU in °C: static | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `57°C`, then `81°C` |
| D18 | 7 | The same in °F: scrolling | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `134°F` scrolling |
| D19 | 8 | Temperature Guardian → Separate → GPU in °C | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `38°C` |
| D20 | 8 | The same in °F | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `100°F` scrolling |
| D21 | 9 | Temperature Guardian → Combined → CPU / GPU in °C | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `57°C` over `38°C` |
| D22 | 9 | The same in °F, both lines scrolling together | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `134°F` over `100°F` |
| D23 | 10 | Power Guardian → Separate → CPU: scrolling, even at one digit | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `65W` scrolling |
| D24 | 11 | Power Guardian → Separate → GPU | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `250W` scrolling |
| D25 | 12 | Power Guardian → Combined → CPU / GPU | [display](display.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22: `65W` over `250W` |

## Display: behaviour and rotation

| ID | Mode | Item | Spec | Evidence | Owner check |
|---|---|---|---|---|---|
| D26 | - | A new reading shows with the next snapshot, about 1 s later | [display](display.md#updates) | capture: changed value drawn 957 ms later | ok 2026-10-22: `57°C` → `81°C` within seconds |
| D27 | - | Switching into a static mode replaces the picture in one frame, on DeepCreative's next sensor snapshot; OpenDeepCool switches on the next frame | [display](display.md#mode-switches) | capture: transition steps; transition probe, 22 switches | ok 2026-10-23: mode 12 scrolling, then mode 1: `42%` at once, no scrolling or blank |
| D28 | - | Switching into a scrolling mode: the old picture scrolls out left, the new text follows from the right. DeepCreative first replays the paused text of the last scrolling mode, with stale readings, from where it stopped; OpenDeepCool never does | [display](display.md#mode-switches) | capture: three manual switches and two transition-probe runs of 2026-10-23, 22 of 22 static-to-scrolling switches | ok 2026-10-23: saw the stale `999W` over `34W`, and `150W` between `52%` and `96°F` and between `52%` and `65W`; the stale text is trash and isn't reproduced |
| D29 | - | Rotate Screen 0–3 turns the whole finished frame 0°, 90°, 180°, 270° clockwise | [display](display.md#rotation) | capture: matrix sweeps, every mode at every value | ok 2026-10-22: 90°, 180°, 270° clockwise |
| D30 | - | Scrolling text moves along the turned picture | [display](display.md#rotation) | capture: matrix sweeps | ok 2026-10-22: modes 7 in °F and 10, turned |
| D31 | - | DeepCreative's on-screen preview doesn't turn; only the cooler does (not a feature to copy) | [display](display.md#rotation) | owner: 2026-10-22 | ok 2026-10-22 |
| D35 | - | A reading that changes while text scrolls: DeepCreative finishes the 21-column cycle with the old reading; OpenDeepCool shows the new text on the next frame, at the same scroll position | [display](display.md#scrolling-layout-temperature-in-f-and-power) | capture: sweeps, all 60 changes between scrolling texts join at a cycle end (re-analysed 2026-10-24) | reviewed 2026-10-24: owner chose to update in place |
| D36 | - | DeepCreative's limits (99 in static layouts; the last two digits from 1000 in scrolling ones) aren't reproduced: OpenDeepCool draws every value in full with the same glyphs, scrolling automatically whenever a value doesn't fit the static layout; in Music Mode only the value area scrolls. Every value is capped at 9999 everywhere: 10 000 and more are drawn `9999` (owner, 2026-10-25) | [display](display.md#values) | capture: limit steps of 2026-10-24 | reviewed 2026-10-24: owner decision, after trying smaller drawings on the cooler |
| D37 | - | No layout setting: every mode, °F and power included, is static while its values are 99 or less and scrolls by itself while any value is 100 or more. One smooth movement: scrolling starts from the current picture, stops only at the home position (the static picture), and a reading change never resets the position; DeepCreative fixes it per mode (°F and power always scroll) | [display](display.md#modes) | owner: 2026-10-24 | reviewed 2026-10-24: owner decision, replacing a per-mode setting decided earlier the same day |
| D38 | - | A mode switch (or a unit change) shows the new mode's picture at its home position on the next frame; nothing scrolls out. Replaces OpenDeepCool's earlier scroll-out rule in D28 | [display](display.md#mode-switches) | owner: 2026-10-24 | reviewed 2026-10-24: owner decision |
| D39 | - | Static alignment: one value centred in 14 columns (with an odd number of spare columns, the extra one goes on the left); two values both end at column 14. DeepCreative uses fixed columns for both (units 6–8, symbol from 10). A unit change resets the picture like a mode switch (confirmed) | [display](display.md#static-layout-usage-and-temperature-in-c) | owner: 2026-10-24 | reviewed 2026-10-24: owner decision |
| D40 | - | A rotation change resets the picture to home like a mode switch; a strip still at home stops at once when the values fit; in two-value modes the longer value's first digit stays put; negative, not-a-number and infinite readings draw dashes | [display](display.md#values) | owner: 2026-10-24 | reviewed 2026-10-24: owner decisions |
| D41 | - | Choosing what's already set changes nothing; a unit change in a mode without a temperature is only saved; choosing another GPU resets like a mode switch; the app's Refresh takes fresh readings and restarts the picture from home. DeepCreative has no Refresh | [display](display.md#mode-switches) | owner: 2026-10-24 | reviewed 2026-10-24: owner decisions |

## Music Mode

| ID | Mode | Item | Spec | Evidence | Owner check |
|---|---|---|---|---|---|
| M1 | - | Spectrum layout: 14 solid bars from the bottom, bottom row always lit, at most 12 rows full screen and 6 rows split | [music](music.md#spectrum-layout-as-deepcreative) | capture: every frame of the passes; matrix sweep | ok 2026-10-22: seen in M4–M15 |
| M2 | - | Silence shows a flat line one pixel high | [music](music.md#spectrum-layout-as-deepcreative) | capture: silent pass; 2026-10-23, mode 19 with the monitor at digital zero, 312 identical frames | ok 2026-10-23: flat line along the bottom, nothing above it |
| M3 | - | Spectrum analysis is OpenDeepCool's own (Winamp-style, whole audible range), from the default output's monitor; DeepCreative's 5–160 Hz analysis isn't copied | [music](music.md#spectrum-analysis-opendeepcools-own) | capture: test tones and sweep; owner decision 2026-10-22 | ok 2026-10-22: owner decision |
| M4 | 13 | Full Screen: spectrum in rows 3–14 | [music](music.md#modes) | capture: matrix sweep, all rotations | ok 2026-10-22 |
| M5 | 14 | Emoji 1 + spectrum: loop of 900 ms, independent of the sound | [music](music.md#emoji-1-mode-14) | capture: silent and loud passes, matrix sweep | ok 2026-10-22 |
| M6 | 15 | Emoji 2 + spectrum: loop of 900 ms | [music](music.md#emoji-2-mode-15) | capture: as M5 | ok 2026-10-22 |
| M7 | 16 | Emoji 3 + spectrum: loop of 900 ms | [music](music.md#emoji-3-mode-16) | capture: as M5 | ok 2026-10-22 |
| M8 | 17 | Emoji 4 + spectrum: loop of 1100 ms | [music](music.md#emoji-4-mode-17) | capture: as M5 | ok 2026-10-22 |
| M9 | 18 | Temperature + Spectrum → CPU in °C: value in rows 2–6 | [music](music.md#value-area-modes-1822) | capture: matrix sweep, all rotations | ok 2026-10-22 |
| M10 | 18 | The same in °F. DeepCreative draws the Gpu temperature here; OpenDeepCool draws the CPU Temperature | [music](music.md#value-area-modes-1822) | capture: `MusicLoud 18`, unit 1, and matrix sweep | ok 2026-10-22: DeepCreative's `32°F`, then `100°F` (the GPU) |
| M11 | 19 | Temperature + Spectrum → GPU in °C | [music](music.md#value-area-modes-1822) | capture: matrix sweep, all rotations; 2026-10-23, GPU 36 °C with every other metric distinct | ok 2026-10-23: `36°C` (the GPU) above the spectrum |
| M12 | 19 | The same in °F, scrolling | [music](music.md#value-area-modes-1822) | capture: matrix sweep, all rotations | ok 2026-10-22: `113°F` scrolling |
| M13 | 20 | Usage + Spectrum → CPU | [music](music.md#value-area-modes-1822) | capture: matrix sweep, all rotations | ok 2026-10-22; also at 90° |
| M14 | 21 | Usage + Spectrum → GPU | [music](music.md#value-area-modes-1822) | capture: matrix sweep, all rotations | ok 2026-10-22 |
| M15 | 22 | Usage + Spectrum → RAM | [music](music.md#value-area-modes-1822) | capture: matrix sweep, all rotations | ok 2026-10-22 |
| M16 | - | OpenDeepCool's spectrum: 14 log-spaced bands from 40 Hz to 16 kHz, −75 to −25 dB, bars that rise at once and fall in the *bar fall* time, peak dots that fall in the *peak fall* time, in every Music mode including the emoji ones; DeepCreative draws no peaks | [music](music.md#opendeepcools-spectrum-precisely) | owner: 2026-10-25, on the preview page | reviewed 2026-10-25: owner decision |
| M17 | - | Peak dots on or off, bar fall and peak fall (sliders with Winamp's five stops; defaults moderate and slow, as in Winamp) are settings on the app's configuration screen | [settings](settings.md#what-the-settings-app-must-offer) | owner: 2026-10-25 | reviewed 2026-10-25: owner decision |
| M18 | - | Delay from sound to frame: at most 150 ms for 95 % of frames | [music](music.md#opendeepcools-spectrum-precisely) | owner: 2026-10-25 | reviewed 2026-10-25: owner decision |
| M19 | 14–17 | A reset restarts the emoji loop from its first picture | [music](music.md#emoji-animations-modes-1417) | owner: 2026-10-25, on the preview page | reviewed 2026-10-25: owner decision |
| M20 | 23 | Music → Dancer (OpenDeepCool's own): a stick figure on the whole grid, standing still in silence, moving to its next pose on each bass beat (sways when soft, big moves when loud) | [music](music.md#dancer-mode-23-opendeepcools-own) | owner: 2026-10-25, on the preview page | reviewed 2026-10-25: owner approved |

## Sensors

| ID | Mode | Item | Spec | Evidence | Owner check |
|---|---|---|---|---|---|
| S1 | - | Readings refresh once per second; CPU usage and power are averages over that second, GPU usage and power are the driver's values as read | [sensors](sensors.md#updates-and-rounding) | system: the service log polls every 1000 ms | reviewed 2026-10-23: DeepCreative polls every 1000 ms; OpenDeepCool reads once per second |
| S2 | - | CPU Temperature: the CPU package temperature | [sensors](sensors.md#metrics) | code: helper's label list; system: Linux source agreed with `sensors` | ok 2026-10-23: rule and `sensors` both 31.0 °C, live |
| S3 | - | CPU Usage: total load across all logical CPUs, from `/proc/stat` | [sensors](sensors.md#metrics) | code; system | ok 2026-10-23: rule and `top` agree, idle about 1 % and 50.6 % with 10 of 20 CPUs loaded |
| S4 | - | CPU Power: package power from the RAPL energy counter, which is root-only; Phase 6 decides how the display service reads it | [sensors](sensors.md#cpu-power) | code; system: counter root-only | ok 2026-10-23: zone `package-0` found by name; `energy_uj` root-only (Permission denied); reading tested in Phase 6 |
| S5 | - | Gpu temperature: the main GPU temperature, not its hot spot or memory | [sensors](sensors.md#metrics) | code: HWiNFO sample; system | ok 2026-10-23: rule (`edge`) and `sensors` both 29 °C; junction and memory not used |
| S6 | - | Gpu usage: the chosen GPU's busy percentage | [sensors](sensors.md#metrics) | code; system; equivalence with HWiNFO inferred | ok 2026-10-23: `gpu_busy_percent` 2 % idle, 99–100 % under an `ffmpeg` Vulkan load, back to 2 % after |
| S7 | - | Gpu power: the board power reading | [sensors](sensors.md#metrics) | code; system; equivalence with HWiNFO inferred | ok 2026-10-23: same file as `sensors` PPT; about 20 W idle, 220–310 W under load, flickering within a second |
| S8 | - | Memory Load: (MemTotal − MemAvailable) ÷ MemTotal | [sensors](sensors.md#metrics) | code: HWiNFO sample; system | reviewed 2026-10-23: used ÷ total, where used = total − available |
| S9 | - | Cooler fans (DeepCreative's CPU Fan I/O) and pump: the user picks a fan input for each, from every sensor chip except a graphics card's, with none chosen by default; their RPM shows in the settings app, never on the cooler | [sensors](sensors.md#fan-and-pump-speed) | code: LP page settings, helper's fan list (traced 2026-10-23) | reviewed 2026-10-23: owner decision |
| S10 | - | GPU choice: dedicated before integrated; a dropdown only with more than one GPU, set to the dedicated one at first; kept across reboots | [sensors](sensors.md#gpu-choice) | owner: 2026-10-23 decision | reviewed 2026-10-23: owner decision |

## Settings

| ID | Mode | Item | Spec | Evidence | Owner check |
|---|---|---|---|---|---|
| T1 | - | First start: mode 1, rotation 0, Celsius, no fan inputs | [settings](settings.md#settings) | capture and code: start with the data folder moved aside | ok 2026-10-23: first start with the data folder moved aside showed `3%` upright (mode 1, rotation 0), after `03 08` |
| T2 | - | Mode, rotation and unit come back after a restart | [settings](settings.md#settings) | capture: restart into mode 9, °F, rotation 2 | ok 2026-10-22: `32°F` upside down and scrolling after the restart |
| T3 | - | Rotate Screen is one button: each press adds a quarter turn clockwise, 3 wraps to 0 | [settings](settings.md#settings) | code: LP page (`rotatingScreenfun`); capture: 2026-10-23, the button pressed 4 times on `23%` | reviewed 2026-10-23: every frame matched the rendered `23%` turned 90°, 180°, 270°, then upright again, about 0.3 s after each press |
| T4 | - | Temperature unit is one global setting; the device options' field of the same name has no effect | [settings](settings.md#settings) | capture: field at 1, global Celsius, mode 7 drew `57°C` | reviewed 2026-10-23: global unit seen switching every temperature mode; the device field is a hidden setting, not sent again (AGENTS.md rule 8) |
| T6 | - | Start-up: `03 08`, then the saved picture at once; OpenDeepCool skips DeepCreative's 3 s blank before a scrolling mode | [settings](settings.md#start-up) | capture: restarts of 2026-10-22 and 2026-10-23, window on the home page; owner watched the cooler | ok 2026-10-23 |

## Sign-off

Signed off by the owner on 2026-10-23: all 73 rows checked, on the cooler or by reviewing their evidence. P9 was added on 2026-10-24 and reviewed by the owner the same day.
