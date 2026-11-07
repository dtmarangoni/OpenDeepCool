//! The Configuration page (`docs/design/settings-app.md`, Configuration page): every other
//! setting, in three cards. Display: the rotation and, with more than one GPU, the GPU. Cooler:
//! the cooler fans' and the pump's inputs. Music: the peak dots and the bars' and peaks' fall.
//! Each control asks the service at once and shows the new value once the service confirms it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::accessible::Property;
use gtk::glib;
use gtk::prelude::*;
use odc_ipc::{FallStop, Input};

use super::Request;
use super::dropdowns::{Dropdown, Entry};
use super::names::{Names, group};
use crate::model::{self, MISSING_INPUT_HINT};

/// The five fall stops, slowest first, as the scales hold them: 0 to 4.
pub const STOPS: [FallStop; 5] = [
    FallStop::Slower,
    FallStop::Slow,
    FallStop::Moderate,
    FallStop::Fast,
    FallStop::Faster,
];

/// A stop's name on the scale's marks and in its value text.
pub fn stop_name(stop: FallStop) -> &'static str {
    match stop {
        FallStop::Slower => "Slower",
        FallStop::Slow => "Slow",
        FallStop::Moderate => "Moderate",
        FallStop::Fast => "Fast",
        FallStop::Faster => "Faster",
    }
}

/// The stop a scale's value stands on: the nearest whole stop.
pub fn stop_of(value: f64) -> FallStop {
    let index = value.round().clamp(0.0, 4.0) as usize;
    STOPS[index]
}

/// A stop's value on the scale.
pub fn value_of(stop: FallStop) -> f64 {
    STOPS.iter().position(|s| *s == stop).unwrap_or(0) as f64
}

/// What the service confirmed, which each control goes back to until a change is confirmed.
#[derive(Default)]
struct Confirmed {
    rotation: u8,
    gpus: Vec<String>,
    fans: Vec<Option<Input>>,
    pump: Vec<Option<Input>>,
    bar_fall: Option<FallStop>,
    peak_fall: Option<FallStop>,
}

pub struct ConfigPage {
    root: gtk::Box,
    rotations: [gtk::ToggleButton; 4],
    gpu_row: gtk::Box,
    gpu: Dropdown,
    gpu_in_use: gtk::Label,
    /// The group named "GPU in use" around it (decisions log D72).
    gpu_in_use_group: gtk::Box,
    gpu_hint: gtk::Label,
    fans: Dropdown,
    fans_hint: gtk::Label,
    pump: Dropdown,
    pump_hint: gtk::Label,
    inputs_hint: gtk::Label,
    peaks: gtk::Switch,
    bar_fall: gtk::Scale,
    peak_fall: gtk::Scale,
    confirmed: Rc<RefCell<Confirmed>>,
    /// The scales' last asked stops, so a drag asks once a stop.
    asked: Rc<[Cell<Option<FallStop>>; 2]>,
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

/// A card's row: the setting's name and hint on the left, its control on the right.
fn row(
    card: &gtk::Box,
    title: &str,
    hint: Option<&str>,
    control: &impl IsA<gtk::Widget>,
) -> gtk::Box {
    let line = gtk::Box::builder().spacing(24).build();
    let left = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    left.append(&small(title, &[]));
    if let Some(hint) = hint {
        left.append(&small(hint, &["muted", "small"]));
    }
    line.append(&left);
    line.append(control);
    card.append(&line);
    line
}

fn card(root: &gtk::Box, heading: &str, name: &str, names: &Names) -> gtk::Box {
    let card = gtk::Box::builder()
        .accessible_role(gtk::AccessibleRole::Group)
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .css_classes(["card"])
        .build();
    names.name(&card, name);
    card.append(&small(heading, &["card-heading"]));
    root.append(&card);
    card
}

/// The most characters a line under a control asks room for, so a long hint wraps instead of
/// widening its control.
const HINT_CHARS: i32 = 40;

fn right(width: i32) -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .width_request(width)
        .build()
}

fn scale(names: &Names, name: &str) -> gtk::Scale {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 4.0, 1.0);
    scale.set_round_digits(0);
    scale.set_digits(0);
    scale.set_draw_value(false);
    scale.set_width_request(320);
    for (i, stop) in STOPS.iter().enumerate() {
        scale.add_mark(i as f64, gtk::PositionType::Bottom, Some(stop_name(*stop)));
    }
    names.name(&scale, name);
    scale
}

