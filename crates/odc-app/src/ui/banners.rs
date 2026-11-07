//! The banners (`docs/design/settings-app.md`, Banners): a `GtkRevealer` holding a `GtkBox` of
//! an icon, the text and an optional button, GTK's own suggestion since `GtkInfoBar` was
//! deprecated. The service banner, or at most one cooler banner, chosen by `CoolerState`.

use gtk::glib;
use gtk::prelude::*;

use super::names::Names;
use crate::model::{Banner, SERVICE_BANNER};

pub struct Banners {
    service: gtk::Revealer,
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
        let [bold, fix, command, after] = SERVICE_BANNER;
        let service_box = gtk::Box::builder()
            .accessible_role(gtk::AccessibleRole::Group)
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .css_classes(["banner", "error"])
            .build();
        service_box.append(
            &gtk::Image::builder()
                .icon_name("dialog-error-symbolic")
                .valign(gtk::Align::Start)
                .build(),
        );
        let words = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .hexpand(true)
            .build();
        words.append(&text(&bold_then(bold, fix)));
        words.append(
            &gtk::Label::builder()
                .label(command)
                .selectable(true)
                .xalign(0.0)
                .halign(gtk::Align::Start)
                .css_classes(["command"])
                .build(),
        );
        words.append(&text(&glib::markup_escape_text(after)));
        service_box.append(&words);
        let copy_content = gtk::Box::builder().spacing(6).build();
        copy_content.append(&gtk::Image::from_icon_name("edit-copy-symbolic"));
        copy_content.append(&gtk::Label::new(Some("Copy")));
        let copy = gtk::Button::builder()
            .child(&copy_content)
            .valign(gtk::Align::Start)
            .build();
        copy.connect_clicked(move |button| button.clipboard().set_text(command));
        service_box.append(&copy);
        names.name(&service_box, "Service banner");
        names.describe(
            &service_box,
            Some(&format!("{bold} {fix} {command} {after}")),
        );
        names.name(&copy, "Copy start command");

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
            service: revealer(&service_box),
            cooler: revealer(&cooler_box),
            cooler_box,
            cooler_icon,
            cooler_label,
        }
    }

    /// The banners, top first.
    pub fn widgets(&self) -> [&gtk::Revealer; 2] {
        [&self.service, &self.cooler]
    }

    /// Shows the service banner, or `cooler`'s, or none.
    pub fn show(&self, names: &Names, service_down: bool, cooler: Option<Banner>) {
        self.service.set_reveal_child(service_down);
        self.service.set_visible(service_down);
        let cooler = cooler.filter(|_| !service_down);
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

    /// Whether the service banner shows.
    pub fn service_shown(&self) -> bool {
        self.service.reveals_child()
    }
}
