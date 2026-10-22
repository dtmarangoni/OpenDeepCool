# Display: Load Mode, Temperature Guardian, Power Guardian, Creative Mode, rotation

What DeepCreative 1.2.14 draws on the LP360 in modes 1–12 and in Creative Mode, precisely enough to reproduce every frame. How frames are sent is in [protocol.md](protocol.md); evidence labels are defined in [README.md](README.md).

**Proof.** A script that applies only the rules below reproduces, pixel for pixel, every steady frame of 132 sweep steps: every mode 1–12, both temperature units, all four rotations, and values from 0 to 1999. One step is a recorded one-off (see [Observed one-off](#observed-one-off)). Evidence: capture: sweeps of 2026-10-22, with values fed to DeepCreative through its sensor readings (`research/hook/render-check.py`, local).

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
| A static mode (Load Mode, °C temperature, Creative Mode), from any mode | The new picture replaces the old one in a single frame, 135–460 ms after the switch | capture: 2026-10-22, transition steps |
| A scrolling mode (°F temperature, power) | The old picture scrolls out to the left, and the new text follows it in from the right, one column per tick; steady cycling within about 3 s | capture: 2026-10-22, transition steps |

The 135–460 ms is DeepCreative's own delay between the switch in its window and the next frame. OpenDeepCool can switch on the next frame.

## Rotation

The Rotate Screen setting has four values. Value *r* turns the **whole finished frame** *r* × 90° clockwise:

| Value | Turn |
|---|---|
| 0 | none |
| 1 | 90° clockwise |
| 2 | 180° |
| 3 | 270° clockwise |

The turn is applied last, so scrolling text moves along the turned picture: after undoing the turn, every frame is the normal left-scrolling picture. Evidence: capture: rotation steps of 2026-10-22, with modes 1, 4, 7 in °F and 10 at each value.

## Creative Mode

- **Mode 100:** the blank canvas, all pixels off. Capture: Phase 1.
- **Mode 100 + n:** saved image n, drawn exactly as stored: stored row 1 is the top row, stored column 1 the left column. Static. Capture and owner: an image drawn in DeepCreative's editor on 2026-10-22, with an L in the top-left and a dot in the bottom-right, was shown unchanged.
- **Stored images:** a list of `{index, data}`, where `data` is 14 rows of 14 values, 1 = on. DeepCreative's editor saves nothing else per image. Code and capture: the image the owner drew, 2026-10-22.
- **Images only.** DeepCreative 1.2.14 has no Creative Mode animations; the PRD lists them for after v1.

## Observed one-off

Once, after switching from mode 9 in °C straight to mode 7 in °F with the CPU reading at 0, the display stayed blank for the whole 14-second step. Repeating the same sequence three times showed `32°F` normally. Treat it as a DeepCreative glitch, not as behaviour to reproduce. Evidence: capture: sweeps of 2026-10-22.
