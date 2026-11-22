//! The Configuration page (`docs/design/settings-app.md`, Configuration page): its bar, with the
//! back icon and the title, then every other setting, in cards. Theme: whether to follow the
//! system's theme, and dark or light. Display: the rotation, the temperature unit and, with more
//! than one GPU, the GPU. Music: the peak dots and the bars' and peaks' fall. (Startup, the last
//! card, is `startup.rs`.)
//! Each control asks the service at once and shows the new value once the service confirms it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::accessible::Property;
use gtk::glib;
use gtk::prelude::*;
use odc_ipc::{FallStop, TemperatureUnit, WindowCorners, WindowOpacity};

use super::Request;
use super::dropdowns::{Dropdown, Entry};
use super::names::{Names, group};
use super::page::{icon_button, page, page_bar, scroller_of};
use crate::model;

/// The request a unit toggle sends.
pub fn unit_request(unit: TemperatureUnit) -> Request {
    Request::SetTemperatureUnit(unit)
}

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
    /// The pressed Dark or Light button the card shows.
    dark: Option<bool>,
    rotation: u8,
    unit: Option<TemperatureUnit>,
    gpus: Vec<String>,
    bar_fall: Option<FallStop>,
    peak_fall: Option<FallStop>,
}

pub struct ConfigPage {
    /// The page: its bar over the scrolled cards.
    root: gtk::Box,
    /// The box the cards are in: the Startup card moves into it.
    cards: gtk::Box,
    /// Every row's label block, in one horizontal size group.
    labels: gtk::SizeGroup,
    use_system: gtk::Switch,
    dark: gtk::ToggleButton,
    light: gtk::ToggleButton,
    /// The Default Theme subcategory: its heading and rows, insensitive together while the
    /// system's theme is followed.
    default_theme: gtk::Box,
    /// Window Opacity's and Window Corners' buttons, in the order of [`OPACITIES`] and
    /// [`CORNERS`], and the index each was last confirmed at.
    opacity: Vec<gtk::ToggleButton>,
    corners: Vec<gtk::ToggleButton>,
    saved_opacity: Rc<Cell<usize>>,
    saved_corners: Rc<Cell<usize>>,
    theme_icon: gtk::Image,
    theme_line: gtk::Label,
    /// An empty box at the page's end, as tall as the Startup card, while the card is on the off
    /// screen: the page, and so the window, keeps its height (Size and scrolling).
    spacer: gtk::Box,
    back: gtk::Button,
    rotations: [gtk::ToggleButton; 4],
    celsius: gtk::ToggleButton,
    fahrenheit: gtk::ToggleButton,
    gpu_row: gtk::Box,
    gpu: Dropdown,
    gpu_in_use: gtk::Label,
    /// The group named "GPU in use" around it (decisions log D72).
    gpu_in_use_group: gtk::Box,
    gpu_hint: gtk::Label,
    peaks: gtk::Switch,
    bar_fall: gtk::Scale,
    peak_fall: gtk::Scale,
    confirmed: Rc<RefCell<Confirmed>>,
    /// The scales' last asked stops, so a drag asks once a stop.
    asked: Rc<[Cell<Option<FallStop>>; 2]>,
    applying: Rc<Cell<bool>>,
}

/// Window Opacity's words: the button, its accessible name and the setting (Omarchy's own
/// words for its Window Opacity).
const OPACITIES: [(&str, &str, WindowOpacity); 3] = [
    ("Opaque", "Opaque window", WindowOpacity::Opaque),
    (
        "Transparent",
        "Transparent window",
        WindowOpacity::Transparent,
    ),
    ("Glassy", "Glassy window", WindowOpacity::Glassy),
];

/// Window Corners' words, the same.
const CORNERS: [(&str, &str, WindowCorners); 2] = [
    ("Sharp", "Sharp corners", WindowCorners::Sharp),
    ("Rounded", "Rounded corners", WindowCorners::Rounded),
];

