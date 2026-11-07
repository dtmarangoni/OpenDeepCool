//! The window (`docs/design/settings-app.md`, Window): the header bar with the page switcher and
//! Refresh, the banners, the preview column, and the pages; or, in a remote or inactive
//! session, one message. It keeps what the app knows of the service and the window's own
//! state, and shows the view model's [`View`] of them after every change.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};
use odc_ipc::{CoolerState, State};
use odc_protocol::Frame;

use super::banners::Banners;
use super::config_page::ConfigPage;
use super::display_page::DisplayPage;
use super::names::Names;
use super::preview::Preview;
use super::{REFRESHED_FOR, REFUSED_FOR, Reason, Refused, Request, refusal_text};
use crate::model::{
    Column, Dot, NOT_AVAILABLE_HEADING, NOT_AVAILABLE_TEXT, Page, SERVICE_DOWN_PAGES, SendingFails,
    Service, Speed, Ui, View, view,
};
use crate::pci_ids;
use crate::theme::Tokens;

/// The window's default and smallest size (Window).
pub const DEFAULT_SIZE: (i32, i32) = (1040, 720);
pub const MIN_SIZE: (i32, i32) = (720, 560);

/// The preview column's width, and the most characters its wrapping lines ask room for.
const COLUMN_WIDTH: i32 = 262;
const COLUMN_CHARS: i32 = 28;

/// The status items' accessible names, in the column's order.
const STATUS_NAMES: [&str; 3] = ["Cooler status", "Service status", "Music sound status"];

/// The keyboard shortcuts (Keyboard).
const ACCELS: [(&str, &[&str]); 4] = [
    ("win.page-display", &["<Control>1"]),
    ("win.page-config", &["<Control>2"]),
    ("win.refresh", &["F5", "<Control>r"]),
    ("win.close", &["<Control>w", "<Control>q"]),
];

/// Says a text to screen readers.
type Announcer = Box<dyn Fn(&str)>;

/// What the view is made from.
struct Inputs {
    service: Service,
    ui: Ui,
    sending: SendingFails,
    /// The PCI ID list's names of the GPUs listed now.
    products: HashMap<(u16, u16), String>,
    gpu_ids: Vec<(u16, u16)>,
    pci_ids: PathBuf,
}

struct Header {
    switcher: gtk::Box,
    display: gtk::ToggleButton,
    config: gtk::ToggleButton,
    refresh: gtk::Button,
}

struct StatusRow {
    row: gtk::Box,
    dot: gtk::Box,
    label: gtk::Label,
}

struct SpeedRow {
    value: gtk::Label,
    link: gtk::Button,
}

struct ColumnWidgets {
    caption: gtk::Label,
    line: gtk::Label,
    reason_box: gtk::Box,
    reason: gtk::Label,
    status: [StatusRow; 3],
    fans: SpeedRow,
    pump: SpeedRow,
}

struct Inner {
    window: gtk::ApplicationWindow,
    send: Rc<dyn Fn(Request)>,
    names: Names,
    inputs: RefCell<Inputs>,
    shown: RefCell<Option<View>>,
    header: Header,
    feedback: gtk::Box,
    feedback_icon: gtk::Image,
    feedback_label: gtk::Label,
    feedback_timer: RefCell<Option<glib::SourceId>>,
    banners: Banners,
    screens: gtk::Stack,
    pages: gtk::Stack,
    not_available: gtk::Box,
    column: ColumnWidgets,
    preview: Preview,
    display: DisplayPage,
    config: ConfigPage,
    provider: gtk::CssProvider,
    sending_timer: RefCell<Option<glib::SourceId>>,
    /// The not-available state was announced since it was entered.
    announced: Cell<bool>,
    /// Says a text to screen readers: GTK's `announce`, or a test's recorder (decisions log
    /// D49: GTK's test accessibility backend crashes on it).
    announcer: RefCell<Announcer>,
    /// What lives as long as the window, such as the theme it follows.
    kept: RefCell<Vec<Box<dyn std::any::Any>>>,
    actions: [gio::SimpleAction; 3],
}

/// The window; clones share it.
#[derive(Clone)]
pub struct Window(Rc<Inner>);

/// A window held without keeping it.
#[derive(Clone)]
pub struct WeakWindow(Weak<Inner>);

impl WeakWindow {
    pub fn upgrade(&self) -> Option<Window> {
        self.0.upgrade().map(Window)
    }
}

