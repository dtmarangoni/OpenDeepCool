//! The mockups' sheet and the app draw OpenDeepCool's own themes with the same palette (Phase 11a
//! Task 64). `docs/design/mockups-src/app.css` holds each theme's colours and control alphas as the
//! design states them, and `theme.rs` holds them as the app draws them; each change of the
//! palette used to be three hand edits (the sheet, `theme.rs` and the mockups' own test), and a
//! missed one passed, since every test read its own copy. This test ties the sheet to the app;
//! `tools/mockups/tests/mockups.rs` already ties its palettes to the sheet, and each side tests the
//! contrast of its copy against the design's table.

use std::fs;
use std::path::Path;

use odc_app::theme::{Rgb, Tokens};

/// The declarations of the first block of `css` that opens with `selector`, by name.
fn declarations(css: &str, selector: &str) -> Vec<(String, String)> {
    let start = css
        .find(selector)
        .unwrap_or_else(|| panic!("{selector}: no block"));
    let body = &css[start..];
    let body = &body[body.find('{').expect("a block") + 1..body.find('}').expect("its end")];
    body.split(';')
        .filter_map(|declaration| {
            let declaration = declaration
                .rsplit("*/")
                .next()
                .unwrap_or(declaration)
                .trim();
            let (name, value) = declaration.split_once(':')?;
            Some((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

fn value<'a>(block: &'a [(String, String)], name: &str) -> &'a str {
    block
        .iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("{name}: not in the block"))
        .1
        .as_str()
}

fn hex(colour: Rgb) -> String {
    let [r, g, b] = colour.0;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn percent(text: &str) -> f64 {
    text.strip_suffix('%')
        .unwrap_or_else(|| panic!("{text}: not a percentage"))
        .parse::<f64>()
        .expect("a number")
        / 100.0
}

#[test]
fn the_mockups_sheet_and_the_app_draw_the_same_palettes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/design/mockups-src/app.css");
    let css = fs::read_to_string(path).expect("the mockups' stylesheet");
    for (dark, selector) in [
        (true, ":root[data-theme=\"dark\"]"),
        (false, ":root[data-theme=\"light\"]"),
    ] {
        let sheet = declarations(&css, selector);
        let app = Tokens::default_theme(dark);
        let name = if dark { "dark" } else { "light" };
        for (token, want) in [
            ("--bg", app.bg),
            ("--surface", app.surface),
            ("--fg", app.fg),
            ("--muted", app.muted),
            ("--accent", app.accent),
            ("--red", app.red),
            ("--yellow", app.yellow),
            ("--green", app.green),
        ] {
            assert_eq!(
                value(&sheet, token),
                hex(want),
                "{name} {token}: the sheet and theme.rs differ"
            );
        }
        let c = app.controls;
        for (token, want) in [
            ("--a-normal-fill", c.normal_fill.alpha),
            ("--a-normal-border", c.normal_border.alpha),
            ("--a-hover-fill", c.hover_fill.alpha),
            ("--a-hover-border", c.hover_border.alpha),
            ("--a-selected-fill", c.selected_fill.alpha),
            ("--a-pressed-fill", c.pressed_fill.alpha),
        ] {
            let got = percent(value(&sheet, token));
            assert!(
                (got - want).abs() < 1e-9,
                "{name} {token}: the sheet says {got}, theme.rs {want}"
            );
        }
        assert_eq!(
            value(&sheet, "--radius"),
            format!("{}px", app.radius),
            "{name} --radius"
        );
        assert_eq!(
            value(&sheet, "--radius-box"),
            format!("{}px", app.radius_box),
            "{name} --radius-box"
        );
    }
}
