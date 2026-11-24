//! The window's opacity and corners in a headless weston (Phase 11a Tasks 45 and 51; layer 3;
//! `docs/design/settings-app.md`, Window opacity and corners): the stylesheet of every style loads
//! with no CSS problem (`backdrop-filter` included), the window's ground has the alpha of its mode
//! and a card a denser one while the preview's frame stays solid, the window's corners follow
//! the own theme's choice, and the own light theme draws its tiles as cards and frames the chosen
//! one (Task 61). Behind the `weston-tests` feature.

mod common;

use std::process::ExitCode;
use std::time::Duration;

use gtk::prelude::*;
use odc_app::theme::Tokens;
use odc_ipc::{State, WindowCorners, WindowOpacity};

use common::{
    Check, Picture, Renderer, base, close, near as colour_near, run_for, save_png, shown,
};
use odc_app::theme::Rgb;

fn main() -> ExitCode {
    common::run(&CHECKS)
}

const CHECKS: [(&str, Check); 5] = [
    (
        "no_style_gives_a_css_problem_and_the_glass_rule_is_accepted",
        css_clean,
    ),
    ("the_window_has_the_alpha_of_its_mode", alphas),
    ("the_corners_follow_the_own_theme_s_choice", corners),
    (
        "an_omarchy_theme_s_mode_changes_the_window_and_leaves_its_corners",
        omarchy,
    ),
    (
        "the_own_light_theme_draws_its_tiles_as_cards_and_frames_the_chosen_one",
        own_light_tiles,
    ),
];

const OPACITIES: [WindowOpacity; 3] = [
    WindowOpacity::Opaque,
    WindowOpacity::Transparent,
    WindowOpacity::Glassy,
];
const ALPHAS: [f64; 3] = [1.0, 0.985, 0.75];

/// The own theme as the service's settings make it, with no system theme to follow.
fn own(opacity: WindowOpacity, corners: WindowCorners) -> State {
    State {
        use_system_theme: Some(false),
        window_opacity: Some(opacity),
        window_corners: Some(corners),
        ..base()
    }
}

/// The alpha of a pixel as a fraction, and whether it's within `slack` of `want`.
fn near(got: u8, want: f64, slack: f64) -> bool {
    (f64::from(got) / 255.0 - want).abs() <= slack
}

// Every theme's stylesheet parses with no problem: the own theme's 12 settings and each mode of
// an Omarchy theme, with the glass rule (`backdrop-filter`) the window's class `glassy` uses.
fn css_clean(app: &gtk::Application) {
    let _ = app;
    let mut sheets = Vec::new();
    for dark in [true, false] {
        for opacity in OPACITIES {
            for corners in [WindowCorners::Sharp, WindowCorners::Rounded] {
                sheets.push((
                    format!("own dark {dark}, {opacity:?}, {corners:?}"),
                    Tokens::default_theme(dark)
                        .with_window(opacity, corners)
                        .css(),
                ));
            }
        }
    }
    for opacity in OPACITIES {
        let omarchy = Tokens {
            opacity,
            corners: None,
            radius: 0,
            radius_box: 0,
            ..Tokens::default_theme(true)
        };
        sheets.push((format!("Omarchy, {opacity:?}"), omarchy.css()));
    }
    for (what, css) in sheets {
        let provider = gtk::CssProvider::new();
        let problems = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let seen = std::rc::Rc::clone(&problems);
        provider.connect_parsing_error(move |_, section, error| {
            seen.borrow_mut()
                .push(format!("{error} at {}", section.start_location().lines()));
        });
        provider.load_from_string(&css);
        assert!(
            problems.borrow().is_empty(),
            "{what}: {:?}",
            problems.borrow()
        );
    }
}

