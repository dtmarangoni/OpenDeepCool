# Cool Blue Forest: OpenDeepCool's own theme, dark and light (Phase 11a Task 59; the light theme redone in Tasks 61 and 62)

The final values of the own theme's two palettes, for `crates/odc-app/src/theme.rs` (`DARK`, `LIGHT`) and `tools/mockups/tests/mockups.rs` (`PALETTES`). The design is [settings-app.md](settings-app.md), "OpenDeepCool's own theme", D51 and, for the light theme, D53 and D54; `mockups-src/app.css` already holds these values. **Task 59** changed the eight colours of each theme and two of the six control alphas (the normal border and the hover border). **Task 61 (2026-11-24)** changed two of Snowday's colours, its ground and its card. **Task 62 (2026-11-24, the owner: "the light theme is too bright now")** changed five of Snowday's eight colours, its ground, its card, its muted text and two of its status colours, and nothing else in either palette: the Dusk values, the six alphas of both themes, the selection alpha 0.25 and the radii are as Task 59 left them. What else Tasks 61 and 62 settled (the window class `own-light` and its rules, the title's size) is in the design document, not here.

**Every figure here is a hand computation** (WCAG 2 relative luminance, the sRGB formula worked to six digits; the designer has no shell): the dark ones of 2026-11-23, the light ones of 2026-11-24 (Task 62's). The implementer recomputes them with `the_default_theme_has_the_contrast_the_design_states` and pins them. A row under its need, or a figure that differs from this file by more than a tenth, goes back to the designer with its pair.

## Task 62 (2026-11-24): what changed in Snowday

The owner found Task 61's light theme too bright. The measurements (from a screenshot of the installed app; the design's D54 and its answers have the whole account): the card, the tiles and the preview's frame were `#f9fafb`, relative luminance 0.955 and CIE lightness 98.2, a white by any eye (2 steps of 255 between its channels), and they cover about 60 per cent of the Configuration page's window; the ground `#d9e3ec` was 0.757 and L\* 89.7, and so were the picture's unlit dots on the frame, so the left column was one pale field with a white card in it. The window's mean lightness was L\* 95.1; it is now 89.5. The class `own-light` and its seven rules are not the cause and stay.

| Token | Task 61, history | Task 62 | Why |
|---|---|---|---|
| `bg` | Hoarfrost `#d9e3ec`, Y 0.757, L\* 89.7 | **`#c4d3e1`**, Y 0.638, L\* 83.8 | Swatch 2 lightened by about 44 per cent white and given 1.5 times a plain mix's chroma: 29 steps between its channels, where a plain mix at that lightness has 19 and reads grey. Hue 209 and HSL saturation 33 per cent, as before. It can't go lower while the accent is the owner's swatch 4: the accent needs 4.5 against it, which is Y 0.568 at the least, and 0.61 for a 6 per cent margin |
| `surface` | Fresh snow `#f9fafb`, Y 0.955, L\* 98.2 | **Powder snow `#e6ebf1`**, Y 0.826, L\* 92.8 | Swatch 2 with about 74 per cent white, a touch bluer than the plain mix (`#e6ebef`). It is no longer a white: 1.27:1 over the ground (the step was 1.24:1, 8.5 points of L\*; it is 9.0 now) |
| `muted` | Ash `#555862`, Y 0.098 | **`#4f525b`**, Y 0.085 | Swatch 5 twenty-five per cent of the way to swatch 1 (it was thirty): on the darker ground the old Ash is 4.6:1, 3 per cent over its need |
| `red` | Holly `#b33b32`, Y 0.129 | **`#a9322a`**, Y 0.109 | The error banner's icon sits on Holly at 16 per cent over the ground, which is darker now: with the old Holly the icon is 3.1:1 against its 3.0, with this one 3.5 |
| `yellow` | Resin `#785200`, Y 0.100 | **`#704d00`**, Y 0.088 | The warning text on the ground (row 7) would be 4.6 with the old Resin; it is 5.0 now |

