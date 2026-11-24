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

/// A page's bar with the title in the middle of the whole panel (owner, 2026-11-22: "Display"
/// and "Configuration" centred inside the right panel): `leading` (the back icon) at the start,
/// `trailing` (Refresh) at the end, and the title centred over the bar's full width, whatever the
/// icons beside it. A `GtkCenterBox` centres its middle child on the box, not on what the two
/// ends leave.
pub fn page_bar(
    leading: Option<&gtk::Button>,
    title: &str,
    trailing: Option<&gtk::Button>,
) -> gtk::CenterBox {
    // 18 px below the bar and the content's 6 above it: 24 px from the title to the content
    // (owner, 2026-11-19: 14 px; 2026-11-22: more).
    let bar = gtk::CenterBox::builder()
        .margin_top(12)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .css_classes(["page-bar"])
        .build();
    bar.set_start_widget(leading);
    bar.set_center_widget(Some(
        &gtk::Label::builder()
            .label(title)
            .css_classes(["page-title"])
            .build(),
    ));
    bar.set_end_widget(trailing);
    bar
}

/// A page's scrolled window (Size and scrolling): it scrolls down where the window is capped, asks
/// for its child's natural size so the window is as big as the page, and never scrolls sideways
/// itself: the window's own scroller does. It takes no focus; its content does.
pub fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .hexpand(true)
        .vexpand(true)
        .focusable(false)
        .child(child)
        .build();
    keep_natural_height(&scroller);
    scroller
}

/// Makes `scroller`'s content keep its natural height where the scroller is shorter, and scroll,
/// instead of being squeezed to its minimum: a viewport gives its child its minimum size by
/// default, which unwraps a row's hint into one line (`GtkScrollable:vscroll-policy`).
pub fn keep_natural_height(scroller: &gtk::ScrolledWindow) {
    if let Some(viewport) = scroller.child().and_downcast::<gtk::Viewport>() {
        viewport.set_vscroll_policy(gtk::ScrollablePolicy::Natural);
        // A control that takes the focus scrolls into view (Keyboard), whatever GTK's default.
        viewport.set_scroll_to_focus(true);
    }
}

/// The scrolled window of a [`page`], for the tests.
pub fn scroller_of(page: &gtk::Box) -> gtk::ScrolledWindow {
    page.last_child()
        .and_downcast::<gtk::ScrolledWindow>()
        .expect("a page ends with its scrolled content")
}

/// A page: its bar, then its content in a scrolled window.
pub fn page(bar: &impl IsA<gtk::Widget>, content: &impl IsA<gtk::Widget>) -> gtk::Box {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    page.append(bar);
    page.append(&scrolled(content));
    page
}

/// A left-aligned label that wraps, with `classes`.
pub fn label(text: &str, classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(classes)
        .build()
}

/// A vertical box `spacing` px between its children.
pub fn vbox(spacing: i32) -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(spacing)
        .build()
}