/// A row of linked toggle buttons, one word each (Window Opacity, Window Corners): a press asks
/// `ask(value)` and the pressed button goes back to the one last confirmed, `saved`, until the
/// service confirms the change, as every control of the page does.
#[allow(clippy::too_many_arguments)]
fn words_row<T: Copy + 'static>(
    parent: &gtk::Box,
    labels: &gtk::SizeGroup,
    names: &Names,
    title: &str,
    words: &[(&str, &str, T)],
    send: &Rc<dyn Fn(Request)>,
    applying: &Rc<Cell<bool>>,
    saved: &Rc<Cell<usize>>,
    ask: fn(T) -> Request,
) -> Vec<gtk::ToggleButton> {
    let box_ = gtk::Box::builder()
        .accessible_role(gtk::AccessibleRole::Group)
        .css_classes(["linked"])
        .build();
    names.name(&box_, title);
    let buttons: Rc<RefCell<Vec<gtk::ToggleButton>>> = Rc::new(RefCell::new(Vec::new()));
    for (i, (label, name, value)) in words.iter().enumerate() {
        let button = gtk::ToggleButton::builder().label(*label).build();
        if let Some(first) = buttons.borrow().first() {
            button.set_group(Some(first));
        }
        names.name(&button, name);
        box_.append(&button);
        let (send, applying, saved, others, value) = (
            Rc::clone(send),
            Rc::clone(applying),
            Rc::clone(saved),
            Rc::clone(&buttons),
            *value,
        );
        button.connect_toggled(move |button| {
            if applying.get() || !button.is_active() || saved.get() == i {
                return;
            }
            send(ask(value));
            // Back to the saved button until the service confirms the change.
            applying.set(true);
            if let Some(back) = others.borrow().get(saved.get()) {
                back.set_active(true);
            }
            applying.set(false);
        });
        buttons.borrow_mut().push(button);
    }
    row(parent, labels, title, None, &box_);
    let all = buttons.borrow().to_vec();
    #[allow(clippy::let_and_return)]
    all
}

fn small(text: &str, classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(classes)
        .build()
}

/// The most characters the Theme card's line asks room for.
const THEME_LINE_CHARS: i32 = 72;

/// The most characters a hint under a row's title asks room for: the label block, and so the
/// column of controls, stays about this wide (Rows).
const HINT_WIDTH_CHARS: i32 = 28;