// The ground, a card and the preview's frame in each of the three modes, from the pixels GTK
// draws: the ground has the mode's alpha (the preview column paints none of its own), a card a
// denser alpha (its plate over the ground), and the preview's frame is solid in every mode.
fn alphas(app: &gtk::Application) {
    let (window, _) = shown(app);
    for (opacity, alpha) in OPACITIES.into_iter().zip(ALPHAS) {
        window.state(own(opacity, WindowCorners::Rounded));
        WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
        run_for(Duration::from_millis(300));
        let renderer = Renderer::new();
        let w = window.widget();
        let picture = Picture::of(&renderer.0, w);
        let place = |widget: &gtk::Widget| widget.compute_bounds(w).expect("a place");
        // The ground: the preview column's foot, outside the picture and the status items.
        let ground = picture.rgba(20, w.height() - 20);
        assert!(
            near(ground[3], alpha, 0.012),
            "{opacity:?}: the ground's alpha is {} of 255, want {alpha}",
            ground[3]
        );
        // A card, at its top-left corner inside its padding: the plate over the ground.
        let card = place(
            &window
                .names()
                .find("Theme settings")
                .expect("the Theme card"),
        );
        let on_card = picture.rgba(card.x() as i32 + 6, card.y() as i32 + 6);
        let want = 1.0 - (1.0 - alpha) * (1.0 - alpha);
        assert!(
            near(on_card[3], want, 0.012),
            "{opacity:?}: a card's alpha is {} of 255, want {want}",
            on_card[3]
        );
        // The preview's frame is solid in every mode.
        let frame = place(
            window
                .names()
                .find("Cooler display preview")
                .and_then(|p| p.parent())
                .expect("the frame")
                .upcast_ref(),
        );
        let in_frame = picture.rgba(frame.x() as i32 + 3, frame.y() as i32 + 3);
        assert_eq!(
            in_frame[3], 255,
            "{opacity:?}: the preview's frame is solid"
        );
        // The glass rule is the window's class, only in Glassy.
        assert_eq!(
            w.has_css_class("glassy"),
            opacity == WindowOpacity::Glassy,
            "{opacity:?}"
        );
        drop(renderer);
    }
    close(window);
}

// The window's top-left corner: round (transparent) while the own theme is Rounded, square
// (the ground's alpha) while it is Sharp; the class `own-corners` is on in both.
fn corners(app: &gtk::Application) {
    let (window, _) = shown(app);
    for corners in [WindowCorners::Rounded, WindowCorners::Sharp] {
        window.state(own(WindowOpacity::Opaque, corners));
        run_for(Duration::from_millis(300));
        let renderer = Renderer::new();
        let w = window.widget();
        assert!(w.has_css_class("own-corners"), "{corners:?}");
        let picture = Picture::of(&renderer.0, w);
        let corner = picture.rgba(0, 0);
        let inside = picture.rgba(w.width() / 2, w.height() / 2);
        assert_eq!(inside[3], 255);
        match corners {
            WindowCorners::Rounded => {
                assert!(corner[3] < 64, "rounded: the corner pixel is {corner:?}")
            }
            WindowCorners::Sharp => {
                assert_eq!(corner[3], 255, "sharp: the corner pixel is {corner:?}")
            }
        }
        drop(renderer);
    }
    close(window);
}

// With an Omarchy theme followed, the window takes Omarchy's mode in its tokens and leaves the
// corners alone: no `own-corners`, whatever the service's saved settings say, which are not
// applied either (opaque and rounded saved here, glassy in the theme).
fn omarchy(app: &gtk::Application) {
    let (window, _) = shown(app);
    let theme = |opacity| Tokens {
        opacity,
        corners: None,
        radius: 0,
        radius_box: 0,
        ..Tokens::default_theme(true)
    };
    window.omarchy_theme(Some(theme(WindowOpacity::Glassy)));
    window.state(State {
        use_system_theme: Some(true),
        window_opacity: Some(WindowOpacity::Opaque),
        window_corners: Some(WindowCorners::Sharp),
        ..base()
    });
    run_for(Duration::from_millis(300));
    let w = window.widget();
    assert!(w.has_css_class("glassy"), "the Omarchy mode is applied");
    assert!(
        !w.has_css_class("own-corners"),
        "the corners are the compositor's"
    );
    let renderer = Renderer::new();
    let picture = Picture::of(&renderer.0, w);
    let ground = picture.rgba(20, w.height() - 20);
    assert!(near(ground[3], 0.75, 0.012), "ground alpha {}", ground[3]);
    drop(renderer);
    // Its mode changes live, and the saved Sharp is still not applied.
    window.omarchy_theme(Some(theme(WindowOpacity::Opaque)));
    run_for(Duration::from_millis(300));
    assert!(!w.has_css_class("glassy"));
    assert!(!w.has_css_class("own-corners"));
    // Turned off, the own theme takes the saved settings: opaque and sharp.
    window.state(State {
        use_system_theme: Some(false),
        window_opacity: Some(WindowOpacity::Glassy),
        window_corners: Some(WindowCorners::Sharp),
        ..base()
    });
    run_for(Duration::from_millis(300));
    assert!(w.has_css_class("glassy") && w.has_css_class("own-corners"));
    close(window);
}