Charcoal (`fg`), Deep slate (`accent`) and Spruce (`green`) stay, and so do the six alphas: row 12 is 3.4 (3.6 before), still 12.6 per cent over its need. The selection becomes `#a1b4c6` (the accent at 0.25 over the new ground; the red channel is 160.5 exactly, which `blend` rounds half away from zero to 161).

The 15 light figures, in the test's order. Task 61's: `[10.1, 12.6, 5.5, 6.8, 5.9, 7.3, 5.4, 6.7, 4.5, 4.1, 9.8, 3.6, 8.2, 4.6, 7.1]`. **Task 62's: `[8.6, 11.0, 5.1, 6.5, 5.0, 6.4, 5.0, 6.4, 4.3, 3.5, 8.6, 3.4, 7.1, 4.0, 6.2]`.** Every row passes; the tightest margin of the light theme is 11.1 per cent (row 7), where Task 61's was 19 and the dark theme's is 13.4 (row 6): the light theme is the tighter of the two now, which is the price of a ground that is not pale.

## Task 61 (2026-11-24): what changed in Snowday (history)

The owner found the light theme bad for the fourth time ("take a photo and look at it"). The cause was mostly structure, not colour (the design's D53 and its answers say what, and the class `own-light` fixes it), but the ground and the card moved too, because a lighter ground is what lets a card step from it. Task 62 moved them again; Task 61's are kept here as history.

| Token | Task 59 | Task 61 | Task 62 |
|---|---|---|---|
| `bg` | `#d3dce3` (swatch 2 with 55 per cent white) | `#d9e3ec` | `#c4d3e1` |
| `surface` | `#f5f7f9` (90 per cent white) | `#f9fafb` | `#e6ebf1` |

The 15 light figures, Task 59's: `[9.5, 12.3, 5.1, 6.6, 5.5, 7.1, 5.0, 6.5, 4.2, 3.9, 9.5, 3.5, 7.7, 4.3, 6.7]`.

## The scheme, and what each theme takes from it

The owner's colour scheme (2026-11-23): Folk & Fey's "Cool Blue Forest", from its Winter Forest Collection (an Instagram post whose caption reads "soft icy grey, frosted blue, muted steel blue, deep slate blue, and dark charcoal tones of a snow-covered winter forest"). The designer did not see the image: the printed labels and the pixel readings were given in the brief. Where label and pixel differ by one unit the label is used; the one real mismatch is swatch 5, whose label is `#1d2d3f` and whose pixels are `#2d3039`: **the pixels' colour is used** (owner's brief), and the label is not used anywhere.

| # | The caption's name | Label | Pixels | Hue, HSL saturation | Where it is used |
|---|---|---|---|---|---|
| 1 | soft icy grey | `#b3b6c2` | `#b3b7c2` | 228, 11 % | the dark theme's muted text exactly (Icy grey); the far end of the light theme's muted text |
| 2 | frosted blue | `#9eb1c0` | `#9fb0c0` | 206, 21 % | the light theme's ground and card, lightened with white (a little more chroma in the ground since Task 61, and, since Task 62, the ground at about 44 and the card at about 74 per cent white) |
| 3 | muted steel blue | `#6e91af` | `#6e91af` | 208, 29 % | the dark theme's accent, lifted |
| 4 | deep slate blue | `#365675` | `#375675` | 210, 37 % | the light theme's accent exactly (Deep slate); the dark card's far end |
| 5 | dark charcoal | `#1d2d3f` | **`#2d3039`** | 225, 12 % | the dark theme's ground exactly, the light theme's ink exactly (Charcoal); the light muted text's near end |

The idea is **Cool Blue Forest**, in two lights: **Dusk** (the dark theme: a winter forest at dusk) and **Snowday** (the light theme: the same forest in daylight). Two rules: **the greys speak, the blues build and mark** (swatches 1 and 5, 11 to 12 per cent saturated, are the text and the dark ground; swatches 2, 3 and 4, 21 to 37 per cent, are the card, the light ground and the accent), and **one swatch plays two roles** (Charcoal is Dusk's ground and Snowday's ink). Hues are HSL, to the degree.

