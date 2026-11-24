//! Music, modes 13–16 and 18–26 (`docs/spec/music.md`, Modes): what each draws above its
//! spectrum, and the frame they make together. Mode 13 draws the spectrum alone in rows 3–14; 14–16
//! an emoji in rows 1–7 ([`crate::EmojiLoop`]) and 18–22 a value in rows 2–6, over the spectrum in
//! rows 9–14; 23 the Dancer's pose ([`crate::Pose`]) on the whole grid, and 24–26 an emoji on the
//! whole grid with no spectrum.

use odc_protocol::Frame;
use odc_sensors::Exact;

use crate::glyph::{CELSIUS, FAHRENHEIT, Glyph, PERCENT};
use crate::mode::{Mode, mode_info};
use crate::picture::{Band, Picture, layout_at};
use crate::spectrum::{Columns, Spectrum};
use crate::value::{Shown, fahrenheit, number};
use crate::{Unit, Values};

/// The value's top row in modes 18–22.
const VALUE_TOP: usize = 2;

/// *H*, the spectrum area's height: 12 rows in mode 13, 6 in modes 14–16 and 18–22, and `None` in
/// a mode without a spectrum (point "OpenDeepCool's spectrum, precisely").
pub fn spectrum_height(mode: Mode) -> Option<u8> {
    match mode.number() {
        13 => Some(12),
        14..=16 | 18..=22 => Some(6),
        _ => None,
    }
}

/// The value area of modes 18–22: the metric the mode names, from its own reading only
/// (AGENTS.md rule 11; DeepCreative's mode 18 °F quirk not reproduced), with the display's value
/// rules, in rows 2–6. A temperature follows `unit`; usage and Memory Load are `%`. `None` for a
/// mode without a value.
pub fn value_picture(mode: Mode, values: &Values, unit: Unit) -> Option<Picture> {
    let (rule, symbol): (fn(Option<Exact>) -> Shown, &'static Glyph) = match (mode.number(), unit) {
        (18 | 19, Unit::Celsius) => (number, &CELSIUS),
        (18 | 19, Unit::Fahrenheit) => (fahrenheit, &FAHRENHEIT),
        (20..=22, _) => (number, &PERCENT),
        _ => return None,
    };
    let [metric] = mode_info(mode).metrics else {
        unreachable!("modes 18–22 show one value each")
    };
    let band = Band {
        shown: rule(values.get(*metric)),
        symbol,
    };
    Some(layout_at(&[band], &[VALUE_TOP]))
}

/// `mode`'s frame from what it draws above its spectrum and the spectrum now, before rotation: the
/// spectrum in its area when the mode has one ([`spectrum_height`]), `top` alone otherwise (the
/// emoji and the Dancer drawn alone, modes 23 to 26). The one composition the cooler's engine
/// and the app's tiles share.
pub fn compose(mode: Mode, top: &Frame, spectrum: &Spectrum, peaks: bool) -> Frame {
    match spectrum_height(mode) {
        Some(h) => with_spectrum(top, &spectrum.draw(h, peaks), h),
        None => *top,
    }
}

/// A frame with a spectrum, before rotation: `top`'s rows above the area, and `columns` drawn in
/// the bottom `h` rows, each bar from the floor (row 14) up and its dot above it.
///
/// # Panics
///
/// When `h` is outside 1–14, or a column is taller than `h`: programming errors.
pub fn with_spectrum(top: &Frame, columns: &Columns, h: u8) -> Frame {
    let size = Frame::SIZE;
    let h = usize::from(h);
    assert!((1..=size).contains(&h), "a spectrum area of {h} rows");
    let mut frame = *top;
    for (c, column) in columns.iter().enumerate() {
        let bar = usize::from(column.bar);
        let dot = column.dot.map(usize::from);
        assert!(
            bar <= h && dot.is_none_or(|d| d <= h),
            "column {} above its area",
            c + 1
        );
        for height in 1..=h {
            let lit = height <= bar || dot == Some(height);
            frame.set(size + 1 - height, c + 1, lit);
        }
    }
    frame
}
