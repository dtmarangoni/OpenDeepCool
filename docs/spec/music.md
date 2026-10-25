# Music Mode

What DeepCreative 1.2.14 shows in Music Mode (modes 13–22), what OpenDeepCool does differently, and OpenDeepCool's own Dancer (mode 23). Grid, glyphs, value rules, scrolling and [rotation](display.md#rotation) are as in [display.md](display.md); evidence labels are defined in [README.md](README.md).

**Proof.** A script that applies only the rules below checks 104 matrix sweep steps: modes 13–22 in all four rotations, modes 18 and 19 in both temperature units, and three value sets for the value modes, with tones playing. After undoing the rotation, the value area and the emoji pictures match pixel for pixel, and every spectrum frame follows the layout rules below. In every step the bars rose above the floor. Evidence: capture: matrix sweep of 2026-10-22 (`research/hook/render-check.py`, local). The expected frame of every case is in [frames/](frames/), with `?` marking spectrum pixels.

**Owner checks, 2026-10-22, on the cooler:** modes 13–17; mode 18 in °C and in °F (the quirk below); mode 19 in °F; modes 20, 21 and 22; mode 20 at 90°.

## Modes

| Mode | Page and option | Top area (rows 1–8) | Spectrum area |
|---|---|---|---|
| 13 | Music Mode → Full Screen | — | rows 3–14 |
| 14–17 | Music Mode → Split → Emoji Animation + Spectrum, emojis 1–4 | the emoji animation | rows 9–14 |
| 18 | Split → Temperature + Spectrum → CPU | CPU Temperature | rows 9–14 |
| 19 | Split → Temperature + Spectrum → GPU | Gpu temperature | rows 9–14 |
| 20 | Split → Usage + Spectrum → CPU | CPU Usage | rows 9–14 |
| 21 | Split → Usage + Spectrum → GPU | Gpu usage | rows 9–14 |
| 22 | Split → Usage + Spectrum → RAM | Memory Load | rows 9–14 |
| 23 | OpenDeepCool's own: Dancer | the whole grid: a dancing figure ([Dancer](#dancer-mode-23-opendeepcools-own)) | — |

Evidence: code: LP page modes (Phase 1 report); capture: Music Mode passes of 2026-10-22, in silence and with a loud bass chord.

## Spectrum: layout (as DeepCreative)

