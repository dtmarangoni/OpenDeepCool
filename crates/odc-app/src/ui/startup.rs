//! The Start at boot switch's widgets (`docs/design/settings-app.md`, Start at boot): the
//! Startup card, with its row, note and hint, and the terminal commands. One card serves both
//! places, moved between them so its names stay one for AT-SPI (Where it sits): the last card of
//! the Configuration page, with its heading and the commands inside it when a note brings them;
//! and the off screen, without its heading, with the commands always under it.

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use super::Request;
use super::names::{Names, group};
use crate::boot::{self, Commands, Place, Style, Switch};

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
    /// Where the commands go inside the card, between the note and the hint.
    inside: gtk::Box,
    terminal: gtk::Box,
    lead: gtk::Label,
    lines: gtk::Label,
    copy: gtk::Button,
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

        let switch = gtk::Switch::builder()
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
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
        left.append(&small(boot::TITLE, &[]));
        left.append(&small(boot::SUBTITLE, &["muted", "small"]));
        line.append(&left);
        line.append(&switch);
        card.append(&line);

        let note_box = group().spacing(6).visible(false).build();
        names.name(&note_box, "Start at boot status");
        let note_icon = gtk::Image::builder().valign(gtk::Align::Start).build();
        let note = small("", &[]);
        note.set_hexpand(true);
        note.set_max_width_chars(NOTE_CHARS);
        note_box.append(&note_icon);
        note_box.append(&note);
        card.append(&note_box);
        let inside = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        card.append(&inside);
        let hint = small(boot::HINT, &["muted", "small"]);
        // Room for a line of about this many characters, so the card stays narrow on the off
        // screen; on the Configuration page it fills the page as the other cards do.
        hint.set_max_width_chars(NOTE_CHARS);
        card.append(&hint);

        let terminal = group()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .visible(false)
            .build();
        names.name(&terminal, "Terminal commands");
        let lead = small("", &["muted", "small"]);
        let lines = gtk::Label::builder()
            .selectable(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["command"])
            .build();
        let copy_content = gtk::Box::builder().spacing(6).build();
        copy_content.append(&gtk::Image::from_icon_name("edit-copy-symbolic"));
        copy_content.append(&gtk::Label::new(Some("Copy")));
        let copy = gtk::Button::builder()
            .child(&copy_content)
            .valign(gtk::Align::Start)
            .build();
        names.name(&copy, "Copy terminal commands");
        let shown = lines.clone();
        copy.connect_clicked(move |button| button.clipboard().set_text(&shown.text()));
        let commands = gtk::Box::builder().spacing(8).build();
        commands.append(&lines);
        commands.append(&copy);
        terminal.append(&lead);
        terminal.append(&commands);
        inside.append(&terminal);

        StartupCard {
            card,
            heading,
            block: left,
            in_group: Cell::new(false),
            switch,
            note_box,
            note_icon,
            note,
            inside,
            terminal,
            lead,
            lines,
            copy,
            applying,
        }
    }

    /// Puts the card at `place`: at the end of `parent`, and the commands inside it, or after it
    /// in `parent` on the off screen. On the page its label block joins `labels`, the page's
    /// size group, so the switch starts where the other controls do; on the off screen, where
    /// the card is alone, it leaves the group and takes only its own width (Rows).
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
        match place {
            Place::Card => move_to(&self.terminal, &self.inside),
            Place::OffScreen => move_to(&self.terminal, parent),
        }
        self.heading.set_visible(place == Place::Card);
        self.tidy();
    }

    /// The slot inside the card takes no room while it holds nothing to show: the commands' own
    /// `visible`, not `is_visible`, which also asks the slot itself.
    fn tidy(&self) {
        self.inside
            .set_visible(self.commands_inside() && self.terminal.get_visible());
    }

    /// Hides the card alone: the off screen in a session that may not flip it keeps the commands
    /// (Who may flip it).
    pub fn hide_card(&self) {
        self.card.set_visible(false);
    }

    /// Hides the card and the commands: the off screen before the first read.
    pub fn hide_all(&self) {
        self.card.set_visible(false);
        self.terminal.set_visible(false);
        self.tidy();
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
                    Style::Plain => (None, None),
                    Style::Info => (Some("dialog-information-symbolic"), Some("muted")),
                    Style::Warning => (Some("dialog-warning-symbolic"), Some("warn-text")),
                };
                self.note_icon.set_icon_name(icon);
                self.note_icon.set_visible(icon.is_some());
                let classes: Vec<&str> = class.into_iter().collect();
                self.note.set_css_classes(&classes);
                self.note_icon.set_css_classes(&classes);
                self.note_box.set_visible(true);
            }
            None => self.note_box.set_visible(false),
        }
        names.describe(
            &self.switch,
            switch.note.as_ref().map(|note| note.text.as_str()),
        );
        self.show_commands(names, switch.commands);
    }

    fn show_commands(&self, names: &Names, commands: Option<Commands>) {
        let Some(commands) = commands else {
            self.terminal.set_visible(false);
            names.describe(&self.terminal, None);
            self.tidy();
            return;
        };
        let text = commands.lines.join("\n");
        self.lead.set_text(commands.lead);
        self.lines.set_text(&text);
        self.terminal.set_visible(true);
        self.tidy();
        names.describe(
            &self.terminal,
            Some(&format!("{} {}", commands.lead, commands.lines.join(" "))),
        );
    }

    pub fn switch(&self) -> &gtk::Switch {
        &self.switch
    }

    pub fn card(&self) -> &gtk::Box {
        &self.card
    }

    pub fn copy(&self) -> &gtk::Button {
        &self.copy
    }

    /// The note's words, while it shows.
    pub fn note(&self) -> Option<String> {
        self.note_box
            .is_visible()
            .then(|| self.note.text().to_string())
    }

    /// The commands' lead-in and lines, while they show.
    pub fn commands(&self) -> Option<(String, String)> {
        self.terminal
            .is_visible()
            .then(|| (self.lead.text().to_string(), self.lines.text().to_string()))
    }

    /// Whether the commands sit inside the card.
    pub fn commands_inside(&self) -> bool {
        self.terminal.parent().as_ref() == Some(self.inside.upcast_ref())
    }

    /// Whether the card's heading shows.
    pub fn heading_shown(&self) -> bool {
        self.heading.is_visible()
    }
}
