//! The Display page (`docs/design/settings-app.md`, Display page): its bar, with the title and
//! the Refresh icon, then the tiles in four groups, Load, Temperature and Power, and Music, that
//! act as one radio group. Activating a tile (Enter, Space or a click) asks for its mode at once;
//! focus alone never does. The boxes select nothing themselves, since GTK's selection would
//! follow focus: the saved mode's tile is marked from the service's state (decisions log D55).
//! The temperature tiles are drawn in the unit the Configuration page sets.
//!
//! The tiles are live (Phase 11a Task 14; design: Live tiles): each is drawn by `odc_render` from
//! the service's readings and moves as the cooler's picture does. One tick of 100 ms, counted
//! from the page's frame clock, steps them all together, and it exists only while a tile is on
//! screen, the window isn't minimised or suspended and the system hasn't asked for reduced
//! motion. The Music tiles, the emoji too, move with the service's `Levels`, once for each, and only while the
//! sound is live.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk::glib;
use gtk::prelude::*;
use odc_render::{Kind, Mode, MusicSettings, Unit};

use super::Request;
use super::names::{Names, group};
use super::page::{icon_button, page, page_bar, scroller_of};
use super::tiles::TileWidget;
use crate::live::{Beat, Live, sample_values};
use crate::model::{self, LiveInput, group_modes, group_name};
use crate::theme::Tokens;

/// The request a tile's activation sends.
fn tile_request(mode: u16) -> Request {
    Request::SetMode(u32::from(mode))
}

/// The most tiles in a row, per group: Load a full row, Temperature and Power three each, side
/// by side, and Music's thirteen in three rows of five, five and the last three, the emoji alone
/// (owner, 2026-11-23: six emoji options now, three of them alone), so the window stays as wide
/// as Load's six.
fn per_line(kind: Kind) -> u32 {
    match kind {
        Kind::Load => 6,
        Kind::Music => 5,
        Kind::Temperature | Kind::Power => 3,
    }
}

struct GroupWidgets {
    reasons: gtk::Label,
    /// The group named "<group> unavailable reasons" around them (decisions log D72).
    reasons_group: gtk::Box,
}

pub struct DisplayPage {
    /// The page: its bar over the scrolled tiles.
    root: gtk::Box,
    refresh: gtk::Button,
    groups: Vec<GroupWidgets>,
    /// The tiles and what moves them.
    motion: Rc<Motion>,
}

/// What the tiles are drawn from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Input {
    unit: Unit,
    live: LiveInput,
}

fn unit_of(unit: odc_ipc::TemperatureUnit) -> Unit {
    match unit {
        odc_ipc::TemperatureUnit::Celsius => Unit::Celsius,
        odc_ipc::TemperatureUnit::Fahrenheit => Unit::Fahrenheit,
    }
}

/// The tiles, their pictures and the tick that moves them, shared with the callbacks.
struct Motion {
    /// Every tile, mode 1 first.
    tiles: Vec<TileWidget>,
    root: gtk::Box,
    live: RefCell<Live>,
    beat: RefCell<Beat>,
    /// The frame clock's callback: there exactly while the tiles move.
    tick: RefCell<Option<gtk::TickCallbackId>>,
    input: RefCell<Input>,
    /// A tile picture is on screen.
    mapped: Cell<bool>,
    /// The window is minimised or suspended.
    away: Cell<bool>,
    /// The system asks for reduced motion.
    reduced: Cell<bool>,
    /// The input changed while no tile was visible, so no picture was redrawn: they are drawn
    /// again from it when a tile is visible (design: Live tiles, When the tick runs).
    stale: Cell<bool>,
    /// Steps taken since the app started, for the tests.
    steps: Cell<u64>,
}

impl Motion {
    fn new(tiles: Vec<TileWidget>, root: gtk::Box) -> Rc<Motion> {
        let input = Input {
            unit: Unit::Celsius,
            live: LiveInput {
                values: sample_values(),
                music: MusicSettings::default(),
                sound: false,
                restart: (1, odc_ipc::TemperatureUnit::Celsius, String::new()),
            },
        };
        let motion = Rc::new(Motion {
            tiles,
            root,
            live: RefCell::new(Live::new(
                input.unit,
                input.live.music,
                input.live.values.clone(),
            )),
            beat: RefCell::new(Beat::new()),
            tick: RefCell::new(None),
            input: RefCell::new(input),
            mapped: Cell::new(false),
            away: Cell::new(false),
            reduced: Cell::new(false),
            stale: Cell::new(false),
            steps: Cell::new(0),
        });
        motion.push_all();
        let weak = Rc::downgrade(&motion);
        motion.root.connect_map({
            let weak = weak.clone();
            move |_| Motion::set(&weak, |m| m.mapped.set(true))
        });
        motion
            .root
            .connect_unmap(move |_| Motion::set(&weak, |m| m.mapped.set(false)));
        motion
    }

    /// `change`, then the tick follows what the tiles are allowed.
    fn set(weak: &Weak<Motion>, change: impl FnOnce(&Motion)) {
        if let Some(motion) = weak.upgrade() {
            change(&motion);
            motion.follow();
        }
    }

