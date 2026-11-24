//! The pointer reaches only controls (Phase 11a Task 56; owner, 2026-11-23: "dont allow select
//! components, items, labels, anything, only buttons can be triggered, but again... nothing
//! should be selected").
//!
//! GTK would let a click select a label's text (where a label is selectable), and would move the
//! keyboard focus to whatever was clicked, which draws the focus ring around it. This app has
//! nothing to select or copy, so every label is made not selectable and not focusable, and a
//! control (a button, a switch, a scale, a dropdown, a tile) takes no focus from a click, so a
//! click operates it and leaves no ring or highlight behind. The keyboard is untouched: Tab still
//! reaches every control, and the ring shows as it does for the keyboard (Keyboard, in the design).

use gtk::prelude::*;

/// Whether `widget` is something the user operates.
fn is_control(widget: &gtk::Widget) -> bool {
    widget.is::<gtk::Button>()
        || widget.is::<gtk::Switch>()
        || widget.is::<gtk::Scale>()
        || widget.is::<gtk::DropDown>()
        || widget.is::<gtk::FlowBoxChild>()
        || widget.is::<gtk::MenuButton>()
}

/// Makes `root` and everything under it pointer-safe, as the module says.
pub fn calm(root: &impl IsA<gtk::Widget>) {
    let widget = root.upcast_ref::<gtk::Widget>();
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        label.set_selectable(false);
        label.set_focusable(false);
    } else if is_control(widget) {
        widget.set_focus_on_click(false);
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        calm(&next);
        child = next.next_sibling();
    }
}