/// Snowday's card (`surface`) and accent, from the design's palette table.
const SNOWDAY_CARD: Rgb = Rgb([0xf9, 0xfa, 0xfb]);
const SNOWDAY_ACCENT: Rgb = Rgb([0x36, 0x56, 0x75]);

// The own light theme's tiles are cards with no outline at rest, and the chosen tile has a 2 px
// accent frame (its 1 px border and a 1 px inset ring); the window class `own-light` is set only
// for that theme, so the own dark theme and an Omarchy theme keep their outlined wells. The
// pictures of both pages in the light theme are saved for a person to look at
// (`target/tmp/window_style/`).
fn own_light_tiles(app: &gtk::Application) {
    let (window, _) = shown(app);
    let w = window.widget();
    let tile = |mode: u16| {
        window
            .display_page()
            .tiles()
            .iter()
            .find(|t| t.mode.number() == mode)
            .expect("a tile")
            .child
            .clone()
    };
    let place = |widget: &gtk::FlowBoxChild| widget.compute_bounds(w).expect("a place");
    window.state(State {
        dark_theme: Some(false),
        ..own(WindowOpacity::Opaque, WindowCorners::Rounded)
    });
    run_for(Duration::from_millis(300));
    assert!(w.has_css_class("own-light"), "the own light theme");
    let renderer = Renderer::new();
    let picture = Picture::of(&renderer.0, w);
    // A tile at rest (mode 2): the card's colour at its edge, outside any outline, and inside.
    let rest = place(&tile(2));
    let (left, middle) = (rest.x() as i32, (rest.y() + rest.height() / 2.0) as i32);
    assert!(
        colour_near(picture.at(left, middle), SNOWDAY_CARD),
        "a tile's edge is the card's colour, with no outline: {:?}",
        picture.at(left, middle)
    );
    assert!(colour_near(picture.at(left + 6, middle), SNOWDAY_CARD));
    // The chosen tile (mode 1): two accent pixels, then the card's colour inside.
    let chosen = place(&tile(1));
    let (left, middle) = (
        chosen.x() as i32,
        (chosen.y() + chosen.height() / 2.0) as i32,
    );
    for dx in [0, 1] {
        assert!(
            colour_near(picture.at(left + dx, middle), SNOWDAY_ACCENT),
            "the frame's pixel {dx} is the accent: {:?}",
            picture.at(left + dx, middle)
        );
    }
    assert!(
        colour_near(picture.at(left + 3, middle), SNOWDAY_CARD),
        "inside the frame is the card: {:?}",
        picture.at(left + 3, middle)
    );
    let display = save_png(&renderer.0, w, "light-display");
    WidgetExt::activate_action(w, "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(300));
    let configuration = save_png(&renderer.0, w, "light-configuration");
    println!(
        "pictures: {} {}",
        display.display(),
        configuration.display()
    );
    drop(renderer);
    // The own dark theme keeps its outlined wells: no class, and a tile's edge is its outline.
    WidgetExt::activate_action(w, "win.display", None).ok();
    window.state(State {
        dark_theme: Some(true),
        ..own(WindowOpacity::Opaque, WindowCorners::Rounded)
    });
    run_for(Duration::from_millis(300));
    assert!(!w.has_css_class("own-light"), "the own dark theme");
    // An Omarchy theme, light as well, has no class either: its corners are the compositor's.
    window.omarchy_theme(Some(Tokens {
        corners: None,
        radius: 0,
        radius_box: 0,
        ..Tokens::default_theme(false)
    }));
    window.state(State {
        use_system_theme: Some(true),
        ..base()
    });
    run_for(Duration::from_millis(300));
    assert!(!w.has_css_class("own-light"), "a light Omarchy theme");
    close(window);
}
