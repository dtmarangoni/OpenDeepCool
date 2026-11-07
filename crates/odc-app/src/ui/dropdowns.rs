//! The Configuration page's dropdowns (`docs/design/settings-app.md`, Configuration page): a
//! `GtkDropDown` over a `GtkStringList` of the entries' labels. Its button shows the view's own
//! words; its open list shows a check mark, the label and a muted note at the right (an RPM, or
//! a GPU's note), each item named by its label and described by its note. No item type of the
//! app's own: the workspace forbids the `unsafe` that GObject subclassing generates, so the rows
//! look their entry up by position.
//!
//! The dropdown shows a choice only once the service confirms it: a user's pick is handed to
//! [`Dropdown::connect_picked`], and the confirmed entry is shown again until the next
//! [`Dropdown::show`]. The notes of an open list update in place (Cooler: live RPM).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

/// One entry: its label, and the note at the right of the open list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub label: String,
    pub note: String,
}

/// A row of the open list, while it's bound.
struct Row {
    item: gtk::ListItem,
    check: gtk::Image,
    label: gtk::Label,
    note: gtk::Label,
}

/// Where a user's pick goes.
type Picked = Box<dyn Fn(u32)>;

#[derive(Default)]
struct Shared {
    entries: RefCell<Vec<Entry>>,
    /// The confirmed entry, or none.
    chosen: Cell<Option<u32>>,
    button_text: RefCell<String>,
    buttons: RefCell<Vec<gtk::Label>>,
    rows: RefCell<Vec<Row>>,
    /// The dropdown is being set from the view: no pick.
    applying: Cell<bool>,
    picked: RefCell<Option<Picked>>,
}

impl Shared {
    fn bind(&self, row: &Row) {
        let position = row.item.position();
        let entries = self.entries.borrow();
        let Some(entry) = usize::try_from(position).ok().and_then(|p| entries.get(p)) else {
            return;
        };
        row.label.set_text(&entry.label);
        row.note.set_text(&entry.note);
        // Hidden by opacity, so every label lines up whether its row is chosen or not.
        row.check
            .set_opacity(if self.chosen.get() == Some(position) {
                1.0
            } else {
                0.0
            });
        row.item.set_accessible_label(&entry.label);
        row.item.set_accessible_description(&entry.note);
    }
}

#[derive(Clone)]
pub struct Dropdown {
    widget: gtk::DropDown,
    model: gtk::StringList,
    shared: Rc<Shared>,
}

impl Dropdown {
    pub fn new() -> Dropdown {
        let model = gtk::StringList::new(&[]);
        let shared = Rc::new(Shared::default());
        let widget = gtk::DropDown::builder().model(&model).build();

        // The button: the view's own words.
        let button = gtk::SignalListItemFactory::new();
        let s = Rc::clone(&shared);
        button.connect_setup(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let label = gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            label.set_text(&s.button_text.borrow());
            item.set_child(Some(&label));
            s.buttons.borrow_mut().push(label);
        });
        let s = Rc::clone(&shared);
        button.connect_teardown(move |_, item| {
            if let Some(child) = item
                .downcast_ref::<gtk::ListItem>()
                .and_then(ListItemExt::child)
            {
                s.buttons
                    .borrow_mut()
                    .retain(|l| *l.upcast_ref::<gtk::Widget>() != child);
            }
        });
        widget.set_factory(Some(&button));

