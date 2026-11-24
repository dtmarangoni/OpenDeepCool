//! The tiles (`docs/design/settings-app.md`, Display page): one per mode, a 63 px picture and
//! its caption. The picture comes from the service's own drawing code, `odc_render`, from the
//! service's readings and drawn by `live::Live` (Phase 11a Task 14); the widget draws the frame
//! it is given, and only when that differs from the last.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::accessible::Property;
use gtk::gio;
use gtk::prelude::*;
use odc_protocol::Frame;
use odc_render::{Mode, mode_info};

use super::dots;
use super::names::Names;
use crate::model::{Tile, group_name, option};
use crate::theme::{Rgb, Rgba, Tokens, UNLIT_ALPHA};

/// The tile picture's pitch, px per dot: 14 × 4.5 = 63 px, as the mockups draw it.
pub const PITCH: f64 = 4.5;
pub const SIDE: i32 = 63;

/// The most characters a tile's caption asks room for before it wraps: the widest one-line
/// caption, "CPU temperature", fits.
const CAPTION_CHARS: i32 = 16;

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
    /// The colours: the theme's foreground, or the disabled colour on an insensitive tile.
    fg: Cell<Rgb>,
    disabled: Cell<Rgb>,
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
        // A caption wraps at the width of the longest one that fits (the Music tiles' "Coffee
        // break + Spectrum" is two lines), so the tile stays as wide as before and its words are
        // never cut.
        let caption = gtk::Label::builder()
            .label(&caption_text)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::Word)
            .max_width_chars(CAPTION_CHARS)
            .justify(gtk::Justification::Center)
            .build();
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
            disabled: Cell::new(tokens.disabled()),
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
                // Plain text in the tile's own colour, even for a lost reading: warning-coloured
                // small text on the selected fill misses 4.5:1 in the light theme (design: Design
                // review, Task 15). The warning colour stays on the reason under the preview.
                self.flag.set_css_classes(&["flag", "small"]);
                self.flag.set_visible(true);
            }
            None => self.flag.set_visible(false),
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
        self.disabled.set(tokens.disabled());
        self.recolour();
    }

    /// The lit colour for the tile's sensitivity, and a redraw.
    fn recolour(&self) {
        let mut drawn = self.drawn.borrow_mut();
        let fg = self.fg.get();
        drawn.lit = if self.child.is_sensitive() {
            fg
        } else {
            self.disabled.get()
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
    use crate::live::{Live, sample_values};
    use odc_render::{MusicSettings, Unit, spectrum_height};

    // Only the temperature tiles follow the unit, and only the spectrum tiles the peak dots.
    // (`tests/live.rs` checks every tile's picture against `odc_render`'s.)
    #[test]
    fn only_temperatures_follow_the_unit_and_only_spectra_the_peaks() {
        let picture = |mode: Mode, unit: Unit, peaks: bool| {
            let music = MusicSettings {
                peaks,
                ..MusicSettings::default()
            };
            *Live::new(unit, music, sample_values()).frame(mode)
        };
        for mode in Mode::all() {
            let unit_matters =
                picture(mode, Unit::Celsius, true) != picture(mode, Unit::Fahrenheit, true);
            assert_eq!(
                unit_matters,
                [7, 8, 9, 18, 19].contains(&mode.number()),
                "mode {}",
                mode.number()
            );
            let peaks_matter =
                picture(mode, Unit::Celsius, true) != picture(mode, Unit::Celsius, false);
            assert_eq!(
                peaks_matter,
                spectrum_height(mode).is_some(),
                "mode {}",
                mode.number()
            );
        }
    }
}