- **Columns:** 14 bars, one per column, from column 1 (left) to 14 (right).
- **Bars:** each is solid, from the bottom row up, with no gaps. Capture: every frame of the 2026-10-22 passes.
- **Floor:** the bottom row (row 14) is always lit, so silence shows a flat line one pixel high.
- **OpenDeepCool's peak dots** (owner, 2026-10-25): when they're on, each column may also have one lit pixel above its bar, with off pixels between them ([below](#opendeepcools-spectrum-precisely)). DeepCreative draws no peaks.
- **Height:** full screen, 1 to 12 rows (rows 14 up to 3); split modes, 1 to 6 rows (rows 14 up to 9). Rows above stay off.
- **Updates:** the bars change from frame to frame while sound plays. Capture: Phase 1, with every frame different during music.

Evidence: capture: 2026-10-22, the loud-chord pass reached rows 3 and 9 but never above.

## Spectrum: analysis (OpenDeepCool's own)

DeepCreative's own analysis is **not reproduced** (owner decision, 2026-10-22; see the PRD).
- **What DeepCreative does under Wine:** its 14 columns each cover one band about 10.8 Hz wide, so the whole display spans only about 5–160 Hz. Alternate frames also show a weaker copy at half the frequency, which may be a Wine artefact. Capture: test tones and a sweep, 2026-10-22.
- **What OpenDeepCool does instead:** imitate Winamp's classic analyser, with bands spread across the whole audible range over the 14 columns. The rules below were chosen by the owner on 2026-10-25, after watching them on a preview page that drew the 14 × 14 grid at the cooler's 100 ms tick.
- **Source:** the monitor of the default audio output, as the PRD says, at 48 000 samples per second, the channels averaged into one.

### OpenDeepCool's spectrum, precisely

Every Music mode (13–22) draws its spectrum this way, the emoji modes included (owner, 2026-10-25). *H* is the spectrum area's height: 12 rows in mode 13, 6 rows in modes 14–22.

1. **Analysis, at every tick:** the latest 2048 samples, windowed and transformed as the W3C Web Audio API's `AnalyserNode` defines it: a Blackman window with α = 0.16, then X[k] = (1/N) Σ x̂[n] e^(−2πikn/N), with N = 2048 and no smoothing over time. Each bin's level is 20 log₁₀ |X[k]| dB. Evidence: W3C Web Audio API 1.1, AnalyserNode, "FFT windowing and smoothing over time" (read 2026-10-25). The preview page used the browser's own `AnalyserNode` with 4096 samples; OpenDeepCool uses 2048 to keep the delay target below.
2. **Columns:** 14 bands, their edges spaced evenly on a log scale from 40 Hz to 16 kHz: edge *i* = 40 × 400^(*i*/14) Hz, *i* = 0 … 14. Column *c* (1 = left) takes the highest level of the bins whose frequency is at least edge *c* − 1 and below edge *c*, and at least the one bin nearest edge *c* − 1.
3. **Loudness:** −75 dB or less is level 0, −25 dB or more level 1, linear in between.
4. **Falloff stops: Winamp's** (owner, 2026-10-25). Bars and peaks each have Winamp's five falloff settings, slower, slow, moderate, fast and faster, with Winamp's per-frame values: bars 3, 6, 12, 16 and 32; peaks 1.05, 1.1, 1.2, 1.4 and 1.6. Winamp's defaults are moderate for bars and slow for peaks. Evidence: Webamp `packages/webamp/js/components/VisPainter.ts` (read 2026-10-25), which reports them from Winamp 5.666 and 2.63 executables and names the defaults "of a fresh new Winamp installation". Winamp's frame rate isn't documented (unverified), so one Winamp frame is taken as 30 ms, which makes Winamp's default bar fall the 0.6 s the owner chose; the other stops keep Winamp's ratios. A tick is 10/3 Winamp frames.

   | Stop | Bar value | Bar: full height falls in | Peak value | Peak: full height falls in |
   |---|---|---|---|---|
   | slower | 3 | 2.4 s | 1.05 | 2.6 s |
   | slow | 6 | 1.2 s | **1.1 (default)** | 1.5 s |
   | moderate | **12 (default)** | 0.6 s | 1.2 | 0.9 s |
   | fast | 16 | 0.45 s | 1.4 | 0.57 s |
   | faster | 32 | 0.225 s | 1.6 | 0.45 s |
5. **Bars:** a bar rises to a higher level at once. Otherwise it falls by *b* ÷ 72 of the full height per tick (*b* = the bar value above), never below the current level. It's drawn max(1, round(level × *H*)) rows high, from the floor up.
6. **Peak dots** (when on): each column keeps the level *p₀* its peak was last set to and the ticks *t* since then. After the bar is updated: if the bar is at the peak or above, *p₀* = the bar and *t* = 0; otherwise *t* = *t* + 1 and the peak is max(0, *p₀* − (3 ÷ 3840) × (*g*^(*t* × 10/3) − 1) ÷ (*g* − 1)), with *g* the peak value above. That is Winamp's recurrence (a speed of 3/256 pixel on a 15-pixel display, growing by *g* every frame), summed over the frames of each tick. The dot is one pixel at height round(peak × *H*) rows, drawn only when that is above the bar and within the area.
7. **Settings** (owner, 2026-10-25), on the settings app's configuration screen ([settings.md](settings.md#what-the-settings-app-must-offer)):
   - peak dots on or off, on by default;
   - *bar fall*: a slider with the five stops, moderate by default;
   - *peak fall*: a slider with the five stops, slow by default.

   A change shows on the next frame and resets nothing: the bars and peaks keep their levels. In a mode without a spectrum it is only saved.
8. **Continuity:** the bars and peaks keep their levels across a switch between Music modes, a rotation change and Refresh, and are drawn at the new mode's *H*. Entering a Music mode from any other mode starts them at level 0.
9. **Delay** (owner, 2026-10-25): at most 150 ms for 95 % of frames, from the sound reaching the output's monitor to the frame written to the cooler. The 100 ms tick alone adds 0–100 ms.

The values in points 2 and 3 are starting values the owner accepted on the preview page; changing them later is a spec change, not a setting.

## Value area (modes 18–22)

The value is drawn in rows 2–6 with the [display rules](display.md):
- the same value rules;
- °C and `%` static, in the [static layout](display.md#static-layout-usage-and-temperature-in-c);
- °F scrolling, in the [scrolling layout](display.md#scrolling-layout-temperature-in-f-and-power).

Examples: `57°C`, `38°C`, `42%`, `7%`, `63%`; in °F, modes 18 and 19 scroll. Capture: 2026-10-22, with injected values.

**OpenDeepCool: no caps** (owner, 2026-10-24). The value area is static while the value fits (99 or less, °F included) and scrolls by itself when it's 100 or more, for example `100%` or `134°F`, in rows 2–6. Only the value scrolls; the spectrum bars below it never do ([display.md](display.md#values), OpenDeepCool: no caps).

**DeepCreative quirk, not reproduced:** in mode 18 with °F, DeepCreative draws the **Gpu temperature**, not the CPU Temperature the option names. With CPU 57 °C and GPU 38 °C it scrolled `100°F` (the GPU) instead of `134°F`; with the GPU at 0 it scrolled `32°F`. Mode 18 in °C and mode 19 in both units draw the right metric. Capture: music sweep step `MusicLoud 18`, unit 1, and the matrix sweep, in all four rotations; owner: 2026-10-22, `32°F` and then `100°F` on the cooler.

**OpenDeepCool:** mode 18 shows the CPU Temperature in both units. Every mode shows the metric it names (owner decision, 2026-10-22; AGENTS.md rule 11).

## Dancer (mode 23, OpenDeepCool's own)

A stick figure dances to the music on the whole 14 × 14 grid, in place of the bars (owner, 2026-10-25: requested, then approved on the preview page). DeepCreative has nothing like it, so mode 23 is OpenDeepCool's own number, after DeepCreative's 1–22. It shows no value and no spectrum; the peak dots and falloff settings don't apply to it.

**Poses.** Eleven pictures, numbered as the rules below use them. Each is drawn from a skeleton (a 3 × 3 ring for the head; one-pixel lines from the neck to the hip, from the shoulder to each elbow and hand, and from the hip to each knee and foot), and the pictures below are what counts. "Left" is the left of the picture as the cooler shows it, before [rotation](display.md#rotation).

```
 0 standing still    1 sway left      2 sway right     3 arms up    
.....###......   ....###.......   ......###.....   .....###......
.....#.#......   ....#.#.......   ......#.#.....   ...#.#.#.#....
.....###......   ....###.......   ......###.....   ...#.###.#....
......#.......   .....#........   .......#......   ....#####.....
......#.......   .....#........   .......#......   ......#.......
.....###......   ...####.......   ......####....   ......#.......
....#.#.#.....   ..#...##......   .....##...#...   ......#.......
....#.#.#.....   ..#...#.#.....   ....#.#...#...   ......#.......
....#.#.#.....   ......#.#.....   ....#.#.......   ......#.......
......#.......   ......##......   .....##.......   .....#.#......
.....#.#......   .....#.#......   .....#.#......   .....#.#......
.....#.#......   .....#..#.....   ....#..#......   ....#...#.....
.....#.#......   ....#...#.....   ....#...#.....   ...#.....#....
.....#.#......   ....#...#.....   ....#...#.....   ...#.....#....
```

```
 4 squat          5 left knee      6 right knee     7 point left 
..............   .....###......   .....###......   .....###......
..............   .....#.#......   .....#.#......   ..#..#.#......
.....###......   .....###......   .....###......   ...#.###......
.....#.#......   ...#..#.......   ......#..#....   ....###.......
.....###......   ...#..#.......   ......#..#....   ......#.......
......#.......   ....####......   .....####.....   ......##......
......#.......   ......#.#.....   ....#.#.......   ......##......
....#####.....   ......#..#....   ...#..#.......   ......#.#.....
..##..#..##...   ......#..#....   ...#..#.......   ......#.#.....
......#.......   ....###.......   ......###.....   ......##......
......#.......   .....#.#......   .....#.#......   .....#.#......
....##.##.....   .....#.#......   .....#.#......   .....#..#.....
....#...#.....   .......#......   .....#........   .....#...#....
....#...#.....   .......#......   .....#........   .....#...#....
```

```
 8 point right    9 arms out      10 jump       
.....###......   .....###......   .....###......
.....#.#..#...   .....#.#......   ...#.#.#.#....
.....###.#....   .....###......   ...#.###.#....
......###.....   ......#.......   ....#####.....
......#.......   ..#########...   ......#.......
.....##.......   ......#.......   ......#.......
.....##.......   ......#.......   ......#.......
....#.#.......   ......#.......   ......#.......
....#.#.......   ......#.......   ......#.......
.....##.......   ......#.......   .....#.#......
.....#.#......   .....#.#......   ....#...#.....
....#..#......   .....#.#......   .....#.#......
...#...#......   ....#...#.....   .....#.#......
...#...#......   ....#...#.....   ..............
```
**Rules,** at every tick, from the 14 column levels of [OpenDeepCool's spectrum](#opendeepcools-spectrum-precisely) (point 3, before any bar fall):

- **Inputs:** *bass* is the highest level of columns 1–4 (40 Hz to about 222 Hz); *loudest* is the highest level of all 14.
- **State:** the pose; the previous tick's *bass*; the ticks since the last beat; the quiet ticks; the step in the big-move routine; the sway side. At a reset (start-up, a switch into mode 23, a rotation change, another GPU, Refresh), the state starts again: pose 0, previous *bass* 0, 99 ticks since a beat, 0 quiet ticks, step 0, and the next sway goes right, as the emoji loops restart ([Emoji animations](#emoji-animations-modes-1417)).
- **Each tick, in order:**
  1. Add one to the ticks since the last beat.
  2. If *loudest* is below 0.1, add one to the quiet ticks; otherwise set them to 0.
  3. **Silence:** with 3 quiet ticks or more (300 ms), the pose is 0, standing still and straight.
  4. **Beat:** otherwise, if *bass* is at least 0.35, rose by at least 0.12 since the previous tick, and at least 2 ticks have passed since the last beat: the ticks since the last beat become 0, and
     - with *bass* at 0.6 or more, the pose is the next of the big-move routine: 3, 4, 5, 6, 7, 8, 9, 10, 4, 3 (arms up, squat, left knee, right knee, point left, point right, arms out, jump, squat, arms up), then from the start again;
     - below 0.6, a sway: the side flips and the pose is 2 (sway right) or 1 (sway left). The first sway after a reset is to the right.
  5. **No beat for a while:** otherwise, if 8 ticks or more have passed since the last beat, *loudest* is at least 0.1, and the ticks since the last beat minus 8 are a multiple of 5: a sway, as in point 4.
  6. Otherwise the pose stays.
  7. Keep *bass* as the previous tick's.
- The pose is drawn on the whole grid, then rotated.

## Emoji animations (modes 14–17)

- **Timing:** each animation is a fixed loop on a 100 ms tick. It plays the same in silence and with sound, so the music doesn't drive it.
- **OpenDeepCool: a reset restarts the loop** (owner, 2026-10-25). A reset (start-up, a switch into the mode, a rotation change, another GPU, Refresh; [display.md](display.md#mode-switches)) shows the loop's first picture on the next frame, as a value's picture restarts from home.
- **Area:** the pictures occupy rows 1–7; rows 8–14 are the spectrum area.

Evidence: capture: 2026-10-22, the silent and loud passes give the same pictures and loop.

Every picture is shown in full for rows 1–7. Rows not listed are all off.

### Emoji 1 (mode 14)

Loop: picture 2 for 400 ms, then picture 1 → 2 → 1 → 2 → 1, 100 ms each. That's 900 ms in all.

```
picture 1        picture 2
..............   ...#......#...   row 1
..............   ....#....#....   row 2
...###..###...   .....#..#.....   row 3
..............   ....#....#....   row 4
..............   ...#......#...   row 5
..............   ..............   row 6
.....####.....   .....####.....   row 7
```

### Emoji 2 (mode 15)

Loop: picture 1 for 400 ms, then picture 2 → 1 → 2 → 1 → 2, 100 ms each. That's 900 ms in all.

```
picture 1        picture 2
...#......#...   ..............   row 1
..#.#....#.#..   ..............   row 2
.#...#..#...#.   ..###....###..   row 3
..............   ..............   row 4
..............   ..............   row 5
..............   ..............   row 6
.....####.....   .....####.....   row 7
```

### Emoji 3 (mode 16)

Loop: picture 1 for 400 ms, then picture 2 → 1 → 2 → 1 → 2, 100 ms each. That's 900 ms in all.

```
picture 1        picture 2
..............   ..............   row 1
..###....###..   ..............   row 2
..###....###..   ..###....###..   row 3
..###....###..   ..............   row 4
..............   ..............   row 5
....#....#....   ....#....#....   row 6
.....####.....   .....####.....   row 7
```

### Emoji 4 (mode 17)

Loop: picture 1 (100 ms) → 2 (100) → 3 (100) → 2 (100) → 1 (200) → 4 (500). That's 1100 ms in all.

```
picture 1        picture 2        picture 3        picture 4
..............   ..............   ..............   ..............   row 1
.#####..#####.   .#####..#####.   .#####..#####.   .#####..#####.   row 2
...##.....##..   ..##.....##...   .##.....##....   ....##.....##.   row 3
..............   ..............   ..............   ..............   row 4
..............   ..............   ..............   ..............   row 5
.....####.....   .....####.....   .....####.....   .....####.....   row 6
..............   ..............   ..............   ..............   row 7
```

Timings are DeepCreative's 100 ms tick, measured from the frames it sent: each duration within the send jitter of 88–104 ms per tick.
