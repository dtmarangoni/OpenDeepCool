# Music Mode

What DeepCreative 1.2.14 shows in Music Mode (modes 13–22), and what OpenDeepCool does differently. Grid, glyphs, value rules, scrolling and [rotation](display.md#rotation) are as in [display.md](display.md); evidence labels are defined in [README.md](README.md).

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

Evidence: code: LP page modes (Phase 1 report); capture: Music Mode passes of 2026-10-22, in silence and with a loud bass chord.

## Spectrum: layout (as DeepCreative)

- **Columns:** 14 bars, one per column, from column 1 (left) to 14 (right).
- **Bars:** each is solid, from the bottom row up, with no gaps. Capture: every frame of the 2026-10-22 passes.
- **Floor:** the bottom row (row 14) is always lit, so silence shows a flat line one pixel high.
- **Height:** full screen, 1 to 12 rows (rows 14 up to 3); split modes, 1 to 6 rows (rows 14 up to 9). Rows above stay off.
- **Updates:** the bars change from frame to frame while sound plays. Capture: Phase 1, with every frame different during music.

Evidence: capture: 2026-10-22, the loud-chord pass reached rows 3 and 9 but never above.

## Spectrum: analysis (OpenDeepCool's own)

DeepCreative's own analysis is **not reproduced** (owner decision, 2026-10-22; see the PRD).
- **What DeepCreative does under Wine:** its 14 columns each cover one band about 10.8 Hz wide, so the whole display spans only about 5–160 Hz. Alternate frames also show a weaker copy at half the frequency, which may be a Wine artefact. Capture: test tones and a sweep, 2026-10-22.
- **What OpenDeepCool does instead:** imitate Winamp's classic analyser, with bands spread across the whole audible range over the 14 columns. Winamp's falloff and peak behaviour are unverified; Phase 9 designs them, and the owner tunes them by eye.
- **Source:** the monitor of the default audio output, as the PRD says.

## Value area (modes 18–22)

The value is drawn in rows 2–6 with the [display rules](display.md):
- the same value rules;
- °C and `%` static, in the [static layout](display.md#static-layout-usage-and-temperature-in-c);
- °F scrolling, in the [scrolling layout](display.md#scrolling-layout-temperature-in-f-and-power).

Examples: `57°C`, `38°C`, `42%`, `7%`, `63%`; in °F, modes 18 and 19 scroll. Capture: 2026-10-22, with injected values.

**OpenDeepCool: no caps** (owner, 2026-10-24). A value that doesn't fit the static layout, for example `100%`, scrolls in the value area, rows 2–6. Only the value scrolls; the spectrum bars below it never do ([display.md](display.md#values), OpenDeepCool: no caps).

**DeepCreative quirk, not reproduced:** in mode 18 with °F, DeepCreative draws the **Gpu temperature**, not the CPU Temperature the option names. With CPU 57 °C and GPU 38 °C it scrolled `100°F` (the GPU) instead of `134°F`; with the GPU at 0 it scrolled `32°F`. Mode 18 in °C and mode 19 in both units draw the right metric. Capture: music sweep step `MusicLoud 18`, unit 1, and the matrix sweep, in all four rotations; owner: 2026-10-22, `32°F` and then `100°F` on the cooler.

**OpenDeepCool:** mode 18 shows the CPU Temperature in both units. Every mode shows the metric it names (owner decision, 2026-10-22; AGENTS.md rule 11).

## Emoji animations (modes 14–17)

- **Timing:** each animation is a fixed loop on a 100 ms tick. It plays the same in silence and with sound, so the music doesn't drive it.
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
