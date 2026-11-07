//! The accessible names (`docs/design/settings-app.md`, Accessible names): every interactive
//! widget gets a fixed English label, and reasons go in its description. GTK has no getter for
//! them, so each is also kept here, for the tests to check against the design's table; what a
//! screen reader reads is checked through AT-SPI (Phase 10 plan, Task 8).

use std::cell::RefCell;

use gtk::accessible::Property;
use gtk::prelude::*;

/// A box with the group role, which may carry a name (decisions log D48): around a label that
/// needs a fixed name, since GTK names a label by its own text, again at every change of it
/// (`gtk_label_set_text`, GTK's source), so a name given to the label never reaches AT-SPI;
/// the group's name stays, and the label's text is the value (decisions log D72).
pub fn group() -> gtk::builders::BoxBuilder {
    gtk::Box::builder().accessible_role(gtk::AccessibleRole::Group)
}

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
    /// Gives `widget` the accessible label `label`. A button with a text label is labelled by
    /// that label (`gtk_button_set_label` sets the `LABELLED_BY` relation, GTK's source), which
    /// wins over its own label, as in ARIA's name computation, so the relation is dropped:
    /// otherwise AT-SPI names the `°C` toggle "°C", not "Celsius" (decisions log D68).
    pub fn name(&self, widget: &impl IsA<gtk::Widget>, label: &str) {
        let widget: &gtk::Widget = widget.as_ref();
        widget.update_property(&[Property::Label(label)]);
        widget.reset_relation(gtk::AccessibleRelation::LabelledBy);
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
