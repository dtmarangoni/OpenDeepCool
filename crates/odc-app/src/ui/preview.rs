//! The preview (`docs/design/settings-app.md`, Preview column): the frame the service last sent,
//! drawn as the cooler shows it. It redraws only when a frame differs from the last one, and
//! once on a theme change, so a still picture costs nothing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use odc_protocol::Frame;

use super::dots;
use crate::theme::{Rgb, Tokens};

/// The grid's pitch, px per dot: 14 × 15 = 210 px inside the 226 px frame's 8 px padding.
pub const PITCH: f64 = 15.0;
/// The drawing's side, px.
pub const SIDE: i32 = 210;

struct State {
    frame: Option<Frame>,
    lit: Rgb,
    unlit: Rgb,
}

#[derive(Clone)]
pub struct Preview {
    area: gtk::DrawingArea,
    state: Rc<RefCell<State>>,
    draws: Rc<Cell<u32>>,
}

impl Preview {
    pub fn new(tokens: &Tokens) -> Preview {
        let area = gtk::DrawingArea::builder()
            .accessible_role(gtk::AccessibleRole::Img)
            .content_width(SIDE)
            .content_height(SIDE)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        let state = Rc::new(RefCell::new(State {
            frame: None,
            lit: tokens.fg,
            unlit: tokens.unlit(),
        }));
        let draws = Rc::new(Cell::new(0));
        let (drawn, count) = (Rc::clone(&state), Rc::clone(&draws));
        area.set_draw_func(move |_, cr, _, _| {
            count.set(count.get() + 1);
            let state = drawn.borrow();
            dots::draw(cr, state.frame.as_ref(), PITCH, state.lit, state.unlit);
        });
        Preview { area, state, draws }
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    /// Shows `frame`, or every dot unlit; redraws only when it differs from the last.
    pub fn set_frame(&self, frame: Option<Frame>) {
        let mut state = self.state.borrow_mut();
        if state.frame != frame {
            state.frame = frame;
            self.area.queue_draw();
        }
    }

    /// The frame shown.
    pub fn frame(&self) -> Option<Frame> {
        self.state.borrow().frame
    }

    /// The theme's colours; redraws once.
    pub fn set_theme(&self, tokens: &Tokens) {
        let mut state = self.state.borrow_mut();
        state.lit = tokens.fg;
        state.unlit = tokens.unlit();
        self.area.queue_draw();
    }

    /// How often it has drawn, for the tests.
    pub fn draws(&self) -> u32 {
        self.draws.get()
    }
}
