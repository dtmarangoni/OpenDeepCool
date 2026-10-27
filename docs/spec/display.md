# Display: Load Mode, Temperature Guardian, Power Guardian, rotation

What DeepCreative 1.2.14 draws on the LP360 in modes 1–12, precisely enough to reproduce every frame. How frames are sent is in [protocol.md](protocol.md); evidence labels are defined in [README.md](README.md).

**Proof.** A script that applies only the rules below reproduces, pixel for pixel, every steady frame of 340 sweep steps, with values from 0 to 1999. These include the **full test matrix**: every mode 1–12, in all four rotations, in both temperature units where a temperature is shown, with three value sets per case. Every metric had its own distinct value, so a mode drawing the wrong metric would have failed. Every scrolling case showed its full cycle. One step is a recorded one-off (see [Observed one-off](#observed-one-off)). Evidence: capture: sweeps of 2026-10-22, the matrix sweep included, with values fed to DeepCreative through its sensor readings (`research/hook/render-check.py`, local). The expected frame of every case is in [frames/](frames/).

## Grid

- 14 × 14 pixels, each on or off. **Row 1 is the top and column 1 the left**, as the cooler shows them (owner: 2026-10-22).
- DeepCreative draws every picture this way round, then applies [rotation](#rotation) to the whole frame last.

## Modes

DeepCreative's mode numbers. Each metric is named as it appears in DeepCreative's sensor readings; [sensors.md](sensors.md) maps them to Linux.

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

Combined layouts exist in one order only, as listed. Evidence: code: LP page modes (Phase 1 report); capture: sweeps.

**OpenDeepCool's names** (owner, 2026-10-24): **Load**, **Temperature**, **Power** and **Music**, in code and in the app. Never "Guardian", and no "Mode" after the name. The names in the table above, and elsewhere in this spec where DeepCreative is described, are DeepCreative's.

**OpenDeepCool: static unless a value doesn't fit** (owner, 2026-10-24). DeepCreative fixes the layout per mode: static for usage and °C, scrolling for °F and power. OpenDeepCool has no layout setting. Every mode, °F and power included, uses the [static layout](#static-layout-usage-and-temperature-in-c) while each value it shows fits there (99 or less, or dashes), and switches to the [scrolling layout](#scrolling-layout-temperature-in-f-and-power) by itself while any value is 100 or more ([Values](#values), OpenDeepCool: no caps). Both layouts use the glyphs below.

## Values

How DeepCreative turns a reading into the number drawn. OpenDeepCool's own rules follow the table: no caps, the 9999 cap, and the nearest whole number for every measurement.

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
- **Every value is capped at 9999, everywhere in OpenDeepCool** (owner, 2026-10-25). A reading whose number would be 10 000 or more, in the unit drawn (for °F, after the conversion), is drawn as `9999`. So a value has at most four digits: `9999W` is 21 columns wide, with a 25-column cycle. With OpenDeepCool's rounding (below): 5536 °C gives `9997°F` (9996.8); 5537 °C gives `9999°F` (9998.6); from 5537.5 °C the number is 10 000 or more, drawn `9999°F`. The same capped number is what the settings app's preview, the D-Bus interface and the service's journal show for that metric. The cooler's fan and pump speeds, shown only in the app, are capped the same way at 9999 RPM (owner, 2026-10-25). Dashes stay for unavailable, negative, not-a-number and infinite readings only.
- **Automatic scrolling, no setting:** every mode is static while its values fit (99 or less). When a value is 100 or more (`100%`, `100°C`, `100°F`, `100W`), the picture scrolls by itself, so no digit is lost. This applies to °F and power too, which DeepCreative always scrolls: `65W` and `98°F` are static in OpenDeepCool.
- **One smooth movement, never reset** (owner, 2026-10-24). The static picture and the scrolling text are the same strip of columns: the static layout's spacing (digit, blank column, digit, blank column, symbol) is the scrolling layout's spacing.
  - **Digit places:** each digit takes a place of 4 columns (3 for the digit, 1 blank), followed by the symbol. In a two-value mode both values end at the same column, so their last digits line up.
  - **Home position** (owner, 2026-10-24): where the static layout places the picture ([Static layout](#static-layout-usage-and-temperature-in-c), OpenDeepCool: alignment). A picture wider than 14 columns has its first digit at column 2. A static picture is the strip stopped at home.
  - **Starting to scroll:** when a value reaches 100, the new digits are drawn where the static picture's first digit place was (column 2 for a two-digit value, column 4 for a centred one-digit value), so the whole number shows and only the symbol is cut. The strip then moves left from there, one column per frame. After a reset (a mode switch, for example) a picture wider than 14 columns starts with its first digit at column 2.
  - **The cycle:** the strip followed by 4 blank columns, at least 21 columns as in DeepCreative (`100W` is 17 columns, so 21; `1234W` is 21, so 25).
  - **Stopping:** when every value is back to 99 or less, the strip keeps moving until it reaches home, then stops there. No jump, no restart. If the strip is at home at that moment and hasn't moved on yet, it stops at once (owner, 2026-10-24).
  - **Rising again before it stopped:** only the digits change; the strip keeps moving on its cycle.
  - **Every change of a number** keeps the strip's position (Scrolling layout, point 5). The position is never reset by a reading, or by switching between moving and stopped.
  - **A mode switch resets it** (owner, 2026-10-24): the new mode's picture appears at home on the next frame ([Mode switches](#mode-switches)). A temperature unit change and a rotation change reset it the same way.
  - **Two values whose lengths change while scrolling** (owner, 2026-10-24): the strip's first column, the first digit place of the longer value, stays put; the shorter value stays aligned to the longer one's right end, so it moves. This holds when the other value becomes the longer one, too.
- **Music Mode:** only the value area (rows 2–6) scrolls; the spectrum below it keeps moving with the sound ([music.md](music.md#value-area-modes-1822)).
- The expected frames in [frames/](frames/) follow these rules (regenerated 2026-10-24, and again on 2026-10-26 for the nearest rule).

**OpenDeepCool: the nearest whole number, in exact whole-number arithmetic** (owner, 2026-10-26).
- **Rounding:** every measurement rounds to the nearest whole number, halves up: usage, Memory Load, temperatures, power.
- **°F** is rounded once, from the exact °C reading converted to °F.
- **DeepCreative**, in the table above, rounds usage, temperature and power up, and for °F rounds the °C reading up, converts it, then drops the fraction. OpenDeepCool doesn't follow it here.
- **Exact arithmetic:** every number drawn is computed from its source's own whole-number units, in integer arithmetic, never through floating point. Floating point can flip a rounding: 23 busy jiffies in 40 is exactly 57.5 %, which draws `58`, but `23/40 × 100` is `57.49999999999999` in IEEE doubles, which would draw `57`.

| Metric | Source units | Number drawn, before the 9999 cap |
|---|---|---|
| CPU Usage | jiffies over the interval; busy = total − (idle + iowait) | (200 · busy + total) div (2 · total) |
| Gpu usage | the driver's whole percent | as read |
| Memory Load | kB; used = MemTotal − MemAvailable | (200 · used + total) div (2 · total) |
| CPU Temperature, Gpu temperature in °C | millidegrees *m* (hwmon; NVML's whole °C as *m* = 1000 · °C) | (*m* + 500) div 1000, for *m* ≥ 0 |
| The same in °F | the same *m* | (9 · *m* + 162 500) div 5000: 9/5 · *m*/1000 + 32, rounded once |
| CPU Power, Gpu power, from an energy counter | µJ over an interval measured in whole µs | (2 · ΔµJ + Δµs) div (2 · Δµs) (µJ per µs is watts) |
| Gpu power, from a power reading | µW (amdgpu), or mW (NVML) | (µW + 500 000) div 1 000 000; (mW + 500) div 1000 |

- For example: 4 busy jiffies in 1000 is 0.4 % and draws `0` (DeepCreative: `1`); 37 200 m°C draws `37°C` (DeepCreative: `38°C`); 57 °C draws `135°F`, from 134.6 (DeepCreative: `134°F`).
- A negative reading is drawn as dashes ([Unavailable values](#unavailable-values)).
- The cooler's fan and pump speeds, shown only in the app, are whole RPM as read, capped at 9999.
- Music's spectrum isn't a number drawn: its analysis stays in floating point ([music.md](music.md#opendeepcools-spectrum-precisely)).

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
- **One value:** centred in the 14 columns. The spare columns are shared evenly; when their number is odd, the extra one goes on the left. So a single spare column is on the left and the picture ends at column 14 (`42%`, `65W`, `98°F`, as DeepCreative), and `7%` (9 wide, 5 spare) has 3 blank columns on the left and 2 on the right, columns 4–12.
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
- **When it happens:** the reading is unavailable (no driver rule, no device, no permission), a read failed and the sensor is being found again, or a rate metric (CPU Usage, CPU Power, Gpu power from an energy counter) is in its first second after start-up, or after the PC wakes from sleep ([sensors.md](sensors.md#updates-and-rounding)), and has only one sample. A reading no sensor can give, negative, not a number or infinite, is drawn as dashes too (owner, 2026-10-24). With OpenDeepCool's exact whole-number readings, only a negative one can occur (owner, 2026-10-26). A reading of 10 000 or more isn't one of these: it is capped at 9999 ([Values](#values); owner, 2026-10-25).
- In the settings app, a mode whose metric is persistently unavailable can't be chosen ([settings.md](settings.md#what-the-settings-app-must-offer)). Dashes on the cooler cover the cases the app can't prevent: the saved mode losing its reading after it was chosen, short read failures, and the first second.

## Mode switches

| Switch into | What the display does | Evidence |
|---|---|---|
| A static mode (Load Mode, °C temperature), from any mode | The new picture replaces the old one in a single frame. A scrolling old picture keeps scrolling until then | capture: 2026-10-22, transition steps; 2026-10-23, transition probe |
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

**OpenDeepCool: a mode switch jumps** (owner, 2026-10-24). The next frame shows the new mode's picture at its home position, with current readings: nothing scrolls out, and no old text comes back. If a value of the new mode is 100 or more, the strip starts moving from home ([Values](#values), OpenDeepCool: one smooth movement). A change of the rotation, of the chosen GPU, or of the temperature unit in a mode that shows a temperature is treated the same way (owner, 2026-10-24). Choosing what is already set changes nothing. A unit change in a mode without a temperature changes nothing on the cooler either: the unit is only saved, for the next temperature mode. The app's **Refresh** button also resets: it takes fresh readings at once and shows the picture at home, scrolling from there if a value is 100 or more.

**Test (Phases 7–8):** the transition tests switch between every pair of modes, with a different reading for every metric, with values below and above 100. The frame after the switch must be the new mode's picture at home, and every frame after it must follow the new mode's movement. Nothing of the old picture may appear after the switch. The full list is in [Test scenarios](#test-scenarios-opendeepcool).

### Movement, precisely

The rules above as one procedure, which the Rust implementation and the test oracle both follow. Columns are numbered 1–14 on the unrotated picture; rotation is applied to the finished frame.

- **The picture:** the strip of the mode's values (both bands, aligned so that they end at the same column), its width *w*, its cycle length *L* = *w* + max(4, 21 − *w*), and its home column *h*: where the static layout puts its first column, or 2 when *w* is over 14. It **needs to scroll** when any value is 100 or more.
- **State:** *x*, the column of the strip's first column (it may be off the screen), and whether the strip is **moving**. The strip repeats every *L* columns: copies start at *x*, *x* + *L*, *x* − *L*, and so on.
- **Each tick:**
  1. Apply what changed since the last tick: a reset, or a new picture.
  2. If the strip is moving, doesn't need to scroll, and *x* = *h*: it stops.
  3. Draw the frame at *x*.
  4. If it's moving: *x* = *x* − 1, then while *x* ≤ *h* − *L*: *x* = *x* + *L*.
- **A reset** (start-up, a mode switch, a rotation change, a unit change in a mode that shows a temperature, another GPU, Refresh): the new picture, *x* = *h*, moving if it needs to scroll.
- **A new picture without a reset** (a reading changed): the copy whose first column is on the screen, the rightmost one if two are, keeps its first column where it was; if none is, the copy at *x* does. With the new *h* and *L*, while *x* ≤ *h* − *L*: *x* = *x* + *L*. *x* may end up right of *h*, for example a centred `7%` (*x* = 4) turning into `100%` (*h* = 2): the strip then moves left from where it is. The strip starts moving if the new picture needs to scroll, and a moving strip keeps moving.
- **Choosing what is already set**, and a unit change in a mode without a temperature: nothing changes.

## Test scenarios (OpenDeepCool)

The display's behaviour over time, as the automated tests of OpenDeepCool's Rust implementation must check it (owner, 2026-10-24). These are OpenDeepCool's own rules: DeepCreative handles these cases differently (caps, fixed layouts, a scroll that finishes its cycle), so the scenarios are never compared with DeepCreative's captures. Each scenario is a sequence of readings and settings changes at chosen ticks, with the frame expected at every tick, generated from this spec's rules like [frames/](frames/). Below, *L* is the cycle length in frames and *k* the number of columns the strip has moved past home.

**Values and movement** (the owner's list first):

| ID | Scenario | Expected |
|---|---|---|
| S1 | A value of 100 or more | Scrolls from home: the whole number first, the symbol cut; then one column per frame, an *L*-frame cycle, repeating |
| S2 | Static → 100 or more | The digits change on the next frame, where the static picture's first digit place was, and the strip starts moving from there |
| S3 | 100 or more → 99 or less | The strip keeps moving until home, then stops: (*L* − *k*) mod *L* more frames |
| S4 | 100 or more → 99 or less → 100 or more, before home | Never stops; only the digits change; the position never jumps |
| S5 | Static → 100 or more → 99 or less → 100 or more → 99 or less, every change before home | Stops at home after the last change |
| S6 | 100 or more → 99 or less, one column after home (*k* = 1) | *L* − 1 more frames, then stops at home. Tested for every *k* from 0 to *L* − 1; at *k* = 0, still at home and not yet moved on, it stops at once |
| S7 | Rounding at the limit (the nearest whole number, halves up; owner, 2026-10-26) | Usage 99.49 → `99` static, 99.5 → `100` scrolls; Memory Load 99.49 → `99`, 99.5 → `100`; °C 99.49 and 99.5; °F: 37.49 °C → 99.48 → `99°F` static, 37.5 °C → 99.5 → `100°F` scrolls; power 99.49 and 99.5 |
| S8 | Length changes while scrolling: 3 ↔ 4 digits (999 ↔ 1000), and past the cap (9999 → 10000 → 9999) | The first digit place stays put; the cycle grows or shrinks (21 ↔ 25); 10000 draws `9999`, so the frames are the same as at 9999; home is still reached and stopped at |
| S9 | Length changes while static: 9 ↔ 10 | One value is re-centred: `9%` in columns 4–12, `10%` in 2–14. Two values stay ending at column 14 |
| S10 | Two values, one 100 or more and the other 99 or less | Both scroll together, last digits aligned; then each of the two crosses the limit in turn, both at once, and one up while the other comes down on the same tick (it keeps scrolling) |
| S10b | Two values scrolling, lengths changing: the longer one grows or shrinks, the shorter one grows past it (`1000` over `65` → `99` over `1500`) | The strip's first column stays put; the shorter value stays aligned to the longer one's right end |
| S11 | A value becomes unavailable, or comes back: while static, while scrolling, in one or both bands, and → a value of 100 or more | Dashes take two digit places, with the symbol; the movement rules don't change |
| S12 | The longest values: 4 digits (`9999%`, `9999W`, `9999°F`), in one band and in both, and readings far past the cap (123456 W, 10⁹ °C) | *L* = 25; every digit shown; anything past 9 999 is drawn `9999` (owner, 2026-10-25: capped at 9999 everywhere) |
| S13 | Readings arrive every 10th tick; the cycle is 21 or more | A change is tested at every *k* of the cycle |

**Settings, mode switches and the device:**

| ID | Scenario | Expected |
|---|---|---|
| S14 | A mode switch at every *k*, into each kind of mode (static, scrolling from home, Music Mode) and from each | The next frame is the new mode's picture at home; nothing of the old one |
| S15 | A temperature unit change, static and scrolling (`57°C` → `135°F`) | In a mode that shows a temperature: like a mode switch. In any other mode: nothing changes on the cooler; the unit is saved and used by the next temperature mode |
| S16 | A rotation change (each value, and Rotate Screen presses) while scrolling and while static | Like a mode switch: the next frame is the turned picture at home |
| S17 | Start-up into each mode: values of 99 or less, 100 or more, and the first second's dashes turning into a value of 100 or more | The saved picture at home on the first frame after `03 08` |
| S18 | The cooler disconnects while scrolling and comes back; and the PC wakes from standby, with the device node gone or still there (owner, 2026-10-26) | The position follows the ticks while it's away; the first frame back starts with `03 08`. After a wake-up, a new `03 08` comes before the next frame even when the node survived, and the rate metrics show dashes for their first second |
| S19 | Readings outside what a sensor can give, in each band, static and scrolling: negative ones (with exact readings, not-a-number and infinite can't occur) | Never a crash; drawn as dashes, like an unavailable value |
| S20 | Value rules at every length boundary, for every metric, on both sides of each rounding point: 0, 0.49 ↔ 0.5, 9.49 ↔ 9.5, 99.49 ↔ 99.5, 999.49 ↔ 999.5, 9999.49 ↔ 9999.5; in °F, 5536 ↔ 5537 °C (`9997°F` ↔ `9999°F`) and 5537.49 ↔ 5537.5 °C (`9999°F`, then 10 000 capped) | Each number as OpenDeepCool's rules give it, in full up to 9999; 10000 and more are drawn `9999` |
| S21 | A reading change and a settings change on the same tick | The settings change wins: the new picture at home, with the new reading |
| S22 | A tick whose deadline is missed (the frame is skipped) while scrolling | The strip moves one column per frame sent; nothing bunches up |
| S23 | A change the service refuses (an unavailable mode, a later phase's mode) | Nothing changes on the cooler |
| S24 | Choosing the mode, unit, rotation or GPU that is already set, static and scrolling, at every *k* | Nothing changes: the strip keeps its position and movement |
| S25 | Choosing another GPU, static and scrolling, at every *k* | Like a mode switch: the next frame is the picture at home with the new GPU's readings |
| S26 | The app's Refresh, static and scrolling, at every *k*, with values below and above 100 | Fresh readings at once; the next frame is the picture at home, and it scrolls from there if a value is 100 or more |

**Which modes, units and values:** every scenario runs in every mode that shows a value (1–12 and 18–22), in both temperature units where the mode shows a temperature, for single values and for combined values, with the change in the top value, the bottom value, and both. Every rotation is checked too: the expected frames of a rotation are the rotation-0 frames turned. In Music Mode (18–22) only the value area (rows 2–6) moves, and the spectrum keeps following the sound in every frame. The modes without values (13–17 full screen and emoji, 23 Dancer) run S14 to S18: switches into and out of them at every *k*, rotation, start-up and reconnect.

## Rotation

The Rotate Screen setting has four values. Value *r* turns the **whole finished frame** *r* × 90° clockwise:

| Value | Turn |
|---|---|
| 0 | none |
| 1 | 90° clockwise |
| 2 | 180° |
| 3 | 270° clockwise |

The turn is applied last, so scrolling text moves along the turned picture: after undoing the turn, every frame is the normal left-scrolling picture. The same holds in every mode, Music Mode included. Evidence: capture: the matrix sweeps of 2026-10-22, every mode at every rotation value; owner: 2026-10-22, modes 1, 4, 7 in °F and 10, and Music Mode 20 at 90°.

DeepCreative's on-screen preview of the display doesn't turn; only the cooler does (owner: 2026-10-22). So a screenshot of DeepCreative's window can't show rotation, and every rotation check compares the frames sent to the cooler.

## Observed one-off

Once, after switching from mode 9 in °C straight to mode 7 in °F with the CPU reading at 0, the display stayed blank for the whole 14-second step. Repeating the same sequence three times showed `32°F` normally. Treat it as a DeepCreative glitch, not as behaviour to reproduce. Evidence: capture: sweeps of 2026-10-22.
