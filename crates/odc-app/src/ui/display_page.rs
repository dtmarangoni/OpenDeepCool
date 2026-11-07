//! The Display page (`docs/design/settings-app.md`, Display page): the temperature unit, then
//! the tiles in four groups, Load, Temperature and Power, and Music, that act as one radio
//! group. Activating a tile (Enter, Space or a click) asks for its mode at once; focus alone
//! never does. The boxes select nothing themselves, since GTK's selection would follow focus:
//! the saved mode's tile is marked from the service's state (decisions log D55).

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use odc_ipc::TemperatureUnit;
use odc_render::{Kind, Mode, Unit, mode_info};

use super::Request;
use super::names::Names;
use super::tiles::{self, TileWidget};
use crate::model::{self, group_name};
use crate::theme::Tokens;

/// The request a tile's activation sends.
pub fn tile_request(mode: u16) -> Request {
    Request::SetMode(u32::from(mode))
}

/// The request a unit toggle sends.
pub fn unit_request(unit: TemperatureUnit) -> Request {
    Request::SetTemperatureUnit(unit)
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
}

pub struct DisplayPage {
    root: gtk::Box,
    celsius: gtk::ToggleButton,
    fahrenheit: gtk::ToggleButton,
    groups: Vec<GroupWidgets>,
    /// Every tile, mode 1 first.
    tiles: Vec<TileWidget>,
    /// The unit the service confirmed.
    unit: Rc<Cell<TemperatureUnit>>,
    /// The unit and peak setting the pictures are drawn with.
    drawn: Cell<Option<(TemperatureUnit, bool)>>,
    /// The page is setting its own controls: no request.
    applying: Rc<Cell<bool>>,
}

impl DisplayPage {
    pub fn new(names: &Names, send: Rc<dyn Fn(Request)>, tokens: &Tokens) -> DisplayPage {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        let unit = Rc::new(Cell::new(TemperatureUnit::Celsius));
        let applying = Rc::new(Cell::new(false));

        // The unit.
        let celsius = gtk::ToggleButton::builder().label("°C").build();
        let fahrenheit = gtk::ToggleButton::builder()
            .label("°F")
            .group(&celsius)
            .build();
        celsius.set_active(true);
        names.name(&celsius, "Celsius");
        names.name(&fahrenheit, "Fahrenheit");
        for (button, wanted) in [
            (&celsius, TemperatureUnit::Celsius),
            (&fahrenheit, TemperatureUnit::Fahrenheit),
        ] {
            let (send, unit, applying) = (Rc::clone(&send), Rc::clone(&unit), Rc::clone(&applying));
            let (c, f) = (celsius.clone(), fahrenheit.clone());
            button.connect_toggled(move |button| {
                if applying.get() || !button.is_active() || unit.get() == wanted {
                    return;
                }
                send(unit_request(wanted));
                // Back to the saved unit until the service confirms the change.
                applying.set(true);
                match unit.get() {
                    TemperatureUnit::Celsius => c.set_active(true),
                    TemperatureUnit::Fahrenheit => f.set_active(true),
                }
                applying.set(false);
            });
        }
        let unit_row = gtk::Box::builder().spacing(12).build();
        unit_row.append(&gtk::Label::new(Some("Temperature unit")));
        let toggles = gtk::Box::builder().css_classes(["linked"]).build();
        toggles.append(&celsius);
        toggles.append(&fahrenheit);
        unit_row.append(&toggles);
        root.append(&unit_row);

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
                .visible(false)
                .css_classes(["muted", "small"])
                .build();
            names.name(&reasons, &format!("{name} unavailable reasons"));
            column.append(&reasons);
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
                _ => root.append(&column),
            }
            if kind == Kind::Power {
                // Load first, then Temperature and Power side by side, then Music.
                root.insert_child_after(&pair, root.last_child().as_ref());
            }
            groups.push(GroupWidgets { reasons });
        }
        DisplayPage {
            root,
            celsius,
            fahrenheit,
            groups,
            tiles,
            unit,
            drawn: Cell::new(None),
            applying,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Shows `page` and draws the pictures in its unit with `peaks`.
    pub fn show(&self, names: &Names, page: &model::DisplayPage, peaks: bool) {
        self.unit.set(page.unit);
        self.applying.set(true);
        match page.unit {
            TemperatureUnit::Celsius => self.celsius.set_active(true),
            TemperatureUnit::Fahrenheit => self.fahrenheit.set_active(true),
        }
        self.applying.set(false);
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
            widgets.reasons.set_visible(!group.reasons.is_empty());
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

    /// Which unit toggle is on.
    pub fn unit_shown(&self) -> TemperatureUnit {
        if self.fahrenheit.is_active() {
            TemperatureUnit::Fahrenheit
        } else {
            TemperatureUnit::Celsius
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each of the 23 tiles asks for its own mode, and each unit toggle for its unit.
    #[test]
    fn every_tile_and_unit_asks_for_its_own() {
        for mode in 1..=23_u16 {
            assert_eq!(tile_request(mode), Request::SetMode(u32::from(mode)));
        }
        assert_eq!(
            unit_request(TemperatureUnit::Celsius),
            Request::SetTemperatureUnit(TemperatureUnit::Celsius)
        );
        assert_eq!(
            unit_request(TemperatureUnit::Fahrenheit),
            Request::SetTemperatureUnit(TemperatureUnit::Fahrenheit)
        );
    }
}
