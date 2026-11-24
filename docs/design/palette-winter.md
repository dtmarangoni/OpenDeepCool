# Cool Blue Forest: OpenDeepCool's own theme, dark and light (Phase 11a Task 59)

The final values of the own theme's two palettes, for `crates/odc-app/src/theme.rs` (`DARK`, `LIGHT`) and `tools/mockups/tests/mockups.rs` (`PALETTES`). The design is [settings-app.md](settings-app.md), "OpenDeepCool's own theme" and D51; `mockups-src/app.css` already holds these values. **The eight colours of each theme change, and two of the six control alphas of each** (the normal border and the hover border; the reason is below). The fills, the selected and pressed fills, the selection alpha 0.25, the radii, the tokens and every rule stay as they are.

**Every figure here is a hand computation** (WCAG 2 relative luminance, the sRGB formula worked to six digits; the designer has no shell). The implementer recomputes them with `the_default_theme_has_the_contrast_the_design_states` (both copies) and pins them. A row under its need, or a figure that differs from this file by more than a tenth, goes back to the designer with its pair.

## The scheme, and what each theme takes from it

The owner's colour scheme (2026-11-23): Folk & Fey's "Cool Blue Forest", from its Winter Forest Collection (an Instagram post whose caption reads "soft icy grey, frosted blue, muted steel blue, deep slate blue, and dark charcoal tones of a snow-covered winter forest"). The designer did not see the image: the printed labels and the pixel readings were given in the brief. Where label and pixel differ by one unit the label is used; the one real mismatch is swatch 5, whose label is `#1d2d3f` and whose pixels are `#2d3039`: **the pixels' colour is used** (owner's brief), and the label is not used anywhere.

| # | The caption's name | Label | Pixels | Hue, HSL saturation | Where it is used |
|---|---|---|---|---|---|
| 1 | soft icy grey | `#b3b6c2` | `#b3b7c2` | 228, 11 % | the dark theme's muted text exactly (Icy grey); the far end of the light theme's muted text |
| 2 | frosted blue | `#9eb1c0` | `#9fb0c0` | 206, 21 % | the light theme's ground and card, tinted with white |
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
| `bg` | Hoarfrost | `#d3dce3` | `0xd3, 0xdc, 0xe3` | 206 | Swatch 2 (158, 177, 192) with 55 per cent white: (211.4, 219.9, 226.7), rounded. The swatch's own hue. CIE lightness about 87; a paler ground gives the card under 1.2:1 |
| `surface` | Fresh snow | `#f5f7f9` | `0xf5, 0xf7, 0xf9` | 210 | Swatch 2 with 90 per cent white: (245.3, 247.2, 248.7), rounded. 1.29:1 over Hoarfrost |
| `fg` | Charcoal | `#2d3039` | `0x2d, 0x30, 0x39` | 225 | Swatch 5 exactly: Dusk's ground is Snowday's ink. 9.5:1 on the ground, 12.3 on the card. The label's `#1d2d3f` (a navy, 10.1 and 13.0) would make the ink a second blue beside the accent, 1.8:1 from it with the same saturation (37 per cent) |
| `muted` | Ash | `#555862` | `0x55, 0x58, 0x62` | 226 | Swatch 5 thirty per cent of the way to swatch 1: (85.2, 88.2, 98.1), rounded. Grey like the text, so the accent stays the only blue ink: 5.1:1 on the ground, 6.6 on the card |
| `accent` | Deep slate | `#365675` | `0x36, 0x56, 0x75` | 210 | Swatch 4 exactly (the label; the pixels read `#375675`): 5.5:1 on the ground, 7.1 on the card |
| `red` | Holly | `#b33b32` | `0xb3, 0x3b, 0x32` | 4 | A deep red: 4.2 on the ground |
| `yellow` | Resin | `#785200` | `0x78, 0x52, 0x00` | 41 | The amber that reaches 4.5:1 on a pale ground is this dark: 5.0 on the ground, 6.5 on the card |
| `green` | Spruce | `#2f7756` | `0x2f, 0x77, 0x56` | 153 | A deep spruce green, 57 degrees from the accent: 3.9 on the ground |

Alphas: `[0.04, 0.62, 0.07, 0.78, 0.14, 0.20]` (the normal border was 0.56 and the hover border 0.74).