    /// Whether a tile can be seen: on screen, the window shown.
    fn visible(&self) -> bool {
        self.mapped.get() && !self.away.get()
    }

    /// Whether the tiles may move: seen, and no reduced motion.
    fn allowed(&self) -> bool {
        self.visible() && !self.reduced.get()
    }

    fn moving(&self) -> bool {
        self.tick.borrow().is_some()
    }

    /// Starts the tick when the tiles may move and it isn't there, and stops it when they may
    /// not and it is. A start draws every tile at home; a stop leaves every tile at rest; tiles
    /// seen again at rest are drawn from what the input became meanwhile.
    fn follow(self: &Rc<Self>) {
        let (allowed, moving) = (self.allowed(), self.moving());
        if self.visible() && !allowed && !moving && self.stale.replace(false) {
            self.rebuild();
        }
        match (allowed, moving) {
            (true, false) => {
                self.rebuild();
                self.beat.borrow_mut().restart();
                let weak = Rc::downgrade(self);
                let id = self.root.add_tick_callback(move |_, clock| {
                    if let Some(motion) = weak.upgrade() {
                        motion.frame(clock.frame_time());
                    }
                    glib::ControlFlow::Continue
                });
                *self.tick.borrow_mut() = Some(id);
            }
            (false, true) => {
                if let Some(id) = self.tick.borrow_mut().take() {
                    id.remove();
                }
                self.rebuild();
            }
            _ => {}
        }
    }

    /// One call of the frame clock: a step when 100 ms are due.
    fn frame(&self, now: i64) {
        if self.beat.borrow_mut().due(now) {
            let changed = self.live.borrow_mut().tick();
            self.steps.set(self.steps.get() + 1);
            self.push(&changed);
        }
    }

    /// Every tile at home, as the input says, and the sound not live until its first `Levels`.
    fn rebuild(&self) {
        self.stale.set(false);
        let input = self.input.borrow();
        *self.live.borrow_mut() =
            Live::new(input.unit, input.live.music, input.live.values.clone());
        drop(input);
        self.push_all();
    }

    fn push(&self, modes: &[Mode]) {
        let live = self.live.borrow();
        for mode in modes {
            // By the tile's own mode: the modes aren't numbered 1 to the tile count (17 is none).
            if let Some(tile) = self.tiles.iter().find(|tile| tile.mode == *mode) {
                tile.set_picture(*live.frame(*mode));
            }
        }
    }

    fn push_all(&self) {
        let live = self.live.borrow();
        for tile in &self.tiles {
            tile.set_picture(*live.frame(tile.mode));
        }
    }

    /// The tiles' new input from the service's state. A restart event, or tiles at rest, draw
    /// every tile again from it; moving tiles take a new reading as the cooler does, and the
    /// Music settings from the next picture.
    fn take(&self, unit: Unit, live: &LiveInput) {
        let next = Input {
            unit,
            live: live.clone(),
        };
        let previous = self.input.replace(next.clone());
        if previous == next {
            return;
        }
        // Nothing is drawn while no tile can be seen: the input is taken in, and the pictures are
        // drawn from it when one can.
        if !self.visible() {
            self.stale.set(true);
            return;
        }
        let restart = previous.live.restart != next.live.restart;
        if !self.moving() || restart {
            self.rebuild();
            if self.moving() {
                self.beat.borrow_mut().restart();
            }
            return;
        }
        let mut changed = Vec::new();
        {
            let mut live = self.live.borrow_mut();
            if previous.live.values != next.live.values {
                live.set_values(next.live.values.clone());
            }
            if previous.live.music != next.live.music {
                changed.extend(live.set_music(next.live.music));
            }
            if !next.live.sound {
                changed.extend(live.set_sound(false));
            }
        }
        self.push(&changed);
    }

    /// One tick's `Levels`: while the tiles move and the sound can be live, the first starts the
    /// Music tiles at rest and each steps them once; otherwise it is dropped.
    fn levels(&self, levels: &[f64; odc_render::COLUMNS]) {
        if !self.moving() || !self.input.borrow().live.sound {
            return;
        }
        let mut changed = Vec::new();
        {
            let mut live = self.live.borrow_mut();
            if !live.sound() {
                changed.extend(live.set_sound(true));
            }
            changed.extend(live.levels(levels));
        }
        self.push(&changed);
    }

    /// Every tile at home again, together: a successful Refresh.
    fn restart(&self) {
        if self.moving() {
            self.rebuild();
            self.beat.borrow_mut().restart();
        }
    }
}

