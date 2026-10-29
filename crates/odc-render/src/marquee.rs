//! The movement of `docs/spec/display.md`, "Movement, precisely", as a pure state machine: no
//! clock and no I/O, so every test scenario runs as a unit test. Columns are numbered 1–14 on
//! the unrotated picture; the caller turns the finished frame.

use odc_protocol::Frame;

use crate::picture::Picture;

const SCREEN: i32 = Frame::SIZE as i32;

/// The picture on show, where its strip's first column is, and whether it moves.
#[derive(Clone, Debug)]
pub struct Marquee {
    picture: Picture,
    /// *x*: the column of a copy's first column; copies repeat every *L* columns.
    x: i32,
    moving: bool,
}

impl Marquee {
    /// Starts as after a reset.
    pub fn new(picture: Picture) -> Marquee {
        Marquee {
            x: picture.home(),
            moving: picture.must_scroll(),
            picture,
        }
    }

    /// A reset (start-up, a mode switch, a rotation change, a unit change in a mode that shows a
    /// temperature, another GPU, Refresh): the picture at home, moving if it must scroll.
    pub fn reset(&mut self, picture: Picture) {
        *self = Marquee::new(picture);
    }

    /// A new picture without a reset: a reading changed.
    pub fn update(&mut self, picture: Picture) {
        if !self.moving && !picture.must_scroll() {
            // A stopped picture is always at home (owner, 2026-10-29).
            self.x = picture.home();
        } else {
            // The copy whose first column is on the screen keeps it; if none is, the copy at x
            // does. L is at least 21, so at most one first column is on the screen.
            let cycle = self.picture.cycle();
            let mut x = (1..=SCREEN)
                .find(|column| (column - self.x).rem_euclid(cycle) == 0)
                .unwrap_or(self.x);
            while x <= picture.home() - picture.cycle() {
                x += picture.cycle();
            }
            self.x = x;
            self.moving |= picture.must_scroll();
        }
        self.picture = picture;
    }

    /// One tick, after this tick's changes: a moving strip that needs no scrolling stops at home;
    /// then the frame is drawn, and a moving strip moves one column left.
    pub fn tick(&mut self) -> Frame {
        if self.moving && !self.picture.must_scroll() && self.x == self.picture.home() {
            self.moving = false;
        }
        let frame = self.picture.frame_at(self.x);
        if self.moving {
            self.x -= 1;
            while self.x <= self.picture.home() - self.picture.cycle() {
                self.x += self.picture.cycle();
            }
        }
        frame
    }

    pub fn picture(&self) -> &Picture {
        &self.picture
    }

    /// *x*, where the next frame is drawn.
    pub fn position(&self) -> i32 {
        self.x
    }

    pub fn is_moving(&self) -> bool {
        self.moving
    }
}