impl ConfigPage {
    pub fn new(names: &Names, send: Rc<dyn Fn(Request)>) -> ConfigPage {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        let confirmed = Rc::new(RefCell::new(Confirmed::default()));
        let applying = Rc::new(Cell::new(false));
        let asked: Rc<[Cell<Option<FallStop>>; 2]> = Rc::new([Cell::new(None), Cell::new(None)]);

        // Display: the rotation, then the GPU.
        let display = card(&root, "Display", "Display settings", names);
        let rotations: [gtk::ToggleButton; 4] = std::array::from_fn(|r| {
            gtk::ToggleButton::builder()
                .label(format!("{}°", r * 90))
                .build()
        });
        let toggles = gtk::Box::builder()
            .css_classes(["linked"])
            .valign(gtk::Align::Center)
            .build();
        for (r, toggle) in rotations.iter().enumerate() {
            if r > 0 {
                toggle.set_group(Some(&rotations[0]));
            }
            names.name(toggle, &format!("Rotation {}°", r * 90));
            toggles.append(toggle);
        }
        rotations[0].set_active(true);
        for (r, toggle) in rotations.iter().enumerate() {
            let rotation = u8::try_from(r).unwrap_or(0);
            let (send, confirmed, applying) = (
                Rc::clone(&send),
                Rc::clone(&confirmed),
                Rc::clone(&applying),
            );
            let all = rotations.clone();
            toggle.connect_toggled(move |toggle| {
                let saved = confirmed.borrow().rotation;
                if applying.get() || !toggle.is_active() || rotation == saved {
                    return;
                }
                send(Request::SetRotation(rotation));
                applying.set(true);
                all[usize::from(saved)].set_active(true);
                applying.set(false);
            });
        }
        row(
            &display,
            "Rotation",
            Some("Turns the picture clockwise"),
            &toggles,
        );

        let gpu = Dropdown::new();
        names.name(gpu.widget(), "GPU");
        let gpu_in_use = small("", &["muted", "small"]);
        let gpu_in_use_group = group().visible(false).build();
        names.name(&gpu_in_use_group, "GPU in use");
        gpu_in_use_group.append(&gpu_in_use);
        let gpu_hint = small("", &["muted", "small"]);
        for line in [&gpu_in_use, &gpu_hint] {
            line.set_max_width_chars(HINT_CHARS);
        }
        let gpu_box = right(320);
        gpu_box.append(gpu.widget());
        gpu_box.append(&gpu_in_use_group);
        gpu_box.append(&gpu_hint);
        let gpu_row = row(
            &display,
            "GPU",
            Some("Where the display's GPU values come from"),
            &gpu_box,
        );
        let (s, c) = (Rc::clone(&send), Rc::clone(&confirmed));
        gpu.connect_picked(move |position| {
            if let Some(pci) = c.borrow().gpus.get(position as usize) {
                s(Request::ChooseGpu(pci.clone()));
            }
        });

        // Cooler: the two inputs.
        let cooler = card(&root, "Cooler", "Cooler settings", names);
        let input_row = |title: &str, name: &str| {
            let dropdown = Dropdown::new();
            names.name(dropdown.widget(), name);
            let hint = small(MISSING_INPUT_HINT, &["warn-text", "small"]);
            hint.set_max_width_chars(HINT_CHARS);
            hint.set_visible(false);
            let column = right(320);
            column.append(dropdown.widget());
            column.append(&hint);
            row(&cooler, title, None, &column);
            (dropdown, hint)
        };
        let (fans, fans_hint) = input_row("Cooler fans input", "Cooler fans input");
        let (pump, pump_hint) = input_row("Pump input", "Pump input");
        let (s, c) = (Rc::clone(&send), Rc::clone(&confirmed));
        fans.connect_picked(move |position| {
            if let Some(input) = c.borrow().fans.get(position as usize) {
                s(Request::SetCoolerFanInput(input.clone()));
            }
        });
        let (s, c) = (Rc::clone(&send), Rc::clone(&confirmed));
        pump.connect_picked(move |position| {
            if let Some(input) = c.borrow().pump.get(position as usize) {
                s(Request::SetPumpInput(input.clone()));
            }
        });
        let inputs_hint = small("", &["muted", "small"]);
        cooler.append(&inputs_hint);

        // Music: the peak dots and the two falls.
        let music = card(&root, "Music", "Music settings", names);
        let peaks = gtk::Switch::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build();
        names.name(&peaks, "Peak dots");
        let (s, a) = (Rc::clone(&send), Rc::clone(&applying));
        // The switch keeps its state until the service confirms the change (GtkSwitch's
        // delayed state: the handler returns Stop).
        peaks.connect_state_set(move |_, on| {
            if !a.get() {
                s(Request::SetMusicPeaks(on));
            }
            glib::Propagation::Stop
        });
        row(
            &music,
            "Peak dots",
            Some("A dot above each bar that falls slowly"),
            &peaks,
        );
        let bar_fall = scale(names, "Bar fall");
        let peak_fall = scale(names, "Peak fall");
        for (index, scale) in [&bar_fall, &peak_fall].into_iter().enumerate() {
            let (s, c, a, asked) = (
                Rc::clone(&send),
                Rc::clone(&confirmed),
                Rc::clone(&applying),
                Rc::clone(&asked),
            );
            scale.connect_value_changed(move |scale| {
                let stop = stop_of(scale.value());
                scale.update_property(&[Property::ValueText(stop_name(stop))]);
                if a.get() {
                    return;
                }
                let saved = if index == 0 {
                    c.borrow().bar_fall
                } else {
                    c.borrow().peak_fall
                };
                if Some(stop) == saved || Some(stop) == asked[index].get() {
                    return;
                }
                asked[index].set(Some(stop));
                s(if index == 0 {
                    Request::SetMusicBarFall(stop)
                } else {
                    Request::SetMusicPeakFall(stop)
                });
            });
        }
        row(&music, "Bar fall", None, &bar_fall);
        row(&music, "Peak fall", None, &peak_fall);
        music.append(&small(
            "For every Music option except Dancer. Changes show on the next frame.",
            &["muted", "small"],
        ));

        ConfigPage {
            root,
            rotations,
            gpu_row,
            gpu,
            gpu_in_use,
            gpu_in_use_group,
            gpu_hint,
            fans,
            fans_hint,
            pump,
            pump_hint,
            inputs_hint,
            peaks,
            bar_fall,
            peak_fall,
            confirmed,
            asked,
            applying,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Shows the service's confirmed settings.
    pub fn show(&self, names: &Names, page: &model::ConfigPage) {
        // The confirmed values first: the controls' handlers read them while they're set.
        let rotation = page.rotation.min(3);
        {
            let mut confirmed = self.confirmed.borrow_mut();
            confirmed.rotation = rotation;
            confirmed.gpus = page
                .gpu
                .iter()
                .flat_map(|gpu| gpu.entries.iter().map(|e| e.pci.clone()))
                .collect();
            confirmed.fans = page
                .cooler_fans
                .entries
                .iter()
                .map(|e| e.input.clone())
                .collect();
            confirmed.pump = page.pump.entries.iter().map(|e| e.input.clone()).collect();
            confirmed.bar_fall = Some(page.bar_fall);
            confirmed.peak_fall = Some(page.peak_fall);
        }
        self.applying.set(true);
        self.rotations[usize::from(rotation)].set_active(true);

        match &page.gpu {
            Some(gpu) => {
                self.gpu_row.set_visible(true);
                let entries = gpu
                    .entries
                    .iter()
                    .map(|e| Entry {
                        label: e.label.clone(),
                        note: e.note.unwrap_or("").to_owned(),
                    })
                    .collect();
                let button = gpu
                    .entries
                    .get(gpu.chosen)
                    .map(|e| e.label.clone())
                    .unwrap_or_default();
                self.gpu
                    .show(entries, u32::try_from(gpu.chosen).ok(), &button);
                self.gpu_in_use
                    .set_text(gpu.in_use.as_deref().unwrap_or(""));
                self.gpu_in_use_group.set_visible(gpu.in_use.is_some());
                self.gpu_hint.set_text(gpu.hint.unwrap_or(""));
                self.gpu_hint.set_visible(gpu.hint.is_some());
                names.describe(
                    self.gpu.widget(),
                    (!gpu.description.is_empty()).then_some(gpu.description.as_str()),
                );
            }
            None => self.gpu_row.set_visible(false),
        }

        for (dropdown, hint, row) in [
            (&self.fans, &self.fans_hint, &page.cooler_fans),
            (&self.pump, &self.pump_hint, &page.pump),
        ] {
            if row.entries.is_empty() {
                // No fan sensors: the note alone, and nothing to pick.
                dropdown.show(
                    vec![Entry {
                        label: row.button.clone(),
                        note: String::new(),
                    }],
                    Some(0),
                    &row.button,
                );
            } else {
                let entries = row
                    .entries
                    .iter()
                    .map(|e| Entry {
                        label: e.label.clone(),
                        note: e.rpm.clone(),
                    })
                    .collect();
                dropdown.show(
                    entries,
                    row.chosen.and_then(|c| u32::try_from(c).ok()),
                    &row.button,
                );
            }
            dropdown.widget().set_sensitive(row.sensitive);
            if row.missing {
                dropdown.widget().add_css_class("warn");
            } else {
                dropdown.widget().remove_css_class("warn");
            }
            hint.set_visible(row.missing);
            names.describe(dropdown.widget(), row.missing.then_some("not found"));
        }
        self.inputs_hint.set_text(page.inputs_hint);

        self.peaks.set_active(page.peaks);
        self.peaks.set_state(page.peaks);
        for (index, (scale, stop)) in [
            (&self.bar_fall, page.bar_fall),
            (&self.peak_fall, page.peak_fall),
        ]
        .into_iter()
        .enumerate()
        {
            self.asked[index].set(None);
            scale.set_value(value_of(stop));
            scale.update_property(&[Property::ValueText(stop_name(stop))]);
        }
        self.applying.set(false);
    }

    pub fn gpu(&self) -> &Dropdown {
        &self.gpu
    }

    pub fn fans(&self) -> &Dropdown {
        &self.fans
    }

    pub fn pump(&self) -> &Dropdown {
        &self.pump
    }

    /// Whether the GPU row shows, and its line and hint while they show.
    pub fn gpu_row(&self) -> Option<(Option<String>, Option<String>)> {
        let shown = |l: &gtk::Label| l.is_visible().then(|| l.text().to_string());
        let in_use = self
            .gpu_in_use_group
            .is_visible()
            .then(|| self.gpu_in_use.text().to_string());
        self.gpu_row
            .is_visible()
            .then(|| (in_use, shown(&self.gpu_hint)))
    }

    /// Whether each input's missing hint shows, and the hint under the inputs.
    pub fn input_hints(&self) -> (bool, bool, String) {
        (
            self.fans_hint.is_visible(),
            self.pump_hint.is_visible(),
            self.inputs_hint.text().to_string(),
        )
    }

    /// The confirmed rotation's toggle, the switch and the scales as shown.
    pub fn shown(&self) -> (usize, bool, f64, f64) {
        (
            self.rotations
                .iter()
                .position(|t| t.is_active())
                .unwrap_or(0),
            self.peaks.is_active(),
            self.bar_fall.value(),
            self.peak_fall.value(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The five stops map to the scale's five values and back, named as the marks show them.
    #[test]
    fn the_scale_stops_map_to_the_five_names_both_ways() {
        let names = ["Slower", "Slow", "Moderate", "Fast", "Faster"];
        for (i, stop) in STOPS.iter().enumerate() {
            assert_eq!(value_of(*stop), i as f64);
            assert_eq!(stop_of(i as f64), *stop);
            assert_eq!(stop_name(*stop), names[i]);
        }
        // A value between stops stands on the nearest; one outside, on the nearest end.
        assert_eq!(stop_of(1.4), FallStop::Slow);
        assert_eq!(stop_of(1.6), FallStop::Moderate);
        assert_eq!(stop_of(-1.0), FallStop::Slower);
        assert_eq!(stop_of(9.0), FallStop::Faster);
        assert_eq!(
            STOPS.map(FallStop::as_str),
            ["slower", "slow", "moderate", "fast", "faster"]
        );
    }
}
