//! The Display page (`docs/design/settings-app.md`, Display page): its bar, with the title and
//! the Refresh icon, then the tiles in four groups, Load, Temperature and Power, and Music, that
//! act as one radio group. Activating a tile (Enter, Space or a click) asks for its mode at once;
//! focus alone never does. The boxes select nothing themselves, since GTK's selection would
//! follow focus: the saved mode's tile is marked from the service's state (decisions log D55).
//! The temperature tiles are drawn in the unit the Configuration page sets.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use odc_ipc::TemperatureUnit;
use odc_render::{Kind, Mode, Unit, mode_info};

use super::Request;
use super::names::{Names, group};
use super::page::{icon_button, page, page_bar};
use super::tiles::{self, TileWidget};
use crate::model::{self, group_name};
use crate::theme::Tokens;

/// The request a tile's activation sends.
pub fn tile_request(mode: u16) -> Request {
    Request::SetMode(u32::from(mode))
}

/// The most tiles in a row, per group: Load a full row, Temperature and Power three each, side
/// by side, Music two rows.
fn per_line(kind: Kind) -> u32 {
    match kind {
        Kind::Load | Kind::Music => 6,
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
    /// Every tile, mode 1 first.
    tiles: Vec<TileWidget>,
    /// The unit and peak setting the pictures are drawn with.
    drawn: Cell<Option<(TemperatureUnit, bool)>>,
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
            let modes: Vec<u16> = Mode::all()
                .filter(|m| mode_info(*m).kind == kind)
                .map(Mode::number)
                .collect();
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
        // The page bar: the title, and Refresh at its end (Refresh).
        let refresh = gtk::Button::builder().action_name("win.refresh").build();
        icon_button(
            &refresh,
            names,
            "view-refresh-symbolic",
            "Refresh display",
            "Refresh the display: fresh readings, and the picture starts again",
        );
        let root = page(&page_bar(None, "Display", Some(&refresh)), &content);
        DisplayPage {
            root,
            refresh,
            groups,
            tiles,
            drawn: Cell::new(None),
        }
    }

    /// The page: its bar over the scrolled tiles.
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// The Refresh icon, for the tests.
    pub fn refresh(&self) -> &gtk::Button {
        &self.refresh
    }

    /// Shows `page` and draws the pictures in its unit with `peaks`.
    pub fn show(&self, names: &Names, page: &model::DisplayPage, peaks: bool) {
        if self.drawn.get() != Some((page.unit, peaks)) {
            self.drawn.set(Some((page.unit, peaks)));
            let unit = match page.unit {
                TemperatureUnit::Celsius => Unit::Celsius,
                TemperatureUnit::Fahrenheit => Unit::Fahrenheit,
            };
            for tile in &self.tiles {
                tile.set_picture(tiles::picture(tile.mode, unit, peaks));
            }
        }
        for (group, widgets) in page.groups.iter().zip(&self.groups) {
            widgets.reasons.set_text(&group.reasons.join("\n"));
            widgets.reasons_group.set_visible(!group.reasons.is_empty());
            for tile in &group.tiles {
                if let Some(widget) = self.tiles.get(usize::from(tile.mode) - 1) {
                    widget.show(names, tile);
                }
            }
        }
    }

    pub fn set_theme(&self, tokens: &Tokens) {
        for tile in &self.tiles {
            tile.set_theme(tokens);
        }
    }

    /// Every tile, mode 1 first, for the tests.
    pub fn tiles(&self) -> &[TileWidget] {
        &self.tiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each of the 23 tiles asks for its own mode.
    #[test]
    fn every_tile_asks_for_its_own_mode() {
        for mode in 1..=23_u16 {
            assert_eq!(tile_request(mode), Request::SetMode(u32::from(mode)));
        }
    }
}
