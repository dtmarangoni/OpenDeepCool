# Meltwater: OpenDeepCool's own theme, dark and light (Phase 11a Task 54)

The final values of the own theme's two palettes, for `crates/odc-app/src/theme.rs` (`DARK`, `LIGHT`) and `tools/mockups/tests/mockups.rs` (`PALETTES`). The design is [settings-app.md](settings-app.md), "OpenDeepCool's own theme" and D46; `mockups-src/app.css` already holds these values. **Only the eight colours of each theme change.** The six control alphas, the selection alpha 0.25, the radii, the tokens and every rule stay as they are.

**Every figure here is a hand computation** (WCAG 2 relative luminance, the sRGB formula worked to six digits; the designer has no shell). The implementer recomputes them with `the_default_theme_has_the_contrast_the_design_states` (both copies) and pins them. A row under its need, or a figure that differs from this file by more than a tenth, goes back to the designer with its pair.

## The two palettes

The idea is **Meltwater**: one hue family, 200 to 210 degrees, in two states of cold water. The dark theme is the deep (a navy sea at night, one glacier-cyan highlight); the light theme is the ice (a pale frost under a deep-ocean ink). Hues are HSL, to the degree.

### Dark, the deep

| Token | Name | Hex | Rust triple | Hue | Why |
|---|---|---|---|---|---|
| `bg` | Abyss | `#0a1f33` | `0x0a, 0x1f, 0x33` | 209 | The ground: a navy of 67 per cent saturation and CIE lightness about 11, so a colour and not a stand-in for black; deeper than Coolant's, which gives the cyan room |
| `surface` | Current | `#11344f` | `0x11, 0x34, 0x4f` | 206 | The card and the preview's frame, one tonal step up (1.30:1 over Abyss) |
| `fg` | Spray | `#e6f3fa` | `0xe6, 0xf3, 0xfa` | 201 | A cool white, never warm; the lit dots are ice on the deep |
| `muted` | Haze | `#8db2c9` | `0x8d, 0xb2, 0xc9` | 203 | A mist-blue for hints and notes, 5.7:1 on the card |
| `accent` | Glacier | `#6bd3e8` | `0x6b, 0xd3, 0xe8` | 190 | The ground's own hue, lit: the midpoint of Coolors' `48CAE4` and `90E0EF` (both 190); greener and icier than the stock sky blue (198 to 201) |
| `red` | Buoy | `#ff665c` | `0xff, 0x66, 0x5c` | 4 | A red, not a pink; Coolant's Signal moved five units |
| `yellow` | Lantern | `#f6c553` | `0xf6, 0xc5, 0x53` | 42 | An amber that reads as the warm warning on a cool ground |
| `green` | Sea glass | `#52d68a` | `0x52, 0xd6, 0x8a` | 145 | A sea green, 44 degrees from the accent |

Alphas (unchanged): `[0.05, 0.44, 0.09, 0.60, 0.16, 0.24]`.

### Light, the ice

| Token | Name | Hex | Rust triple | Hue | Why |
|---|---|---|---|---|---|
| `bg` | Rime | `#cfe3f0` | `0xcf, 0xe3, 0xf0` | 204 | A pale ice blue ground, cool and not grey; paler would give the card under 1.2:1 |
| `surface` | Floe | `#f5fafd` | `0xf5, 0xfa, 0xfd` | 203 | The card: a near-white with a breath of the same blue (1.26:1 over Rime) |
| `fg` | Fathom | `#0b2742` | `0x0b, 0x27, 0x42` | 209 | A deep navy ink, the dark theme's Abyss hue again |
| `muted` | Brine | `#3a5971` | `0x3a, 0x59, 0x71` | 206 | The ink's muted step: 5.6:1 on Rime and 7.0 on Floe, off the rounding edges |
| `accent` | Marine | `#085f7d` | `0x08, 0x5f, 0x7d` | 195 | A deep ocean cerulean-teal: Coolors' `007EA7` (195) made darker, since it is only 3.5:1 on Rime |
| `red` | Lifebuoy | `#bd3326` | `0xbd, 0x33, 0x26` | 5 | A deep red, clear on pale blue |
| `yellow` | Ochre | `#7f5300` | `0x7f, 0x53, 0x00` | 39 | The amber that reaches 4.5:1 on a pale ground, which is this dark |
| `green` | Kelp | `#1a7c4e` | `0x1a, 0x7c, 0x4e` | 152 | A sea green, plainly green |

