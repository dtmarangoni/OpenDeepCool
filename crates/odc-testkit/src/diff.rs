//! The picture diff: a failing comparison prints the expected picture and the frame side by side,
//! with the differing pixels marked (PRD, Testing Strategy, layer 1).

use odc_protocol::Frame;

use crate::expected::{Picture, pixel_char, pixel_matches};

/// Every mismatch message contains this, so a test can recognise one.
pub const MISMATCH: &str = "the frame differs from the expected picture";

/// Panics with both pictures side by side when `actual` doesn't show `expected`. `context` says
/// which case was compared, for example its heading.
pub fn assert_matches(expected: &Picture, actual: &Frame, context: &str) {
    if let Some(report) = describe(expected, actual) {
        panic!("{context}: {report}");
    }
}

/// The side-by-side report of a mismatch, or `None` when `actual` shows `expected`.
pub fn describe(expected: &Picture, actual: &Frame) -> Option<String> {
    if expected.matches(actual) {
        return None;
    }
    let mut differing = 0;
    let mut rows = String::new();
    for r in 1..=Frame::SIZE {
        let (mut want, mut got, mut marks) = (String::new(), String::new(), String::new());
        for c in 1..=Frame::SIZE {
            let (pixel, on) = (expected.get(r, c), actual.get(r, c));
            want.push(pixel_char(pixel));
            got.push(if on { '#' } else { '.' });
            if pixel_matches(pixel, on) {
                marks.push('.');
            } else {
                marks.push('X');
                differing += 1;
            }
        }
        rows.push_str(&format!("    {want}  {got}  {marks}\n"));
    }
    let pixels = if differing == 1 { "pixel" } else { "pixels" };
    Some(format!(
        "{MISMATCH} in {differing} {pixels}\n    expected        actual          X = differs\n{rows}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture_with_one_pixel() -> Picture {
        let mut rows = [".............."; Frame::SIZE];
        rows[2] = "....#.........";
        Picture::from_rows(&rows)
    }

    #[test]
    fn a_matching_frame_has_no_report() {
        let mut frame = Frame::blank();
        frame.set(3, 5, true);
        assert_eq!(describe(&picture_with_one_pixel(), &frame), None);
        assert_matches(&picture_with_one_pixel(), &frame, "same picture");
    }

    #[test]
    fn the_report_shows_both_pictures_and_marks_the_difference() {
        let report = describe(&picture_with_one_pixel(), &Frame::blank()).expect("a mismatch");
        assert!(report.contains(&format!("{MISMATCH} in 1 pixel\n")));
        let row_3 = report.lines().nth(4).expect("row 3");
        assert_eq!(row_3, "    ....#.........  ..............  ....X.........");
        assert_eq!(report.lines().count(), 2 + Frame::SIZE);
    }

    #[test]
    #[should_panic(expected = "rotation 1: the frame differs from the expected picture in 1 pixel")]
    fn a_one_pixel_difference_panics() {
        assert_matches(&picture_with_one_pixel(), &Frame::blank(), "rotation 1");
    }
}
