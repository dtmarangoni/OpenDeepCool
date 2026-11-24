//! The Start at boot switch's widgets (`docs/design/settings-app.md`, Start at boot): the
//! Startup card, with its row and note. One card serves both places, moved between them so its
//! names stay one for AT-SPI (Where it sits): the last card of the Configuration page, with its
//! heading; and the off screen, without its heading. The app has no terminal commands: they are
//! in the README (owner, 2026-11-19).

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use super::Request;
use super::names::{Names, group};
use super::pointer;
use crate::boot::{self, Place, Style, Switch};

/// The most characters the card's lines ask room for.
const NOTE_CHARS: i32 = 72;

pub struct StartupCard {
    card: gtk::Box,
    heading: gtk::Label,
    /// The row's label block: in the Configuration page's size group while the card is on the
    /// page, so its switch starts where the other controls do (Rows).
    block: gtk::Box,
    in_group: Cell<bool>,
    switch: gtk::Switch,
    note_box: gtk::Box,
    note_icon: gtk::Image,
    note: gtk::Label,
    /// The app sets the switch itself: its `state-set` is ignored meanwhile.
    applying: Rc<Cell<bool>>,
}

fn small(text: &str, classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(classes)
        .build()
}

/// Moves `widget` into `parent`, at its end, unless it's there already.
fn move_to(widget: &impl IsA<gtk::Widget>, parent: &gtk::Box) {
    let widget = widget.as_ref();
    if widget.parent().as_ref() == Some(parent.upcast_ref()) {
        return;
    }
    if let Some(old) = widget.parent().and_downcast::<gtk::Box>() {
        old.remove(widget);
    }
    parent.append(widget);
}

impl StartupCard {
    pub fn new(names: &Names, send: Rc<dyn Fn(Request)>) -> StartupCard {
        let card = gtk::Box::builder()
            .accessible_role(gtk::AccessibleRole::Group)
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .css_classes(["card"])
            .build();
        names.name(&card, "Startup settings");
        let heading = small("Startup", &["card-heading"]);
        card.append(&heading);

        // Level with the top of the title, as every row's control is (owner, 2026-11-22).
        let switch = gtk::Switch::builder()
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .build();
        names.name(&switch, boot::TITLE);
        let applying = Rc::new(Cell::new(false));
        let a = Rc::clone(&applying);
        // The trough keeps systemd's state until the flip ends (GtkSwitch's delayed state: the
        // handler returns Stop, so only the handle moves).
        switch.connect_state_set(move |_, on| {
            if !a.get() {
                send(Request::StartAtBoot(on));
            }
            glib::Propagation::Stop
        });
        let line = gtk::Box::builder().spacing(24).build();
        let left = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .build();
        left.append(&small(boot::TITLE, &["row-title"]));
        line.append(&left);
        line.append(&switch);
        card.append(&line);

        let note_box = group().spacing(6).visible(false).build();
        names.name(&note_box, "Start at boot status");
        let note_icon = gtk::Image::builder().valign(gtk::Align::Start).build();
        let note = small("", &["small"]);
        note.set_hexpand(true);
        note.set_max_width_chars(NOTE_CHARS);
        note_box.append(&note_icon);
        note_box.append(&note);
        card.append(&note_box);
        // The card moves between the page and the off screen, so it is calmed on its own (the
        // window's pass at its build may not reach it): a click on its switch focuses nothing.
        pointer::calm(&card);
        StartupCard {
            card,
            heading,
            block: left,
            in_group: Cell::new(false),
            switch,
            note_box,
            note_icon,
            note,
            applying,
        }
    }

    /// Puts the card at `place`: at the end of `parent`. On the page its label block joins
    /// `labels`, the page's size group, so the switch starts where the other controls do; on the
    /// off screen, where the card is alone, it leaves the group and takes only its own width
    /// (Rows).
    pub fn place(&self, place: Place, parent: &gtk::Box, labels: &gtk::SizeGroup) {
        move_to(&self.card, parent);
        match (place, self.in_group.get()) {
            (Place::Card, false) => {
                labels.add_widget(&self.block);
                self.in_group.set(true);
            }
            (Place::OffScreen, true) => {
                labels.remove_widget(&self.block);
                self.in_group.set(false);
            }
            _ => {}
        }
        self.heading.set_visible(place == Place::Card);
    }

    /// Hides the card: the off screen in a session that may not flip it, and before the first
    /// read (Who may flip it).
    pub fn hide_card(&self) {
        self.card.set_visible(false);
    }

    /// Shows `switch`, naming its note in its description.
    pub fn show(&self, names: &Names, switch: &Switch) {
        self.card.set_visible(true);
        self.applying.set(true);
        self.switch.set_active(switch.handle);
        self.switch.set_state(switch.trough);
        self.applying.set(false);
        self.switch.set_sensitive(switch.sensitive);
        match &switch.note {
            Some(note) => {
                self.note.set_text(&note.text);
                let (icon, class) = match note.style {
                    Style::Info => ("dialog-information-symbolic", "muted"),
                    Style::Warning => ("dialog-warning-symbolic", "warn-text"),
                };
                self.note_icon.set_icon_name(Some(icon));
                self.note_icon.set_visible(true);
                self.note.set_css_classes(&["small", class]);
                self.note_icon.set_css_classes(&[class]);
                self.note_box.set_visible(true);
            }
            None => self.note_box.set_visible(false),
        }
        names.describe(
            &self.switch,
            switch.note.as_ref().map(|note| note.text.as_str()),
        );
    }

    pub fn switch(&self) -> &gtk::Switch {
        &self.switch
    }

    pub fn card(&self) -> &gtk::Box {
        &self.card
    }

    /// The note's words, while it shows.
    pub fn note(&self) -> Option<String> {
        self.note_box
            .is_visible()
            .then(|| self.note.text().to_string())
    }

    /// Whether the card's heading shows.
    pub fn heading_shown(&self) -> bool {
        self.heading.is_visible()
    }
}
