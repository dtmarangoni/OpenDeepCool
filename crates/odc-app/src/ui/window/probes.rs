//! The window's parts and state as the tests read them: the view shown last, the widgets and the
//! things a person would see. A part of [`Window`](super::Window), split from `window.rs`.

use gtk::prelude::*;

use super::Window;
use crate::model::View;
use crate::ui::banners::Banners;
use crate::ui::config_page::ConfigPage;
use crate::ui::display_page::DisplayPage;
use crate::ui::names::Names;
use crate::ui::preview::Preview;
use crate::ui::startup::StartupCard;

impl Window {
    /// The view shown last, for the tests.
    pub fn shown(&self) -> Option<View> {
        self.0.shown.borrow().clone()
    }

    pub fn names(&self) -> &Names {
        &self.0.names
    }

    pub fn preview(&self) -> &Preview {
        &self.0.preview
    }

    pub fn banners(&self) -> &Banners {
        &self.0.banners
    }

    pub fn display_page(&self) -> &DisplayPage {
        &self.0.display
    }

    pub fn config_page(&self) -> &ConfigPage {
        &self.0.config
    }

    pub fn startup(&self) -> &StartupCard {
        &self.0.startup
    }

    /// The off screen's heading, while it shows.
    pub fn off_heading(&self) -> Option<String> {
        (self.screen().1 == "off" && self.0.off_message.is_visible())
            .then(|| self.0.off_heading.text().to_string())
    }

    /// Whether the Startup card sits on the off screen.
    pub fn startup_on_off_screen(&self) -> bool {
        self.0.startup.card().parent().as_ref() == Some(self.0.off.upcast_ref())
    }

    /// The feedback label's words, while it shows.
    pub fn feedback(&self) -> Option<String> {
        self.0
            .feedback
            .is_visible()
            .then(|| self.0.feedback_label.text().to_string())
    }

    /// The screen shown: `main` or `not-available`; and the page: `display`, `config` or `off`.
    pub fn screen(&self) -> (String, String) {
        let name = |stack: &gtk::Stack| {
            stack
                .visible_child_name()
                .map(|n| n.to_string())
                .unwrap_or_default()
        };
        (name(&self.0.screens), name(&self.0.pages))
    }

    /// Whether the gear and the close icon show, and whether they can be used.
    pub fn header(&self) -> [(bool, bool); 2] {
        let h = &self.0.header;
        [
            (h.gear.is_visible(), h.gear.is_sensitive()),
            (h.close.is_visible(), h.close.is_sensitive()),
        ]
    }

    /// Whether the gear is pressed: the Configuration page shows.
    pub fn gear_pressed(&self) -> bool {
        self.0.header.gear.is_active()
    }

    /// How far the title's centre is from the header bar's, in pixels, once the bar has its
    /// size (Window: the title stays in the middle whatever sits at the end).
    pub fn title_offset(&self) -> Option<f32> {
        let h = &self.0.header;
        let bounds = h.title.compute_bounds(&h.bar)?;
        Some(bounds.x() + bounds.width() / 2.0 - h.bar.width() as f32 / 2.0)
    }

    /// The window's scrolled window (the cap and the sideways scroll) and the preview column's,
    /// for the tests (Size and scrolling).
    pub fn scrollers(&self) -> (&gtk::ScrolledWindow, &gtk::ScrolledWindow) {
        (&self.0.window_scroller, &self.0.column_scroller)
    }

    /// The header bar's height, for the tests.
    pub fn header_height(&self) -> i32 {
        self.0.header.bar.height()
    }

    /// The gear, the back icon and Refresh, for the tests that press or focus them.
    pub fn gear(&self) -> &gtk::ToggleButton {
        &self.0.header.gear
    }

    /// The column's words: caption, reason and the status items shown.
    pub fn column_words(&self) -> Vec<String> {
        let c = &self.0.column;
        let mut words = vec![c.caption.text().to_string()];
        if c.reason_box.is_visible() {
            words.push(c.reason.text().to_string());
        }
        for row in c.status.iter().filter(|r| r.row.is_visible()) {
            words.push(row.label.text().to_string());
        }
        words
    }

    /// The not-available message box, for the tests.
    pub fn not_available(&self) -> &gtk::Box {
        &self.0.not_available
    }

    /// Whether the Configuration, Back and Refresh actions can run.
    pub fn actions_enabled(&self) -> [bool; 3] {
        let [a, b, c] = &self.0.actions;
        [a.is_enabled(), b.is_enabled(), c.is_enabled()]
    }
}
