//! The window (`docs/design/settings-app.md`, Window): the header bar, with the title in the
//! middle and the gear and the close icon at the end, the cooler banner, the preview column, and
//! the pages (Display and Configuration), or the off screen while the service isn't running; or,
//! in a remote or inactive session, one message. It keeps what the app knows of the service, of
//! the Start at boot switch and of the window's own state, and shows the view model's [`View`]
//! of them after every change. What the window follows (the system's preference, reduced motion,
//! its own state on the compositor, and the theme) is in `window/follow.rs`, and what the tests
//! read of it in `window/probes.rs`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};
use odc_ipc::{CoolerState, State, WindowCorners, WindowOpacity};
use odc_protocol::Frame;
use odc_render::COLUMNS;

use super::banners::Banners;
use super::config_page::ConfigPage;
use super::display_page::DisplayPage;
use super::names::{Names, group};
use super::page::{icon_button, keep_natural_height, scrolled};
use super::pointer;
use super::preview::Preview;
use super::size;
use super::startup::StartupCard;
use super::{REFRESHED_FOR, REFUSED_FOR, Reason, Refused, Request, refusal_text};
use crate::boot::{self, Boot, Place};
use crate::choice;
use crate::model::{
    Column, Dot, NOT_AVAILABLE_HEADING, NOT_AVAILABLE_TEXT, Page, SendingFails, Service, Ui, View,
    view,
};
use crate::pci_ids;
use crate::theme::Tokens;

mod follow;
mod probes;

pub use follow::{away, reduced_motion};

/// The preview column's width, and the most characters its wrapping lines ask room for.
const COLUMN_WIDTH: i32 = 262;
const COLUMN_CHARS: i32 = 28;

/// The most characters the off screen's lines and the not-available message's ask room for, so
/// a long line wraps instead of widening the window (Size and scrolling).
const OFF_SCREEN_CHARS: i32 = 48;
const NOT_AVAILABLE_CHARS: i32 = 40;

/// The room added above the status items to the column's 10 px between its children: 26 px, so
/// the first status item stands 36 px under the caption (owner, 2026-11-22; 14 and 24 before).
const STATUS_GAP: i32 = 26;

/// The status items' accessible names, in the column's order.
const STATUS_NAMES: [&str; 3] = ["Cooler status", "Service status", "Music sound status"];

/// The keyboard shortcuts (Keyboard).
const ACCELS: [(&str, &[&str]); 4] = [
    ("win.configuration", &["<Control>comma"]),
    ("win.back", &["Escape", "<Alt>Left"]),
    ("win.refresh", &["F5", "<Control>r"]),
    ("win.close", &["<Control>w", "<Control>q"]),
];

/// What the page actions are, in the order of [`Inner::actions`].
const ACTION_NAMES: [&str; 3] = ["configuration", "back", "refresh"];

/// Says a text to screen readers.
type Announcer = Box<dyn Fn(&str)>;

/// What the view is made from.
struct Inputs {
    service: Service,
    /// The Start at boot switch.
    boot: Boot,
    ui: Ui,
    sending: SendingFails,
    /// The PCI ID list's names of the GPUs listed now.
    products: HashMap<(u16, u16), String>,
    gpu_ids: Vec<(u16, u16)>,
    pci_ids: PathBuf,
}

/// The header bar's parts: the title in the middle, and the gear and the close icon at the end.
struct Header {
    bar: gtk::HeaderBar,
    title: gtk::Label,
    gear: gtk::ToggleButton,
    close: gtk::Button,
}

struct StatusRow {
    row: gtk::Box,
    dot: gtk::Box,
    label: gtk::Label,
}

struct ColumnWidgets {
    caption: gtk::Label,
    reason_box: gtk::Box,
    reason: gtk::Label,
    status: [StatusRow; 3],
}

