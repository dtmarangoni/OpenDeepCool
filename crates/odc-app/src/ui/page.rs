//! The parts the pages and the header share (`docs/design/settings-app.md`, Window): the
//! frameless icon buttons, the page bar, and a page, which is its bar over a scrolled content so
//! the bar never scrolls away.

use gtk::prelude::*;

use super::names::Names;

/// An icon-only button for the header or a page bar: `icon`, no frame (the `icon-button`
/// class), and `name` as its accessible name and `tooltip` as its tooltip. A button with an
/// icon and no text is named by the app, never by its icon.
pub fn icon_button(
    button: &(impl IsA<gtk::Button> + IsA<gtk::Widget>),
    names: &Names,
    icon: &str,
    name: &str,
    tooltip: &str,
) {
    button.set_icon_name(icon);
    button.add_css_class("icon-button");
    button.set_tooltip_text(Some(tooltip));
    names.name(button, name);
}

/// A page's bar: `leading` (the back icon) at the start, then the title, then `trailing`
/// (Refresh) at the end.
pub fn page_bar(
    leading: Option<&gtk::Button>,
    title: &str,
    trailing: Option<&gtk::Button>,
) -> gtk::Box {
    let bar = gtk::Box::builder()
        .spacing(8)
        .margin_top(12)
        .margin_start(18)
        .margin_end(18)
        .css_classes(["page-bar"])
        .build();
    if let Some(button) = leading {
        bar.append(button);
    }
    bar.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["page-title"])
            .build(),
    );
    if let Some(button) = trailing {
        bar.append(button);
    }
    bar
}

/// A scrolled window holding `child`: vertically as the content needs, and sideways only where
/// the window is narrower than the content's own minimum (Rows, Narrow windows), so the
/// content's minimum never sets the window's.
pub fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .child(child)
        .build()
}

/// A page: its bar, then its content in a scrolled window.
pub fn page(bar: &gtk::Box, content: &impl IsA<gtk::Widget>) -> gtk::Box {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    page.append(bar);
    page.append(&scrolled(content));
    page
}
