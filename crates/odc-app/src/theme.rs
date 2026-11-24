//! The theme (`docs/design/settings-app.md`, Theme): the current Omarchy theme's `colors.toml`
//! and `shell.toml` `[controls]`, or OpenDeepCool's default theme, as the design's tokens, and
//! the CSS the app loads: a `:root` block of custom properties, then the app's stylesheet,
//! which uses only `var(--…)`.
//!
//! Every value is parsed before it reaches the CSS (owner, 2026-10-27; solution review P10-05):
//! a colour only as `#RRGGBB`, `#RRGGBBAA`, Hyprland's `rgb(RRGGBB)` or `rgba(RRGGBBAA)`, or a
//! Hyprland gradient of those (its first colour); an alpha only as a number from 0 to 1. The CSS
//! holds only the values written back from what was parsed. A missing or rejected key takes the
//! default theme's value, and a problem names it.

use std::fmt::Write as _;

use odc_ipc::{WindowCorners, WindowOpacity};

use crate::window_style;

/// The app's stylesheet, after the `:root` block.
const STYLESHEET: &str = include_str!("app.css");

/// The widest border a theme may give a control, in px; wider is rejected (decisions log D30).
const MAX_BORDER_WIDTH: f64 = 16.0;

/// A colour without alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub [u8; 3]);

/// A colour with an alpha from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub rgb: Rgb,
    pub alpha: f64,
}

/// A border's four widths in px, top, right, bottom, left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Widths(pub [f64; 4]);

/// The controls' fills and borders, from `shell.toml` `[controls]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub normal_fill: Rgba,
    pub normal_border: Rgba,
    pub normal_border_width: Widths,
    pub hover_fill: Rgba,
    pub hover_border: Rgba,
    pub focus_fill: Rgba,
    pub focus_border: Rgba,
    pub selected_fill: Rgba,
    /// Read and kept like every other control colour, but no rule uses it: the stylesheet draws
    /// a selected tile and toggle with `--accent` (design, Theme, Controls).
    pub selected_border: Rgba,
    pub pressed_fill: Rgba,
}

/// The design's tokens (Theme, Tokens and where they come from).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    /// Whether GTK's own parts take the dark scheme (`colors.toml` `mode`).
    pub dark: bool,
    pub bg: Rgb,
    pub surface: Rgb,
    pub fg: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub red: Rgb,
    pub yellow: Rgb,
    pub green: Rgb,
    /// Selected text's background.
    pub selection: Rgb,
    pub controls: Controls,
    /// Corner radius in px of a control: 0 with an Omarchy theme, 4 in OpenDeepCool's own.
    pub radius: u8,
    /// The same of what holds controls (cards, the preview's frame, popovers): 0, and 10.
    pub radius_box: u8,
    /// How see-through the window is (Phase 11a Tasks 45 and 51): with an Omarchy theme, Omarchy's
    /// own Window Opacity, read from its toggle files; with OpenDeepCool's own, the service's.
    pub opacity: WindowOpacity,
    /// OpenDeepCool's own theme's corners, sharp or rounded, which also give the window, the
    /// switches and the knobs their shapes; `None` with an Omarchy theme, whose corners the
    /// compositor and the theme's own zero radii keep (Window opacity and corners).
    pub corners: Option<WindowCorners>,
}

/// A theme and what was wrong with its files, one line each, for the journal.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub tokens: Tokens,
    pub problems: Vec<String>,
}

/// One default palette: the design's Default theme table.
struct Palette {
    bg: Rgb,
    surface: Rgb,
    fg: Rgb,
    muted: Rgb,
    accent: Rgb,
    red: Rgb,
    yellow: Rgb,
    green: Rgb,
    /// Normal fill, normal border, hover fill, hover border, selected fill, pressed fill.
    alphas: [f64; 6],
}

