//! The window's size (`docs/design/settings-app.md`, Window, Size and scrolling): the window has
//! no size of its own. It takes its content's natural size, capped at the smallest monitor's
//! size less a margin, and scrolls inside the cap.

use gtk::prelude::*;

/// What the cap leaves of the smallest monitor on each axis: the shell's panels and the window's
/// own frame. A judgment, not a measurement (Unverified points, Task 9).
pub const MARGIN: i32 = 48;

/// The header bar's height, which the window scroller's content cap leaves out: the cap is the
/// window's, and the scroller holds what is under the bar. About 47 px in the design's font.
pub const HEADER: i32 = 48;

/// The cap never goes below this, so a monitor reported smaller than a usable window gives the
/// window something to draw in.
const FLOOR: (i32, i32) = (320, 240);

/// The window's cap, from each monitor's width and height in application pixels: the smallest
/// width and the smallest height, each less [`MARGIN`], and at least the floor. `None` when no
/// monitor is known, so GTK's own bounds are the only limit.
pub fn cap(monitors: &[(i32, i32)]) -> Option<(i32, i32)> {
    let width = monitors.iter().map(|m| m.0).min()?;
    let height = monitors.iter().map(|m| m.1).min()?;
    Some((
        (width - MARGIN).max(FLOOR.0),
        (height - MARGIN).max(FLOOR.1),
    ))
}

/// Every monitor of `display`: its width and height, in application pixels.
pub fn monitors(display: &gtk::gdk::Display) -> Vec<(i32, i32)> {
    let list = display.monitors();
    (0..list.n_items())
        .filter_map(|i| list.item(i).and_downcast::<gtk::gdk::Monitor>())
        .map(|monitor| {
            let geometry = monitor.geometry();
            (geometry.width(), geometry.height())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // No monitor: no cap. One: its size less the margin. Several: the smallest on each axis,
    // taken separately. A tiny one: the floor.
    #[test]
    fn the_cap_is_the_smallest_monitor_less_the_margin() {
        assert_eq!(cap(&[]), None);
        assert_eq!(cap(&[(1280, 800)]), Some((1232, 752)));
        assert_eq!(
            cap(&[(2560, 1440), (1280, 1024), (1920, 800)]),
            Some((1232, 752)),
            "the narrowest and the lowest, from different monitors"
        );
        assert_eq!(cap(&[(300, 200)]), Some(FLOOR));
        assert_eq!(cap(&[(1920, 1080), (340, 1080)]), Some((320, 1032)));
    }
}