        // The open list: a check mark, the label, the note.
        let list = gtk::SignalListItemFactory::new();
        list.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let row = gtk::Box::builder().spacing(8).build();
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            let label = gtk::Label::builder().xalign(0.0).hexpand(true).build();
            let note = gtk::Label::builder()
                .xalign(1.0)
                .css_classes(["muted", "num"])
                .build();
            row.append(&check);
            row.append(&label);
            row.append(&note);
            // `row.activate` picks the row as a click does, through GTK's own actions: the
            // row's `listitem.select`, then the list's `list.activate-item`, which the dropdown
            // takes from the list's selection (`row_activated`, GTK's source). GTK gives an open
            // row no AT-SPI action that picks it: its actions take parameters, and the list's
            // AT-SPI selection only moves the highlight (decisions log D69). On the row's own
            // box, since GTK's row widget breaks when it's given actions while bound.
            let activate = gtk::gio::SimpleAction::new("activate", None);
            let (weak_item, weak_row) = (item.downgrade(), row.downgrade());
            activate.connect_activate(move |_, _| {
                if let (Some(item), Some(row)) = (weak_item.upgrade(), weak_row.upgrade()) {
                    let picked = row
                        .activate_action("listitem.select", Some(&(false, false).to_variant()))
                        .and_then(|()| {
                            row.activate_action(
                                "list.activate-item",
                                Some(&item.position().to_variant()),
                            )
                        });
                    if let Err(e) = picked {
                        eprintln!("opendeepcool: a list row's activation: {e}");
                    }
                }
            });
            let actions = gtk::gio::SimpleActionGroup::new();
            actions.add_action(&activate);
            row.insert_action_group("row", Some(&actions));
            item.set_child(Some(&row));
        });
        let s = Rc::clone(&shared);
        list.connect_bind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(row) = item.child().and_then(|c| c.downcast::<gtk::Box>().ok()) else {
                return;
            };
            let parts: Vec<gtk::Widget> =
                std::iter::successors(row.first_child(), |w| w.next_sibling()).collect();
            let [check, label, note] = parts.as_slice() else {
                return;
            };
            let (Ok(check), Ok(label), Ok(note)) = (
                check.clone().downcast::<gtk::Image>(),
                label.clone().downcast::<gtk::Label>(),
                note.clone().downcast::<gtk::Label>(),
            ) else {
                return;
            };
            let row = Row {
                item: item.clone(),
                check,
                label,
                note,
            };
            s.bind(&row);
            s.rows.borrow_mut().push(row);
        });
        let s = Rc::clone(&shared);
        list.connect_unbind(move |_, item| {
            s.rows.borrow_mut().retain(|r| r.item != *item);
        });
        widget.set_list_factory(Some(&list));

        let s = Rc::clone(&shared);
        widget.connect_selected_notify(move |widget| {
            if s.applying.get() {
                return;
            }
            let position = widget.selected();
            if position != gtk::INVALID_LIST_POSITION
                && Some(position) != s.chosen.get()
                && let Some(picked) = s.picked.borrow().as_ref()
            {
                picked(position);
            }
            // Back to the confirmed entry until the service confirms the pick, once GTK is done
            // with this one.
            let (s, widget) = (Rc::clone(&s), widget.clone());
            glib::idle_add_local_once(move || {
                s.applying.set(true);
                widget.set_selected(s.chosen.get().unwrap_or(gtk::INVALID_LIST_POSITION));
                s.applying.set(false);
            });
        });
        Dropdown {
            widget,
            model,
            shared,
        }
    }

    pub fn widget(&self) -> &gtk::DropDown {
        &self.widget
    }

    /// `picked` gets the position the user picks, when it isn't the confirmed one.
    pub fn connect_picked(&self, picked: impl Fn(u32) + 'static) {
        *self.shared.picked.borrow_mut() = Some(Box::new(picked));
    }

    /// Shows `entries`, `chosen` confirmed, and `button` on the button. The same labels keep
    /// their rows, and an open list's notes change in place.
    pub fn show(&self, entries: Vec<Entry>, chosen: Option<u32>, button: &str) {
        let s = &self.shared;
        s.applying.set(true);
        let same = {
            let old = s.entries.borrow();
            old.len() == entries.len() && old.iter().zip(&entries).all(|(a, b)| a.label == b.label)
        };
        *s.entries.borrow_mut() = entries;
        s.chosen.set(chosen);
        if !same {
            let labels: Vec<String> = s.entries.borrow().iter().map(|e| e.label.clone()).collect();
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            self.model.splice(0, self.model.n_items(), &labels);
        }
        let wanted = chosen.unwrap_or(gtk::INVALID_LIST_POSITION);
        if self.widget.selected() != wanted {
            self.widget.set_selected(wanted);
        }
        *s.button_text.borrow_mut() = button.to_owned();
        for label in s.buttons.borrow().iter() {
            label.set_text(button);
        }
        for row in s.rows.borrow().iter() {
            s.bind(row);
        }
        s.applying.set(false);
    }

    /// The entries shown.
    pub fn entries(&self) -> Vec<Entry> {
        self.shared.entries.borrow().clone()
    }

    /// The confirmed entry.
    pub fn chosen(&self) -> Option<u32> {
        self.shared.chosen.get()
    }

    /// The button's words.
    pub fn button_text(&self) -> String {
        self.shared.button_text.borrow().clone()
    }

    /// The open list's bound rows: position, label, note, and whether its check shows.
    pub fn rows(&self) -> Vec<(u32, String, String, bool)> {
        let mut rows: Vec<_> = self
            .shared
            .rows
            .borrow()
            .iter()
            .map(|r| {
                (
                    r.item.position(),
                    r.label.text().to_string(),
                    r.note.text().to_string(),
                    r.check.opacity() > 0.5,
                )
            })
            .collect();
        rows.sort();
        rows
    }

    /// The open list's row at `position`, as assistive technology picks it: its `row.activate`.
    /// False when no bound row is there.
    pub fn activate_row(&self, position: u32) -> bool {
        let row = self
            .shared
            .rows
            .borrow()
            .iter()
            .find(|r| r.item.position() == position)
            .and_then(|r| r.label.parent());
        row.is_some_and(|row| row.activate_action("row.activate", None).is_ok())
    }
}

impl Default for Dropdown {
    fn default() -> Dropdown {
        Dropdown::new()
    }
}