/// OpenDeepCool's own theme, dark, "Dusk": a winter forest at dusk, from the owner's colour scheme
/// (Folk & Fey's Cool Blue Forest, Winter Forest Collection): Charcoal, Slate shade, Snowlight,
/// Icy grey and Steel (`docs/design/settings-app.md`, OpenDeepCool's own theme, and
/// `docs/design/palette-winter.md`; Phase 11a Task 59, after Task 54's Meltwater and Task 44's
/// Coolant, both rejected).
const DARK: Palette = Palette {
    bg: Rgb([0x2d, 0x30, 0x39]),
    surface: Rgb([0x31, 0x3f, 0x51]),
    fg: Rgb([0xe5, 0xeb, 0xf2]),
    muted: Rgb([0xb3, 0xb6, 0xc2]),
    accent: Rgb([0x94, 0xb7, 0xd6]),
    red: Rgb([0xe5, 0x77, 0x6c]),
    yellow: Rgb([0xe0, 0xb6, 0x5c]),
    green: Rgb([0x79, 0xc8, 0x9a]),
    alphas: [0.05, 0.52, 0.09, 0.68, 0.16, 0.24],
};

/// The same forest in daylight, "Snowday": Hoarfrost, Powder snow, Charcoal, Ash and Deep slate
/// (`docs/design/palette-winter.md`; Phase 11a Task 62 toned the ground and the card down, and
/// Ash, Holly and Resin with them, after "the light theme is too bright now").
const LIGHT: Palette = Palette {
    bg: Rgb([0xc4, 0xd3, 0xe1]),
    surface: Rgb([0xe6, 0xeb, 0xf1]),
    fg: Rgb([0x2d, 0x30, 0x39]),
    muted: Rgb([0x4f, 0x52, 0x5b]),
    accent: Rgb([0x36, 0x56, 0x75]),
    red: Rgb([0xa9, 0x32, 0x2a]),
    yellow: Rgb([0x70, 0x4d, 0x00]),
    green: Rgb([0x2f, 0x77, 0x56]),
    alphas: [0.04, 0.62, 0.07, 0.78, 0.14, 0.20],
};

/// The selection's accent over the background, as Omarchy's own `selection-fill-alpha` default
/// (decisions log D31), lowered from 0.35 to 0.25 with OpenDeepCool's own light-toned accent
/// (Phase 11a Task 11), so text on it keeps its contrast.
const SELECTION_ALPHA: f64 = 0.25;

impl Palette {
    fn of(dark: bool) -> &'static Palette {
        if dark { &DARK } else { &LIGHT }
    }
}

impl Tokens {
    /// OpenDeepCool's default theme, dark or light.
    pub fn default_theme(dark: bool) -> Tokens {
        let p = Palette::of(dark);
        Tokens {
            dark,
            bg: p.bg,
            surface: p.surface,
            fg: p.fg,
            muted: p.muted,
            accent: p.accent,
            red: p.red,
            yellow: p.yellow,
            green: p.green,
            selection: blend(p.accent, p.bg, SELECTION_ALPHA),
            controls: default_controls(p.fg, p),
            radius: 4,
            radius_box: 10,
            opacity: WindowOpacity::Opaque,
            corners: Some(WindowCorners::Rounded),
        }
    }

    /// The own theme with the service's window opacity and corners (Phase 11a Task 51): sharp
    /// gives every box, control and the window radius 0, rounded the design's 4, 10 and 12 px.
    pub fn with_window(self, opacity: WindowOpacity, corners: WindowCorners) -> Tokens {
        let (radius, radius_box) = match corners {
            WindowCorners::Rounded => (4, 10),
            WindowCorners::Sharp => (0, 0),
        };
        Tokens {
            opacity,
            corners: Some(corners),
            radius,
            radius_box,
            ..self
        }
    }

    /// The window's own radius in px: 12 for the own theme rounded, else 0 (an Omarchy theme
    /// writes no rule for it).
    pub fn radius_win(&self) -> u8 {
        match self.corners {
            Some(WindowCorners::Rounded) => 12,
            _ => 0,
        }
    }

    /// The switch's trough and the scale's bar: round in every theme, square only while the
    /// own theme's corners are sharp.
    pub fn radius_pill(&self) -> u8 {
        if self.corners == Some(WindowCorners::Sharp) {
            0
        } else {
            9
        }
    }
}

/// The default theme's controls drawn in `colour`, as Omarchy's template writes them: one
/// colour for every state, borders 1 px, focus as hover, and the selected border opaque.
fn default_controls(colour: Rgb, p: &Palette) -> Controls {
    let [
        normal_fill,
        normal_border,
        hover_fill,
        hover_border,
        selected_fill,
        pressed_fill,
    ] = p.alphas;
    let at = |alpha| Rgba { rgb: colour, alpha };
    Controls {
        normal_fill: at(normal_fill),
        normal_border: at(normal_border),
        normal_border_width: Widths([1.0; 4]),
        hover_fill: at(hover_fill),
        hover_border: at(hover_border),
        focus_fill: at(hover_fill),
        focus_border: at(hover_border),
        selected_fill: at(selected_fill),
        selected_border: at(1.0),
        pressed_fill: at(pressed_fill),
    }
}

