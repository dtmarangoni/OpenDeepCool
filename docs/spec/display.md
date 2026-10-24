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

**OpenDeepCool: static unless a value doesn't fit** (owner, 2026-10-24). DeepCreative fixes the layout per mode: static for usage and °C, scrolling for °F and power. OpenDeepCool has no layout setting. Every mode, °F and power included, uses the [static layout](#static-layout-usage-and-temperature-in-c) while each value it shows fits there (99 or less, or dashes), and switches to the [scrolling layout](#scrolling-layout-temperature-in-f-and-power) by itself while any value is 100 or more ([Values](#values), OpenDeepCool: no caps). Both layouts use the glyphs below.

## Values

How a reading becomes the number drawn:

| Metric | Number drawn | Evidence (reading → drawn) |
|---|---|---|
| CPU Usage, Gpu usage | round **up** to a whole number, at most 99 | 0.4 → 1, 7.2 → 8, 9.5 → 10, 99.4 → 99, 100 → 99, 1000 → 99 |
| Memory Load | round to the nearest whole number (halves up), at most 99 | 0.4 → 0, 0.5 → 1, 7.2 → 7, 7.6 → 8, 100 → 99 |
| CPU Temperature, Gpu temperature in °C | round **up**, at most 99 | 37.2 → 38, 57.4 → 58, 100 → 99, 120 → 99, 1000 → 99 |
| The same in °F | round the °C reading up, convert (× 9/5 + 32), then drop the fraction; from 1000 °F on, only the last two digits are drawn, a leading 0 included | 0 → 32, 37.2 → 100, 57 → 134, 100 → 212, 120 → 248, 537 → 998, 538 → 00 (1000 °F) |
| CPU Power, Gpu power | round **up**; from 1000 on, only the last two digits are drawn, a leading 0 included | 57.2 → 58, 999 → 999, 1000 → 00, 1050 → 50, 1234 → 34, 1999 → 99, 10000 → 00 |

Evidence: capture: sweeps of 2026-10-22; the limits (1000 % usage, 1000 °C, 537 and 538 °C in °F, 10000 W) checked one at a time on 2026-10-24, rotation 0. No °C reading gives exactly 999 °F, so 998 °F is the highest three-digit °F. Readings below 0 were not tested; temperatures below 0 °C don't occur with a liquid cooler (owner, 2026-10-24).