struct Inner {
    window: gtk::ApplicationWindow,
    send: Rc<dyn Fn(Request)>,
    names: Names,
    inputs: RefCell<Inputs>,
    shown: RefCell<Option<View>>,
    header: Header,
    /// The scrolled window around everything under the header bar, and the preview column's.
    window_scroller: gtk::ScrolledWindow,
    column_scroller: gtk::ScrolledWindow,
    /// The Omarchy theme, when there is one, and what the window shows now.
    omarchy: Cell<Option<Tokens>>,
    shown_tokens: Cell<Option<Tokens>>,
    /// The service's two theme settings, as it last said (on, dark before it first says), and
    /// the inputs the theme was last resolved from.
    theme_settings: Cell<(bool, bool, WindowOpacity, WindowCorners)>,
    resolved_from: Cell<Option<choice::Inputs>>,
    /// The window is setting the gear itself: its `toggled` is no request.
    applying: Cell<bool>,
    feedback: gtk::Box,
    feedback_icon: gtk::Image,
    feedback_label: gtk::Label,
    feedback_timer: RefCell<Option<glib::SourceId>>,
    banners: Banners,
    screens: gtk::Stack,
    pages: gtk::Stack,
    not_available: gtk::Box,
    /// The off screen: its message, and the box the Startup card moves into.
    off: gtk::Box,
    off_message: gtk::Box,
    off_heading: gtk::Label,
    startup: StartupCard,
    /// The switch had the focus when a flip began: it gets it back at the end.
    flip_focus: Cell<bool>,
    /// The off screen's switch took the first focus since the off screen was entered.
    off_focused: Cell<bool>,
    /// The switch's note last shown, so a new one is announced once.
    note_said: RefCell<Option<String>>,
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
            .resizable(false)
            .build();

        // The header bar: the title in the middle, the gear and the close icon at the end, and
        // no window buttons of the desktop's own, so none is drawn twice (Window).
        let bar = gtk::HeaderBar::new();
        bar.set_show_title_buttons(false);
        let title = gtk::Label::builder()
            .label("OpenDeepCool")
            .css_classes(["app-name"])
            .build();
        bar.set_title_widget(Some(&title));
        let gear = gtk::ToggleButton::new();
        icon_button(
            &gear,
            &names,
            "emblem-system-symbolic",
            "Configuration",
            "Configuration",
        );
        let close = gtk::Button::builder().action_name("win.close").build();
        icon_button(&close, &names, "window-close-symbolic", "Close", "Close");
        let end = gtk::Box::builder().spacing(4).build();
        end.append(&gear);
        end.append(&close);
        bar.pack_end(&end);
        window.set_titlebar(Some(&bar));

        // The banner, then the screens.
        let banners = Banners::new(&names);
        let content = vbox(0);
        content.append(banners.widget());

        // The preview column.
        let preview = Preview::new(tokens);
        names.name(preview.widget(), "Cooler display preview");
        let frame = gtk::Box::builder()
            .css_classes(["preview"])
            .halign(gtk::Align::Start)
            .build();
        frame.append(preview.widget());
        // The caption and the status items are centred in the column (owner, 2026-11-19).
        let caption = label("", &["caption"]);
        caption.set_xalign(0.5);
        caption.set_justify(gtk::Justification::Center);
        let reason = label("", &["warn-text", "small"]);
        // A label's fixed name goes on a group around it: GTK names a label by its own text,
        // again at each change, so a name never sticks to it (decisions log D72).
        let reason_box = group().spacing(6).visible(false).build();
        names.name(&reason_box, "Unavailable reading");
        reason_box.append(
            &gtk::Image::builder()
                .icon_name("dialog-warning-symbolic")
                .css_classes(["warn-text"])
                .valign(gtk::Align::Start)
                .build(),
        );
        reason_box.append(&reason);
        // The status items stand 24 px from what is above them (the column's 10 and 14 more), to
        // set them apart from the caption (owner, 2026-11-19).
        let status_box = vbox(4);
        status_box.set_margin_top(STATUS_GAP);
        let status = STATUS_NAMES.map(|name| {
            let row = group().spacing(8).halign(gtk::Align::Center).build();
            names.name(&row, name);
            let dot = gtk::Box::builder()
                .css_classes(["dot"])
                .valign(gtk::Align::Center)
                .build();
            let text = label("", &[]);
            row.append(&dot);
            row.append(&text);
            status_box.append(&row);
            StatusRow {
                row,
                dot,
                label: text,
            }
        });
        // The column keeps the design's width (the mockups' 262 px), however long its lines.
        let column_box = vbox(10);
        column_box.add_css_class("side-box");
        column_box.set_hexpand(false);
        column_box.set_width_request(COLUMN_WIDTH);
        for line in [&caption, &reason] {
            line.set_max_width_chars(COLUMN_CHARS);
        }
        for widget in [
            frame.upcast_ref::<gtk::Widget>(),
            caption.upcast_ref(),
            reason_box.upcast_ref(),
            status_box.upcast_ref(),
        ] {
            column_box.append(widget);
        }