/// The theme from the Omarchy theme's `colors.toml` and `shell.toml`, each `None` when it's
/// missing or unreadable. Without `colors.toml`, the whole default theme, dark.
pub fn theme(colors: Option<&str>, shell: Option<&str>) -> Theme {
    let mut problems = Vec::new();
    let Some(colors) = colors else {
        return Theme {
            tokens: Tokens::default_theme(true),
            problems,
        };
    };
    let colors: toml::Table = match colors.parse() {
        Ok(table) => table,
        Err(error) => {
            problems.push(format!(
                "colors.toml: not TOML ({}); the default theme is used",
                one_line(&error.to_string())
            ));
            return Theme {
                tokens: Tokens::default_theme(true),
                problems,
            };
        }
    };
    let dark = match colors.get("mode") {
        Some(toml::Value::String(mode)) if mode == "dark" => true,
        Some(toml::Value::String(mode)) if mode == "light" => false,
        other => {
            problems.push(missing_or_bad("colors.toml", "mode", other, "dark"));
            true
        }
    };
    let p = Palette::of(dark);
    let mut colour = |key: &str, default: Rgb| match colors.get(key) {
        Some(value) => match value.as_str().and_then(parse_colour) {
            // The window and its text stay opaque: a base colour's own alpha is dropped.
            Some(parsed) => parsed.rgb,
            None => {
                problems.push(rejected("colors.toml", key, value));
                default
            }
        },
        None => {
            problems.push(missing("colors.toml", key));
            default
        }
    };
    let bg = colour("background", p.bg);
    let surface = colour("lighter_background", p.surface);
    let fg = colour("foreground", p.fg);
    let muted = colour("muted", p.muted);
    let accent = colour("accent", p.accent);
    let red = colour("red", p.red);
    let yellow = colour("yellow", p.yellow);
    let green = colour("green", p.green);
    // A missing key takes this theme's own accent over its own background, not the default's.
    let selection = colour("selection", blend(accent, bg, SELECTION_ALPHA));
    let controls = controls(shell, fg, p, &mut problems);
    Theme {
        tokens: Tokens {
            dark,
            bg,
            surface,
            fg,
            muted,
            accent,
            red,
            yellow,
            green,
            selection,
            controls,
            radius: 0,
            radius_box: 0,
            opacity: WindowOpacity::Opaque,
            corners: None,
        },
        problems,
    }
}

/// `shell.toml` `[controls]`. A missing or rejected colour key takes the theme's own foreground,
/// as Omarchy's template writes it, and an alpha or width the default theme's (decisions log
/// D32).
fn controls(shell: Option<&str>, fg: Rgb, p: &Palette, problems: &mut Vec<String>) -> Controls {
    let defaults = default_controls(fg, p);
    let table = match shell.map(str::parse::<toml::Table>) {
        None => {
            problems.push("shell.toml: missing; the default controls are used".to_owned());
            return defaults;
        }
        Some(Err(error)) => {
            problems.push(format!(
                "shell.toml: not TOML ({}); the default controls are used",
                one_line(&error.to_string())
            ));
            return defaults;
        }
        Some(Ok(table)) => table,
    };
    let empty = toml::Table::new();
    let section = match table.get("controls") {
        Some(toml::Value::Table(section)) => section,
        _ => {
            problems.push("shell.toml: no [controls]; the default controls are used".to_owned());
            &empty
        }
    };
    let mut read = Reader {
        section,
        problems,
        quiet: section.is_empty(),
    };
    let normal = read.colour("normal-color", fg);
    let normal_border = read.colour("normal-border", fg);
    let hover = read.colour("hover-cursor-color", fg);
    let hover_border = read.colour("hover-cursor-border", fg);
    let focus = read.colour("focus-color", fg);
    let focus_border = read.colour("focus-border", fg);
    let selected = read.colour("selected-color", fg);
    let selected_border = read.colour("selected-border", fg);
    let d = defaults;
    Controls {
        normal_fill: normal.times(read.alpha("normal-fill-alpha", d.normal_fill.alpha)),
        normal_border: normal_border
            .times(read.alpha("normal-border-alpha", d.normal_border.alpha)),
        normal_border_width: read.widths("normal-border-width", d.normal_border_width),
        hover_fill: hover.times(read.alpha("hover-cursor-fill-alpha", d.hover_fill.alpha)),
        hover_border: hover_border
            .times(read.alpha("hover-cursor-border-alpha", d.hover_border.alpha)),
        focus_fill: focus.times(read.alpha("focus-fill-alpha", d.focus_fill.alpha)),
        focus_border: focus_border.times(read.alpha("focus-border-alpha", d.focus_border.alpha)),
        selected_fill: selected.times(read.alpha("selected-fill-alpha", d.selected_fill.alpha)),
        selected_border: selected_border
            .times(read.alpha("selected-border-alpha", d.selected_border.alpha)),
        pressed_fill: normal.times(read.alpha("pressed-fill-alpha", d.pressed_fill.alpha)),
    }
}