**OpenDeepCool: no caps** (owner, 2026-10-24).
- These limits are DeepCreative's own rules, not the hardware's: the PC draws every pixel, and the cooler's only limit is its 14 × 14 grid of on/off pixels ([protocol.md](protocol.md)).
- With the 3 × 5 digits, the static layout fits two digits and a symbol, and the scrolling field three digits; DeepCreative caps or cuts everything longer.
- OpenDeepCool draws every value in full, with the same glyphs (owner, 2026-10-24). Smaller glyphs were considered and rejected: the owner's trials on the cooler showed them hard to read.
- **Automatic scrolling, no setting:** every mode is static while its values fit (99 or less). When a value is 100 or more (`100%`, `100°C`, `100°F`, `100W`), the picture scrolls by itself, so no digit is lost. This applies to °F and power too, which DeepCreative always scrolls: `65W` and `98°F` are static in OpenDeepCool.
- **One smooth movement, never reset** (owner, 2026-10-24). The static picture and the scrolling text are the same strip of columns: the static layout's spacing (digit, blank column, digit, blank column, symbol) is the scrolling layout's spacing.
  - **Digit places:** each digit takes a place of 4 columns (3 for the digit, 1 blank), followed by the symbol. In a two-value mode both values end at the same column, so their last digits line up.
  - **Home position** (owner, 2026-10-24): where the static layout places the picture ([Static layout](#static-layout-usage-and-temperature-in-c), OpenDeepCool: alignment). A picture wider than 14 columns has its first digit at column 2. A static picture is the strip stopped at home.
  - **Starting to scroll:** when a value reaches 100, the whole number is drawn at home, from column 2 (`100` in columns 2–12), and its symbol starts at column 14, so the symbol is cut until the strip moves. The strip then moves left from there, one column per frame.
  - **The cycle:** the strip followed by 4 blank columns, at least 21 columns as in DeepCreative (`100W` is 17 columns, so 21; `1234W` is 21, so 25).
  - **Stopping:** when every value is back to 99 or less, the strip keeps moving until it reaches home, then stops there. No jump, no restart.
  - **Rising again before it stopped:** only the digits change; the strip keeps moving on its cycle.
  - **Every change of a number** keeps the strip's position (Scrolling layout, point 5). The position is never reset by a reading, or by switching between moving and stopped.
  - **A mode switch resets it** (owner, 2026-10-24): the new mode's picture appears at home on the next frame ([Mode switches](#mode-switches)).
- **Music Mode:** only the value area (rows 2–6) scrolls; the spectrum below it keeps moving with the sound ([music.md](music.md#value-area-modes-1822)).
- Until the spec is updated, the expected frames in [frames/](frames/) follow DeepCreative: its caps for value set 3 (usage 100, 120 °C, 1234 W, GPU 999 W), and °F and power always scrolling. They're regenerated with these rules.

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
| 10–14 | `%` for usage; `°C` (columns 10–13) for temperature. OpenDeepCool also draws `°F` and `W` here, both 5 wide, when °F or power is static |

So a one-digit number sits in the units place and the tens place stays blank, for example `5°C` and `0%`. The frame is the same every time until the number changes. Evidence: capture: sweeps of 2026-10-22.

DeepCreative uses these fixed columns for one value and for two: it never centres. Everything ends at column 14, except `°C`, which ends at column 13.

**OpenDeepCool: alignment** (owner, 2026-10-24). The picture is the number (digits 3 wide, a blank column after each) followed by its symbol, and is placed by its width:
- **One value:** centred in the 14 columns. When only one column is left over, it goes on the left, so the picture ends at column 14 (`42%`, `65W`, `98°F`, as DeepCreative). An odd number of spare columns otherwise: see the open question in the PRD.
- **Two values:** both end at column 14, so their last digits and symbols line up. `°C` moves one column right of DeepCreative's place here.

## Scrolling layout (temperature in °F, and power)

Within a band, the text runs right to left in a fixed 21-column cycle:

1. **Text:** each digit (3 columns) followed by one blank column, then the symbol (`°F` or `W`, both 5 columns). For example, `345W` is 17 columns: `3 4 5` and `W`.
2. **Field:** the text is right-aligned in a 17-column field, followed by 4 blank columns: 21 columns in all. A one-digit value leaves 8 blank columns before it, a two-digit value 4, and a three-digit value none.
3. **Display:** the frame shows 14 consecutive columns of that 21-column cycle, wrapping around. The window moves one column to the left **with every frame**, on the 100 ms tick ([protocol.md](protocol.md#timing)), so a full cycle takes 21 frames, 2.1 s.
   - Capture, whole log, re-analysed 2026-10-24: 22,874 frame-to-frame changes are one-column shifts; about 10 larger jumps are mode switches.
   - Corrected 2026-10-24: this point first called the scroll step a clock of its own, independent of the frames. An independent 100 ms clock sampled by frames would have shown a two-column jump every 25 frames or so.
4. **Two values** (modes 9 and 12): both bands use the same field and move together, so a shorter value stays right-aligned under the longer one.
5. **A reading that changes while the text scrolls** (owner, 2026-10-24): the frame after the new reading shows the new text at the same position in the 21-column cycle. The scroll doesn't restart and doesn't wait.
   - A longer or shorter number keeps its first digit place where it was and grows or shrinks to the right ([Values](#values), OpenDeepCool: one smooth movement).
   - Both values of a two-value mode follow the same rule, and so does a value turning into dashes or back.
   - **DeepCreative, not reproduced:** the text on show finishes its 21-column cycle with the old reading, and the next cycle carries the new one, so a change can take up to about 3 s to appear. All 60 changes between scrolling texts in the sweeps (reading changes, and switches between scrolling modes) join exactly at the end of a cycle, and every frame is a one-column shift. Evidence: capture: sweeps of 2026-10-22, re-analysed 2026-10-24 (local analysis script).

Power scrolls at every value, even one digit (`7W`). Temperature scrolls only in °F. Evidence: capture: sweeps of 2026-10-22 (every frame change is a one-column left shift; cycle 21 columns at every value).

## Updates

- A new reading appears with the next sensor snapshot, which DeepCreative receives about once per second. In the sweep, a changed value was drawn 957 ms after the reading changed. Evidence: capture: 2026-10-22, step 3.
- In a scrolling mode, OpenDeepCool also shows the new reading on the next frame, at the same scroll position ([Scrolling layout](#scrolling-layout-temperature-in-f-and-power), point 5). DeepCreative waits for the end of the cycle.
- Frames keep being sent every 100 ms tick whether or not anything changed ([protocol.md](protocol.md#timing)).

## Unavailable values

OpenDeepCool's own rule (owner, 2026-10-24). DeepCreative has no equivalent: under Wine, with no readings, it drew 0, which would be a wrong number (AGENTS.md rule 11).

- When a value the current mode shows can't be read, the cooler shows **dashes** in its place until the reading returns. The mode doesn't change, and the other value of a two-value mode is drawn as usual.
- **Dash glyph** (3 × 5, middle row lit):

  ```
  ...
  ...
  ###
  ...
  ...
  ```

- **Static layout:** a dash in the tens place (columns 2–4) and in the units place (columns 6–8). The symbol stays: `--%`, `--°C`.
- **Scrolling layout:** drawn like a two-digit number whose digits are both dashes: `--°F`, `--W`, in the same 21-column cycle.
- **When it happens:** the reading is unavailable (no driver rule, no device, no permission), a read failed and the sensor is being found again, or a rate metric (CPU Usage, CPU Power) is in its first second after start-up and has only one sample.
- In the settings app, a mode whose metric is persistently unavailable can't be chosen ([settings.md](settings.md#what-the-settings-app-must-offer)). Dashes on the cooler cover the cases the app can't prevent: the saved mode losing its reading after it was chosen, short read failures, and the first second.

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

**OpenDeepCool: a mode switch jumps** (owner, 2026-10-24). The next frame shows the new mode's picture at its home position, with current readings: nothing scrolls out, and no old text comes back. If a value of the new mode is 100 or more, the strip starts moving from home ([Values](#values), OpenDeepCool: one smooth movement). A change of the temperature unit is treated the same way, since it changes the digits and the symbol together.

**Test (Phases 7–8):** the transition tests switch between every pair of modes, with a different reading for every metric, with values below and above 100. The frame after the switch must be the new mode's picture at home, and every frame after it must follow the new mode's movement. Nothing of the old picture may appear after the switch.

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