/// A card's row (`docs/design/settings-app.md`, Rows): the label block, the setting's title with
/// its hint under it, then the control, 24 px apart and left-aligned, nothing expanding. Every
/// label block is in `labels`, so the controls of all the cards start at one x. The control's top
/// is the block's top, whatever the control (owner, 2026-11-22: the switches level with their
/// labels, which the Peak dots' hint made a half-hint lower when they were centred).
fn row(
    card: &gtk::Box,
    labels: &gtk::SizeGroup,
    title: &str,
    hint: Option<&str>,
    control: &impl IsA<gtk::Widget>,
) -> gtk::Box {
    let line = gtk::Box::builder().spacing(24).build();
    let block = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .build();
    block.append(&small(title, &[]));
    if let Some(hint) = hint {
        let hint = small(hint, &["muted", "small"]);
        hint.set_wrap_mode(gtk::pango::WrapMode::Word);
        hint.set_width_chars(HINT_WIDTH_CHARS);
        hint.set_max_width_chars(HINT_WIDTH_CHARS);
        block.append(&hint);
    }
    labels.add_widget(&block);
    control.set_halign(gtk::Align::Start);
    control.set_hexpand(false);
    control.set_valign(gtk::Align::Start);
    line.append(&block);
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

/// A control made of several lines, `width` wide.
fn control_box(width: i32) -> gtk::Box {
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
        // Under the page bar the cards start 6 px down (Window, Page bar), which with the bar's
        // 12 above, 32 and 18 below makes 68 px from the panel's top to the first card; the same
        // 68 px stand under the last card (owner, 2026-11-22).
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(6)
            .margin_bottom(68)
            .margin_start(18)
            .margin_end(18)
            .build();
        // Every row's label block, so the controls of all the cards start at one x (Rows).
        let labels = gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal);
        let confirmed = Rc::new(RefCell::new(Confirmed::default()));
        let applying = Rc::new(Cell::new(false));
        let asked: Rc<[Cell<Option<FallStop>>; 2]> = Rc::new([Cell::new(None), Cell::new(None)]);

        // Theme: whether to follow the system's, dark or light, and the line saying which is used.
        let theme = card(&root, "Theme", "Theme settings", names);
        let use_system = gtk::Switch::builder().build();
        names.name(&use_system, "Use system theme");
        let (s, a) = (Rc::clone(&send), Rc::clone(&applying));
        // The handle moves at the press and the trough shows the saved state until the service
        // confirms, as the Peak dots switch does.
        use_system.connect_state_set(move |_, on| {
            if !a.get() {
                s(Request::SetUseSystemTheme(on));
            }
            glib::Propagation::Stop
        });
        // The row and its line under it are one box, 2 px apart as a hint is from its title in a
        // label block (owner, 2026-11-22: "the same you used in Peak dots"), not the card's 14 px.
        let use_system_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .build();
        row(
            &use_system_box,
            &labels,
            "Use system theme",
            None,
            &use_system,
        );
        theme.append(&use_system_box);
        // The line under Use system theme (owner, 2026-11-19; it was at the card's foot): one line
        // in every state, so the card keeps its height.
        let theme_icon = gtk::Image::builder()
            .icon_name("dialog-warning-symbolic")
            .css_classes(["warn-text"])
            .valign(gtk::Align::Start)
            .visible(false)
            .build();
        let theme_line = small("", &["hint", "small", "muted"]);
        theme_line.set_max_width_chars(THEME_LINE_CHARS);
        let theme_status = group().spacing(6).build();
        names.name(&theme_status, "Theme status");
        theme_status.append(&theme_icon);
        theme_status.append(&theme_line);
        use_system_box.append(&theme_status);
        let dark = gtk::ToggleButton::builder().label("Dark").build();
        let light = gtk::ToggleButton::builder()
            .label("Light")
            .group(&dark)
            .build();
        dark.set_active(true);
        names.name(&dark, "Dark theme");
        names.name(&light, "Light theme");
        let dark_or_light = gtk::Box::builder()
            .accessible_role(gtk::AccessibleRole::Group)
            .css_classes(["linked"])
            .build();
        for (button, wanted) in [(&dark, true), (&light, false)] {
            dark_or_light.append(button);
            let (send, confirmed, applying) = (
                Rc::clone(&send),
                Rc::clone(&confirmed),
                Rc::clone(&applying),
            );
            let (d, l) = (dark.clone(), light.clone());
            button.connect_toggled(move |button| {
                let saved = confirmed.borrow().dark;
                if applying.get() || !button.is_active() || saved == Some(wanted) {
                    return;
                }
                send(Request::SetDarkTheme(wanted));
                // Back to the saved button until the service confirms the change.
                applying.set(true);
                if saved == Some(false) {
                    l.set_active(true);
                } else {
                    d.set_active(true);
                }
                applying.set(false);
            });
        }
        // The Default Theme, a child of the Theme card as Use system theme is: its heading and
        // three rows, insensitive together while the system's theme is followed.
        // 14 px between its rows and 10 more above it, so it stands 24 px under the line above.
        let default_theme = gtk::Box::builder()
            .accessible_role(gtk::AccessibleRole::Group)
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .margin_top(10)
            .css_classes(["subcategory"])
            .build();
        names.name(&default_theme, "Default Theme");
        default_theme.append(&small("Default Theme", &["sub-heading"]));
        names.name(&dark_or_light, "Appearance");
        row(&default_theme, &labels, "Appearance", None, &dark_or_light);
        let saved_opacity = Rc::new(Cell::new(0));
        let saved_corners = Rc::new(Cell::new(0));
        let opacity = words_row(
            &default_theme,
            &labels,
            names,
            "Window Opacity",
            &OPACITIES,
            &send,
            &applying,
            &saved_opacity,
            Request::SetWindowOpacity,
        );
        let corners = words_row(
            &default_theme,
            &labels,
            names,
            "Window Corners",
            &CORNERS,
            &send,
            &applying,
            &saved_corners,
            Request::SetWindowCorners,
        );
        theme.append(&default_theme);
        // Display: the rotation, then the GPU.
        let display = card(&root, "Display", "Display settings", names);
        let rotations: [gtk::ToggleButton; 4] = std::array::from_fn(|r| {
            gtk::ToggleButton::builder()
                .label(format!("{}°", r * 90))
                .build()
        });
        let toggles = gtk::Box::builder().css_classes(["linked"]).build();
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
        row(&display, &labels, "Rotation", None, &toggles);

        // The temperature unit, after Rotation: the same kind of control.
        let celsius = gtk::ToggleButton::builder().label("°C").build();
        let fahrenheit = gtk::ToggleButton::builder()
            .label("°F")
            .group(&celsius)
            .build();
        celsius.set_active(true);
        names.name(&celsius, "Celsius");
        names.name(&fahrenheit, "Fahrenheit");
        let unit_toggles = gtk::Box::builder().css_classes(["linked"]).build();
        for (button, wanted) in [
            (&celsius, TemperatureUnit::Celsius),
            (&fahrenheit, TemperatureUnit::Fahrenheit),
        ] {
            unit_toggles.append(button);
            let (send, confirmed, applying) = (
                Rc::clone(&send),
                Rc::clone(&confirmed),
                Rc::clone(&applying),
            );
            let (c, f) = (celsius.clone(), fahrenheit.clone());
            button.connect_toggled(move |button| {
                let saved = confirmed.borrow().unit;
                if applying.get() || !button.is_active() || saved == Some(wanted) {
                    return;
                }
                send(unit_request(wanted));
                // Back to the saved unit until the service confirms the change.
                applying.set(true);
                match saved {
                    Some(TemperatureUnit::Fahrenheit) => f.set_active(true),
                    _ => c.set_active(true),
                }
                applying.set(false);
            });
        }
        row(&display, &labels, "Temperature unit", None, &unit_toggles);

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
        let gpu_box = control_box(320);
        gpu_box.append(gpu.widget());
        gpu_box.append(&gpu_in_use_group);
        gpu_box.append(&gpu_hint);
        let gpu_row = row(
            &display,
            &labels,
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

        // Music: the peak dots and the two falls.
        let music = card(&root, "Music", "Music settings", names);
        let peaks = gtk::Switch::builder().build();
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
            &labels,
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
        row(&music, &labels, "Bar fall", None, &bar_fall);
        row(&music, &labels, "Peak fall", None, &peak_fall);

        // The page bar: the back icon, then the title (Window, Page bar).
        let back = gtk::Button::builder().action_name("win.back").build();
        icon_button(
            &back,
            names,
            "go-previous-symbolic",
            "Back",
            "Back to Display",
        );
        // Where the Startup card is while the service is down: as tall as the card, so the page
        // doesn't get shorter (the window's height is the tallest page's).
        let spacer = gtk::Box::builder().build();
        root.append(&spacer);
        let page = page(&page_bar(Some(&back), "Configuration", None), &root);
        ConfigPage {
            root: page,
            cards: root,
            labels,
            use_system,
            dark,
            light,
            default_theme,
            opacity,
            corners,
            saved_opacity,
            saved_corners,
            theme_icon,
            theme_line,
            spacer,
            back,
            rotations,
            celsius,
            fahrenheit,
            gpu_row,
            gpu,
            gpu_in_use,
            gpu_in_use_group,
            gpu_hint,
            peaks,
            bar_fall,
            peak_fall,
            confirmed,
            asked,
            applying,
        }
    }

    /// The page: its bar over the scrolled cards.
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// The page's scrolled window, for the tests.
    pub fn scroller(&self) -> gtk::ScrolledWindow {
        scroller_of(&self.root)
    }

    /// The box the cards are in.
    pub fn cards(&self) -> &gtk::Box {
        &self.cards
    }

    /// The empty box that holds the Startup card's room on the page while the card is away.
    pub fn spacer(&self) -> &gtk::Box {
        &self.spacer
    }

    /// The room the spacer holds: the card's height on this page.
    pub fn reserve(&self, height: i32) {
        self.spacer.set_height_request(height);
        // GTK keeps the size it last measured of a hidden widget: ask for it again, or the room
        // the spacer holds stays the one measured before.
        self.spacer.queue_resize();
    }

    /// The size group of the rows' label blocks, which the Startup card joins on this page.
    pub fn label_group(&self) -> &gtk::SizeGroup {
        &self.labels
    }

    /// The back icon, which takes the focus when the page opens.
    pub fn back(&self) -> &gtk::Button {
        &self.back
    }

    /// Shows the service's confirmed settings.
    pub fn show(&self, names: &Names, page: &model::ConfigPage) {
        // The confirmed values first: the controls' handlers read them while they're set.
        let rotation = page.rotation.min(3);
        {
            let mut confirmed = self.confirmed.borrow_mut();
            confirmed.rotation = rotation;
            confirmed.unit = Some(page.unit);
            confirmed.gpus = page
                .gpu
                .iter()
                .flat_map(|gpu| gpu.entries.iter().map(|e| e.pci.clone()))
                .collect();
            confirmed.bar_fall = Some(page.bar_fall);
            confirmed.peak_fall = Some(page.peak_fall);
        }
        self.applying.set(true);
        self.rotations[usize::from(rotation)].set_active(true);
        self.show_theme(names, &page.theme);
        match page.unit {
            TemperatureUnit::Celsius => self.celsius.set_active(true),
            TemperatureUnit::Fahrenheit => self.fahrenheit.set_active(true),
        }

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

    /// The Theme card: the switch, the pressed button and which controls can be used, and the
    /// line. Called with the page's controls held (`applying`).
    fn show_theme(&self, names: &Names, card: &model::ThemeCard) {
        self.confirmed.borrow_mut().dark = Some(card.dark);
        self.use_system.set_active(card.use_system);
        self.use_system.set_state(card.use_system);
        self.use_system.set_sensitive(card.use_system_sensitive);
        if card.dark {
            self.dark.set_active(true);
        } else {
            self.light.set_active(true);
        }
        // The Default Theme is dimmed together while the system's theme is followed: the heading
        // and the three titles take the class `muted`, and each button is insensitive and says
        // why (each control, never only its box).
        for label in self.default_theme_labels() {
            if card.toggles_sensitive {
                label.remove_css_class("muted");
            } else {
                label.add_css_class("muted");
            }
        }
        let opacity = OPACITIES
            .iter()
            .position(|(_, _, o)| *o == card.opacity)
            .unwrap_or(0);
        let corners = CORNERS
            .iter()
            .position(|(_, _, c)| *c == card.corners)
            .unwrap_or(0);
        self.saved_opacity.set(opacity);
        self.saved_corners.set(corners);
        self.opacity[opacity].set_active(true);
        self.corners[corners].set_active(true);
        for button in [&self.dark, &self.light]
            .into_iter()
            .chain(&self.opacity)
            .chain(&self.corners)
        {
            button.set_sensitive(card.toggles_sensitive);
            names.describe(
                button,
                (!card.toggles_sensitive).then_some("Set by the system theme."),
            );
        }
        self.theme_line.set_text(card.line.text());
        let warning = card.line.is_warning();
        self.theme_icon.set_visible(warning);
        self.theme_line.set_css_classes(if warning {
            &["hint", "small", "warn-text"]
        } else {
            &["hint", "small", "muted"]
        });
    }

    /// What the Theme card shows, for the tests: the switch and its sensitivity, the pressed
    /// button (true for Dark), whether the buttons can be used, and the line.
    pub fn theme_shown(&self) -> (bool, bool, bool, bool, String) {
        (
            self.use_system.is_active(),
            self.use_system.is_sensitive(),
            self.dark.is_active(),
            self.dark.is_sensitive() && self.light.is_sensitive(),
            self.theme_line.text().to_string(),
        )
    }

    /// The Default Theme's heading and the three rows' titles, in order.
    fn default_theme_labels(&self) -> Vec<gtk::Widget> {
        let mut labels = Vec::new();
        let mut child = self.default_theme.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            // The heading is a label; each row is a box of its label block and its control.
            match widget.downcast_ref::<gtk::Box>() {
                Some(line) => {
                    labels.extend(line.first_child().and_then(|block| block.first_child()))
                }
                None => labels.push(widget),
            }
        }
        labels
    }

    /// The Default Theme as shown, for the tests: whether its buttons can be used, and the
    /// pressed Window Opacity and Window Corners buttons.
    pub fn default_theme_shown(&self) -> (bool, Option<WindowOpacity>, Option<WindowCorners>) {
        (
            [&self.dark, &self.light]
                .into_iter()
                .chain(&self.opacity)
                .chain(&self.corners)
                .all(|b| b.is_sensitive()),
            OPACITIES
                .iter()
                .zip(&self.opacity)
                .find(|(_, b)| b.is_active())
                .map(|((_, _, o), _)| *o),
            CORNERS
                .iter()
                .zip(&self.corners)
                .find(|(_, b)| b.is_active())
                .map(|((_, _, c), _)| *c),
        )
    }

    /// Whether the line's warning icon shows, for the tests.
    pub fn theme_warning(&self) -> bool {
        self.theme_icon.is_visible()
    }

    /// Which unit toggle is on.
    pub fn unit_shown(&self) -> TemperatureUnit {
        if self.fahrenheit.is_active() {
            TemperatureUnit::Fahrenheit
        } else {
            TemperatureUnit::Celsius
        }
    }

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

    // Each unit toggle asks for its unit.
    #[test]
    fn each_unit_toggle_asks_for_its_unit() {
        assert_eq!(
            unit_request(TemperatureUnit::Celsius),
            Request::SetTemperatureUnit(TemperatureUnit::Celsius)
        );
        assert_eq!(
            unit_request(TemperatureUnit::Fahrenheit),
            Request::SetTemperatureUnit(TemperatureUnit::Fahrenheit)
        );
    }

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