Alphas (unchanged): `[0.04, 0.56, 0.07, 0.74, 0.14, 0.20]`.

## For `theme.rs`

```rust
const DARK: Palette = Palette {
    bg: Rgb([0x0a, 0x1f, 0x33]),      // Abyss
    surface: Rgb([0x11, 0x34, 0x4f]), // Current
    fg: Rgb([0xe6, 0xf3, 0xfa]),      // Spray
    muted: Rgb([0x8d, 0xb2, 0xc9]),   // Haze
    accent: Rgb([0x6b, 0xd3, 0xe8]),  // Glacier
    red: Rgb([0xff, 0x66, 0x5c]),     // Buoy
    yellow: Rgb([0xf6, 0xc5, 0x53]),  // Lantern
    green: Rgb([0x52, 0xd6, 0x8a]),   // Sea glass
    alphas: [0.05, 0.44, 0.09, 0.60, 0.16, 0.24],
};

const LIGHT: Palette = Palette {
    bg: Rgb([0xcf, 0xe3, 0xf0]),      // Rime
    surface: Rgb([0xf5, 0xfa, 0xfd]), // Floe
    fg: Rgb([0x0b, 0x27, 0x42]),      // Fathom
    muted: Rgb([0x3a, 0x59, 0x71]),   // Brine
    accent: Rgb([0x08, 0x5f, 0x7d]),  // Marine
    red: Rgb([0xbd, 0x33, 0x26]),     // Lifebuoy
    yellow: Rgb([0x7f, 0x53, 0x00]),  // Ochre
    green: Rgb([0x1a, 0x7c, 0x4e]),   // Kelp
    alphas: [0.04, 0.56, 0.07, 0.74, 0.14, 0.20],
};
```

The doc comments of both constants name Meltwater and the tones instead of Coolant's (Lagoon, Tide, Foam, Fog and Flare; Frost, Glacier, Deep, Slate and Flare). `Tokens::default_theme`'s `selection` is `blend(accent, bg, 0.25)`, rounded: dark `(34, 76, 96)` = `#224c60`, light `(157, 194, 211)` = `#9dc2d3`.

## For `tools/mockups/tests/mockups.rs` (`PALETTES`)

```text
default dark : background "#0a1f33", surface "#11344f", foreground "#e6f3fa", muted "#8db2c9",
               accent "#6bd3e8", red "#ff665c", yellow "#f6c553", green "#52d68a", selected 0.16, border 0.44
default light: background "#cfe3f0", surface "#f5fafd", foreground "#0b2742", muted "#3a5971",
               accent "#085f7d", red "#bd3326", yellow "#7f5300", green "#1a7c4e", selected 0.14, border 0.56
```

`app.css` already has them (the first two blocks, same property names). The sample Omarchy theme and its figures (text 11.9, muted 5.4, accent 7.2) are untouched. Regenerate the mockups: `cargo run -p odc-mockups -- docs/design/mockups`.

## The 15 figures, in the test's order

The pairs are the test's: rows 1 to 10 are `--fg`, `--muted`, `--accent` and `--yellow` on `--bg` and `--surface`, then `--red` and `--green` on `--bg`; row 11 `--fg` on `--fg` at `selected` over `--surface`; row 12 `--fg` at the normal border alpha over `--bg`, against `--bg`; row 13 `--fg` against `--fg` at 0.12 over `--bg`; row 14 `--accent` on `--fg` at `selected` over `--bg`; row 15 `--fg` on the rounded `blend(accent, bg, 0.25)`. The first column is the one to pin; the second is the hand value to three decimals.