        // The pages: Display and Configuration, each its bar over its scrolled content, and the
        // off screen; one stack, no motion, and one size so the window doesn't change with it.
        let pages = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::None)
            .hhomogeneous(true)
            .vhomogeneous(true)
            .hexpand(true)
            .vexpand(true)
            .build();
        let display_page = DisplayPage::new(&names, Rc::clone(&send), tokens);
        pages.add_named(display_page.widget(), Some("display"));
        let config_page = ConfigPage::new(&names, Rc::clone(&send));
        pages.add_named(config_page.widget(), Some("config"));
        let startup = StartupCard::new(&names, Rc::clone(&send));
        // Until the card has stood on the page, its room there is its natural height.
        config_page.reserve(startup.card().measure(gtk::Orientation::Vertical, -1).1);

        // The off screen (D10): the message, then the Startup card, centred.
        let off = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        let off_message = group()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .build();
        names.name(&off_message, "Service message");
        let off_heading = label("", &["heading"]);
        let off_text = label(boot::OFF_SCREEN_TEXT, &["muted"]);
        for line in [&off_heading, &off_text] {
            line.set_justify(gtk::Justification::Center);
            line.set_xalign(0.5);
            line.set_max_width_chars(OFF_SCREEN_CHARS);
            off_message.append(line);
        }
        off.append(&off_message);
        pages.add_named(&scrolled(&off), Some("off"));
        // The column scrolls down on its own, so the picture stays put while a page scrolls.
        let column_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_width(true)
            .propagate_natural_height(true)
            .vexpand(true)
            .focusable(false)
            .css_classes(["side"])
            .child(&column_box)
            .build();
        keep_natural_height(&column_scroller);
        let main = gtk::Box::builder().vexpand(true).build();
        main.append(&column_scroller);
        // The feedback label is centred over the pages, the right panel, not over the window
        // (owner, 2026-11-19); the pages keep the window's room as they did.
        let pages_area = gtk::Overlay::builder()
            .child(&pages)
            .hexpand(true)
            .vexpand(true)
            .build();
        main.append(&pages_area);

        // Not available in this session: two labels, centred, wrapping.
        // A named box takes the group role: ARIA lets no generic one carry a name (D48).
        let not_available = gtk::Box::builder()
            .accessible_role(gtk::AccessibleRole::Group)
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        for (text, classes) in [
            (NOT_AVAILABLE_HEADING, &["heading"][..]),
            (NOT_AVAILABLE_TEXT, &["muted"][..]),
        ] {
            let line = label(text, classes);
            line.set_justify(gtk::Justification::Center);
            line.set_xalign(0.5);
            line.set_max_width_chars(NOT_AVAILABLE_CHARS);
            not_available.append(&line);
        }
        names.name(&not_available, "Not available in this session");
        names.describe(
            &not_available,
            Some(&format!("{NOT_AVAILABLE_HEADING}. {NOT_AVAILABLE_TEXT}")),
        );
        // Not homogeneous: the not-available message is a small window of its own.
        let screens = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::None)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .vexpand(true)
            .build();
        screens.add_named(&main, Some("main"));
        screens.add_named(&not_available, Some("not-available"));
        content.append(&screens);

        // The feedback label over the pages (added to their overlay below).
        let feedback_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
        feedback_icon.add_css_class("warn-text");
        let feedback_label = label("", &[]);
        let feedback = group()
            .spacing(6)
            .css_classes(["feedback"])
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            // 64 px above the panel's foot (owner, 2026-11-22: three times the 16 px, then one
            // more step of 16).
            .margin_bottom(64)
            .visible(false)
            .build();
        names.name(&feedback, "Change feedback");
        feedback.append(&feedback_icon);
        feedback.append(&feedback_label);
        // The window takes its content's natural size, capped at the smallest monitor's less a
        // margin, and scrolls inside the cap (Size and scrolling).
        let window_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_width(true)
            .propagate_natural_height(true)
            .hexpand(true)
            .vexpand(true)
            .focusable(false)
            .child(&content)
            .build();
        if let Some((width, height)) = size::cap(&size::monitors(&WidgetExt::display(&window))) {
            window_scroller.set_max_content_width(width);
            window_scroller.set_max_content_height(height - size::HEADER);
        }
        pages_area.add_overlay(&feedback);
        window.set_child(Some(&window_scroller));
        // A click operates a control and selects or focuses nothing else (Task 56).
        pointer::calm(&window);

        // The one style provider, at APPLICATION priority (Theme, How it's applied).
        let provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &WidgetExt::display(&window),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let actions = ACTION_NAMES.map(|name| {
            let action = gio::SimpleAction::new(name, None);
            window.add_action(&action);
            action
        });
        let close_action = gio::SimpleAction::new("close", None);
        window.add_action(&close_action);
        for (action, accels) in ACCELS {
            app.set_accels_for_action(action, accels);
        }

        let inner = Rc::new(Inner {
            window,
            send,
            names,
            inputs: RefCell::new(Inputs {
                service: Service::Down,
                boot: Boot::default(),
                ui: Ui::default(),
                sending: SendingFails::default(),
                products: HashMap::new(),
                gpu_ids: Vec::new(),
                pci_ids,
            }),
            shown: RefCell::new(None),
            header: Header {
                bar,
                title,
                gear,
                close,
            },
            window_scroller,
            column_scroller,
            omarchy: Cell::new(None),
            shown_tokens: Cell::new(None),
            theme_settings: Cell::new((true, true, WindowOpacity::Opaque, WindowCorners::Rounded)),
            resolved_from: Cell::new(None),
            applying: Cell::new(false),
            feedback,
            feedback_icon,
            feedback_label,
            feedback_timer: RefCell::new(None),
            banners,
            screens,
            pages,
            not_available,
            off,
            off_message,
            off_heading,
            startup,
            flip_focus: Cell::new(false),
            off_focused: Cell::new(false),
            note_said: RefCell::new(None),
            column: ColumnWidgets {
                caption,
                reason_box,
                reason,
                status,
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
        window.connect(&close_action);
        window.set_theme(tokens);
        window.follow_preference();
        window.follow_motion();
        window.follow_toplevel();
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
        let [configuration, back, refresh] = &self.0.actions;
        let go = with(|w| w.go(Page::Configuration));
        configuration.connect_activate(move |_, _| go());
        let go = with(|w| w.go(Page::Display));
        back.connect_activate(move |_, _| go());
        let ask = with(|w| (w.0.send)(Request::Refresh));
        refresh.connect_activate(move |_, _| ask());
        let shut = with(|w| w.0.window.close());
        close.connect_activate(move |_, _| shut());
        // The gear is a toggle: pressed is the Configuration page, and pressing it again goes
        // back. The window setting it itself is no request.
        let go = with(|w| {
            if !w.0.applying.get() {
                w.go(if w.0.header.gear.is_active() {
                    Page::Configuration
                } else {
                    Page::Display
                });
            }
        });
        self.0.header.gear.connect_toggled(move |_| go());
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

    /// The Start at boot switch: what systemd says, and the flip that runs. At a flip's start the
    /// switch's focus is noted; at its end, screen readers hear the switch's state, and the
    /// switch gets the focus back when it had it (A flip, step by step).
    pub fn boot(&self, boot: Boot) {
        let before = std::mem::replace(&mut self.0.inputs.borrow_mut().boot, boot.clone());
        if !before.flipping() && boot.flipping() {
            self.0.flip_focus.set(self.0.startup.switch().is_focus());
        }
        let ended = before.flipping() && !boot.flipping();
        if ended {
            let state = if boot.on() {
                boot::NOW_ON
            } else {
                boot::NOW_OFF
            };
            (self.0.announcer.borrow())(state);
        }
        self.render();
        if ended && self.0.flip_focus.replace(false) {
            self.0.startup.switch().grab_focus();
        }
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
        self.0.theme_settings.set((
            state.use_system_theme.unwrap_or(true),
            state.dark_theme.unwrap_or(true),
            state.window_opacity.unwrap_or(WindowOpacity::Opaque),
            state.window_corners.unwrap_or(WindowCorners::Rounded),
        ));
        inputs.service = Service::Up(Box::new(state));
        let wait = inputs.sending.follow(cooler, Instant::now());
        drop(inputs);
        self.apply_theme();
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

    /// One tick's `Levels` from the service, for the live Music tiles.
    pub fn levels(&self, levels: &[f64; COLUMNS]) {
        self.0.display.levels(levels);
    }

    /// The service confirmed a Refresh.
    pub fn refreshed(&self) {
        // The cooler's picture starts again, and so do the tiles, together with it.
        self.0.display.restart();
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

    /// The user asked for `page`: the gear, a shortcut or the back icon. The page's title is
    /// announced, and the focus goes to the back icon when the page opens and to the gear when
    /// it closes (Window, The pages); the window returning to a page after the service did
    /// does neither.
    fn go(&self, page: Page) {
        {
            let mut inputs = self.0.inputs.borrow_mut();
            if inputs.ui.page == page {
                return;
            }
            inputs.ui.page = page;
        }
        self.render();
        if !matches!(*self.0.shown.borrow(), Some(View::Normal(_))) {
            return;
        }
        match page {
            Page::Configuration => {
                (self.0.announcer.borrow())("Configuration");
                self.0.config.back().grab_focus();
            }
            Page::Display => {
                (self.0.announcer.borrow())("Display");
                self.0.header.gear.grab_focus();
            }
        }
    }

    /// The gear: shown or not, usable or not, pressed or not, without asking for a page.
    fn show_gear(&self, visible: bool, sensitive: bool, pressed: bool) {
        let gear = &self.0.header.gear;
        self.0.applying.set(true);
        gear.set_visible(visible);
        gear.set_sensitive(sensitive);
        gear.set_active(pressed);
        self.0.applying.set(false);
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
            view(&inputs.service, inputs.ui, &inputs.products, &inputs.boot)
        };
        let inner = &self.0;
        match &shown {
            View::NotAvailable => {
                inner.screens.set_visible_child_name("not-available");
                self.show_gear(false, false, false);
                inner.banners.show(&inner.names, None);
                self.enable_actions([false; 3]);
                inner.off_focused.set(false);
                if !inner.announced.replace(true) {
                    (inner.announcer.borrow())(&format!(
                        "{NOT_AVAILABLE_HEADING}. {NOT_AVAILABLE_TEXT}"
                    ));
                }
            }
            View::ServiceDown(down) => {
                inner.announced.set(false);
                inner.screens.set_visible_child_name("main");
                self.show_gear(true, false, false);
                inner.banners.show(&inner.names, None);
                self.enable_actions([false; 3]);
                inner.pages.set_visible_child_name("off");
                inner.preview.set_frame(None);
                self.show_column(&down.column);
                self.show_off_screen(&down.screen);
            }
            View::Normal(normal) => {
                inner.announced.set(false);
                inner.screens.set_visible_child_name("main");
                let on_config = normal.page == Page::Configuration;
                self.show_gear(true, true, on_config);
                inner.banners.show(&inner.names, normal.banner);
                // Configuration and Refresh work on either page; Back only on Configuration.
                self.enable_actions([true, on_config, true]);
                inner.off_focused.set(false);
                inner
                    .pages
                    .set_visible_child_name(if on_config { "config" } else { "display" });
                self.show_column(&normal.column);
                inner.display.show(&inner.names, &normal.display);
                inner.config.show(&inner.names, &normal.config);
                inner.startup.place(
                    Place::Card,
                    inner.config.cards(),
                    inner.config.label_group(),
                );
                inner.config.spacer().set_visible(false);
                inner.startup.show(&inner.names, &normal.startup);
                self.announce_note(normal.startup.note.as_ref());
            }
        }
        *inner.shown.borrow_mut() = Some(shown);
    }

    /// The off screen: its heading and text, then the Startup card; nothing until the first read
    /// of systemd, so no heading flashes.
    fn show_off_screen(&self, screen: &boot::OffScreen) {
        let inner = &self.0;
        let startup = &inner.startup;
        // The card leaves the Configuration page: the page keeps its room, so the window, which
        // is as tall as the tallest page, doesn't shrink (Size and scrolling).
        // `compute_bounds` is the card's whole box; `height()` leaves its padding out.
        if let Some(bounds) = startup.card().compute_bounds(inner.config.cards())
            && bounds.height() > 0.0
        {
            inner.config.reserve(bounds.height().round() as i32);
        }
        startup.place(Place::OffScreen, &inner.off, inner.config.label_group());
        inner.config.spacer().set_visible(true);
        let Some(heading) = screen.heading else {
            inner.off_message.set_visible(false);
            startup.hide_card();
            inner.names.describe(&inner.off_message, None);
            return;
        };
        inner.off_message.set_visible(true);
        inner.off_heading.set_text(heading);
        inner.names.describe(
            &inner.off_message,
            Some(&format!("{heading}. {}", boot::OFF_SCREEN_TEXT)),
        );
        startup.show(&inner.names, &screen.switch);
        if !screen.card {
            startup.hide_card();
            return;
        }
        self.announce_note(screen.switch.note.as_ref());
        // The switch takes the first focus when it's sensitive, so Space turns OpenDeepCool on.
        if screen.switch.sensitive && !inner.off_focused.replace(true) {
            startup.switch().grab_focus();
        }
    }

    /// Announces the switch's note when a new one shows.
    fn announce_note(&self, note: Option<&boot::Note>) {
        let text = note.map(|note| note.text.clone());
        let mut said = self.0.note_said.borrow_mut();
        if text != *said {
            if let Some(text) = &text {
                (self.0.announcer.borrow())(text);
            }
            *said = text;
        }
    }

    /// Which of the page actions work: Configuration, Back and Refresh.
    fn enable_actions(&self, on: [bool; 3]) {
        for (action, on) in self.0.actions.iter().zip(on) {
            action.set_enabled(on);
        }
    }

    fn show_column(&self, column: &Column) {
        let widgets = &self.0.column;
        let names = &self.0.names;
        widgets.caption.set_text(&column.caption);
        names.describe(self.0.preview.widget(), Some(&column.description()));
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
    }
}