fn label(text: &str, classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(classes)
        .build()
}

fn vbox(spacing: i32) -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(spacing)
        .build()
}

fn icon_and_label(icon: &str, text: &str) -> gtk::Box {
    let content = gtk::Box::builder().spacing(6).build();
    content.append(&gtk::Image::from_icon_name(icon));
    content.append(&gtk::Label::new(Some(text)));
    content
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(true)
        .vexpand(true)
        .child(child)
        .build()
}

impl Window {
    /// The window of `app`, sending what the user asks to `send`; the GPUs' names come from the
    /// PCI ID list at `pci_ids`.
    pub fn new(
        app: &gtk::Application,
        send: impl Fn(Request) + 'static,
        pci_ids: PathBuf,
        tokens: &Tokens,
    ) -> Window {
        let names = Names::default();
        let send: Rc<dyn Fn(Request)> = Rc::new(send);
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("OpenDeepCool")
            .default_width(DEFAULT_SIZE.0)
            .default_height(DEFAULT_SIZE.1)
            .build();
        window.set_size_request(MIN_SIZE.0, MIN_SIZE.1);

        // The header bar.
        let bar = gtk::HeaderBar::new();
        bar.pack_start(&label("OpenDeepCool", &["app-name"]));
        let display = gtk::ToggleButton::builder().label("Display").build();
        let config = gtk::ToggleButton::builder()
            .label("Configuration")
            .group(&display)
            .build();
        display.set_active(true);
        names.name(&display, "Display");
        names.name(&config, "Configuration");
        let switcher = gtk::Box::builder().css_classes(["linked"]).build();
        switcher.append(&display);
        switcher.append(&config);
        bar.set_title_widget(Some(&switcher));
        let refresh = gtk::Button::builder()
            .child(&icon_and_label("view-refresh-symbolic", "Refresh"))
            .action_name("win.refresh")
            .build();
        names.name(&refresh, "Refresh display");
        bar.pack_end(&refresh);
        window.set_titlebar(Some(&bar));

        // The banners, then the screens.
        let banners = Banners::new(&names);
        let content = vbox(0);
        for banner in banners.widgets() {
            content.append(banner);
        }

        // The preview column.
        let preview = Preview::new(tokens);
        names.name(preview.widget(), "Cooler display preview");
        let frame = gtk::Box::builder()
            .css_classes(["preview"])
            .halign(gtk::Align::Start)
            .build();
        frame.append(preview.widget());
        let caption = label("", &["caption"]);
        let line = label("", &["muted", "small"]);
        let reason = label("", &["warn-text", "small"]);
        names.name(&reason, "Unavailable reading");
        let reason_box = gtk::Box::builder().spacing(6).visible(false).build();
        reason_box.append(
            &gtk::Image::builder()
                .icon_name("dialog-warning-symbolic")
                .css_classes(["warn-text"])
                .valign(gtk::Align::Start)
                .build(),
        );
        reason_box.append(&reason);
        let status_box = vbox(4);
        let status = STATUS_NAMES.map(|name| {
            let row = gtk::Box::builder().spacing(8).build();
            let dot = gtk::Box::builder()
                .css_classes(["dot"])
                .valign(gtk::Align::Center)
                .build();
            let text = label("", &[]);
            names.name(&text, name);
            row.append(&dot);
            row.append(&text);
            status_box.append(&row);
            StatusRow {
                row,
                dot,
                label: text,
            }
        });
        let speeds = vbox(4);
        let speed_row = |title: &str, value_name: &str, link_name: &str| {
            let row = gtk::Box::builder().spacing(8).build();
            let name = label(title, &[]);
            name.set_hexpand(true);
            let value = label("", &["num"]);
            value.set_xalign(1.0);
            names.name(&value, value_name);
            let link = gtk::Button::builder()
                .label("Choose input")
                .css_classes(["link"])
                .visible(false)
                .build();
            names.name(&link, link_name);
            row.append(&name);
            row.append(&value);
            row.append(&link);
            speeds.append(&row);
            SpeedRow { value, link }
        };
        let fans = speed_row(
            "Cooler fans",
            "Cooler fans speed",
            "Choose cooler fans input",
        );
        let pump = speed_row("Pump", "Pump speed", "Choose pump input");
        // The column keeps the design's width (the mockups' 262 px), however long its lines.
        let column_box = vbox(10);
        column_box.add_css_class("side");
        column_box.set_hexpand(false);
        column_box.set_width_request(COLUMN_WIDTH);
        for line in [&caption, &line, &reason] {
            line.set_max_width_chars(COLUMN_CHARS);
        }
        for widget in [
            frame.upcast_ref::<gtk::Widget>(),
            caption.upcast_ref(),
            line.upcast_ref(),
            reason_box.upcast_ref(),
            status_box.upcast_ref(),
            speeds.upcast_ref(),
        ] {
            column_box.append(widget);
        }

        // The pages.
        let pages = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::None)
            .hexpand(true)
            .vexpand(true)
            .build();
        let display_page = DisplayPage::new(&names, Rc::clone(&send), tokens);
        pages.add_named(&scrolled(display_page.widget()), Some("display"));
        let config_page = ConfigPage::new(&names, Rc::clone(&send));
        pages.add_named(&scrolled(config_page.widget()), Some("config"));
        let down = label(SERVICE_DOWN_PAGES, &["muted"]);
        down.set_halign(gtk::Align::Center);
        down.set_valign(gtk::Align::Center);
        down.set_justify(gtk::Justification::Center);
        pages.add_named(&down, Some("down"));
        let main = gtk::Box::builder().vexpand(true).build();
        main.append(&column_box);
        main.append(&pages);