**Why the border alphas move.** The scheme's grounds are mid-toned, so the same alpha gives a weaker line. At the old alphas row 12 would be 3.4 (dark) and 3.0 (light, on the edge), and a control's border on a card in the dark theme 3.1. At the new ones: 4.2 and 3.5 on the ground, and 3.7 and 3.9 on a card. Hover still firms a border up (0.68 over 0.52, 0.78 over 0.62).

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
    bg: Rgb([0xd3, 0xdc, 0xe3]),      // Hoarfrost
    surface: Rgb([0xf5, 0xf7, 0xf9]), // Fresh snow
    fg: Rgb([0x2d, 0x30, 0x39]),      // Charcoal
    muted: Rgb([0x55, 0x58, 0x62]),   // Ash
    accent: Rgb([0x36, 0x56, 0x75]),  // Deep slate
    red: Rgb([0xb3, 0x3b, 0x32]),     // Holly
    yellow: Rgb([0x78, 0x52, 0x00]),  // Resin
    green: Rgb([0x2f, 0x77, 0x56]),   // Spruce
    alphas: [0.04, 0.62, 0.07, 0.78, 0.14, 0.20],
};
```

The doc comments of both constants (`theme.rs`, lines 104 to 107 and 120 to 121) name Cool Blue Forest, Dusk and Snowday and the tones, and point to this file instead of `palette-ocean.md`. `Tokens::default_theme`'s `selection` is `blend(accent, bg, 0.25)`, rounded half away from zero: dark (70.75, 81.75, 96.25) = `(71, 82, 96)` = `#475260`; light (171.75, 186.5, 199.5) = `(172, 187, 200)` = `#acbbc8` (two channels sit exactly on a half, and `f64::round` takes them up).

## For `tools/mockups/tests/mockups.rs` (`PALETTES`)

```text
default dark : background "#2d3039", surface "#313f51", foreground "#e5ebf2", muted "#b3b6c2",
               accent "#94b7d6", red "#e5776c", yellow "#e0b65c", green "#79c89a", selected 0.16, border 0.52
default light: background "#d3dce3", surface "#f5f7f9", foreground "#2d3039", muted "#555862",
               accent "#365675", red "#b33b32", yellow "#785200", green "#2f7756", selected 0.14, border 0.62
```

`app.css` already has them (the first two blocks, same property names, `--a-normal-border` and `--a-hover-border` changed with them). The test's two notes on muted text over the selected fill change too: dark 3.6 becomes **3.4** (hand 3.447, so the exact figure may read 3.5; still under 4.5) and light 5.3 becomes **5.1** (hand 5.122). The sample Omarchy theme and its figures (text 11.9, muted 5.4, accent 7.2) are untouched. Regenerate the mockups: `cargo run -p odc-mockups -- docs/design/mockups`.

## The 15 figures, in the test's order

The pairs are the test's: rows 1 to 10 are `--fg`, `--muted`, `--accent` and `--yellow` on `--bg` and `--surface`, then `--red` and `--green` on `--bg`; row 11 `--fg` on `--fg` at `selected` over `--surface`; row 12 `--fg` at the normal border alpha over `--bg`, against `--bg`; row 13 `--fg` against `--fg` at 0.12 over `--bg`; row 14 `--accent` on `--fg` at `selected` over `--bg`; row 15 `--fg` on the rounded `blend(accent, bg, 0.25)`. The first column is the one to pin; the second is the hand value to three decimals.

| # | Need | Dark, pin | Dark, hand | Light, pin | Light, hand |
|---|---|---|---|---|---|
| 1 | 4.5 | 11.0 | 10.981 | 9.5 | 9.487 |
| 2 | 4.5 | 8.9 | 8.925 | 12.3 | 12.272 |
| 3 | 4.5 | 6.5 | 6.519 | 5.1 | 5.109 |
| 4 | 4.5 | 5.3 | 5.298 | 6.6 | 6.610 |
| 5 | 4.5 | 6.3 | 6.278 | 5.5 | 5.507 |
| 6 | 4.5 | 5.1 | 5.103 | 7.1 | 7.124 |
| 7 | 4.5 | 6.9 | 6.913 | 5.0 | 5.030 |
| 8 | 4.5 | 5.6 | 5.619 | 6.5 | 6.506 |
| 9 | 3.0 | 4.5 | 4.510 | 4.2 | 4.213 |
| 10 | 3.0 | 6.6 | 6.615 | 3.9 | 3.882 |
| 11 | 4.5 | 5.8 | 5.806 | 9.5 | 9.510 |
| 12 | 3.0 | 4.2 | 4.175 | 3.5 | 3.519 |
| 13 | 3.0 | 7.8 | 7.812 | 7.7 | 7.730 |
| 14 | 3.0 | 4.0 | 3.972 | 4.3 | 4.330 |
| 15 | 4.5 | 6.6 | 6.618 | 6.7 | 6.712 |

