//! The mockups as committed, and the palettes' contrast (Phase 10 plan, Task 0;
//! `docs/design/settings-app.md`, Default theme).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use odc_mockups::{Json, PAGES_DIR, SOURCES, pages, render};

#[test]
fn writing_in_memory_gives_every_committed_mockup_byte_for_byte() {
    let read = |name: &str| fs::read_to_string(Path::new(SOURCES).join(name)).expect(name);
    let written = render(&read("app.css"), &read("app.js"));
    for (file, html) in &written {
        let committed = fs::read_to_string(Path::new(PAGES_DIR).join(file)).expect(file);
        assert!(
            committed == *html,
            "{file} differs: run `cargo run -p odc-mockups -- docs/design/mockups`"
        );
    }
    let names: BTreeSet<String> = written.iter().map(|(f, _)| (*f).to_owned()).collect();
    let committed: BTreeSet<String> = fs::read_dir(PAGES_DIR)
        .expect("the mockups folder")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .collect();
    assert_eq!(names, committed, "a page without its state, or the reverse");
    assert_eq!(names.len(), 24);
}

#[test]
fn the_json_is_written_as_the_mockups_script_reads_it() {
    let value = Json::Array(vec![
        Json::Null,
        Json::Bool(true),
        Json::Int(-3),
        Json::Str("°F \"x\" \\ \n\u{1}".to_owned()),
    ]);
    assert_eq!(value.text(), r#"[null, true, -3, "°F \"x\" \\ \n\u0001"]"#);
    let object = Json::Object(vec![("a".to_owned(), Json::Int(1))])
        .with("b", Json::Int(2))
        .with("a", Json::Int(3));
    assert_eq!(object.text(), r#"{"a": 3, "b": 2}"#);
}

#[test]
fn every_page_has_its_own_file_and_title() {
    let all = pages();
    let files: BTreeSet<&str> = all.iter().map(|p| p.file).collect();
    let titles: BTreeSet<&str> = all.iter().map(|p| p.title).collect();
    assert_eq!(files.len(), all.len());
    assert_eq!(titles.len(), all.len());
}

/// One palette: its colours, and the alphas of the selected fill and a control's border.
struct Palette {
    name: &'static str,
    background: &'static str,
    surface: &'static str,
    foreground: &'static str,
    muted: &'static str,
    accent: &'static str,
    red: &'static str,
    yellow: &'static str,
    green: &'static str,
    selected: f64,
    border: f64,
}

/// The default theme, dark and light, and the mockups' sample Omarchy theme, as the design and
/// `app.css` give them.
const PALETTES: [Palette; 3] = [
    Palette {
        name: "default dark",
        background: "#2d3039",
        surface: "#313f51",
        foreground: "#e5ebf2",
        muted: "#b3b6c2",
        accent: "#94b7d6",
        red: "#e5776c",
        yellow: "#e0b65c",
        green: "#79c89a",
        selected: 0.16,
        border: 0.52,
    },
    Palette {
        name: "default light",
        background: "#d3dce3",
        surface: "#f5f7f9",
        foreground: "#2d3039",
        muted: "#555862",
        accent: "#365675",
        red: "#b33b32",
        yellow: "#785200",
        green: "#2f7756",
        selected: 0.14,
        border: 0.62,
    },
    Palette {
        name: "sample Omarchy",
        background: "#1f2120",
        surface: "#2a2c2a",
        foreground: "#e4dccb",
        muted: "#9c9585",
        accent: "#d9a35b",
        red: "#e07a6e",
        yellow: "#d9b45b",
        green: "#98bf7c",
        selected: 0.18,
        border: 0.40,
    },
];

type Rgb = [f64; 3];

fn rgb(hex: &str) -> Rgb {
    let hex = hex.trim_start_matches('#');
    std::array::from_fn(|i| {
        f64::from(u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex")) / 255.0
    })
}

/// WCAG 2's relative luminance.
fn luminance(c: Rgb) -> f64 {
    let f = |v: f64| {
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

/// `fg` at `alpha` over `bg`.
fn blend(fg: Rgb, bg: Rgb, alpha: f64) -> Rgb {
    std::array::from_fn(|i| alpha * fg[i] + (1.0 - alpha) * bg[i])
}

/// WCAG 2's contrast ratio.
fn ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Each check of the design's table: its name, what it needs, and the value the table gives
/// for the dark and the light default theme, to one decimal.
#[test]
fn the_default_theme_has_the_contrast_the_design_states() {
    let checks = |p: &Palette| -> Vec<(&'static str, f64, f64)> {
        let (bg, surface, fg) = (rgb(p.background), rgb(p.surface), rgb(p.foreground));
        let selected = blend(fg, surface, p.selected);
        // The gear and the back icon sit on the background, so their selected fill is over it.
        let selected_on_bg = blend(fg, bg, p.selected);
        // The selection: the accent at 0.25 over the background (Phase 11a Task 11).
        let selection = blend(rgb(p.accent), bg, 0.25);
        vec![
            ("text on background", 4.5, ratio(fg, bg)),
            ("text on surface", 4.5, ratio(fg, surface)),
            ("muted on background", 4.5, ratio(rgb(p.muted), bg)),
            ("muted on surface", 4.5, ratio(rgb(p.muted), surface)),
            ("accent on background", 4.5, ratio(rgb(p.accent), bg)),
            ("accent on surface", 4.5, ratio(rgb(p.accent), surface)),
            ("yellow on background", 4.5, ratio(rgb(p.yellow), bg)),
            ("yellow on surface", 4.5, ratio(rgb(p.yellow), surface)),
            ("red on background", 3.0, ratio(rgb(p.red), bg)),
            ("green on background", 3.0, ratio(rgb(p.green), bg)),
            ("text on the selected fill", 4.5, ratio(fg, selected)),
            ("control border", 3.0, ratio(blend(fg, bg, p.border), bg)),
            ("lit dot against unlit", 3.0, ratio(fg, blend(fg, bg, 0.12))),
            (
                "accent on the selected fill",
                3.0,
                ratio(rgb(p.accent), selected_on_bg),
            ),
            ("text on the selection", 4.5, ratio(fg, selection)),
        ]
    };
    let table: [(&str, [f64; 15]); 2] = [
        (
            "default dark",
            [
                11.0, 8.9, 6.5, 5.3, 6.3, 5.1, 6.9, 5.6, 4.5, 6.6, 5.8, 4.2, 7.8, 4.0, 6.6,
            ],
        ),
        (
            "default light",
            [
                9.5, 12.3, 5.1, 6.6, 5.5, 7.1, 5.0, 6.5, 4.2, 3.9, 9.5, 3.5, 7.7, 4.3, 6.7,
            ],
        ),
    ];
    let tenths = |x: f64| (x * 10.0).round() / 10.0;
    for palette in &PALETTES {
        let got = checks(palette);
        for (name, need, value) in &got {
            assert!(
                value >= need,
                "{}: {name} {value:.2} < {need}",
                palette.name
            );
        }
        if let Some((_, want)) = table.iter().find(|(n, _)| *n == palette.name) {
            for ((name, _, value), want) in got.iter().zip(want) {
                assert!(
                    (tenths(*value) - want).abs() < 1e-9,
                    "{}: {name} is {value:.2}, the design says {want}",
                    palette.name
                );
            }
        }
    }
    // The design's note on muted text on the selected fill (below 4.5 in the dark theme, so
    // nothing muted is drawn on a selected tile; above it in the light one), and its sample
    // theme's figures.
    let dark = &PALETTES[0];
    let selected = blend(rgb(dark.foreground), rgb(dark.surface), dark.selected);
    assert!(ratio(rgb(dark.muted), selected) < 4.5);
    assert!((tenths(ratio(rgb(dark.muted), selected)) - 3.4).abs() < 1e-9);
    let light = &PALETTES[1];
    let selected = blend(rgb(light.foreground), rgb(light.surface), light.selected);
    assert!((tenths(ratio(rgb(light.muted), selected)) - 5.1).abs() < 1e-9);
    let sample = &PALETTES[2];
    let bg = rgb(sample.background);
    for (colour, want) in [
        (sample.foreground, 11.9),
        (sample.muted, 5.4),
        (sample.accent, 7.2),
    ] {
        assert!(
            (tenths(ratio(rgb(colour), bg)) - want).abs() < 1e-9,
            "{colour}"
        );
    }
}

/// The `--name: value` pairs of `app.css`'s block for `selector`.
fn block(css: &str, selector: &str) -> Vec<(String, String)> {
    let start = css
        .find(selector)
        .unwrap_or_else(|| panic!("{selector}: no block"));
    let body = &css[start..];
    let body = &body[body.find('{').expect("a block") + 1..body.find('}').expect("its end")];
    body.split(';')
        .filter_map(|decl| {
            let decl = decl.rsplit("*/").next().unwrap_or(decl).trim();
            let (name, value) = decl.split_once(':')?;
            Some((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

// The palettes above are the ones the mockups draw with: each colour and alpha as `app.css`
// gives it for its theme.
#[test]
fn the_palettes_are_the_mockups_own() {
    let css = fs::read_to_string(Path::new(SOURCES).join("app.css")).expect("app.css");
    for (palette, selector) in PALETTES.iter().zip([
        r#":root[data-theme="dark"]"#,
        r#":root[data-theme="light"]"#,
        r#":root[data-theme="omarchy"]"#,
    ]) {
        let values = block(&css, selector);
        let get = |name: &str| {
            values
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
                .unwrap_or_else(|| panic!("{selector}: no {name}"))
        };
        for (name, colour) in [
            ("--bg", palette.background),
            ("--surface", palette.surface),
            ("--fg", palette.foreground),
            ("--muted", palette.muted),
            ("--accent", palette.accent),
            ("--red", palette.red),
            ("--yellow", palette.yellow),
            ("--green", palette.green),
        ] {
            assert_eq!(get(name), colour, "{}: {name}", palette.name);
        }
        let percent = |name: &str| {
            get(name)
                .trim_end_matches('%')
                .parse::<f64>()
                .expect("a percentage")
                / 100.0
        };
        assert!((percent("--a-selected-fill") - palette.selected).abs() < 1e-9);
        assert!((percent("--a-normal-border") - palette.border).abs() < 1e-9);
    }
}