## The two palettes

### Dark, Dusk

| Token | Name | Hex | Rust triple | Hue | Source and why |
|---|---|---|---|---|---|
| `bg` | Charcoal | `#2d3039` | `0x2d, 0x30, 0x39` | 225 | Swatch 5, its pixels' colour, exactly. CIE lightness about 20: a colour, not a stand-in for near-black (`#0b0b0b` and `#111` are about 3 and 5) |
| `surface` | Slate shade | `#313f51` | `0x31, 0x3f, 0x51` | 214 | Swatch 5 two fifths of the way to swatch 4: (48.6, 63.2, 81.0), rounded. One tonal step up, 1.23:1 over Charcoal, and bluer (saturation 25 per cent). Half way, `#324357`, would be 1.30:1 but leaves the accent 4.8 and the muted text 5.0 on it; swatch 4 itself leaves them 3.6 and 3.8 |
| `fg` | Snowlight | `#e5ebf2` | `0xe5, 0xeb, 0xf2` | 212 | A snow-white at the blue-greys' hue, 92 per cent lightness. The muted text is swatch 1 itself, so the text must stand a clear step above it (1.7:1); lit dots are this colour |
| `muted` | Icy grey | `#b3b6c2` | `0xb3, 0xb6, 0xc2` | 228 | Swatch 1 exactly: 6.5:1 on the ground, 5.3 on the card |
| `accent` | Steel | `#94b7d6` | `0x94, 0xb7, 0xd6` | 208 | Swatch 3 (110, 145, 175) lifted by 38, 38 and 39: the same hue and the same chroma (66 steps between the lowest and highest channel, 65 in the swatch) at a lightness that reaches 5.1:1 on the card. Swatch 3 as it is gives 3.2 there; swatch 2 as it is gives 4.8, with a lightness within 10 per cent of the muted text's and half Steel's chroma (34 steps against 66), so a mark would hardly differ from a hint |
| `red` | Rowan | `#e5776c` | `0xe5, 0x77, 0x6c` | 5 | A red with a coral lean, never pink: hue 5. Lighter than the scheme's swatches, since a status must be seen: 4.5:1 on the ground |
| `yellow` | Larch | `#e0b65c` | `0xe0, 0xb6, 0x5c` | 41 | Amber, the colour of larch needles: 6.9 on the ground, 5.6 on the card |
| `green` | Lichen | `#79c89a` | `0x79, 0xc8, 0x9a` | 145 | A pale lichen green, 63 degrees of hue from the accent |

Alphas: `[0.05, 0.52, 0.09, 0.68, 0.16, 0.24]` (the normal border was 0.44 and the hover border 0.60).

### Light, Snowday

