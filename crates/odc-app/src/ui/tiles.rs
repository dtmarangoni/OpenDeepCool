//! The tiles (`docs/design/settings-app.md`, Display page): one per mode, a 63 px picture and
//! its caption. The picture comes from the service's own drawing code, `odc_render`, with the
//! design's fixed sample values, upright, in the current unit and with the current peak
//! setting; it redraws only when one of those, or the theme, changes.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::accessible::Property;
use gtk::gio;
use gtk::prelude::*;
use odc_protocol::Frame;
use odc_render::{
    COLUMNS, EmojiLoop, Mode, Pose, Spectrum, Unit, Values, mode_info, spectrum_height,
    with_spectrum,
};
use odc_sensors::{Exact, Metric};

use super::dots;
use super::names::Names;
use crate::model::{Flag, Tile, group_name, option};
use crate::theme::{Rgb, Rgba, Tokens, UNLIT_ALPHA};

/// The tile picture's pitch, px per dot: 14 × 4.5 = 63 px, as the mockups draw it.
pub const PITCH: f64 = 4.5;
pub const SIDE: i32 = 63;

/// The design's sample values (Display page, Tile content).
pub fn sample_values() -> Values {
    [
        (Metric::CpuUsage, 42),
        (Metric::GpuUsage, 63),
        (Metric::MemoryLoad, 51),
        (Metric::CpuTemperature, 57),
        (Metric::GpuTemperature, 38),
        (Metric::CpuPower, 65),
        (Metric::GpuPower, 88),
    ]
    .into_iter()
    .fold(Values::new(), |values, (metric, n)| {
        values.with(metric, Exact::whole(n))
    })
}

/// The fixed sample spectrum's bars and peaks, the mockups' (`app.js`, `SAMPLE_SPECTRUM`).
pub const SAMPLE_BARS: [f64; COLUMNS] = [
    0.55, 0.8, 0.95, 0.7, 0.6, 0.5, 0.62, 0.45, 0.38, 0.42, 0.3, 0.22, 0.15, 0.08,
];
pub const SAMPLE_PEAKS: [f64; COLUMNS] = [
    0.7, 0.9, 1.0, 0.85, 0.7, 0.62, 0.7, 0.6, 0.5, 0.5, 0.42, 0.3, 0.22, 0.12,
];

/// The picture of `mode`'s tile: a value's home frame, an emoji loop's first picture or the
/// Dancer standing, with the sample spectrum below where the mode has one.
pub fn picture(mode: Mode, unit: Unit, peaks: bool) -> Frame {
    if mode.number() == 23 {
        return Pose::STILL.frame();
    }
    let top = match mode.number() {
        13 => Frame::blank(),
        14..=17 => EmojiLoop::of(mode).map_or(Frame::blank(), |mut emoji| emoji.tick()),
        _ => match odc_render::picture(mode, &sample_values(), unit) {
            Ok(picture) => picture.frame_at(picture.home()),
            // Every mode is drawn; one that weren't would show no value.
            Err(_) => Frame::blank(),
        },
    };
    match spectrum_height(mode) {
        Some(h) => {
            let columns = Spectrum::at(SAMPLE_BARS, SAMPLE_PEAKS).draw(h, peaks);
            with_spectrum(&top, &columns, h)
        }
        None => top,
    }
}

/// Whether `mode`'s picture follows the unit: it shows a temperature.
pub fn follows_unit(mode: Mode) -> bool {
    mode_info(mode)
        .metrics
        .iter()
        .any(|m| matches!(m, Metric::CpuTemperature | Metric::GpuTemperature))
}

struct Drawn {
    frame: Frame,
    lit: Rgb,
    unlit: Rgba,
}

/// One tile's widgets.
pub struct TileWidget {
    pub mode: Mode,
    pub child: gtk::FlowBoxChild,
    area: gtk::DrawingArea,
    flag: gtk::Label,
    drawn: Rc<RefCell<Drawn>>,
    draws: Rc<Cell<u32>>,
    /// The colours: the theme's foreground, or muted on an insensitive tile.
    fg: Cell<Rgb>,
    muted: Cell<Rgb>,
}

impl TileWidget {
    /// `mode`'s tile, named `<group>: <option>` and captioned with its option.
    pub fn new(names: &Names, mode: Mode, tokens: &Tokens) -> TileWidget {
        let caption_text = option(mode);
        let label = format!("{}: {caption_text}", group_name(mode_info(mode).kind));
        let area = gtk::DrawingArea::builder()
            .accessible_role(gtk::AccessibleRole::Img)
            .content_width(SIDE)
            .content_height(SIDE)
            .halign(gtk::Align::Center)
            .build();
        // The picture is an image named as its tile (Unverified points, answered: roles).
        area.update_property(&[Property::Label(&label)]);
        let drawn = Rc::new(RefCell::new(Drawn {
            frame: Frame::blank(),
            lit: tokens.fg,
            unlit: Rgba {
                rgb: tokens.fg,
                alpha: UNLIT_ALPHA,
            },
        }));
        let draws = Rc::new(Cell::new(0));
        let (state, count) = (Rc::clone(&drawn), Rc::clone(&draws));
        area.set_draw_func(move |_, cr, _, _| {
            count.set(count.get() + 1);
            let state = state.borrow();
            dots::draw(cr, Some(&state.frame), PITCH, state.lit, state.unlit);
        });
        let caption = gtk::Label::builder().label(&caption_text).build();
        let flag = gtk::Label::builder()
            .visible(false)
            .css_classes(["flag", "small"])
            .build();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .build();
        content.append(&area);
        content.append(&caption);
        content.append(&flag);
        let child = gtk::FlowBoxChild::builder()
            .child(&content)
            .css_classes(["tile"])
            .build();
        names.name(&child, &label);
        // `tile.activate`, as Enter, Space or a click: GTK gives a flow box child no AT-SPI
        // action of its own, but offers assistive technology a widget's enabled actions, and
        // only while it's sensitive (decisions log D67).
        let activate = gio::SimpleAction::new("activate", None);
        let weak = child.downgrade();
        activate.connect_activate(move |_, _| {
            if let Some(child) = weak.upgrade()
                && child.is_sensitive()
            {
                child.activate();
            }
        });
        let actions = gio::SimpleActionGroup::new();
        actions.add_action(&activate);
        child.insert_action_group("tile", Some(&actions));
        TileWidget {
            mode,
            child,
            area,
            flag,
            drawn,
            draws,
            fg: Cell::new(tokens.fg),
            muted: Cell::new(tokens.muted),
        }
    }

