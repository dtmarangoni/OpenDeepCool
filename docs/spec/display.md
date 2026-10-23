# Display: Load Mode, Temperature Guardian, Power Guardian, Creative Mode, rotation

What DeepCreative 1.2.14 draws on the LP360 in modes 1–12 and in Creative Mode, precisely enough to reproduce every frame. How frames are sent is in [protocol.md](protocol.md); evidence labels are defined in [README.md](README.md).

**Proof.** A script that applies only the rules below reproduces, pixel for pixel, every steady frame of 340 sweep steps, with values from 0 to 1999. These include the **full test matrix**: every mode 1–12 and Creative Mode, in all four rotations, in both temperature units where a temperature is shown, with three value sets per case. Every metric had its own distinct value, so a mode drawing the wrong metric would have failed. Every scrolling case showed its full cycle. One step is a recorded one-off (see [Observed one-off](#observed-one-off)). Evidence: capture: sweeps of 2026-10-22, the matrix sweep included, with values fed to DeepCreative through its sensor readings (`research/hook/render-check.py`, local). The expected frame of every case is in [frames/](frames/).

## Grid

- 14 × 14 pixels, each on or off. **Row 1 is the top and column 1 the left**, as the cooler shows them (owner: 2026-10-22).
- DeepCreative draws every picture this way round, then applies [rotation](#rotation) to the whole frame last.

## Modes

DeepCreative's mode numbers. Each metric is named as it appears in DeepCreative's sensor readings; [sensors.md](sensors.md) (to come) maps them to Linux.

| Mode | Page and option | Metric(s), top first | Kind |
|---|---|---|---|
| 1 | Load Mode → Separate → CPU | CPU Usage | static |
| 2 | Load Mode → Separate → GPU | Gpu usage | static |
| 3 | Load Mode → Separate → RAM | Memory Load | static |
| 4 | Load Mode → Combined → CPU / GPU | CPU Usage, Gpu usage | static |
| 5 | Load Mode → Combined → CPU / RAM | CPU Usage, Memory Load | static |
| 6 | Load Mode → Combined → GPU / RAM | Gpu usage, Memory Load | static |
| 7 | Temperature Guardian → Separate → CPU | CPU Temperature | static in °C, scrolling in °F |
| 8 | Temperature Guardian → Separate → GPU | Gpu temperature | static in °C, scrolling in °F |
| 9 | Temperature Guardian → Combined → CPU / GPU | CPU Temperature, Gpu temperature | static in °C, scrolling in °F |
| 10 | Power Guardian → Separate → CPU | CPU Power | scrolling |
| 11 | Power Guardian → Separate → GPU | Gpu power | scrolling |
| 12 | Power Guardian → Combined → CPU / GPU | CPU Power, Gpu power | scrolling |
| 100 | Creative Mode → blank canvas | — | static, all off |
| 100 + n | Creative Mode → saved image n | — | static |

Combined layouts exist in one order only, as listed. Evidence: code: LP page modes (Phase 1 report); capture: sweeps.

## Values

How a reading becomes the number drawn:

| Metric | Number drawn | Evidence (reading → drawn) |
|---|---|---|
| CPU Usage, Gpu usage | round **up** to a whole number, at most 99 | 0.4 → 1, 7.2 → 8, 9.5 → 10, 99.4 → 99, 100 → 99 |
| Memory Load | round to the nearest whole number (halves up), at most 99 | 0.4 → 0, 0.5 → 1, 7.2 → 7, 7.6 → 8, 100 → 99 |
| CPU Temperature, Gpu temperature in °C | round **up**, at most 99 | 37.2 → 38, 57.4 → 58, 100 → 99, 120 → 99 |
| The same in °F | round the °C reading up, convert (× 9/5 + 32), then drop the fraction; no upper limit | 0 → 32, 37.2 → 100, 57 → 134, 100 → 212, 120 → 248 |
| CPU Power, Gpu power | round **up**; from 1000 on, only the last two digits are drawn, a leading 0 included | 57.2 → 58, 999 → 999, 1000 → 00, 1050 → 50, 1234 → 34, 1999 → 99 |

Evidence: capture: sweeps of 2026-10-22. Readings below 0 were not tested (unverified).

## Glyphs

Each `#` is a lit pixel. Digits are 3 × 5:

```
0    1    2    3    4    5    6    7    8    9
###  .#.  ###  ###  #.#  ###  ###  ###  ###  ###
#.#  ##.  ..#  ..#  #.#  #..  #..  ..#  #.#  #.#
#.#  .#.  ###  ###  ###  ###  ###  .#.  ###  ###
#.#  .#.  #..  ..#  ..#  ..#  #.#  .#.  #.#  ..#
###  ###  ###  ###  ..#  ###  ###  .#.  ###  ###
```

Symbols, drawn in the same 5-row band as the digits:

```
%        °C      °F       W
##..#    #...    #.###    .....
##.#.    ..##    ..#..    #.#.#
..#..    .#..    ..###    #.#.#
.#.##    .#..    ..#..    #.#.#
#..##    ..##    ..#..    .#.#.
```

- `%`: 5 wide.
- `°C`: the dot (top row, first column) and a 3-wide C on the lower four rows.
- `°F`: the dot, a blank column, and a 3-wide F.
- `W`: 5 wide, on the lower four rows.

Evidence: capture: sweeps of 2026-10-22, with every digit seen at a known reading.

## Bands

A value is drawn in a band 5 rows tall:

- **one value** (modes 1–3, 7, 8, 10, 11): rows 6–10;
- **two values** (modes 4–6, 9, 12): the first metric in rows 2–6, the second in rows 9–13.

## Static layout (usage, and temperature in °C)

Within a band:

| Columns | Content |
|---|---|
| 2–4 | Tens digit; blank when the number is below 10 |
| 6–8 | Units digit |
| 10–14 | `%` for usage; `°C` (columns 10–13) for temperature |

So a one-digit number sits in the units place and the tens place stays blank, for example `5°C` and `0%`. The frame is the same every time until the number changes. Evidence: capture: sweeps of 2026-10-22.

## Scrolling layout (temperature in °F, and power)

Within a band, the text runs right to left in a fixed 21-column cycle:

1. **Text:** each digit (3 columns) followed by one blank column, then the symbol (`°F` or `W`, both 5 columns). For example, `345W` is 17 columns: `3 4 5` and `W`.
2. **Field:** the text is right-aligned in a 17-column field, followed by 4 blank columns: 21 columns in all. A one-digit value leaves 8 blank columns before it, a two-digit value 4, and a three-digit value none.
3. **Display:** the frame shows 14 consecutive columns of that 21-column cycle, wrapping around. The window moves one column to the left **every 100 ms**. That is a scroll tick of its own, independent of the ~104 ms frame interval, so a full cycle takes 2.1 s.
4. **Two values** (modes 9 and 12): both bands use the same field and move together, so a shorter value stays right-aligned under the longer one.

Power scrolls at every value, even one digit (`7W`). Temperature scrolls only in °F. Evidence: capture: sweeps of 2026-10-22 (every frame change is a one-column left shift; cycle 21 columns at every value).

## Updates

- A new reading appears with the next sensor snapshot, which DeepCreative receives about once per second. In the sweep, a changed value was drawn 957 ms after the reading changed. Evidence: capture: 2026-10-22, step 3.
- Frames keep being sent about every 104 ms whether or not anything changed ([protocol.md](protocol.md#timing)).

## Mode switches

| Switch into | What the display does | Evidence |
|---|---|---|
| A static mode (Load Mode, °C temperature, Creative Mode), from any mode | The new picture replaces the old one in a single frame. A scrolling old picture keeps scrolling until then | capture: 2026-10-22, transition steps; 2026-10-23, transition probe |
| A scrolling mode (°F temperature, power), from a scrolling mode | The old text finishes its 21-column cycle, then the new text follows | capture: 2026-10-23, transition probe |
| A scrolling mode, from a static mode | The old picture scrolls out to the left. Behind it, DeepCreative replays the **paused text of the last scrolling mode**, then the new text follows (see the quirk below) | capture: 2026-10-23, transition probe; owner: 2026-10-23 |

DeepCreative applies a switch only on its next sensor snapshot, about once a second. The new static picture appeared 35 ms to 1.9 s after the switch in its window (capture: transition probe, 22 switches). OpenDeepCool switches on the next frame.

**DeepCreative quirk, not reproduced: the paused text comes back.** When a static picture takes over from a scrolling mode, DeepCreative's scrolling text stops where it is and keeps its readings. At the next switch into any scrolling mode, after the old picture has scrolled out:

1. The paused text resumes from the column after the last one shown, even after 30 s in the static mode, and even when it belongs to another mode.
2. It keeps the readings it was drawn with, even if they have changed since.
3. It runs to the end of its 21-column cycle. When it resumed at column 16 or later (5 columns or fewer left), it ran one more full cycle.
4. Then the new text starts its cycle, with current readings.

So the stale text enters wherever it stopped, often in the middle of a digit, glued to the outgoing picture. To the eye it's a fragment of an old value: the owner saw `999W` over `34W` come back between `42%` and `65W`, and a fast `150W` between `52%` and `96°F` and between `52%` and `65W`, which looked "on top of" the outgoing `52%`.

- **Every case fits:** all 22 switches from a static mode into a scrolling mode in two probe runs. In 17 the stale piece is visible, resuming exactly one column after where it stopped. In the other 5 it had stopped after column 12–14 of `150W` or `65W`. All that was left of it was the end of its `W` and blank columns, the same columns the new text (`65W`, `88W`) has at that point of its cycle.
- **Stale readings:** `150W` came back after GPU power had changed to 210 W. `345W` over `34W` came back after mode 12 had already been redrawn as `65W` over `150W` (first probe run, `two-statics`). Which of the readings the paused text keeps is unverified beyond these two cases.
- **Why it was missed on 2026-10-22:** the last scrolling text before the switch into mode 10 was that mode's own `345W`, so the replay looked like the new text.
- **Evidence:** capture: manual switches of 2026-10-23 at 19:04, 19:08 and 19:13, and two runs of the transition probe at 19:20 and 19:34 (`research/hook/transition-probe.mjs` and `transition-analyze.py`, local). The probe records the time of every input and lines it up with every frame sent, with no one watching. Owner: 2026-10-23, on the cooler during the switches and the second probe run.

**OpenDeepCool:** no scrolling state survives a mode switch. When switching into a scrolling mode, the old picture scrolls out to the left, and the very next column is the first column of the new text's cycle, drawn with current readings. Nothing else appears in between.

**Test (Phases 7–8):** the transition tests switch between every pair of kinds (static to scrolling, scrolling to static, scrolling to scrolling, static to static), with a different reading for every metric, and also pass through a static mode between two different scrolling modes. A test fails if any column after the switch belongs to a text other than the outgoing picture or the new one. This is the check `transition-analyze.py` makes on DeepCreative's captures.

## Rotation

The Rotate Screen setting has four values. Value *r* turns the **whole finished frame** *r* × 90° clockwise:

| Value | Turn |
|---|---|
| 0 | none |
| 1 | 90° clockwise |
| 2 | 180° |
| 3 | 270° clockwise |

The turn is applied last, so scrolling text moves along the turned picture: after undoing the turn, every frame is the normal left-scrolling picture. The same holds in every mode, Music Mode and Creative Mode included. Evidence: capture: the matrix sweeps of 2026-10-22, every mode at every rotation value; owner: 2026-10-22, modes 1, 4, 7 in °F and 10, and Music Mode 20 at 90°.

DeepCreative's on-screen preview of the display doesn't turn; only the cooler does (owner: 2026-10-22). So a screenshot of DeepCreative's window can't show rotation, and every rotation check compares the frames sent to the cooler.

## Creative Mode

- **Mode 100:** the blank canvas, all pixels off. Capture: Phase 1.
- **Mode 100 + n:** saved image n, drawn exactly as stored: stored row 1 is the top row, stored column 1 the left column. Static. Capture and owner: an image drawn in DeepCreative's editor on 2026-10-22, with an L in the top-left and a dot in the bottom-right, was shown unchanged.
- **Stored images:** a list of `{index, data}`, where `data` is 14 rows of 14 values, 1 = on. DeepCreative's editor saves nothing else per image. Code and capture: the image the owner drew, 2026-10-22.
- **Images only.** DeepCreative 1.2.14 has no Creative Mode animations; the PRD lists them for after v1.

## Observed one-off

Once, after switching from mode 9 in °C straight to mode 7 in °F with the CPU reading at 0, the display stayed blank for the whole 14-second step. Repeating the same sequence three times showed `32°F` normally. Treat it as a DeepCreative glitch, not as behaviour to reproduce. Evidence: capture: sweeps of 2026-10-22.