impl DisplayPage {
    pub fn new(names: &Names, send: Rc<dyn Fn(Request)>, tokens: &Tokens) -> DisplayPage {
        // Under the page bar the content starts 6 px down (Window, Page bar).
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(6)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        // The tiles, group by group.
        let mut tiles = Vec::new();
        let mut groups = Vec::new();
        let pair = gtk::Box::builder().spacing(24).build();
        for kind in [Kind::Load, Kind::Temperature, Kind::Power, Kind::Music] {
            let name = group_name(kind);
            let column = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(8)
                .build();
            column.append(
                &gtk::Label::builder()
                    .label(name)
                    .xalign(0.0)
                    .css_classes(["group-heading"])
                    .build(),
            );
            let reasons = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .css_classes(["muted", "small"])
                .build();
            let reasons_group = group().visible(false).build();
            names.name(&reasons_group, &format!("{name} unavailable reasons"));
            reasons_group.append(&reasons);
            column.append(&reasons_group);
            let flowbox = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .activate_on_single_click(true)
                .homogeneous(true)
                .min_children_per_line(1)
                .max_children_per_line(per_line(kind))
                .column_spacing(8)
                .row_spacing(8)
                .valign(gtk::Align::Start)
                .build();
            names.name(&flowbox, &format!("{name} options"));
            let modes: Vec<u16> = group_modes(kind).into_iter().map(Mode::number).collect();
            for &mode in &modes {
                let tile = TileWidget::new(names, Mode::new(mode).expect("a mode"), tokens);
                flowbox.append(&tile.child);
                tiles.push(tile);
            }
            let send = Rc::clone(&send);
            flowbox.connect_child_activated(move |_, child| {
                if let Some(&mode) = usize::try_from(child.index())
                    .ok()
                    .and_then(|i| modes.get(i))
                {
                    send(tile_request(mode));
                }
            });
            column.append(&flowbox);
            match kind {
                Kind::Temperature | Kind::Power => pair.append(&column),
                _ => content.append(&column),
            }
            if kind == Kind::Power {
                // Load first, then Temperature and Power side by side, then Music.
                content.insert_child_after(&pair, content.last_child().as_ref());
            }
            groups.push(GroupWidgets {
                reasons,
                reasons_group,
            });
        }
        // The tiles are kept in the modes' own order, whatever order they stand in the groups.
        tiles.sort_by_key(|tile| tile.mode);
        // The page bar: the title centred over the panel, and Refresh at its end (Refresh).
        let refresh = gtk::Button::builder().action_name("win.refresh").build();
        icon_button(
            &refresh,
            names,
            "view-refresh-symbolic",
            "Refresh display",
            "Refresh the display readings",
        );
        let root = page(&page_bar(None, "Display", Some(&refresh)), &content);
        let motion = Motion::new(tiles, root.clone());
        DisplayPage {
            root,
            refresh,
            groups,
            motion,
        }
    }

    /// The page: its bar over the scrolled tiles.
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// The page's scrolled window, for the tests.
    pub fn scroller(&self) -> gtk::ScrolledWindow {
        scroller_of(&self.root)
    }

    /// The Refresh icon, for the tests.
    pub fn refresh(&self) -> &gtk::Button {
        &self.refresh
    }

    /// Shows `page`: the pictures drawn from its readings, unit and Music settings, and each
    /// tile's state.
    pub fn show(&self, names: &Names, page: &model::DisplayPage) {
        self.motion.take(unit_of(page.unit), &page.live);
        for (group, widgets) in page.groups.iter().zip(&self.groups) {
            widgets.reasons.set_text(&group.reasons.join("\n"));
            widgets.reasons_group.set_visible(!group.reasons.is_empty());
            for tile in &group.tiles {
                if let Some(widget) = self
                    .motion
                    .tiles
                    .iter()
                    .find(|w| w.mode.number() == tile.mode)
                {
                    widget.show(names, tile);
                }
            }
        }
    }

    pub fn set_theme(&self, tokens: &Tokens) {
        for tile in &self.motion.tiles {
            tile.set_theme(tokens);
        }
    }

    /// One tick's `Levels` from the service, for the Music tiles.
    pub fn levels(&self, levels: &[f64; odc_render::COLUMNS]) {
        self.motion.levels(levels);
    }

    /// Every tile at home again: a successful Refresh.
    pub fn restart(&self) {
        self.motion.restart();
    }

    /// The window is minimised or suspended, or not any more.
    pub fn set_away(&self, away: bool) {
        self.motion.away.set(away);
        self.motion.follow();
    }

    /// The system asks for reduced motion, or not any more.
    pub fn set_reduced(&self, reduced: bool) {
        self.motion.reduced.set(reduced);
        self.motion.follow();
    }

    /// Whether the tick is running: the tiles move.
    pub fn moving(&self) -> bool {
        self.motion.moving()
    }

    /// The steps the tick has taken since the app started, for the tests.
    pub fn steps(&self) -> u64 {
        self.motion.steps.get()
    }

    /// Every tile, mode 1 first, for the tests.
    pub fn tiles(&self) -> &[TileWidget] {
        &self.motion.tiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each of the 25 tiles asks for its own mode.
    #[test]
    fn every_tile_asks_for_its_own_mode() {
        for mode in Mode::all() {
            let mode = mode.number();
            assert_eq!(tile_request(mode), Request::SetMode(u32::from(mode)));
        }
    }
}