| Token | Name | Hex | Rust triple | Hue | Source and why |
|---|---|---|---|---|---|
| `bg` | Hoarfrost | `#c4d3e1` | `0xc4, 0xd3, 0xe1` | 209 | **Task 62.** Swatch 2 (158, 177, 192) lightened by about 44 per cent white, to CIE lightness 83.8, then given back the chroma that lightening takes away: 29 steps between the lowest and highest channel (a plain mix has 19), HSL saturation 33 per cent (the swatch's 21), hue 209 (the swatch's 206). Powder snow is 1.27:1 over it. *History:* Task 61 `#d9e3ec`, Task 59 `#d3dce3` |
| `surface` | Powder snow | `#e6ebf1` | `0xe6, 0xeb, 0xf1` | 213 | **Task 62.** Swatch 2 with about 74 per cent white, a little bluer than the plain mix: CIE lightness 92.8, HSL saturation 28 per cent. Cards, the light theme's tiles and the preview's frame. It was the near-white Fresh snow (`#f9fafb` in Task 61, `#f5f7f9` in Task 59) |
| `fg` | Charcoal | `#2d3039` | `0x2d, 0x30, 0x39` | 225 | Swatch 5 exactly: Dusk's ground is Snowday's ink. 8.6:1 on the ground, 11.0 on the card. The label's `#1d2d3f` (a navy) would make the ink a second blue beside the accent, 1.8:1 from it with the same saturation (37 per cent) |
| `muted` | Ash | `#4f525b` | `0x4f, 0x52, 0x5b` | 225 | **Task 62.** Swatch 5 twenty-five per cent of the way to swatch 1: (78.5, 81.5, 91.25), rounded half away from zero. Grey like the text, so the accent stays the only blue ink: 5.1:1 on the ground, 6.5 on the card. *History:* `#555862`, thirty per cent |
| `accent` | Deep slate | `#365675` | `0x36, 0x56, 0x75` | 210 | Swatch 4 exactly (the label; the pixels read `#375675`): 5.0:1 on the ground, 6.4 on the card. It is the colour of the chosen frame, the on switch, the scale's fill, the link and the focus ring |
| `red` | Holly | `#a9322a` | `0xa9, 0x32, 0x2a` | 4 | **Task 62.** A deep red, hue 4: 4.3 on the ground, and 3.5 for the error banner's icon on its tint. *History:* `#b33b32` |
| `yellow` | Resin | `#704d00` | `0x70, 0x4d, 0x00` | 41 | **Task 62.** The amber that reaches 5:1 on this ground is this dark: 5.0 on the ground, 6.4 on the card. *History:* `#785200` |
| `green` | Spruce | `#2f7756` | `0x2f, 0x77, 0x56` | 153 | A deep spruce green, 57 degrees from the accent: 3.5 on the ground (4.1 before Task 62), the thinnest margin of a status dot, 17.7 per cent over its 3.0 |

Alphas: `[0.04, 0.62, 0.07, 0.78, 0.14, 0.20]` (the normal border was 0.56 and the hover border 0.74 before Task 59; Tasks 61 and 62 leave them).

