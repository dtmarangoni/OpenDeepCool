//! The `Frame` signal's pixels (`docs/spec/dbus.md`, Signals; Phase 10): the frame the engine
//! drew, rotation included, as 196 bytes, each 0 or 1, row by row from the top left.

use odc_protocol::Frame;

/// The bytes of one `Frame` signal.
pub const FRAME_PIXELS: usize = Frame::SIZE * Frame::SIZE;

/// `frame` as the signal carries it.
pub fn frame_pixels(frame: &Frame) -> Vec<u8> {
    (1..=Frame::SIZE)
        .flat_map(|r| (1..=Frame::SIZE).map(move |c| (r, c)))
        .map(|(r, c)| u8::from(frame.get(r, c)))
        .collect()
}

/// The frame in a signal's `pixels`; `None` unless they're 196 bytes, each 0 or 1.
pub fn frame_from_pixels(pixels: &[u8]) -> Option<Frame> {
    if pixels.len() != FRAME_PIXELS || pixels.iter().any(|&p| p > 1) {
        return None;
    }
    let mut frame = Frame::blank();
    for (i, &p) in pixels.iter().enumerate() {
        frame.set(i / Frame::SIZE + 1, i % Frame::SIZE + 1, p == 1);
    }
    Some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_goes_row_by_row_and_back() {
        let mut frame = Frame::blank();
        frame.set(1, 2, true);
        frame.set(14, 14, true);
        let pixels = frame_pixels(&frame);
        assert_eq!(pixels.len(), 196);
        assert_eq!(pixels[1], 1, "row 1, column 2");
        assert_eq!(pixels[195], 1, "row 14, column 14");
        assert_eq!(pixels.iter().filter(|&&p| p == 1).count(), 2);
        assert_eq!(frame_from_pixels(&pixels), Some(frame));
    }

    #[test]
    fn pixels_of_another_length_or_value_are_refused() {
        assert_eq!(frame_from_pixels(&[0; 195]), None);
        assert_eq!(frame_from_pixels(&[0; 197]), None);
        let mut two = vec![0; 196];
        two[5] = 2;
        assert_eq!(frame_from_pixels(&two), None);
    }
}