/// Reads `[controls]` keys, noting each one missing or rejected.
struct Reader<'a> {
    section: &'a toml::Table,
    problems: &'a mut Vec<String>,
    /// No `[controls]` at all: one problem said so already.
    quiet: bool,
}

impl Reader<'_> {
    fn note_missing(&mut self, key: &str) {
        if !self.quiet {
            self.problems.push(missing("shell.toml", key));
        }
    }

    /// A colour key, with its own alpha (from `#RRGGBBAA` or `rgba(…)`).
    fn colour(&mut self, key: &str, default: Rgb) -> Rgba {
        let fallback = Rgba {
            rgb: default,
            alpha: 1.0,
        };
        match self.section.get(key) {
            None => {
                self.note_missing(key);
                fallback
            }
            Some(value) => value.as_str().and_then(parse_colour).unwrap_or_else(|| {
                self.problems.push(rejected("shell.toml", key, value));
                fallback
            }),
        }
    }

    fn alpha(&mut self, key: &str, default: f64) -> f64 {
        match self.section.get(key) {
            None => {
                self.note_missing(key);
                default
            }
            Some(value) => number(value)
                .filter(|a| (0.0..=1.0).contains(a))
                .unwrap_or_else(|| {
                    self.problems.push(rejected("shell.toml", key, value));
                    default
                }),
        }
    }

    /// A border width: one to four widths, CSS's four sides (`N`, `"Y X"`, `"T X B"`,
    /// `"T R B L"`), then the per-side keys `<key>-top` and so on over them, as `shell.toml`'s
    /// own comment describes.
    fn widths(&mut self, key: &str, default: Widths) -> Widths {
        let mut widths = match self.section.get(key) {
            None => {
                self.note_missing(key);
                default
            }
            Some(value) => four_sides(value).unwrap_or_else(|| {
                self.problems.push(rejected("shell.toml", key, value));
                default
            }),
        };
        for (side, name) in ["top", "right", "bottom", "left"].into_iter().enumerate() {
            let side_key = format!("{key}-{name}");
            if let Some(value) = self.section.get(&side_key) {
                match number(value).filter(|w| width_ok(*w)) {
                    Some(width) => widths.0[side] = width,
                    None => self.problems.push(rejected("shell.toml", &side_key, value)),
                }
            }
        }
        widths
    }
}

impl Rgba {
    /// The same colour, its alpha times `alpha`.
    fn times(self, alpha: f64) -> Rgba {
        Rgba {
            rgb: self.rgb,
            alpha: self.alpha * alpha,
        }
    }
}

/// A TOML number, integer or float, when finite.
fn number(value: &toml::Value) -> Option<f64> {
    let n = match value {
        toml::Value::Integer(n) => *n as f64,
        toml::Value::Float(n) => *n,
        _ => return None,
    };
    n.is_finite().then_some(n)
}

fn width_ok(width: f64) -> bool {
    (0.0..=MAX_BORDER_WIDTH).contains(&width)
}

