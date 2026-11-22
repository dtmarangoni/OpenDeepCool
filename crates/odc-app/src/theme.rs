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

/// OpenDeepCool's own theme, dark, "Coolant": Lagoon, Tide, Foam, Fog and Flare, a petrol-blue
/// family with one rose accent (`docs/design/settings-app.md`, OpenDeepCool's own theme; Phase 11a
/// Task 44, redesigned from Task 11's graphite).
const DARK: Palette = Palette {
    bg: Rgb([0x0f, 0x26, 0x30]),
    surface: Rgb([0x18, 0x38, 0x46]),
    fg: Rgb([0xf1, 0xec, 0xe2]),
    muted: Rgb([0x8f, 0xb0, 0xb8]),
    accent: Rgb([0xfb, 0x7f, 0xb0]),
    red: Rgb([0xff, 0x6b, 0x57]),
    yellow: Rgb([0xf0, 0xc9, 0x4e]),
    green: Rgb([0x5f, 0xd6, 0x8a]),
    alphas: [0.05, 0.44, 0.09, 0.60, 0.16, 0.24],
};

/// The same family in daylight: Frost, Glacier, Deep, Slate and Flare.
const LIGHT: Palette = Palette {
    bg: Rgb([0xd2, 0xe0, 0xe6]),
    surface: Rgb([0xf6, 0xfa, 0xfb]),
    fg: Rgb([0x0e, 0x2a, 0x35]),
    muted: Rgb([0x42, 0x60, 0x6a]),
    accent: Rgb([0xb0, 0x14, 0x5f]),
    red: Rgb([0xb9, 0x3a, 0x22]),
    yellow: Rgb([0x7a, 0x52, 0x00]),
    green: Rgb([0x1d, 0x7a, 0x3f]),
    alphas: [0.04, 0.56, 0.07, 0.74, 0.14, 0.20],
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
pub fn parse_colour(text: &str) -> Option<Rgba> {
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
        let tokens: [(&str, String); 32] = [
            ("bg", self.bg.css()),
            ("surface", self.surface.css()),
            ("ground", at(self.bg, alpha)),
            ("plate", at(self.surface, alpha)),
            ("fg", self.fg.css()),
            ("muted", self.muted.css()),
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
            ("ctl-selected-border", c.selected_border.css()),
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
            ("unlit", fg_at(UNLIT_ALPHA)),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up Omarchy theme, every key present: the mockups' sample colours.
    const COLORS: &str = r##"
mode = "dark"
accent = "#d9a35b"
selection = "#4a4235"
muted = "#9c9585"
background = "#1f2120"
dark_background = "#181a19"
lighter_background = "#2a2c2a"
foreground = "#e4dccb"
red = "#e07a6e"
yellow = "#d9b45b"
green = "#98bf7c"
cyan = "#7fb8b0"
"##;

    const SHELL: &str = r##"
[controls]
normal-color        = "#e4dccb"
normal-fill-alpha   = 0.04
normal-border       = "#e4dccb"
normal-border-width = 1
normal-border-alpha = 0.4
hover-cursor-color        = "#e4dccb"
hover-cursor-fill-alpha   = 0.08
hover-cursor-border       = "#e4dccb"
hover-cursor-border-width = 1
hover-cursor-border-alpha = 0.25
focus-color        = "#e4dccb"
focus-fill-alpha   = 0.08
focus-border       = "rgba(d9a35bee) rgba(98bf7cee) 45deg"
focus-border-width = 1
focus-border-alpha = 0.5
selected-color        = "#e4dccb"
selected-fill-alpha   = 0.18
selected-border       = "#e4dccb"
selected-border-width = 0
selected-border-alpha = 1.0
pressed-fill-alpha   = 0.22
selection-fill-alpha = 0.35

[spacing]
scale = 1.0
"##;

    const ROOT: &str = "\
:root {
  --bg: #1f2120;
  --surface: #2a2c2a;
  --ground: rgba(31, 33, 32, 1);
  --plate: rgba(42, 44, 42, 1);
  --fg: #e4dccb;
  --muted: #9c9585;
  --accent: #d9a35b;
  --red: #e07a6e;
  --yellow: #d9b45b;
  --green: #98bf7c;
  --selection: #4a4235;
  --ctl-normal-fill: rgba(228, 220, 203, 0.04);
  --ctl-normal-border: rgba(228, 220, 203, 0.4);
  --ctl-normal-border-width: 1px;
  --ctl-hover-fill: rgba(228, 220, 203, 0.08);
  --ctl-hover-border: rgba(228, 220, 203, 0.25);
  --ctl-focus-fill: rgba(228, 220, 203, 0.08);
  --ctl-focus-border: rgba(217, 163, 91, 0.467);
  --ctl-selected-fill: rgba(228, 220, 203, 0.18);
  --ctl-selected-border: rgba(228, 220, 203, 1);
  --ctl-pressed-fill: rgba(228, 220, 203, 0.22);
  --radius: 0px;
  --radius-box: 0px;
  --radius-win: 0px;
  --radius-pill: 9px;
  --radius-bar: 3px;
  --radius-knob: 50%;
  --line: rgba(228, 220, 203, 0.12);
  --unlit: rgba(228, 220, 203, 0.12);
  --banner-error: rgba(62, 47, 44, 1);
  --banner-warn: rgba(57, 54, 40, 1);
  --none: transparent;
}
";

    #[test]
    fn a_whole_omarchy_theme_gives_its_own_root_block() {
        let theme = theme(Some(COLORS), Some(SHELL));
        assert_eq!(theme.problems, Vec::<String>::new());
        assert!(theme.tokens.dark);
        assert_eq!(theme.tokens.root(), ROOT);
        assert!(theme.tokens.css().starts_with(ROOT));
    }

    /// A `colors.toml` key, its token, and the default dark value it falls back to.
    type ColourKey = (&'static str, fn(&Tokens) -> Rgb, Rgb);

    fn colour_keys() -> [ColourKey; 9] {
        [
            ("background", |t| t.bg, DARK.bg),
            ("lighter_background", |t| t.surface, DARK.surface),
            ("foreground", |t| t.fg, DARK.fg),
            ("muted", |t| t.muted, DARK.muted),
            ("accent", |t| t.accent, DARK.accent),
            ("red", |t| t.red, DARK.red),
            ("yellow", |t| t.yellow, DARK.yellow),
            ("green", |t| t.green, DARK.green),
            (
                "selection",
                |t| t.selection,
                blend(DARK.accent, DARK.bg, SELECTION_ALPHA),
            ),
        ]
    }

    fn without(text: &str, key: &str) -> String {
        text.lines()
            .filter(|line| line.split('=').next().map(str::trim) != Some(key))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `text` with `key = value`: in place of the key's line, or else right after
    /// `[controls]`, or at the end.
    fn with(text: &str, key: &str, value: &str) -> String {
        let line = format!("{key} = {value}");
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        if let Some(at) = lines
            .iter()
            .position(|l| l.split('=').next().map(str::trim) == Some(key))
        {
            lines[at] = line;
        } else if let Some(at) = lines.iter().position(|l| l == "[controls]") {
            lines.insert(at + 1, line);
        } else {
            lines.push(line);
        }
        lines.join("\n") + "\n"
    }

    #[test]
    fn each_missing_colour_takes_the_default_s_and_names_its_key() {
        let whole = theme(Some(COLORS), Some(SHELL)).tokens;
        for (key, get, default) in colour_keys() {
            // A missing selection is the theme's own accent over its own background (Phase 11a
            // Task 11), not the default theme's.
            let default = if key == "selection" {
                blend(whole.accent, whole.bg, SELECTION_ALPHA)
            } else {
                default
            };
            let theme = theme(Some(&without(COLORS, key)), Some(SHELL));
            assert_eq!(get(&theme.tokens), default, "{key}");
            assert_eq!(
                theme.problems,
                [format!(
                    "colors.toml: `{key}` is missing; the default theme's value is used"
                )]
            );
            // Every other token is the theme's own.
            for (other, get_other, _) in colour_keys() {
                if other != key {
                    assert_eq!(
                        get_other(&theme.tokens),
                        get_other(&whole),
                        "{key}: {other}"
                    );
                }
            }
        }
    }

    /// Each `[controls]` key.
    const CONTROL_KEYS: [&str; 18] = [
        "normal-color",
        "normal-fill-alpha",
        "normal-border",
        "normal-border-width",
        "normal-border-alpha",
        "hover-cursor-color",
        "hover-cursor-fill-alpha",
        "hover-cursor-border",
        "hover-cursor-border-alpha",
        "focus-color",
        "focus-fill-alpha",
        "focus-border",
        "focus-border-alpha",
        "selected-color",
        "selected-fill-alpha",
        "selected-border",
        "selected-border-alpha",
        "pressed-fill-alpha",
    ];

    #[test]
    fn each_missing_control_key_takes_its_default_and_names_it() {
        let whole = theme(Some(COLORS), Some(SHELL)).tokens.controls;
        let fg = Rgb([0xe4, 0xdc, 0xcb]);
        let d = default_controls(fg, &DARK);
        for key in CONTROL_KEYS {
            let got = theme(Some(COLORS), Some(&without(SHELL, key)));
            assert_eq!(
                got.problems,
                [format!(
                    "shell.toml: `{key}` is missing; the default theme's value is used"
                )],
                "{key}"
            );
            let c = got.tokens.controls;
            let changed = [
                c.normal_fill != whole.normal_fill,
                c.normal_border != whole.normal_border,
                c.normal_border_width != whole.normal_border_width,
                c.hover_fill != whole.hover_fill,
                c.hover_border != whole.hover_border,
                c.focus_fill != whole.focus_fill,
                c.focus_border != whole.focus_border,
                c.selected_fill != whole.selected_fill,
                c.selected_border != whole.selected_border,
                c.pressed_fill != whole.pressed_fill,
            ];
            match key {
                // The fixture's alphas and colours mostly equal the default's; those that
                // differ show the fall-back.
                "hover-cursor-border-alpha" => {
                    assert_eq!(c.hover_border.alpha, d.hover_border.alpha);
                    assert!(changed[4]);
                }
                "focus-border" => {
                    assert_eq!(c.focus_border.rgb, fg);
                    assert!(changed[6]);
                }
                "focus-border-alpha" => {
                    assert_eq!(c.focus_border.alpha, (238.0 / 255.0) * d.focus_border.alpha);
                }
                _ => {}
            }
            // Nothing but the key's own token moves.
            let own = match key {
                k if k.starts_with("normal-color") => [0, 9].as_slice(),
                k if k.starts_with("normal-fill") => &[0],
                k if k.starts_with("normal-border-width") => &[2],
                k if k.starts_with("normal-border") => &[1],
                k if k.starts_with("hover-cursor-color") || k.starts_with("hover-cursor-fill") => {
                    &[3]
                }
                k if k.starts_with("hover-cursor-border") => &[4],
                k if k.starts_with("focus-color") || k.starts_with("focus-fill") => &[5],
                k if k.starts_with("focus-border") => &[6],
                k if k.starts_with("selected-color") || k.starts_with("selected-fill") => &[7],
                k if k.starts_with("selected-border") => &[8],
                _ => &[9],
            };
            for (i, moved) in changed.iter().enumerate() {
                assert!(!moved || own.contains(&i), "{key} moved token {i}");
            }
        }
    }

    #[test]
    fn every_accepted_colour_shape_is_written_back() {
        let accepted = [
            ("#12abEF", [0x12, 0xab, 0xef], 1.0),
            ("#12abef80", [0x12, 0xab, 0xef], 128.0 / 255.0),
            ("rgb(12abef)", [0x12, 0xab, 0xef], 1.0),
            ("rgba(12abef80)", [0x12, 0xab, 0xef], 128.0 / 255.0),
            (
                "rgba(12abefff) rgba(000000ff) 45deg",
                [0x12, 0xab, 0xef],
                1.0,
            ),
            ("rgba(12abefff) rgba(000000ff)", [0x12, 0xab, 0xef], 1.0),
            ("rgb(12abef) #000000 -90deg", [0x12, 0xab, 0xef], 1.0),
            ("  #12abef  ", [0x12, 0xab, 0xef], 1.0),
        ];
        for (text, rgb, alpha) in accepted {
            let colour = parse_colour(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(colour.rgb, Rgb(rgb), "{text}");
            assert!((colour.alpha - alpha).abs() < 1e-12, "{text}");
            let colors = with(COLORS, "accent", &format!("{text:?}"));
            let got = theme(Some(&colors), Some(SHELL));
            assert!(got.problems.is_empty(), "{text}: {:?}", got.problems);
            assert!(
                got.tokens.root().contains("  --accent: #12abef;\n"),
                "{text}"
            );
        }
    }

    #[test]
    fn every_malformed_value_takes_the_default_and_never_reaches_the_css() {
        let colours = [
            "\"12abef\"",
            "\"red\"",
            "\"#abc\"",
            "\"#12abe\"",
            "\"#12abef0\"",
            "\"#12abefgg\"",
            "\"#123456;} * { color: red\"",
            "\"#123456;}\"",
            "\"rgb(12abef80)\"",
            "\"rgba(12abef)\"",
            "\"rgba(12, 171, 239, 1)\"",
            "\"0xff12abef\"",
            "\"#12abef 45deg extra\"",
            "\"45deg\"",
            "\"#12abef deg\"",
            "\"\"",
            "42",
            "true",
            "[\"#12abef\"]",
        ];
        for value in colours {
            let got = theme(Some(&with(COLORS, "accent", value)), Some(SHELL));
            assert_eq!(got.tokens.accent, DARK.accent, "{value}");
            assert_eq!(got.problems.len(), 1, "{value}");
            assert!(
                got.problems[0].starts_with("colors.toml: `accent` = "),
                "{value}"
            );
            assert_only_written_values(&got.tokens.css(), value);
            let got = theme(Some(COLORS), Some(&with(SHELL, "normal-border", value)));
            assert_eq!(got.problems.len(), 1, "{value}");
            assert_eq!(
                got.tokens.controls.normal_border.rgb,
                Rgb([0xe4, 0xdc, 0xcb])
            );
            assert_only_written_values(&got.tokens.css(), value);
        }
        let alphas = ["2", "-0.1", "\"x\"", "\"0.4\"", "nan", "inf", "true"];
        for value in alphas {
            let got = theme(Some(COLORS), Some(&with(SHELL, "normal-fill-alpha", value)));
            assert_eq!(
                got.tokens.controls.normal_fill.alpha, DARK.alphas[0],
                "{value}"
            );
            assert_eq!(got.problems.len(), 1, "{value}");
            assert!(
                got.problems[0].starts_with("shell.toml: `normal-fill-alpha` = "),
                "{value}"
            );
            assert_only_written_values(&got.tokens.css(), value);
        }
        let widths = [
            "-1",
            "17",
            "\"1 2 3 4 5\"",
            "\"1px\"",
            "\"\"",
            "\"x\"",
            "nan",
        ];
        for value in widths {
            let got = theme(
                Some(COLORS),
                Some(&with(SHELL, "normal-border-width", value)),
            );
            assert_eq!(
                got.tokens.controls.normal_border_width,
                Widths([1.0; 4]),
                "{value}"
            );
            assert_eq!(got.problems.len(), 1, "{value}");
        }
    }

    /// Every token's value in `css` is one this module writes: a hex colour, an `rgba()` of
    /// numbers, px widths, or the two words `transparent` and `50%`; and nothing of `raw` got in.
    fn assert_only_written_values(css: &str, raw: &str) {
        let (root, rest) = css.split_once("}\n").expect("the :root block");
        for line in root.lines().skip(1) {
            let value = line
                .trim()
                .split_once(": ")
                .and_then(|(_, v)| v.strip_suffix(';'))
                .unwrap_or_else(|| panic!("{line}"));
            let ok = value == "transparent"
                || value == "50%"
                || (value.len() == 7
                    && value.starts_with('#')
                    && value[1..].chars().all(|c| c.is_ascii_hexdigit()))
                || value
                    .strip_prefix("rgba(")
                    .and_then(|v| v.strip_suffix(')'))
                    .is_some_and(|v| v.split(", ").all(|n| n.parse::<f64>().is_ok()))
                || value.split(' ').all(|w| {
                    w.strip_suffix("px")
                        .is_some_and(|n| n.parse::<f64>().is_ok())
                });
            assert!(ok, "{raw}: {line}");
        }
        assert_eq!(rest, STYLESHEET);
        let raw = raw.trim_matches('"');
        if raw.len() > 3 {
            assert!(!css.contains(raw), "{raw} reached the CSS");
        }
    }

    #[test]
    fn border_widths_take_css_s_four_sides_and_the_per_side_keys() {
        let cases = [
            ("2", "2px", [2.0; 4]),
            ("1.5", "1.5px", [1.5; 4]),
            ("\"1 2\"", "1px 2px 1px 2px", [1.0, 2.0, 1.0, 2.0]),
            ("\"1 2 3\"", "1px 2px 3px 2px", [1.0, 2.0, 3.0, 2.0]),
            ("\"1 2 3 4\"", "1px 2px 3px 4px", [1.0, 2.0, 3.0, 4.0]),
        ];
        for (value, css, sides) in cases {
            let got = theme(
                Some(COLORS),
                Some(&with(SHELL, "normal-border-width", value)),
            );
            assert!(got.problems.is_empty(), "{value}");
            assert_eq!(got.tokens.controls.normal_border_width, Widths(sides));
            assert!(
                got.tokens
                    .root()
                    .contains(&format!("  --ctl-normal-border-width: {css};\n")),
                "{value}"
            );
        }
        let shell = with(SHELL, "normal-border-width", "\"1 2\"");
        let shell = with(&shell, "normal-border-width-left", "3");
        let shell = with(&shell, "normal-border-width-top", "99");
        let got = theme(Some(COLORS), Some(&shell));
        assert_eq!(
            got.tokens.controls.normal_border_width,
            Widths([1.0, 2.0, 1.0, 3.0])
        );
        assert_eq!(got.problems.len(), 1, "{:?}", got.problems);
        assert!(got.problems[0].starts_with("shell.toml: `normal-border-width-top` = 99"));
    }

    #[test]
    fn a_light_theme_takes_the_light_defaults_and_gtk_s_light_scheme() {
        let colors = with(COLORS, "mode", "\"light\"");
        let whole = theme(Some(&colors), Some(SHELL));
        assert!(!whole.tokens.dark);
        assert!(whole.problems.is_empty());
        let got = theme(Some(&without(&colors, "accent")), Some(SHELL));
        assert_eq!(got.tokens.accent, LIGHT.accent);
        let got = theme(Some(&colors), Some(&without(SHELL, "normal-border-alpha")));
        assert_eq!(got.tokens.controls.normal_border.alpha, LIGHT.alphas[1]);
    }

    #[test]
    fn a_missing_or_strange_mode_reads_as_dark() {
        for (colors, problem) in [
            (
                without(COLORS, "mode"),
                "colors.toml: `mode` is missing; dark is used",
            ),
            (
                with(COLORS, "mode", "\"sepia\""),
                "colors.toml: `mode` = \"sepia\" isn't accepted; dark is used",
            ),
        ] {
            let got = theme(Some(&colors), Some(SHELL));
            assert!(got.tokens.dark);
            assert_eq!(got.problems, [problem]);
        }
    }

    #[test]
    fn without_colors_toml_the_whole_default_dark_theme() {
        let got = theme(None, Some(SHELL));
        assert_eq!(got.tokens, Tokens::default_theme(true));
        assert!(got.problems.is_empty());
        assert_eq!((got.tokens.radius, got.tokens.radius_box), (4, 10));
        let got = theme(Some("mode = \"light\"\nnot toml ["), Some(SHELL));
        assert_eq!(got.tokens, Tokens::default_theme(true));
        assert_eq!(got.problems.len(), 1);
        assert!(got.problems[0].starts_with("colors.toml: not TOML ("));
    }

    #[test]
    fn without_shell_toml_or_its_controls_the_default_controls_in_the_theme_s_foreground() {
        let fg = Rgb([0xe4, 0xdc, 0xcb]);
        for (shell, problem) in [
            (None, "shell.toml: missing; the default controls are used"),
            (
                Some("[spacing]\nscale = 1.0\n"),
                "shell.toml: no [controls]; the default controls are used",
            ),
        ] {
            let got = theme(Some(COLORS), shell);
            assert_eq!(got.tokens.controls, default_controls(fg, &DARK));
            assert_eq!(got.problems, [problem]);
            assert_eq!(got.tokens.radius, 0);
        }
        let got = theme(Some(COLORS), Some("[controls"));
        assert_eq!(got.tokens.controls, default_controls(fg, &DARK));
        assert!(got.problems[0].starts_with("shell.toml: not TOML ("));
    }

    // The CSS has no opacity, no transition but `none`, no animation, no colour literal or
    // GTK colour reference outside `:root`, and every `var()` it uses is defined there.
    #[test]
    fn the_stylesheet_uses_only_the_tokens_and_never_moves() {
        let css = Tokens::default_theme(true).css();
        let (root, rest) = css.split_once("}\n").expect("the :root block");
        let rest = strip_comments(rest);
        assert!(!rest.contains("opacity"), "opacity");
        for (i, _) in rest.match_indices("transition") {
            assert!(
                rest[i..].starts_with("transition: none;"),
                "{}",
                &rest[i..i + 30]
            );
        }
        for (i, _) in rest.match_indices("animation") {
            assert!(rest[i..].starts_with("animation: none;"));
        }
        for literal in [
            "#",
            "rgb",
            "hsl",
            "hwb",
            "oklab",
            "oklch",
            "color(",
            "color-mix",
            "@",
        ] {
            assert!(!rest.contains(literal), "{literal} outside :root");
        }
        for word in rest.split(|c: char| !c.is_ascii_alphabetic() && c != '-') {
            assert!(
                !NAMED_COLOURS.contains(&word),
                "the named colour {word} outside :root"
            );
        }
        for function in ["alpha(", "shade(", "mix(", "lighter(", "darker("] {
            assert!(!rest.contains(function), "{function}");
        }
        let defined: Vec<&str> = root
            .lines()
            .filter_map(|l| l.trim().strip_prefix("--")?.split(':').next())
            .collect();
        for (i, _) in rest.match_indices("var(--") {
            let name = &rest[i + 6..];
            let name = &name[..name.find(')').expect("var's end")];
            assert!(defined.contains(&name), "var(--{name}) isn't defined");
        }
        assert!(rest.contains("transition: none;"));
    }

    const NAMED_COLOURS: [&str; 16] = [
        "white",
        "black",
        "red",
        "green",
        "blue",
        "yellow",
        "gray",
        "grey",
        "orange",
        "purple",
        "pink",
        "cyan",
        "magenta",
        "silver",
        "transparent",
        "currentcolor",
    ];

    fn strip_comments(css: &str) -> String {
        let mut out = String::new();
        let mut rest = css;
        while let Some(start) = rest.find("/*") {
            out.push_str(&rest[..start]);
            let end = rest[start..].find("*/").expect("a comment's end");
            rest = &rest[start + end + 2..];
        }
        out + rest
    }

    /// A colour as 0–1 channels, unrounded, as the design's table computes it.
    type Exact = [f64; 3];

    fn luminance(c: Exact) -> f64 {
        let f = |v: f64| {
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
    }

    fn ratio(a: impl Into<Exact>, b: impl Into<Exact>) -> f64 {
        let (la, lb) = (luminance(a.into()), luminance(b.into()));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    impl From<Rgb> for Exact {
        fn from(c: Rgb) -> Exact {
            c.unit()
        }
    }

    /// `fg` at `alpha` over `bg`, unrounded.
    fn over(fg: Rgb, bg: Rgb, alpha: f64) -> Exact {
        let (fg, bg) = (fg.unit(), bg.unit());
        std::array::from_fn(|i| alpha * fg[i] + (1.0 - alpha) * bg[i])
    }

    // The window's style, as the design's table of Window opacity and corners gives it
    // (Phase 11a Tasks 45 and 51): the alpha of the ground and the plate by opacity, the blur
    // class's condition, and the radii by corners, for every combination of the own theme and for
    // an Omarchy theme.
    #[test]
    fn the_window_style_follows_the_design_s_tables() {
        use odc_ipc::{WindowCorners as C, WindowOpacity as O};
        for dark in [true, false] {
            for (opacity, alpha) in [
                (O::Opaque, "1"),
                (O::Transparent, "0.985"),
                (O::Glassy, "0.75"),
            ] {
                for (corners, radii) in [
                    (C::Rounded, ("4px", "10px", "12px", "9px", "3px", "50%")),
                    (C::Sharp, ("0px", "0px", "0px", "0px", "0px", "0px")),
                ] {
                    let t = Tokens::default_theme(dark).with_window(opacity, corners);
                    let root = t.root();
                    let has = |token: &str| root.contains(token);
                    let p = Palette::of(dark);
                    for (name, rgb) in [("ground", p.bg), ("plate", p.surface)] {
                        let [r, g, b] = rgb.0;
                        assert!(
                            has(&format!("--{name}: rgba({r}, {g}, {b}, {alpha});")),
                            "{dark} {opacity:?} {corners:?}: {name}\n{root}"
                        );
                    }
                    for (name, value) in [
                        ("radius", radii.0),
                        ("radius-box", radii.1),
                        ("radius-win", radii.2),
                        ("radius-pill", radii.3),
                        ("radius-bar", radii.4),
                        ("radius-knob", radii.5),
                    ] {
                        assert!(
                            has(&format!("--{name}: {value};")),
                            "{dark} {opacity:?} {corners:?}: {name}\n{root}"
                        );
                    }
                    assert!(has("--none: transparent;"));
                    assert_eq!(t.corners, Some(corners));
                }
            }
        }
        // The own theme as it is by default: opaque and rounded.
        let default = Tokens::default_theme(true);
        assert_eq!(
            (default.opacity, default.corners),
            (O::Opaque, Some(C::Rounded))
        );
        // An Omarchy theme: no corners of the app's own, controls and boxes square, the switches
        // and knobs round as before, and the window's alpha only from its mode.
        let omarchy = theme(Some(COLORS), Some(SHELL)).tokens;
        assert_eq!(omarchy.corners, None);
        let root = omarchy.root();
        for want in [
            "--radius: 0px;",
            "--radius-box: 0px;",
            "--radius-win: 0px;",
            "--radius-pill: 9px;",
            "--radius-knob: 50%;",
        ] {
            assert!(root.contains(want), "{want}\n{root}");
        }
        for (opacity, alpha) in [
            (O::Opaque, "1"),
            (O::Transparent, "0.985"),
            (O::Glassy, "0.75"),
        ] {
            let t = Tokens { opacity, ..omarchy };
            assert!(
                t.root()
                    .contains(&format!("--ground: rgba(31, 33, 32, {alpha});"))
            );
        }
        // The blur is a class of the window: the rule is in the stylesheet, applied by the class.
        assert!(STYLESHEET.contains("window.glassy {\n  backdrop-filter: blur(20px);\n}"));
    }

    // The app's default palettes have the contrast of the design's table (Default theme), to
    // one decimal, and keep text at 4.5:1 and borders at 3:1.
    #[test]
    fn the_default_theme_has_the_contrast_the_design_states() {
        let table: [(bool, [f64; 15]); 2] = [
            (
                true,
                [
                    13.3, 10.5, 6.8, 5.4, 6.5, 5.2, 9.8, 7.8, 5.6, 8.6, 6.7, 3.7, 9.5, 4.1, 8.7,
                ],
            ),
            (
                false,
                [
                    11.1, 14.3, 5.0, 6.4, 5.0, 6.4, 5.1, 6.6, 4.2, 4.0, 10.9, 3.3, 8.9, 3.9, 7.3,
                ],
            ),
        ];
        for (dark, want) in table {
            let t = Tokens::default_theme(dark);
            let c = t.controls;
            let selected = over(t.fg, t.surface, c.selected_fill.alpha);
            let got = [
                (4.5, ratio(t.fg, t.bg)),
                (4.5, ratio(t.fg, t.surface)),
                (4.5, ratio(t.muted, t.bg)),
                (4.5, ratio(t.muted, t.surface)),
                (4.5, ratio(t.accent, t.bg)),
                (4.5, ratio(t.accent, t.surface)),
                (4.5, ratio(t.yellow, t.bg)),
                (4.5, ratio(t.yellow, t.surface)),
                (3.0, ratio(t.red, t.bg)),
                (3.0, ratio(t.green, t.bg)),
                (4.5, ratio(t.fg, selected)),
                (3.0, ratio(over(t.fg, t.bg, c.normal_border.alpha), t.bg)),
                (3.0, ratio(t.fg, over(t.fg, t.bg, UNLIT_ALPHA))),
                // The pressed gear and back icon: the accent on the selected fill over the
                // background; and the text on the selection.
                (
                    3.0,
                    ratio(t.accent, over(t.fg, t.bg, c.selected_fill.alpha)),
                ),
                (4.5, ratio(t.fg, t.selection)),
            ];
            for (i, ((need, value), want)) in got.iter().zip(want).enumerate() {
                assert!(value >= need, "dark {dark}, check {i}: {value:.2} < {need}");
                assert!(
                    ((value * 10.0).round() / 10.0 - want).abs() < 1e-9,
                    "dark {dark}, check {i}: {value:.2}, the design says {want}"
                );
            }
            // The default selection keeps text readable (decisions log D31).
            assert!(ratio(t.fg, t.selection) >= 4.5, "dark {dark}: selection");
            // The banners' tints keep their text readable.
            assert!(ratio(t.fg, blend(t.red, t.bg, 0.16)) >= 4.5);
            assert!(ratio(t.fg, blend(t.yellow, t.bg, 0.14)) >= 4.5);
        }
    }
}
