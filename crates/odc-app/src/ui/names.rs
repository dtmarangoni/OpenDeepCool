//! The accessible names (`docs/design/settings-app.md`, Accessible names): every interactive
//! widget gets a fixed English label, and reasons go in its description. GTK has no getter for
//! them, so each is also kept here, for the tests to check against the design's table; what a
//! screen reader reads is checked through AT-SPI (Phase 10 plan, Task 8).

use std::cell::RefCell;

use gtk::accessible::Property;
use gtk::prelude::*;

/// Every named widget, with its label and its last description.
#[derive(Default)]
pub struct Names {
    list: RefCell<Vec<Named>>,
}

struct Named {
    label: String,
    widget: gtk::Widget,
    description: Option<String>,
}

impl Names {
    /// Gives `widget` the accessible label `label`.
    pub fn name(&self, widget: &impl IsA<gtk::Widget>, label: &str) {
        let widget: &gtk::Widget = widget.as_ref();
        widget.update_property(&[Property::Label(label)]);
        self.list.borrow_mut().push(Named {
            label: label.to_owned(),
            widget: widget.clone(),
            description: None,
        });
    }

    /// Sets `widget`'s accessible description, or clears it.
    pub fn describe(&self, widget: &impl IsA<gtk::Widget>, description: Option<&str>) {
        let widget: &gtk::Widget = widget.as_ref();
        widget.update_property(&[Property::Description(description.unwrap_or(""))]);
        if let Some(named) = self
            .list
            .borrow_mut()
            .iter_mut()
            .find(|n| n.widget == *widget)
        {
            named.description = description.map(str::to_owned);
        }
    }

    /// The widget named `label`.
    pub fn find(&self, label: &str) -> Option<gtk::Widget> {
        self.list
            .borrow()
            .iter()
            .find(|n| n.label == label)
            .map(|n| n.widget.clone())
    }

    /// The description given to the widget named `label`.
    pub fn description(&self, label: &str) -> Option<String> {
        self.list
            .borrow()
            .iter()
            .find(|n| n.label == label)
            .and_then(|n| n.description.clone())
    }

    /// Every label, in the order given.
    pub fn labels(&self) -> Vec<String> {
        self.list.borrow().iter().map(|n| n.label.clone()).collect()
    }
}
