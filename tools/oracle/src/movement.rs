//! The movement procedure of `docs/spec/display.md`, "Movement, precisely", step for step: a
//! strip of columns, its position *x* and whether it moves. Columns are numbered 1–14 on the
//! unrotated picture.

use crate::drawing::{Frame, SIZE};
use crate::rules::Strip;

const LAST_COLUMN: i64 = SIZE as i64;

/// The strip on show, where its first column is, and whether it moves.
pub struct Movement {
    strip: Strip,
    x: i64,
    moving: bool,
}

impl Movement {
    /// A reset (start-up, a mode switch, a rotation change, a unit change in a mode that shows a
    /// temperature, another GPU, Refresh): the new picture, *x* = *h*, moving if it needs to
    /// scroll.
    pub fn reset(strip: Strip) -> Movement {
        Movement {
            x: strip.home(),
            moving: strip.needs_scroll(),
            strip,
        }
    }

    /// A new picture without a reset (a reading changed).
    pub fn change(&mut self, strip: Strip) {
        if !self.moving && !strip.needs_scroll() {
            // A stopped picture is always at home (owner, 2026-10-29).
            self.x = strip.home();
        } else {
            // The copy whose first column is on the screen keeps its first column; if none is,
            // the copy at x does. Copies start every L columns, and L is at least 21, so at
            // most one first column is on the screen.
            let length = self.strip.cycle();
            let rightmost = self.x + (LAST_COLUMN - self.x).div_euclid(length) * length;
            let mut x = if rightmost >= 1 { rightmost } else { self.x };
            while x <= strip.home() - strip.cycle() {
                x += strip.cycle();
            }
            self.x = x;
            self.moving = self.moving || strip.needs_scroll();
        }
        self.strip = strip;
    }

    /// One tick, after this tick's changes: stop at home if nothing needs to scroll, draw, move.
    pub fn tick(&mut self) -> Frame {
        if self.moving && !self.strip.needs_scroll() && self.x == self.strip.home() {
            self.moving = false;
        }
        let frame = self.strip.frame_at(self.x);
        if self.moving {
            self.x -= 1;
            while self.x <= self.strip.home() - self.strip.cycle() {
                self.x += self.strip.cycle();
            }
        }
        frame
    }

    /// *x*, for the checks.
    pub fn x(&self) -> i64 {
        self.x
    }

    pub fn moving(&self) -> bool {
        self.moving
    }

    pub fn strip(&self) -> &Strip {
        &self.strip
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Ratio, strip_of};

    fn usage(n: i64) -> Strip {
        strip_of(1, &[Some(Ratio::whole(n))], false)
    }

    /// Ticks until the strip stops, at most `limit` ticks; the number of frames drawn while it
    /// still moved.
    fn frames_until_stopped(m: &mut Movement, limit: usize) -> usize {
        let mut frames = 0;
        while frames < limit {
            m.tick();
            if !m.moving() {
                return frames;
            }
            frames += 1;
        }
        panic!("still moving after {limit} ticks");
    }

    // S6: 100 or more → 99 or less, k columns after home: (L − k) mod L more frames, then it
    // stops at home; at k = 0, still at home, it stops at once.
    #[test]
    fn a_drop_after_k_columns_runs_the_rest_of_the_cycle_and_stops_at_home() {
        let length = usage(100).cycle();
        assert_eq!(length, 21);
        for k in 0..length {
            let mut m = Movement::reset(usage(100));
            let before = if k == 0 { length } else { k };
            for _ in 0..before {
                m.tick();
            }
            m.change(usage(42));
            let frames = frames_until_stopped(&mut m, 100);
            let want = usize::try_from((length - k) % length).expect("a count");
            assert_eq!(frames, want, "k = {k}");
            assert_eq!(m.x(), m.strip().home(), "k = {k}: stopped at home");
        }
    }

    // S4: up, down and up again before home: it never stops, and the position never jumps.
    #[test]
    fn up_down_and_up_again_before_home_never_stops() {
        let mut m = Movement::reset(usage(100));
        for tick in 0..60 {
            match tick {
                5 => m.change(usage(42)),
                10 => m.change(usage(100)),
                _ => {}
            }
            let before = m.x();
            m.tick();
            assert!(m.moving(), "tick {tick}");
            let step = before - m.x();
            assert!(
                step == 1 || step == 1 - m.strip().cycle(),
                "tick {tick}: {step}"
            );
        }
    }

    // S2: a value reaching 100 keeps the static picture's first digit place: column 2 for two
    // digits, column 4 for a centred one-digit value.
    #[test]
    fn a_value_reaching_100_keeps_its_first_digit_place() {
        for (from, place) in [(42, 2), (7, 4)] {
            let mut m = Movement::reset(usage(from));
            m.tick();
            m.change(usage(100));
            assert_eq!(m.x(), place, "from {from}");
            assert_eq!(m.tick(), usage(100).frame_at(place), "from {from}");
            assert_eq!(m.x(), place - 1, "from {from}: then it moves");
        }
    }

    // S9 (owner, 2026-10-29): a stopped picture is always at home, so 9 ↔ 10 re-centres it.
    #[test]
    fn a_stopped_picture_is_re_centred_when_its_width_changes() {
        let mut m = Movement::reset(usage(9));
        assert_eq!(m.tick(), usage(9).frame_at(4));
        m.change(usage(10));
        assert_eq!(m.tick(), usage(10).frame_at(2));
        m.change(usage(9));
        assert_eq!(m.tick(), usage(9).frame_at(4));
        assert!(!m.moving());
    }

    // S8: 999 ↔ 1000 while scrolling: the first digit place stays put and the cycle grows from
    // 21 to 25 and back.
    #[test]
    fn a_longer_number_keeps_its_first_digit_place() {
        let mut m = Movement::reset(usage(999));
        for _ in 0..3 {
            m.tick();
        }
        let x = m.x();
        m.change(usage(1000));
        assert_eq!((m.x(), m.strip().cycle()), (x, 25));
        m.change(usage(999));
        assert_eq!((m.x(), m.strip().cycle()), (x, 21));
    }
}