        // Not available in this session: two labels, centred, wrapping.
        // A named box takes the group role: ARIA lets no generic one carry a name (D48).
        let not_available = gtk::Box::builder()
            .accessible_role(gtk::AccessibleRole::Group)
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        for (text, classes) in [
            (NOT_AVAILABLE_HEADING, &["heading"][..]),
            (NOT_AVAILABLE_TEXT, &["muted"][..]),
        ] {
            let line = label(text, classes);
            line.set_justify(gtk::Justification::Center);
            line.set_xalign(0.5);
            not_available.append(&line);
        }
        names.name(&not_available, "Not available in this session");
        names.describe(
            &not_available,
            Some(&format!("{NOT_AVAILABLE_HEADING}. {NOT_AVAILABLE_TEXT}")),
        );
        let screens = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::None)
            .vexpand(true)
            .build();
        screens.add_named(&main, Some("main"));
        screens.add_named(&not_available, Some("not-available"));
        content.append(&screens);

        // The feedback label over everything.
        let feedback_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
        feedback_icon.add_css_class("warn-text");
        let feedback_label = label("", &[]);
        names.name(&feedback_label, "Change feedback");
        let feedback = gtk::Box::builder()
            .spacing(6)
            .css_classes(["feedback"])
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(12)
            .margin_end(12)
            .visible(false)
            .build();
        feedback.append(&feedback_icon);
        feedback.append(&feedback_label);
        let overlay = gtk::Overlay::builder().child(&content).build();
        overlay.add_overlay(&feedback);
        window.set_child(Some(&overlay));

        // The one style provider, at APPLICATION priority (Theme, How it's applied).
        let provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &WidgetExt::display(&window),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let actions = ["page-display", "page-config", "refresh"].map(|name| {
            let action = gio::SimpleAction::new(name, None);
            window.add_action(&action);
            action
        });
        let close = gio::SimpleAction::new("close", None);
        window.add_action(&close);
        for (action, accels) in ACCELS {
            app.set_accels_for_action(action, accels);
        }

        let inner = Rc::new(Inner {
            window,
            send,
            names,
            inputs: RefCell::new(Inputs {
                service: Service::Down,
                ui: Ui::default(),
                sending: SendingFails::default(),
                products: HashMap::new(),
                gpu_ids: Vec::new(),
                pci_ids,
            }),
            shown: RefCell::new(None),
            header: Header {
                switcher,
                display,
                config,
                refresh,
            },
            feedback,
            feedback_icon,
            feedback_label,
            feedback_timer: RefCell::new(None),
            banners,
            screens,
            pages,
            not_available,
            column: ColumnWidgets {
                caption,
                line,
                reason_box,
                reason,
                status,
                fans,
                pump,
            },
            preview,
            display: display_page,
            config: config_page,
            provider,
            sending_timer: RefCell::new(None),
            announced: Cell::new(false),
            announcer: RefCell::new(Box::new(|_| {})),
            kept: RefCell::new(Vec::new()),
            actions,
        });
        let weak = Rc::downgrade(&inner);
        *inner.announcer.borrow_mut() = Box::new(move |text| {
            if let Some(inner) = weak.upgrade() {
                inner
                    .window
                    .announce(text, gtk::AccessibleAnnouncementPriority::Medium);
            }
        });
        let window = Window(inner);
        window.connect(&close);
        window.set_theme(tokens);
        window.render();
        window
    }

    /// The signal handlers, each holding the window weakly.
    fn connect(&self, close: &gio::SimpleAction) {
        let weak = Rc::downgrade(&self.0);
        let with = move |f: fn(&Window)| {
            let weak: Weak<Inner> = weak.clone();
            move || {
                if let Some(inner) = weak.upgrade() {
                    f(&Window(inner));
                }
            }
        };
        let [page_display, page_config, refresh] = &self.0.actions;
        let go = with(|w| w.go(Page::Display));
        page_display.connect_activate(move |_, _| go());
        let go = with(|w| w.go(Page::Configuration));
        page_config.connect_activate(move |_, _| go());
        let ask = with(|w| (w.0.send)(Request::Refresh));
        refresh.connect_activate(move |_, _| ask());
        let shut = with(|w| w.0.window.close());
        close.connect_activate(move |_, _| shut());
        let go = with(|w| {
            if w.0.header.display.is_active() {
                w.go(Page::Display);
            }
        });
        self.0.header.display.connect_toggled(move |_| go());
        let go = with(|w| {
            if w.0.header.config.is_active() {
                w.go(Page::Configuration);
            }
        });
        self.0.header.config.connect_toggled(move |_| go());
        for link in [&self.0.column.fans.link, &self.0.column.pump.link] {
            let go = with(|w| w.go(Page::Configuration));
            link.connect_clicked(move |_| go());
        }
        // Frames only while the window is mapped (Preview column).
        let watch = with(|w| (w.0.send)(Request::WatchFrames(true)));
        self.0.window.connect_map(move |_| watch());
        let unwatch = with(|w| (w.0.send)(Request::WatchFrames(false)));
        self.0.window.connect_unmap(move |_| unwatch());
        // Connecting again when the window becomes active (Talking to the service).
        let again = with(|w| {
            if w.0.window.is_active() {
                (w.0.send)(Request::Reconnect);
            }
        });
        self.0.window.connect_is_active_notify(move |_| again());
    }

    pub fn widget(&self) -> &gtk::ApplicationWindow {
        &self.0.window
    }

    pub fn downgrade(&self) -> WeakWindow {
        WeakWindow(Rc::downgrade(&self.0))
    }

    /// Keeps `thing` as long as the window.
    pub fn keep(&self, thing: impl std::any::Any) {
        self.0.kept.borrow_mut().push(Box::new(thing));
    }

    /// Whether GTK's own parts take the dark scheme (Theme, the base variant).
    pub fn prefers_dark(&self) -> bool {
        self.0.provider.prefers_color_scheme() == gtk::InterfaceColorScheme::Dark
    }

    /// The service's bus name has no owner.
    pub fn down(&self) {
        let mut inputs = self.0.inputs.borrow_mut();
        inputs.service = Service::Down;
        inputs.sending.follow(None, Instant::now());
        drop(inputs);
        self.stop_sending_timer();
        self.render();
    }

    /// The service refused `GetState()` with `NotAuthorized`.
    pub fn refused(&self) {
        self.0.inputs.borrow_mut().service = Service::Refused;
        self.render();
    }

    /// The service's whole state, after `GetState()` or a `StateChanged` the client merged.
    pub fn state(&self, state: State) {
        let cooler = state.cooler_state;
        let mut inputs = self.0.inputs.borrow_mut();
        let ids: Vec<(u16, u16)> = state
            .gpus
            .iter()
            .flatten()
            .map(|g| (g.vendor, g.device))
            .collect();
        if ids != inputs.gpu_ids {
            // Without the list, every GPU is named by its driver and address.
            let text = fs::read_to_string(&inputs.pci_ids).unwrap_or_default();
            inputs.products = pci_ids::names(&text, &ids);
            inputs.gpu_ids = ids;
        }
        inputs.service = Service::Up(Box::new(state));
        let wait = inputs.sending.follow(cooler, Instant::now());
        drop(inputs);
        if cooler != Some(CoolerState::WriteFailed) {
            self.stop_sending_timer();
        }
        if let Some(wait) = wait {
            self.start_sending_timer(wait);
        }
        self.render();
    }

    /// A frame from the service.
    pub fn frame(&self, frame: Frame) {
        if matches!(*self.0.shown.borrow(), Some(View::Normal(_))) {
            self.0.preview.set_frame(Some(frame));
        }
    }

    /// The service confirmed a Refresh.
    pub fn refreshed(&self) {
        self.show_feedback("Refreshed", false, REFRESHED_FOR);
    }

    /// The service refused a change or a Refresh: each control shows the saved value again.
    pub fn refusal(&self, what: Refused, why: &Reason) {
        self.render();
        let text = refusal_text(what, why);
        self.show_feedback(&text, true, REFUSED_FOR);
        (self.0.announcer.borrow())(&text);
    }

    /// Announcements go to `announce` from now on, for the tests.
    pub fn announce_with(&self, announce: impl Fn(&str) + 'static) {
        *self.0.announcer.borrow_mut() = Box::new(announce);
    }

    /// The theme: the CSS reloaded in the same provider, the preview redrawn once.
    pub fn set_theme(&self, tokens: &Tokens) {
        self.0.provider.load_from_string(&tokens.css());
        self.0.provider.set_prefers_color_scheme(if tokens.dark {
            gtk::InterfaceColorScheme::Dark
        } else {
            gtk::InterfaceColorScheme::Light
        });
        self.0.preview.set_theme(tokens);
        self.0.display.set_theme(tokens);
    }

    fn go(&self, page: Page) {
        let mut inputs = self.0.inputs.borrow_mut();
        if inputs.ui.page == page {
            return;
        }
        inputs.ui.page = page;
        drop(inputs);
        self.render();
    }

    fn start_sending_timer(&self, wait: Duration) {
        self.stop_sending_timer();
        let weak = Rc::downgrade(&self.0);
        let source = glib::timeout_add_local_once(wait, move || {
            if let Some(inner) = weak.upgrade() {
                inner.sending_timer.borrow_mut().take();
                Window(inner).render();
            }
        });
        *self.0.sending_timer.borrow_mut() = Some(source);
    }

    fn stop_sending_timer(&self) {
        if let Some(source) = self.0.sending_timer.borrow_mut().take() {
            source.remove();
        }
    }

    fn show_feedback(&self, text: &str, warning: bool, how_long: Duration) {
        if let Some(source) = self.0.feedback_timer.borrow_mut().take() {
            source.remove();
        }
        self.0.feedback_label.set_text(text);
        self.0.feedback_icon.set_visible(warning);
        self.0.feedback.set_visible(true);
        let weak = Rc::downgrade(&self.0);
        let source = glib::timeout_add_local_once(how_long, move || {
            if let Some(inner) = weak.upgrade() {
                inner.feedback_timer.borrow_mut().take();
                inner.feedback.set_visible(false);
            }
        });
        *self.0.feedback_timer.borrow_mut() = Some(source);
    }

    /// Shows the view of what the window knows now.
    fn render(&self) {
        let shown = {
            let mut inputs = self.0.inputs.borrow_mut();
            inputs.ui.sending_failed_long = inputs.sending.lasted(Instant::now());
            view(&inputs.service, inputs.ui, &inputs.products)
        };
        let inner = &self.0;
        let header = &inner.header;
        match &shown {
            View::NotAvailable => {
                inner.screens.set_visible_child_name("not-available");
                header.switcher.set_visible(false);
                header.switcher.set_sensitive(false);
                header.refresh.set_visible(false);
                inner.banners.show(&inner.names, false, None);
                self.enable_actions(false);
                if !inner.announced.replace(true) {
                    (inner.announcer.borrow())(&format!(
                        "{NOT_AVAILABLE_HEADING}. {NOT_AVAILABLE_TEXT}"
                    ));
                }
            }
            View::ServiceDown(column) => {
                inner.announced.set(false);
                inner.screens.set_visible_child_name("main");
                header.switcher.set_visible(true);
                header.switcher.set_sensitive(false);
                header.refresh.set_visible(true);
                header.refresh.set_sensitive(false);
                inner.banners.show(&inner.names, true, None);
                self.enable_actions(false);
                inner.pages.set_visible_child_name("down");
                inner.preview.set_frame(None);
                self.show_column(column);
            }
            View::Normal(normal) => {
                inner.announced.set(false);
                inner.screens.set_visible_child_name("main");
                header.switcher.set_visible(true);
                header.switcher.set_sensitive(true);
                header.refresh.set_visible(true);
                header.refresh.set_sensitive(true);
                inner.banners.show(&inner.names, false, normal.banner);
                self.enable_actions(true);
                let (page, toggle) = match normal.page {
                    Page::Display => ("display", &header.display),
                    Page::Configuration => ("config", &header.config),
                };
                inner.pages.set_visible_child_name(page);
                if !toggle.is_active() {
                    toggle.set_active(true);
                }
                self.show_column(&normal.column);
                inner
                    .display
                    .show(&inner.names, &normal.display, normal.config.peaks);
                inner.config.show(&inner.names, &normal.config);
            }
        }
        *inner.shown.borrow_mut() = Some(shown);
    }

    fn enable_actions(&self, on: bool) {
        for action in &self.0.actions {
            action.set_enabled(on);
        }
    }

    fn show_column(&self, column: &Column) {
        let widgets = &self.0.column;
        let names = &self.0.names;
        widgets.caption.set_text(&column.caption);
        widgets.line.set_text(column.line);
        names.describe(self.0.preview.widget(), Some(&column.caption));
        widgets.reason_box.set_visible(column.reason.is_some());
        widgets
            .reason
            .set_text(column.reason.as_deref().unwrap_or(""));
        for (i, row) in widgets.status.iter().enumerate() {
            let item = column.status.get(i);
            row.row.set_visible(item.is_some());
            let Some(item) = item else {
                continue;
            };
            row.label.set_text(item.text);
            let class = match item.dot {
                Dot::Good => "good",
                Dot::Warn => "warn",
                Dot::Bad => "bad",
                Dot::Off => "off",
            };
            row.dot.set_css_classes(&["dot", class]);
        }
        for (row, speed) in [(&widgets.fans, column.fans), (&widgets.pump, column.pump)] {
            let choose = speed == Speed::ChooseInput;
            row.link.set_visible(choose);
            row.value.set_visible(!choose);
            row.value.set_text(&speed.text());
            let class = match speed {
                Speed::Rpm(_) => "num",
                Speed::NotFound => "warn-text",
                _ => "muted",
            };
            row.value.set_css_classes(&[class]);
        }
    }

    /// The view shown last, for the tests.
    pub fn shown(&self) -> Option<View> {
        self.0.shown.borrow().clone()
    }

    pub fn names(&self) -> &Names {
        &self.0.names
    }

    pub fn preview(&self) -> &Preview {
        &self.0.preview
    }

    pub fn banners(&self) -> &Banners {
        &self.0.banners
    }

    pub fn display_page(&self) -> &DisplayPage {
        &self.0.display
    }

    pub fn config_page(&self) -> &ConfigPage {
        &self.0.config
    }

    /// The feedback label's words, while it shows.
    pub fn feedback(&self) -> Option<String> {
        self.0
            .feedback
            .is_visible()
            .then(|| self.0.feedback_label.text().to_string())
    }

    /// The screen shown: `main` or `not-available`; and the page: `display`, `config` or `down`.
    pub fn screen(&self) -> (String, String) {
        let name = |stack: &gtk::Stack| {
            stack
                .visible_child_name()
                .map(|n| n.to_string())
                .unwrap_or_default()
        };
        (name(&self.0.screens), name(&self.0.pages))
    }

    /// Whether the switcher and Refresh show, and whether they can be used.
    pub fn header(&self) -> [(bool, bool); 2] {
        let h = &self.0.header;
        [
            (h.switcher.is_visible(), h.switcher.is_sensitive()),
            (h.refresh.is_visible(), h.refresh.is_sensitive()),
        ]
    }

    /// The column's words: caption, line, reason, the status items shown, the two speeds.
    pub fn column_words(&self) -> Vec<String> {
        let c = &self.0.column;
        let mut words = vec![c.caption.text().to_string(), c.line.text().to_string()];
        if c.reason_box.is_visible() {
            words.push(c.reason.text().to_string());
        }
        for row in c.status.iter().filter(|r| r.row.is_visible()) {
            words.push(row.label.text().to_string());
        }
        for row in [&c.fans, &c.pump] {
            words.push(if row.link.is_visible() {
                "Choose input".to_owned()
            } else {
                row.value.text().to_string()
            });
        }
        words
    }

    /// The not-available message box, for the tests.
    pub fn not_available(&self) -> &gtk::Box {
        &self.0.not_available
    }

    /// Whether the page and Refresh actions can run.
    pub fn actions_enabled(&self) -> bool {
        self.0.actions.iter().all(|a| a.is_enabled())
    }
}
