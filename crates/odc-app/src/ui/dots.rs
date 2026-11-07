//! The 14 × 14 round dots the preview and the tiles draw (`docs/design/settings-app.md`,
//! Preview column): lit dots in `--fg`, unlit ones in `--fg` at 0.12, as the mockups' `paint`
//! draws them.

use gtk::cairo;
use odc_protocol::Frame;

use crate::theme::{Rgb, Rgba};

/// A dot's radius, in pitches.
const RADIUS: f64 = 0.38;

/// The centre of the dot at `row`, `col`, counted from 0, with `pitch` px per dot.
pub fn centre(row: usize, col: usize, pitch: f64) -> (f64, f64) {
    (
        col as f64 * pitch + pitch / 2.0,
        row as f64 * pitch + pitch / 2.0,
    )
}

/// Draws `frame`, or every dot unlit without one, from the top left: lit dots opaque, unlit ones
/// in `unlit` with its alpha, over whatever is behind.
pub fn draw(cr: &cairo::Context, frame: Option<&Frame>, pitch: f64, lit: Rgb, unlit: Rgba) {
    let lit = Rgba {
        rgb: lit,
        alpha: 1.0,
    };
    for (on, colour) in [(false, unlit), (true, lit)] {
        let [r, g, b] = colour.rgb.unit();
        cr.set_source_rgba(r, g, b, colour.alpha);
        for row in 0..Frame::SIZE {
            for col in 0..Frame::SIZE {
                // `Frame` numbers its pixels from 1, as the spec does.
                if frame.is_some_and(|f| f.get(row + 1, col + 1)) != on {
                    continue;
                }
                let (x, y) = centre(row, col, pitch);
                cr.new_sub_path();
                cr.arc(x, y, pitch * RADIUS, 0.0, std::f64::consts::TAU);
            }
        }
        // A cairo error leaves the dots undrawn; nothing else can be done about it in a draw.
        let _ = cr.fill();
    }
}
