//! The movement's properties, on random pictures and random changes (Phase 7 plan, Task 2;
//! `docs/spec/display.md`, Movement, precisely). The scenarios S1–S26 run in Task 3.

use odc_render::{Band, Marquee, PERCENT, Picture, Shown, layout};
use proptest::prelude::*;

/// The first columns, from 1 to 14, of a picture's copies drawn at `x`.
fn on_screen(x: i32, cycle: i32) -> Vec<i32> {
    (1..=14)
        .filter(|c| (c - x).rem_euclid(cycle) == 0)
        .collect()
}

fn shown() -> impl Strategy<Value = Shown> {
    prop_oneof![
        Just(Shown::Dashes),
        (0_u16..=9).prop_map(Shown::Number),
        (10_u16..=99).prop_map(Shown::Number),
        (100_u16..=999).prop_map(Shown::Number),
        (1000_u16..=9999).prop_map(Shown::Number),
        prop_oneof![Just(99_u16), Just(100), Just(999), Just(1000), Just(9999)]
            .prop_map(Shown::Number),
    ]
}

fn picture_of(values: Vec<Shown>) -> Picture {
    let bands: Vec<Band> = values
        .into_iter()
        .map(|shown| Band {
            shown,
            symbol: &PERCENT,
        })
        .collect();
    layout(&bands)
}

fn picture() -> impl Strategy<Value = Picture> {
    prop::collection::vec(shown(), 1..=2).prop_map(picture_of)
}

/// A picture with a value of 100 or more.
fn scrolling() -> impl Strategy<Value = Picture> {
    let wide = || (100_u16..=9999).prop_map(Shown::Number);
    prop_oneof![
        wide().prop_map(|a| vec![a]),
        (wide(), shown()).prop_map(|(a, b)| vec![a, b]),
        (shown(), wide()).prop_map(|(a, b)| vec![a, b]),
    ]
    .prop_map(picture_of)
}

/// A picture whose values all fit: 99 or less, or dashes.
fn fitting() -> impl Strategy<Value = Picture> {
    let fits = || prop_oneof![Just(Shown::Dashes), (0_u16..=99).prop_map(Shown::Number)];
    prop::collection::vec(fits(), 1..=2).prop_map(picture_of)
}

#[derive(Clone, Debug)]
enum Op {
    Tick,
    Update(Picture),
    Reset(Picture),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        8 => Just(Op::Tick),
        2 => picture().prop_map(Op::Update),
        1 => picture().prop_map(Op::Reset),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    // Between resets, whatever the readings do: each frame is drawn at x, and a moving strip's
    // first column then moves one column left, wrapping by L, unless it stops at home; an update
    // of a moving strip, or one that starts it moving, keeps the copy that's on the screen; a
    // stopped strip is always at its home (owner, 2026-10-29); after a reset, the first frame is
    // the picture at home.
    #[test]
    fn the_strip_moves_as_the_procedure_says(start in picture(), ops in prop::collection::vec(op(), 1..300)) {
        let mut m = Marquee::new(start);
        for op in ops {
            match op {
                Op::Tick => {
                    let picture = m.picture().clone();
                    let (x, moving) = (m.position(), m.is_moving());
                    let (home, cycle) = (picture.home(), picture.cycle());
                    let stops = moving && !picture.must_scroll() && x == home;
                    prop_assert_eq!(m.tick(), picture.frame_at(x), "the frame is drawn at x");
                    let next = if moving && !stops {
                        if x - 1 <= home - cycle { x - 1 + cycle } else { x - 1 }
                    } else {
                        x
                    };
                    prop_assert_eq!(m.position(), next);
                }
                Op::Update(p) => {
                    let keeps = m.is_moving() || p.must_scroll();
                    let before = on_screen(m.position(), m.picture().cycle());
                    m.update(p);
                    let after = on_screen(m.position(), m.picture().cycle());
                    if keeps {
                        for c in before {
                            prop_assert!(after.contains(&c), "the copy at column {} moved", c);
                        }
                    } else {
                        prop_assert_eq!(m.position(), m.picture().home(), "re-centred at home");
                    }
                }
                Op::Reset(p) => {
                    let home = p.frame_at(p.home());
                    m.reset(p);
                    prop_assert_eq!(m.clone().tick(), home, "after a reset, the picture at home");
                }
            }
            if !m.is_moving() {
                prop_assert_eq!(m.position(), m.picture().home(), "a stopped strip is at home");
            }
        }
    }

    // A strip that needs no scrolling stops the first time it reaches h, within L frames (S3, S6).
    #[test]
    fn a_strip_that_fits_stops_at_home_within_one_cycle(start in scrolling(), moves in 0..60_usize, end in fitting()) {
        let mut m = Marquee::new(start);
        for _ in 0..moves {
            m.tick();
        }
        m.update(end);
        let cycle = m.picture().cycle();
        let home = m.picture().home();
        let mut frames = 0;
        loop {
            let at_home = m.position() == home;
            m.tick();
            if at_home {
                prop_assert!(!m.is_moving(), "it passed home without stopping");
            }
            if !m.is_moving() {
                break;
            }
            frames += 1;
            prop_assert!(frames <= cycle, "still moving after {} frames", frames);
        }
        prop_assert_eq!(m.position(), home);
    }
}

// Movement, precisely: when no copy's first column is on the screen, the copy at x keeps its
// column. If the cycle shrinks meanwhile, here from 25 (`1000%`) to 21 (`99%`), the next copy
// starts L columns after x, so it comes in earlier: at column 14 instead of off the screen.
// Usage can't reach 1000 %, so Load never shows it; Power can (Phase 8).
#[test]
fn a_shrinking_cycle_brings_the_next_copy_in_earlier() {
    let one = |n| picture_of(vec![Shown::Number(n)]);
    let mut m = Marquee::new(one(1000));
    for _ in 0..9 {
        m.tick();
    }
    assert_eq!(m.position(), -7);
    assert!(on_screen(-7, 25).is_empty());
    m.update(one(99));
    assert_eq!(m.position(), -7, "the copy at x keeps its column");
    assert_eq!(on_screen(m.position(), m.picture().cycle()), [14]);
}

// S2: from a centred `7%`, `100%` starts where the 7 was, column 4, then moves left.
#[test]
fn a_value_reaching_100_starts_from_its_first_digit_place() {
    let one = |n| {
        layout(&[Band {
            shown: Shown::Number(n),
            symbol: &PERCENT,
        }])
    };
    let mut m = Marquee::new(one(7));
    m.tick();
    m.update(one(100));
    assert_eq!(m.position(), 4);
    assert_eq!(m.tick(), one(100).frame_at(4));
    assert_eq!(m.position(), 3);
}

// S9 (owner, 2026-10-29): a stopped `9%` turning into `10%` is re-centred at once.
#[test]
fn a_stopped_picture_is_re_centred() {
    let one = |n| {
        layout(&[Band {
            shown: Shown::Number(n),
            symbol: &PERCENT,
        }])
    };
    let mut m = Marquee::new(one(9));
    assert_eq!(m.tick(), one(9).frame_at(4));
    m.update(one(10));
    assert_eq!(m.tick(), one(10).frame_at(2));
    assert!(!m.is_moving());
}
