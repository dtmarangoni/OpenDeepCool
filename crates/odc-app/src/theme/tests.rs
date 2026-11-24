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
  --disabled: #74726a;
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
  --ctl-pressed-fill: rgba(228, 220, 203, 0.22);
  --radius: 0px;
  --radius-box: 0px;
  --radius-win: 0px;
  --radius-pill: 9px;
  --radius-bar: 3px;
  --radius-knob: 50%;
  --line: rgba(228, 220, 203, 0.12);
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
            k if k.starts_with("hover-cursor-color") || k.starts_with("hover-cursor-fill") => &[3],
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

// The disabled colour (design, D55, Task 63): the foreground over the card, opaque, by 0.40
// in a dark theme and 0.55 in a light one; the same derivation gives an Omarchy theme its
// own, from its foreground and card. It is weaker than the theme's muted and still text.
#[test]
fn the_disabled_colour_is_the_foreground_over_the_card() {
    for (dark, want) in [(true, [0x79, 0x84, 0x91]), (false, [0x80, 0x84, 0x8c])] {
        let t = Tokens::default_theme(dark);
        assert_eq!(t.disabled(), Rgb(want), "dark {dark}");
        let on_card = ratio(t.disabled(), t.surface);
        assert!(
            (2.5..=3.5).contains(&on_card),
            "dark {dark}: {on_card:.3} on the card"
        );
        assert!(
            on_card < ratio(t.muted, t.surface),
            "dark {dark}: below muted"
        );
        assert!(
            t.root()
                .contains(&format!("--disabled: {};", t.disabled().css()))
        );
    }
}

// The app's default palettes have the contrast of the design's table (Default theme), to
// one decimal, and keep text at 4.5:1 and borders at 3:1.
#[test]
fn the_default_theme_has_the_contrast_the_design_states() {
    let table: [(bool, [f64; 15]); 2] = [
        (
            true,
            [
                11.0, 8.9, 6.5, 5.3, 6.3, 5.1, 6.9, 5.6, 4.5, 6.6, 5.8, 4.2, 7.8, 4.0, 6.6,
            ],
        ),
        (
            false,
            [
                8.6, 11.0, 5.1, 6.5, 5.0, 6.4, 5.0, 6.4, 4.3, 3.5, 8.6, 3.4, 7.1, 4.0, 6.2,
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
