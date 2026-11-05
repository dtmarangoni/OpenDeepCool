//! The picture diff: a failing comparison prints the expected picture and the frame side by side,
//! with the differing pixels marked (PRD, Testing Strategy, layer 1). And the ordered comparison of
//! a captured scrolling run with a cycle (Phase 8 plan, Task 3).

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

/// Why a captured run isn't consecutive windows of a cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mismatch {
    /// The run is shorter than one cycle, so some window is never seen.
    TooShort { captured: usize, cycle: usize },
    /// The run's first frame is none of the cycle's frames.
    NoStart,
    /// From the start that matches longest, frame `at` of the run differs from the cycle's.
    Differs {
        start: usize,
        at: usize,
        report: String,
    },
}

/// Where `captured` starts in `cycle`: the first phase *p* such that every `captured[i]` is
/// `cycle[(p + i) mod L]`, with at least *L* frames captured, so every window is seen. A run in
/// the wrong order fails; it's never compared as a set (Phase 8 plan, Task 3).
///
/// # Errors
///
/// A [`Mismatch`]: a run shorter than the cycle, a first frame that isn't in it, or the first
/// differing frame from the start that matches longest, with both pictures side by side.
pub fn cycle_match(captured: &[Frame], cycle: &[Frame]) -> Result<usize, Mismatch> {
    if cycle.is_empty() || captured.len() < cycle.len() {
        return Err(Mismatch::TooShort {
            captured: captured.len(),
            cycle: cycle.len(),
        });
    }
    let mut best: Option<(usize, usize)> = None;
    for start in (0..cycle.len()).filter(|&p| cycle[p] == captured[0]) {
        let differs =
            (0..captured.len()).find(|&i| captured[i] != cycle[(start + i) % cycle.len()]);
        let Some(at) = differs else {
            return Ok(start);
        };
        if best.is_none_or(|(_, longest)| at > longest) {
            best = Some((start, at));
        }
    }
    let (start, at) = best.ok_or(Mismatch::NoStart)?;
    let expected = picture_of(&cycle[(start + at) % cycle.len()]);
    let report = describe(&expected, &captured[at]).unwrap_or_default();
    Err(Mismatch::Differs { start, at, report })
}

/// A frame as an expected picture, for the side-by-side report.
fn picture_of(frame: &Frame) -> Picture {
    let rows: Vec<String> = (1..=Frame::SIZE)
        .map(|r| {
            (1..=Frame::SIZE)
                .map(|c| if frame.get(r, c) { '#' } else { '.' })
                .collect()
        })
        .collect();
    let rows: [&str; Frame::SIZE] = std::array::from_fn(|r| rows[r].as_str());
    Picture::from_rows(&rows)
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

    /// A cycle of `len` distinct frames: frame k lights row 1, column k mod 14 + 1, and row 2 when
    /// k is 14 or more.
    fn cycle(len: usize) -> Vec<Frame> {
        (0..len)
            .map(|k| {
                let mut frame = Frame::blank();
                frame.set(1, k % Frame::SIZE + 1, true);
                frame.set(2, 1, k >= Frame::SIZE);
                frame
            })
            .collect()
    }

    /// `len` consecutive windows of `cycle`, from phase `start`.
    fn run(cycle: &[Frame], start: usize, len: usize) -> Vec<Frame> {
        (0..len).map(|i| cycle[(start + i) % cycle.len()]).collect()
    }

    #[test]
    fn the_cycle_matches_from_each_phase() {
        let cycle = cycle(21);
        for start in 0..21 {
            assert_eq!(cycle_match(&run(&cycle, start, 21), &cycle), Ok(start));
            assert_eq!(cycle_match(&run(&cycle, start, 50), &cycle), Ok(start));
        }
    }

    #[test]
    fn a_run_one_frame_short_of_the_cycle_fails() {
        let cycle = cycle(25);
        assert_eq!(
            cycle_match(&run(&cycle, 3, 24), &cycle),
            Err(Mismatch::TooShort {
                captured: 24,
                cycle: 25
            })
        );
    }

    #[test]
    fn two_frames_swapped_fail_at_the_first() {
        let cycle = cycle(21);
        let mut captured = run(&cycle, 5, 30);
        captured.swap(7, 8);
        let Err(Mismatch::Differs { start, at, report }) = cycle_match(&captured, &cycle) else {
            panic!("a swapped pair passed");
        };
        assert_eq!((start, at), (5, 7));
        assert!(report.contains(MISMATCH), "{report}");
    }

    #[test]
    fn another_cycles_windows_fail() {
        let cycle = cycle(21);
        let other: Vec<Frame> = cycle
            .iter()
            .map(|frame| frame.rotated(odc_protocol::Rotation::from_setting(2).expect("2")))
            .collect();
        assert_eq!(
            cycle_match(&run(&other, 0, 21), &cycle),
            Err(Mismatch::NoStart)
        );
        // Its first frame in the cycle, the rest not: the first differing frame is reported.
        let mut captured = run(&cycle, 4, 21);
        captured[1..].copy_from_slice(&run(&other, 5, 20));
        assert!(matches!(
            cycle_match(&captured, &cycle),
            Err(Mismatch::Differs {
                start: 4,
                at: 1,
                ..
            })
        ));
    }
}