As the test's arrays: dark `[11.0, 8.9, 6.5, 5.3, 6.3, 5.1, 6.9, 5.6, 4.5, 6.6, 5.8, 4.2, 7.8, 4.0, 6.6]`, light `[9.5, 12.3, 5.1, 6.6, 5.5, 7.1, 5.0, 6.5, 4.2, 3.9, 9.5, 3.5, 7.7, 4.3, 6.7]`.

- **Within 0.02 of a rounding edge, so a pin may flip by a tenth** (hand figures good to about 0.01): light row 13 (7.730, edge 7.75) and row 14 (4.330, edge 4.35), and the mockups test's dark note, muted text on the selected fill (3.447, edge 3.45). **Within 0.03:** dark rows 5, 12 and 14 (6.278, 4.175, 3.972) and light row 6 (7.124). **Within 0.04:** dark rows 1, 3, 8 and 15 (10.981, 6.519, 5.619, 6.618). A flip is only a pin to change, never a fault; a row under its need is.
- **Tightest margins:** light row 7, the warning text on the ground, 5.0 against 4.5 (11.8 per cent); dark row 6, the accent on the card, 5.1 (13.4 per cent); light row 3, the muted text on the ground, 5.1 (13.5 per cent); light row 12, the control border, 3.5 against 3.0 (17 per cent); dark row 4, the muted text on the card, 5.3 (17.7 per cent). Every other row has 22 per cent or more. If light row 7 misses, darken Resin by one step in the red channel (`#765200` gives about 5.1).
- **Tints the test also asserts** (`--fg` against the status colour at 0.16 and 0.14 over `--bg`, rounded; need 4.5): about 8.8 and 8.2 in the dark theme, 7.6 and 7.9 in the light one.
- **Other pairs, hand values, not pinned:** unlit dots on `--surface` (lit against unlit) 6.5 (dark) and 9.9 (light); the control border on `--surface` 3.7 and 3.9; the status icons on their tints, red 3.6 and 3.4, yellow 5.2 and 4.2 (against 3.0); the knob (`--fg`) against an `--accent` trough 1.7 and 1.7, which the 1 px `--bg` ring answers (row 5: 6.3 and 5.5); muted text on the selected fill 3.4 (dark, so nothing muted is drawn on a selected tile) and 5.1 (light). In the light theme the three inks are close in lightness (Charcoal 0.030, Deep slate 0.087, Ash 0.098 relative luminance): the accent differs from the hint by chroma (63 steps against 13), not by lightness.
- **The Glassy bounds** (see-through window at 0.75, worst backdrop: white under the dark theme, black under the light; not pinned): text on the bare ground 4.9 (dark) and 5.3 (light); text on a card 7.7 and 10.2; muted text on a card 4.6 and 5.5; accent on a card 4.4 and 5.9; muted text on the bare ground 2.9 and 2.8; accent on the bare ground 2.8 and 3.1. The dark theme's bounds are weaker than Meltwater's (6.6, 10.0, 5.1 and 3.3 to 4.3) because its ground is lighter: the status dots are about 2.0 over white (red) in the dark theme.

## What to check, and what to send back

1. Recompute the 15 rows of both themes. Tell me any row under its need, or off by more than a tenth, with its pair and ratio; I change the value, not the need.
2. Look at the regenerated mockups, both themes, at least the display page, the Configuration page, a banner and the glassy page: the accent as a selected tile's border, an on switch's trough, a scale's fill and a link; the warning and error banners; the status dots. Tell me what reads as pink, purple, warm, flat or too grey.
3. In a real window (not a mockup): whether the slate card reads as a card on the charcoal ground at 1.23:1 (if not, `#324357`, half way, gives 1.30 at the price of rows 4 and 6 falling to 5.0 and 4.8); in the light theme, whether a link, a hint and the body text can be told apart (three inks within 1.9:1 of each other); the knob of an on switch in the light theme (dark knob, dark trough, pale ring); and the dark theme next to a Nord-themed Omarchy window, which it resembles (Nord's `#2e3440` ground is within 7 units of Charcoal in every channel).