    /// Shows `tile`'s state: selected, sensitive, its flag and its description.
    pub fn show(&self, names: &Names, tile: &Tile) {
        if tile.selected {
            self.child.add_css_class("selected");
        } else {
            self.child.remove_css_class("selected");
        }
        self.child
            .update_state(&[gtk::accessible::State::Selected(Some(tile.selected))]);
        if self.child.is_sensitive() != tile.sensitive {
            self.child.set_sensitive(tile.sensitive);
            self.recolour();
        }
        match tile.flag {
            Some(flag) => {
                self.flag.set_text(flag.text());
                self.flag.set_css_classes(&[
                    "flag",
                    "small",
                    if flag == Flag::ReadingLost {
                        "warn-text"
                    } else {
                        "muted"
                    },
                ]);
                self.flag.set_visible(true);
            }
            None => self.flag.set_visible(false),
        }
        if tile.flag == Some(Flag::ReadingLost) {
            self.child.add_css_class("lost");
        } else {
            self.child.remove_css_class("lost");
        }
        names.describe(&self.child, tile.description.as_deref());
        self.child.set_tooltip_text(tile.description.as_deref());
    }

    /// Shows `frame`; redraws only when it differs.
    pub fn set_picture(&self, frame: Frame) {
        let mut drawn = self.drawn.borrow_mut();
        if drawn.frame != frame {
            drawn.frame = frame;
            self.area.queue_draw();
        }
    }

    pub fn set_theme(&self, tokens: &Tokens) {
        self.fg.set(tokens.fg);
        self.muted.set(tokens.muted);
        self.recolour();
    }

    /// The lit colour for the tile's sensitivity, and a redraw.
    fn recolour(&self) {
        let mut drawn = self.drawn.borrow_mut();
        let fg = self.fg.get();
        drawn.lit = if self.child.is_sensitive() {
            fg
        } else {
            self.muted.get()
        };
        drawn.unlit = Rgba {
            rgb: fg,
            alpha: UNLIT_ALPHA,
        };
        self.area.queue_draw();
    }

    pub fn picture_widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub fn frame(&self) -> Frame {
        self.drawn.borrow().frame
    }

    /// How often the picture has drawn, for the tests.
    pub fn draws(&self) -> u32 {
        self.draws.get()
    }

    /// The flag's words, while it shows.
    pub fn flag(&self) -> Option<String> {
        self.flag.is_visible().then(|| self.flag.text().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use odc_render::Picture;

    // Each tile shows its mode's picture from the service's drawing code: a value's home frame
    // with the sample values, the first emoji picture, the Dancer standing, and the spectrum
    // below in the modes that have one.
    #[test]
    fn each_tile_draws_its_mode_with_the_sample_values() {
        let home = |mode: Mode, unit| {
            let p: Picture = odc_render::picture(mode, &sample_values(), unit).expect("drawn");
            p.frame_at(p.home())
        };
        for mode in Mode::all() {
            for unit in [Unit::Celsius, Unit::Fahrenheit] {
                for peaks in [true, false] {
                    let got = picture(mode, unit, peaks);
                    let spectrum = |top: &Frame, h| {
                        with_spectrum(
                            top,
                            &Spectrum::at(SAMPLE_BARS, SAMPLE_PEAKS).draw(h, peaks),
                            h,
                        )
                    };
                    let want = match mode.number() {
                        1..=12 => home(mode, unit),
                        13 => spectrum(&Frame::blank(), 12),
                        14..=17 => spectrum(&EmojiLoop::of(mode).expect("emoji").tick(), 6),
                        18..=22 => spectrum(&home(mode, unit), 6),
                        _ => Pose::STILL.frame(),
                    };
                    assert_eq!(got, want, "mode {}", mode.number());
                }
            }
        }
    }

    // Only the temperature tiles follow the unit, and only the spectrum tiles the peak dots.
    #[test]
    fn only_temperatures_follow_the_unit_and_only_spectra_the_peaks() {
        for mode in Mode::all() {
            let unit_matters =
                picture(mode, Unit::Celsius, true) != picture(mode, Unit::Fahrenheit, true);
            assert_eq!(unit_matters, follows_unit(mode), "mode {}", mode.number());
            let peaks_matter =
                picture(mode, Unit::Celsius, true) != picture(mode, Unit::Celsius, false);
            assert_eq!(
                peaks_matter,
                spectrum_height(mode).is_some(),
                "mode {}",
                mode.number()
            );
        }
        assert_eq!(
            Mode::all()
                .filter(|m| follows_unit(*m))
                .map(Mode::number)
                .collect::<Vec<_>>(),
            [7, 8, 9, 18, 19]
        );
    }
}