/// A number, or a string of one to four numbers separated by spaces, as four sides.
fn four_sides(value: &toml::Value) -> Option<Widths> {
    let list: Vec<f64> = match value {
        toml::Value::String(text) => text
            .split_whitespace()
            .map(|part| {
                let n: f64 = part.parse().ok()?;
                n.is_finite().then_some(n)
            })
            .collect::<Option<_>>()?,
        other => vec![number(other)?],
    };
    if !list.iter().all(|w| width_ok(*w)) {
        return None;
    }
    let sides = match *list.as_slice() {
        [all] => [all; 4],
        [vertical, horizontal] => [vertical, horizontal, vertical, horizontal],
        [top, horizontal, bottom] => [top, horizontal, bottom, horizontal],
        [top, right, bottom, left] => [top, right, bottom, left],
        _ => return None,
    };
    Some(Widths(sides))
}

/// A colour in one of the accepted shapes (P10-05), or `None`.
fn parse_colour(text: &str) -> Option<Rgba> {
    let mut parts: Vec<&str> = text.split_whitespace().collect();
    // A gradient's trailing angle, such as `45deg`.
    if parts.len() > 1
        && let Some(angle) = parts.last().and_then(|p| p.strip_suffix("deg"))
        && !angle.is_empty()
        && angle
            .strip_prefix('-')
            .unwrap_or(angle)
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        parts.pop();
    }
    let colours: Vec<Rgba> = parts.iter().map(|p| one_colour(p)).collect::<Option<_>>()?;
    colours.first().copied()
}

fn one_colour(text: &str) -> Option<Rgba> {
    // The hex digits, and how many each shape takes.
    let (hex, lengths): (&str, &[usize]) = if let Some(hex) = text.strip_prefix('#') {
        (hex, &[6, 8])
    } else if let Some(hex) = text.strip_prefix("rgba(").and_then(|t| t.strip_suffix(')')) {
        (hex, &[8])
    } else {
        (text.strip_prefix("rgb(")?.strip_suffix(')')?, &[6])
    };
    if !lengths.contains(&hex.len()) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok();
    let rgb = Rgb([byte(0)?, byte(1)?, byte(2)?]);
    let alpha = if hex.len() == 8 {
        f64::from(byte(3)?) / 255.0
    } else {
        1.0
    };
    Some(Rgba { rgb, alpha })
}

fn missing(file: &str, key: &str) -> String {
    format!("{file}: `{key}` is missing; the default theme's value is used")
}

fn rejected(file: &str, key: &str, value: &toml::Value) -> String {
    let shown: String = one_line(&value.to_string()).chars().take(40).collect();
    format!("{file}: `{key}` = {shown} isn't accepted; the default theme's value is used")
}

fn missing_or_bad(file: &str, key: &str, value: Option<&toml::Value>, used: &str) -> String {
    match value {
        None => format!("{file}: `{key}` is missing; {used} is used"),
        Some(value) => {
            let shown: String = one_line(&value.to_string()).chars().take(40).collect();
            format!("{file}: `{key}` = {shown} isn't accepted; {used} is used")
        }
    }
}

fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// `fg` at `alpha` over `bg`, opaque.
pub fn blend(fg: Rgb, bg: Rgb, alpha: f64) -> Rgb {
    Rgb(std::array::from_fn(|i| {
        let mixed = alpha * f64::from(fg.0[i]) + (1.0 - alpha) * f64::from(bg.0[i]);
        mixed.round().clamp(0.0, 255.0) as u8
    }))
}