| # | Need | Dark, pin | Dark, hand | Light, pin | Light, hand |
|---|---|---|---|---|---|
| 1 | 4.5 | 14.8 | 14.773 | 11.5 | 11.495 |
| 2 | 4.5 | 11.4 | 11.404 | 14.4 | 14.442 |
| 3 | 4.5 | 7.4 | 7.436 | 5.6 | 5.584 |
| 4 | 4.5 | 5.7 | 5.740 | 7.0 | 7.016 |
| 5 | 4.5 | 9.6 | 9.635 | 5.4 | 5.402 |
| 6 | 4.5 | 7.4 | 7.437 | 6.8 | 6.787 |
| 7 | 4.5 | 10.4 | 10.369 | 5.1 | 5.064 |
| 8 | 4.5 | 8.0 | 8.004 | 6.4 | 6.362 |
| 9 | 3.0 | 5.8 | 5.815 | 4.3 | 4.338 |
| 10 | 3.0 | 9.0 | 9.027 | 3.9 | 3.938 |
| 11 | 4.5 | 7.2 | 7.210 | 11.0 | 10.989 |
| 12 | 3.0 | 3.9 | 3.891 | 3.4 | 3.401 |
| 13 | 3.0 | 10.6 | 10.596 | 9.2 | 9.203 |
| 14 | 3.0 | 6.1 | 6.081 | 4.2 | 4.160 |
| 15 | 4.5 | 8.2 | 8.177 | 8.0 | 8.019 |

As the test's arrays: dark `[14.8, 11.4, 7.4, 5.7, 9.6, 7.4, 10.4, 8.0, 5.8, 9.0, 7.2, 3.9, 10.6, 6.1, 8.2]`, light `[11.5, 14.4, 5.6, 7.0, 5.4, 6.8, 5.1, 6.4, 4.3, 3.9, 11.0, 3.4, 9.2, 4.2, 8.0]`.

- **Within 0.02 of a rounding edge, so a pin may flip by a tenth** (nine rows): dark rows 3, 4, 5, 6 and 7 (7.436, 5.740, 9.635, 7.437, 10.369); light rows 7, 8, 9 and 10 (5.064, 6.362, 4.338, 3.938). The mockups test's two notes are close too: muted on the selected fill, dark 3.629 (note: 3.6, still under 4.5) and light 5.338 (note: 5.3). A flip is only a pin to change, never a fault; a row under its need is.
- **Tightest margins:** light row 7 (5.1 against 4.5, 12.5 per cent), light row 12 (3.4 against 3.0), light rows 3 and 5. If light row 12 misses, raise the light border alpha by 0.02 (0.58 gives about 3.6). Dark: row 12 (3.9 against 3.0) and row 4 (5.7 against 4.5).
- **Tints the test also asserts** (`--fg` against the status colour at 0.16 and 0.14 over `--bg`, rounded; need 4.5): about 12.2 and 11.0 in the dark theme, 9.1 and 9.5 in the light one.
- **Other pairs, hand values, not pinned:** unlit dots on `--surface` 8.1 (dark) and 11.4 (light); the control border on `--surface` 3.5 and 3.7; the status icons on their tints, red 4.8 and 3.4, yellow 7.7 and 4.2 (against 3.0); the knob (`--fg`) against an `--accent` trough 1.5 and 2.1, which the 1 px `--bg` ring answers (row 5: 9.6 and 5.4).
- **The Glassy bounds** (see-through window, worst backdrop: white under the dark theme, black under the light; not pinned): text on the bare ground 6.6 and 6.4; text on a card 10.0 and 12.0; muted on a card 5.1 and 5.8; muted and accent on the bare ground 3.3 and 4.3 (dark), 3.1 and 3.0 (light).

## What to check, and what to send back

1. Recompute the 15 rows of both themes. Tell me any row under its need, or off by more than a tenth, with its pair and ratio; I change the value, not the need.
2. Look at the regenerated mockups, both themes, at least the display page, the Configuration page, a banner and the glassy page: the accent as a selected tile's border, an on switch's trough, a scale's fill and a link; the warning and error banners; the status dots. Tell me what reads as pink, purple, warm, flat or too dark.
3. In a real window (not a mockup): the dark ground on the owner's panel (it is the darkest the design has had), a link in the light theme (the accent is 2.1:1 from `--fg`, so it differs from text by hue only) and the knob of an on switch.
