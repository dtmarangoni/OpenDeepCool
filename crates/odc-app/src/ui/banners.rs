//! The banners (`docs/design/settings-app.md`, Banners): a `GtkRevealer` holding a `GtkBox` of
//! an icon and the text, GTK's own suggestion since `GtkInfoBar` was deprecated. At most one
//! cooler banner, chosen by `CoolerState`; the service banner is gone since the off screen
//! (D10).

use gtk::glib;
use gtk::prelude::*;

use super::names::Names;
use crate::model::Banner;

pub struct Banners {
    cooler: gtk::Revealer,
    cooler_box: gtk::Box,
    cooler_icon: gtk::Image,
    cooler_label: gtk::Label,
}

/// `<b>bold</b> rest`, each escaped.
fn bold_then(bold: &str, rest: &str) -> String {
    format!(
        "<b>{}</b> {}",
        glib::markup_escape_text(bold),
        glib::markup_escape_text(rest)
    )
}

fn revealer(child: &impl IsA<gtk::Widget>) -> gtk::Revealer {
    gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::None)
        .reveal_child(false)
        .child(child)
        .build()
}

fn text(markup: &str) -> gtk::Label {
    gtk::Label::builder()
        .use_markup(true)
        .label(markup)
        .wrap(true)
        .xalign(0.0)
        .build()
}

impl Banners {
    pub fn new(names: &Names) -> Banners {
        let cooler_box = gtk::Box::builder()
            .accessible_role(gtk::AccessibleRole::Group)
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .css_classes(["banner", "warn"])
            .build();
        let cooler_icon = gtk::Image::builder()
            .icon_name("dialog-warning-symbolic")
            .valign(gtk::Align::Start)
            .build();
        let cooler_label = text("");
        cooler_label.set_hexpand(true);
        cooler_box.append(&cooler_icon);
        cooler_box.append(&cooler_label);
        names.name(&cooler_box, "Cooler banner");
        Banners {
            cooler: revealer(&cooler_box),
            cooler_box,
            cooler_icon,
            cooler_label,
        }
    }

    /// The banner.
    pub fn widget(&self) -> &gtk::Revealer {
        &self.cooler
    }

    /// Shows `cooler`'s banner, or none.
    pub fn show(&self, names: &Names, cooler: Option<Banner>) {
        self.cooler.set_reveal_child(cooler.is_some());
        self.cooler.set_visible(cooler.is_some());
        let Some(banner) = cooler else {
            names.describe(&self.cooler_box, None);
            return;
        };
        let (bold, rest) = banner.text();
        self.cooler_label.set_markup(&bold_then(bold, rest));
        let (class, icon) = if banner.is_error() {
            ("error", "dialog-error-symbolic")
        } else {
            ("warn", "dialog-warning-symbolic")
        };
        self.cooler_box.set_css_classes(&["banner", class]);
        self.cooler_icon.set_icon_name(Some(icon));
        names.describe(&self.cooler_box, Some(&format!("{bold} {rest}")));
    }

    /// The cooler banner's words, while it shows.
    pub fn cooler_text(&self) -> Option<String> {
        self.cooler
            .reveals_child()
            .then(|| self.cooler_label.text().to_string())
    }
}