impl Rgb {
    /// `#rrggbb`.
    pub fn css(self) -> String {
        let [r, g, b] = self.0;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// Each channel from 0 to 1, for cairo.
    pub fn unit(self) -> [f64; 3] {
        self.0.map(|c| f64::from(c) / 255.0)
    }
}

impl Rgba {
    /// `rgba(r, g, b, a)`, the alpha to three decimals.
    pub fn css(self) -> String {
        let [r, g, b] = self.rgb.0;
        format!("rgba({r}, {g}, {b}, {})", decimal(self.alpha))
    }
}

/// `n` to three decimals, without trailing zeros.
fn decimal(n: f64) -> String {
    let text = format!("{:.3}", n);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// The unlit preview dot's alpha over the background (Preview column).
pub const UNLIT_ALPHA: f64 = 0.12;

/// The share of the foreground in the disabled colour, over the card: 0.40 in a dark theme and
/// 0.55 in a light one, which puts it at about the middle of the theme's own lightness range, so
/// it is weaker than any `muted` and still text (`docs/design/settings-app.md`, Answers decided
/// on 2026-11-24, Task 63, D55).
const DISABLED_ALPHA_DARK: f64 = 0.40;
/// See [`DISABLED_ALPHA_DARK`].
const DISABLED_ALPHA_LIGHT: f64 = 0.55;

impl Tokens {
    /// The `:root` block.
    pub fn root(&self) -> String {
        let c = &self.controls;
        let fg_at = |alpha| {
            Rgba {
                rgb: self.fg,
                alpha,
            }
            .css()
        };
        let widths = c.normal_border_width.0.map(|w| format!("{}px", decimal(w)));
        let widths = if widths.iter().all(|w| *w == widths[0]) {
            widths[0].clone()
        } else {
            widths.join(" ")
        };
        // The window's ground and the cards' plate, and the banners' tints, with the window's
        // alpha (Tasks 45 and 51): solid unless the window is see-through. What must stay solid
        // (the preview's frame, the open list, the notice) uses `surface`.
        let alpha = window_style::alpha(self.opacity);
        let at = |rgb, alpha| Rgba { rgb, alpha }.css();
        let sharp = self.corners == Some(WindowCorners::Sharp);
        let tokens: [(&str, String); 31] = [
            ("bg", self.bg.css()),
            ("surface", self.surface.css()),
            ("ground", at(self.bg, alpha)),
            ("plate", at(self.surface, alpha)),
            ("fg", self.fg.css()),
            ("muted", self.muted.css()),
            ("disabled", self.disabled().css()),
            ("accent", self.accent.css()),
            ("red", self.red.css()),
            ("yellow", self.yellow.css()),
            ("green", self.green.css()),
            ("selection", self.selection.css()),
            ("ctl-normal-fill", c.normal_fill.css()),
            ("ctl-normal-border", c.normal_border.css()),
            ("ctl-normal-border-width", widths),
            ("ctl-hover-fill", c.hover_fill.css()),
            ("ctl-hover-border", c.hover_border.css()),
            ("ctl-focus-fill", c.focus_fill.css()),
            ("ctl-focus-border", c.focus_border.css()),
            ("ctl-selected-fill", c.selected_fill.css()),
            ("ctl-pressed-fill", c.pressed_fill.css()),
            ("radius", format!("{}px", self.radius)),
            ("radius-box", format!("{}px", self.radius_box)),
            ("radius-win", format!("{}px", self.radius_win())),
            ("radius-pill", format!("{}px", self.radius_pill())),
            // A scale's bar: a third of the pill's radius.
            ("radius-bar", format!("{}px", self.radius_pill() / 3)),
            ("radius-knob", if sharp { "0px" } else { "50%" }.to_owned()),
            // Fixed alphas of the design's mockups (`app.css`), computed here so the
            // stylesheet needs no colour functions.
            ("line", fg_at(0.12)),
            ("banner-error", at(blend(self.red, self.bg, 0.16), alpha)),
            ("banner-warn", at(blend(self.yellow, self.bg, 0.14), alpha)),
            // The one literal the stylesheet needs, written once: the frameless icon buttons'
            // fill at rest, which a see-through window's ground shows through.
            ("none", "transparent".to_owned()),
        ];
        let mut root = String::from(":root {\n");
        for (name, value) in tokens {
            let _ = writeln!(root, "  --{name}: {value};");
        }
        root.push_str("}\n");
        root
    }

    /// The whole CSS: the `:root` block, then the stylesheet.
    pub fn css(&self) -> String {
        self.root() + STYLESHEET
    }

    /// The unlit preview dot, opaque over the preview frame's surface.
    pub fn unlit(&self) -> Rgb {
        blend(self.fg, self.surface, UNLIT_ALPHA)
    }

    /// The disabled colour: the foreground over the card, opaque. A disabled control's text, icon,
    /// knob and lit dots, and the frame of a chosen one. An Omarchy theme has no key for it, so
    /// it is derived from its own foreground and card, and never read or logged.
    pub fn disabled(&self) -> Rgb {
        let alpha = if self.dark {
            DISABLED_ALPHA_DARK
        } else {
            DISABLED_ALPHA_LIGHT
        };
        blend(self.fg, self.surface, alpha)
    }
}

#[cfg(test)]
mod tests;