**Why the border alphas moved in Task 59.** The scheme's grounds are mid-toned, so the same alpha gives a weaker line. At the old alphas row 12 would have been 3.4 (dark) and 3.0 (light, on the edge), and a control's border on a card in the dark theme 3.1. At the new ones: 4.2 and 3.4 on the ground (the light figure is Task 62's, on its darker ground; it was 3.5 in Task 59 and 3.6 in Task 61), and 3.7 on a card in both themes. Hover still firms a border up (0.68 over 0.52, 0.78 over 0.62).

## For `theme.rs`

```rust
const DARK: Palette = Palette {
    bg: Rgb([0x2d, 0x30, 0x39]),      // Charcoal
    surface: Rgb([0x31, 0x3f, 0x51]), // Slate shade
    fg: Rgb([0xe5, 0xeb, 0xf2]),      // Snowlight
    muted: Rgb([0xb3, 0xb6, 0xc2]),   // Icy grey
    accent: Rgb([0x94, 0xb7, 0xd6]),  // Steel
    red: Rgb([0xe5, 0x77, 0x6c]),     // Rowan
    yellow: Rgb([0xe0, 0xb6, 0x5c]),  // Larch
    green: Rgb([0x79, 0xc8, 0x9a]),   // Lichen
    alphas: [0.05, 0.52, 0.09, 0.68, 0.16, 0.24],
};

const LIGHT: Palette = Palette {
    bg: Rgb([0xc4, 0xd3, 0xe1]),      // Hoarfrost (Task 62; was d9 e3 ec, d3 dc e3)
    surface: Rgb([0xe6, 0xeb, 0xf1]), // Powder snow (Task 62; was Fresh snow f9 fa fb, f5 f7 f9)
    fg: Rgb([0x2d, 0x30, 0x39]),      // Charcoal
    muted: Rgb([0x4f, 0x52, 0x5b]),   // Ash (Task 62; was 55 58 62)
    accent: Rgb([0x36, 0x56, 0x75]),  // Deep slate
    red: Rgb([0xa9, 0x32, 0x2a]),     // Holly (Task 62; was b3 3b 32)
    yellow: Rgb([0x70, 0x4d, 0x00]),  // Resin (Task 62; was 78 52 00)
    green: Rgb([0x2f, 0x77, 0x56]),   // Spruce
    alphas: [0.04, 0.62, 0.07, 0.78, 0.14, 0.20],
};
```

The doc comments of both constants (`theme.rs`, lines 104 to 108 and 121) name Cool Blue Forest, Dusk and Snowday and the tones, and point to this file; `LIGHT`'s becomes "The same forest in daylight, \"Snowday\": Hoarfrost, Powder snow, Charcoal, Ash and Deep slate (Phase 11a Tasks 61 and 62: a card that is a tile, then a toned-down ground and card)". `Tokens::default_theme`'s `selection` is `blend(accent, bg, 0.25)`, rounded half away from zero: dark (70.75, 81.75, 96.25) = `(71, 82, 96)` = `#475260`; light (160.5, 179.75, 198.0) = `(161, 180, 198)` = `#a1b4c6` (it was `#b0c0ce` on Task 61's ground and `#acbbc8` on Task 59's; the red channel is a tie, 160.5 exactly in binary, which `f64::round` takes to 161).

Besides these two constants, Tasks 61 and 62's code is outside this file: the window class `own-light` (`follow.rs`) and the seven rules under it (`app.css`), and `.app-name`'s size (Task 60). Task 62 changes none of them. They are in the design document, "Answers decided on 2026-11-24, Tasks 60 and 61".

## For `tools/mockups/tests/mockups.rs` (`PALETTES`) and `crates/odc-app/tests/window_style.rs`

```text
default dark : background "#2d3039", surface "#313f51", foreground "#e5ebf2", muted "#b3b6c2",
               accent "#94b7d6", red "#e5776c", yellow "#e0b65c", green "#79c89a", selected 0.16, border 0.52
default light: background "#c4d3e1", surface "#e6ebf1", foreground "#2d3039", muted "#4f525b",
               accent "#365675", red "#a9322a", yellow "#704d00", green "#2f7756", selected 0.14, border 0.62
```

`app.css` already has them (the first two blocks, same property names; the light block's `--bg`, `--surface`, `--muted`, `--red` and `--yellow` changed in Task 62). `window_style.rs`: `SNOWDAY_CARD` becomes `Rgb([0xe6, 0xeb, 0xf1])`; `SNOWDAY_ACCENT` stays `Rgb([0x36, 0x56, 0x75])`.

The test's two notes on muted text over the selected fill: the dark one stays **3.4** (hand 3.447, so the exact figure may read 3.5; still under 4.5), and the light one becomes **5.1** (hand 5.076, 0.026 above the rounding edge 5.05; Task 61's was 5.3, Task 59's 5.1). The sample Omarchy theme and its figures (text 11.9, muted 5.4, accent 7.2) are untouched. Regenerate the mockups: `cargo run -p odc-mockups -- docs/design/mockups`.

## The 15 figures, in the test's order

The pairs are the test's: rows 1 to 10 are `--fg`, `--muted`, `--accent` and `--yellow` on `--bg` and `--surface`, then `--red` and `--green` on `--bg`; row 11 `--fg` on `--fg` at `selected` over `--surface`; row 12 `--fg` at the normal border alpha over `--bg`, against `--bg`; row 13 `--fg` against `--fg` at 0.12 over `--bg`; row 14 `--accent` on `--fg` at `selected` over `--bg`; row 15 `--fg` on `blend(accent, bg, 0.25)`, which the mockups' test takes **unrounded** and the app's test takes as the rounded token `t.selection` (160.5 becomes 161: 6.168 against 6.188, both 6.2). The first column of each theme is the one to pin; the second is the hand value to three decimals. The light hand values are Task 62's; the dark ones are Task 59's, unchanged.

| # | Need | Dark, pin | Dark, hand | Light, pin | Light, hand |
|---|---|---|---|---|---|
| 1 | 4.5 | 11.0 | 10.981 | 8.6 | 8.630 |
| 2 | 4.5 | 8.9 | 8.925 | 11.0 | 10.994 |
| 3 | 4.5 | 6.5 | 6.519 | 5.1 | 5.111 |
| 4 | 4.5 | 5.3 | 5.298 | 6.5 | 6.511 |
| 5 | 4.5 | 6.3 | 6.278 | 5.0 | 5.010 |
| 6 | 4.5 | 5.1 | 5.103 | 6.4 | 6.382 |
| 7 | 4.5 | 6.9 | 6.913 | 5.0 | 5.000 |
| 8 | 4.5 | 5.6 | 5.619 | 6.4 | 6.369 |
| 9 | 3.0 | 4.5 | 4.510 | 4.3 | 4.329 |
| 10 | 3.0 | 6.6 | 6.615 | 3.5 | 3.532 |
| 11 | 4.5 | 5.8 | 5.806 | 8.6 | 8.571 |
| 12 | 3.0 | 4.2 | 4.175 | 3.4 | 3.377 |
| 13 | 3.0 | 7.8 | 7.812 | 7.1 | 7.067 |
| 14 | 3.0 | 4.0 | 3.972 | 4.0 | 3.962 |
| 15 | 4.5 | 6.6 | 6.618 | 6.2 | 6.168 |

As the test's arrays: dark `[11.0, 8.9, 6.5, 5.3, 6.3, 5.1, 6.9, 5.6, 4.5, 6.6, 5.8, 4.2, 7.8, 4.0, 6.6]`, light `[8.6, 11.0, 5.1, 6.5, 5.0, 6.4, 5.0, 6.4, 4.3, 3.5, 8.6, 3.4, 7.1, 4.0, 6.2]`.

The light working, for a recomputation: relative luminance (six digits) of `bg` 0.637605, `surface` 0.825906, `fg` 0.029672, `muted` 0.084523, `accent` 0.087242, `yellow` 0.087526, `red` 0.108834, `green` 0.144699; the selected fill over `surface` (row 11) 0.632831, the control border at 0.62 over `bg` (row 12) 0.153627, the unlit dot (row 13) 0.513051, the selected fill over `bg` (row 14) 0.493811, the selection blend (row 15) 0.441444 unrounded and 0.442968 rounded. The channels, for the blends: row 11's fill is (204.1, 208.82, 215.24), row 12's border (102.38, 109.94, 120.84), row 13's unlit dot (177.88, 191.44, 204.84), row 14's fill (174.86, 188.18, 201.48), row 15's selection (160.5, 179.75, 198).

- **Within 0.02 of a rounding edge, so a pin may flip by a tenth** (hand figures good to about 0.001 if no arithmetic slip, but treat 0.01 as the margin): light **row 14** (3.962, edge 3.95: by 0.012), row 13 (7.067, edge 7.05), row 10 (3.532, edge 3.55), row 15 (6.168, edge 6.15), row 8 (6.369, edge 6.35) and row 1 (8.630, edge 8.65). **Within 0.03:** light rows 9 (4.329, edge 4.35), 11 (8.571, edge 8.55) and 12 (3.377, edge 3.35), and the light note on the muted text over the selected fill (5.076, edge 5.05); dark rows 5, 12 and 14 (6.278, 4.175, 3.972) and the dark note (3.447, edge 3.45). **Within 0.04:** light rows 3, 4 and 6 (5.111, 6.511, 6.382); dark rows 1, 3, 8 and 15 (10.981, 6.519, 5.619, 6.618). A flip is only a pin to change, never a fault; a row under its need is.
- **Tightest margins:** light row 7, the warning text on the ground, 5.000 against 4.5 (11.1 per cent); row 5, the accent on the ground, 5.010 (11.3); row 12, the control border, 3.377 against 3.0 (12.6); row 3, the muted text on the ground, 5.111 (13.6); row 10, the green dot, 3.532 against 3.0 (17.7). Dark row 6, the accent on the card, 5.103 (13.4 per cent) and dark row 4, the muted text on the card, 5.298 (17.7) are the dark theme's. Every other row has 32 per cent or more. If light row 5 or 7 misses, the ground rises one step (`#c5d4e2` adds about 0.05 to rows 5 and 7); if row 12 misses, the normal border alpha rises to 0.64.
- **Tints the test also asserts** (`--fg` against the status colour at 0.16 and 0.14 over `--bg`; need 4.5): about 8.8 and 8.2 in the dark theme, 6.9 and 7.1 in the light one (8.1 and 8.4 on Task 61's ground). The status icons on those tints (not asserted): red 3.5 and yellow 4.1 in the light theme, against 3.0.
- **Other pairs, hand values, not pinned:** unlit dots on `--surface` (lit against unlit) 6.5 (dark) and 8.9 (light), and the unlit dot against its frame 1.24:1 in the light theme (the dots are `#d0d5db` on `#e6ebf1`, L\* 84.9 on 92.8); the control border on `--surface` 3.7 and 3.7; the knob (`--fg`) against an `--accent` trough 1.7 and 1.7, which the 1 px `--bg` ring answers (row 5: 6.3 and 5.0); muted text on the selected fill 3.4 (dark, so nothing muted is drawn on a selected tile) and 5.1 (light); the light chosen frame (`--accent`) on a card 6.4 and on the ground 5.0 (rows 6 and 5); the light hover border (`--fg` at 0.78 over a card) 5.8 against the card; a light tile at rest against the ground (Powder snow over Hoarfrost) 1.27, which WCAG 1.4.11 asks nothing of for a tile identified by its picture and caption; an unavailable tile's caption (`--muted` on the well, `--fg` at 0.04 over the ground) 4.8. In the light theme the three inks are close in lightness (Charcoal 0.030, Deep slate 0.087, Ash 0.085 relative luminance): the accent is 1.7:1 from the text and 1.02:1 from the hint, and differs from the hint by chroma (63 steps against 12), not by lightness. **The reasons for Task 62's three small changes, the old colours on the new ground:** Ash `#555862` 4.6, Resin `#785200` 4.6, and Holly's icon on its banner tint 3.1.
- **The Glassy bounds** (see-through window at 0.75, worst backdrop: white under the dark theme, black under the light; not pinned): text on the bare ground 4.9 (dark) and 4.8 (light); text on a card 7.7 and 9.2; muted text on a card 4.6 and 5.4; accent on a card 4.4 and 5.3; muted text on the bare ground 2.9 and 2.9; accent on the bare ground 2.8 and 2.8. The light column is Task 62's (Task 61's was 5.6, 10.5, 5.7, 6.1, 3.0 and 3.3). The dark theme's bounds are weaker than Meltwater's (6.6, 10.0, 5.1 and 3.3 to 4.3) because its ground is lighter: the status dots are about 2.0 over white (red) in the dark theme, and over black about 2.4 (red) and 2.0 (green) in the light one.

## What to check, and what to send back

1. Recompute the 15 rows of both themes (the dark ones should not have moved). Tell me any row under its need, or off by more than a tenth, with its pair and ratio; I change the value, not the need.
2. Look at the regenerated mockups, both themes, at least the display page, the Configuration page, a banner and the glassy page: the accent as a chosen tile's frame, an on switch's trough, a scale's fill and a link; the warning and error banners; the status dots. Tell me what reads as pink, purple, warm, flat or too grey.
3. In a real window (not a mockup): in the light theme, whether it is calm now and not dull; whether the Powder snow tiles read as cards on the Hoarfrost ground (1.27:1; if not, the card goes to `#eaeff4`, 1.32:1, and rows 2, 4, 6 and 8 rise); whether the 2 px chosen frame is the right weight; whether a link, a hint and the body text can be told apart (three inks within 1.7:1 of each other); the knob of an on switch in the light theme (dark knob, dark trough, pale ring); the error banner's icon in the deeper Holly; and, in the dark theme, whether the slate card reads as a card on the charcoal ground at 1.23:1 (if not, `#324357`, half way, gives 1.30:1 at the price of rows 4 and 6 falling to 5.0 and 4.8).
