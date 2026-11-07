//! The real run's scenarios (Phase 10 plan, Task 9; decisions log D83): the real app against the
//! real dev service on the system bus, with no test session held, so the app's own changes are
//! the service's to take. The driver knows the service only through what the app shows: each
//! change counts once the app shows it, which it does only once the service confirms it. The
//! harness reads the owner's settings first and sets them back afterwards; the preview against
//! the dev recording is the harness's own check, in a test session after these.

use std::thread;
use std::time::Duration;

use atspi::State;

use crate::driver::{Driver, Node, Result, wait};
use crate::scenarios::{TILES, caption, tile};

pub type Scenario = fn(&mut Driver) -> Result<String>;

pub const ALL: [(&str, Scenario); 5] = [
    ("real_connected", connected),
    ("real_tiles", tiles),
    ("real_unit", unit),
    ("real_rotation", rotation),
    ("real_refresh", refresh),
];

/// Waits until `name` shows with `state` on or off.
fn expect(driver: &Driver, role: &str, name: &str, state: State, on: bool) -> Result<()> {
    let what = if on { "with" } else { "without" };
    wait(&format!("{role} {name:?} {what} {state:?}"), || {
        let Some(node) = driver.find(role, name)? else {
            return Ok(None);
        };
        Ok((driver.has(&node, state)? == on).then_some(()))
    })
}

/// The labels under the group `name`, once it shows.
fn texts(driver: &Driver, name: &str) -> Result<Vec<String>> {
    let node = driver.shown("group", name)?;
    driver.texts(&node)
}

/// The tile shown as chosen: the mode the service has saved.
fn chosen(driver: &Driver) -> Result<u32> {
    wait("a chosen tile", || {
        for mode in 1..=23 {
            if let Some(node) = driver.find("grid cell", &tile(mode))?
                && driver.has(&node, State::Selected)?
            {
                return Ok(Some(mode));
            }
        }
        Ok(None)
    })
}

fn page(driver: &Driver, name: &str) -> Result<()> {
    let toggle = driver.shown("toggle button", name)?;
    if !driver.has(&toggle, State::Pressed)? {
        driver.act(&toggle, "Click")?;
    }
    expect(driver, "toggle button", name, State::Pressed, true)
}

/// The app shows the real service: running, a mode chosen, the cooler's status.
fn connected(driver: &mut Driver) -> Result<String> {
    wait("the service running", || {
        Ok(texts(driver, "Service status")?
            .iter()
            .any(|t| t == "Service running")
            .then_some(()))
    })?;
    let mode = chosen(driver)?;
    let cooler = texts(driver, "Cooler status")?.join(" ");
    Ok(format!("{} saved, {cooler}", tile(mode)))
}

/// Each tile the service can show here, chosen in the app, then the saved one again: the
/// service confirms each, and the preview's caption follows (Coverage check, One choice, one
/// mode). A tile unavailable on this PC is insensitive, and left out.
fn tiles(driver: &mut Driver) -> Result<String> {
    page(driver, "Display")?;
    let saved = chosen(driver)?;
    let mut skipped = Vec::new();
    for mode in (1..=23).filter(|&m| m != saved).chain([saved]) {
        let name = tile(mode);
        let node: Node = driver.shown("grid cell", &name)?;
        if !driver.has(&node, State::Sensitive)? {
            skipped.push(name);
            continue;
        }
        driver.act(&node, "tile.activate")?;
        expect(driver, "grid cell", &name, State::Selected, true)?;
        let preview = driver.shown("image", "Cooler display preview")?;
        let want = caption(mode);
        wait(&format!("the preview captioned {want:?}"), || {
            Ok((driver.description(&preview)? == want).then_some(()))
        })?;
    }
    Ok(if skipped.is_empty() {
        format!("{} tiles", TILES.len())
    } else {
        format!("unavailable here: {}", skipped.join(", "))
    })
}

/// The other unit, then the saved one again.
fn unit(driver: &mut Driver) -> Result<String> {
    page(driver, "Display")?;
    let celsius = driver.shown("toggle button", "Celsius")?;
    let (saved, other) = if driver.has(&celsius, State::Pressed)? {
        ("Celsius", "Fahrenheit")
    } else {
        ("Fahrenheit", "Celsius")
    };
    for name in [other, saved] {
        let toggle = driver.shown("toggle button", name)?;
        driver.act(&toggle, "Click")?;
        expect(driver, "toggle button", name, State::Pressed, true)?;
    }
    Ok(format!("{saved} saved"))
}

/// Each rotation, ending on the saved one.
fn rotation(driver: &mut Driver) -> Result<String> {
    page(driver, "Configuration")?;
    let names: Vec<String> = (0..4).map(|r| format!("Rotation {}°", r * 90)).collect();
    let saved = wait("a pressed rotation", || {
        for (turn, name) in names.iter().enumerate() {
            let node = driver.shown("toggle button", name)?;
            if driver.has(&node, State::Pressed)? {
                return Ok(Some(turn));
            }
        }
        Ok(None)
    })?;
    for turn in (1..=4).map(|step| (saved + step) % 4) {
        let node = driver.shown("toggle button", &names[turn])?;
        driver.act(&node, "Click")?;
        expect(driver, "toggle button", &names[turn], State::Pressed, true)?;
        // A frame or two of each turn on the cooler.
        thread::sleep(Duration::from_millis(300));
    }
    page(driver, "Display")?;
    Ok(format!("{} saved", names[saved]))
}

/// Refresh, which the service takes and the app confirms.
fn refresh(driver: &mut Driver) -> Result<String> {
    let button = driver.shown("button", "Refresh display")?;
    driver.act(&button, "Click")?;
    wait("Refreshed", || {
        let Some(group) = driver.find("group", "Change feedback")? else {
            return Ok(None);
        };
        Ok((driver.texts(&group)? == ["Refreshed"]).then_some(()))
    })?;
    Ok(String::new())
}
