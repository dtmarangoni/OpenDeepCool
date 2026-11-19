//! The live tiles' pictures (Phase 11a Task 14; `docs/design/settings-app.md`, Display page,
//! Tile content). Pure: no clock, no GTK and no D-Bus, so every case is a unit test.
//!
//! One [`Live`] holds a state machine for each of the 23 modes' tiles and steps them all together,
//! as the service's engine steps the cooler's picture: a value's strip in a [`Marquee`], so a
//! value of 100 or more scrolls and a changed reading keeps its place; an emoji's [`EmojiLoop`];
//! the Dancer, stepped by the sound's levels; and, below a value or an emoji, one [`Spectrum`]
//! shared by the Music tiles. The pictures are `odc_render`'s own, from the service's `Readings`,
//! so a tile shows what the cooler would show for the same values, upright.
//!
//! The Music tiles are live only while the sound is (`set_sound`): the service sends `Levels`
//! while a Music mode is shown and a helper's sound is attached. Otherwise they show the fixed
//! still sample, as the mockups draw it. The spectrum falls once per `Levels`, which is once per
//! tick of the service's own, so it falls as fast as on the cooler.
//!
//! Each call that can change a picture returns the modes whose frame changed, so the widgets
//! redraw only those.

use std::collections::HashMap;

use odc_ipc::FallStop;
use odc_protocol::Frame;
use odc_render::{
    COLUMNS, Dancer, EmojiLoop, Kind, Marquee, Mode, MusicSettings, Picture, Pose, Spectrum, Stop,
    Unit, Values, mode_info, picture, spectrum_height, value_picture, with_spectrum,
};
use odc_sensors::{Exact, Metric};

/// The cooler's tick, in microseconds on the frame clock: 100 ms.
pub const TICK_US: i64 = 100_000;

/// Counts a frame clock's time into the cooler's 100 ms ticks (design: Live tiles, The tick). The
/// clock calls about 60 times a second; most calls are no step. A step is due at or after its
/// time, and never makes up for a late one: after a stall the next is 100 ms from now, as the
/// cooler's engine skips ticks it missed (`display.md`, S22).
#[derive(Debug, Default)]
pub struct Beat {
    next: Option<i64>,
}

impl Beat {
    pub fn new() -> Beat {
        Beat::default()
    }

    /// Whether a step is due at frame time `now`. The first call after a start, or a
    /// [`restart`](Beat::restart), is never one: the tiles were just drawn at home.
    pub fn due(&mut self, now: i64) -> bool {
        let next = *self.next.get_or_insert(now + TICK_US);
        if now < next {
            return false;
        }
        // On time, or less than a tick late: the schedule stays on its 100 ms grid, so the
        // average is the cooler's. A tick or more late: skipped, never caught up.
        let following = next + TICK_US;
        self.next = Some(if following > now {
            following
        } else {
            now + TICK_US
        });
        true
    }

    /// Forgets the schedule: the next call starts a new one.
    pub fn restart(&mut self) {
        self.next = None;
    }
}

/// The values of the service's `Readings`, each metric by its spec name; a metric left out has no
/// value and draws dashes. `None`, before the app's first answer from the service, is the
/// design's sample values.
pub fn values_from(readings: Option<&HashMap<String, (i64, u64)>>) -> Values {
    let Some(readings) = readings else {
        return sample_values();
    };
    let mut values = Values::new();
    for metric in Metric::ALL {
        if let Some(&(num, den)) = readings.get(metric.spec_name()) {
            // A denominator of 0 is refused when the state is read.
            values.set(metric, Exact::new(num, den));
        }
    }
    values
}

/// The Music settings the service keeps, as `odc_render` takes them.
pub fn music_settings(peaks: bool, bar_fall: FallStop, peak_fall: FallStop) -> MusicSettings {
    let stop = |fall: FallStop| match fall {
        FallStop::Slower => Stop::Slower,
        FallStop::Slow => Stop::Slow,
        FallStop::Moderate => Stop::Moderate,
        FallStop::Fast => Stop::Fast,
        FallStop::Faster => Stop::Faster,
    };
    MusicSettings {
        peaks,
        bar_fall: stop(bar_fall),
        peak_fall: stop(peak_fall),
    }
}

/// The design's sample values (Display page, Tile content): what the tiles show before the first
/// readings arrive and while the service is down.
pub fn sample_values() -> Values {
    [
        (Metric::CpuUsage, 42),
        (Metric::GpuUsage, 63),
        (Metric::MemoryLoad, 51),
        (Metric::CpuTemperature, 57),
        (Metric::GpuTemperature, 38),
        (Metric::CpuPower, 65),
        (Metric::GpuPower, 88),
    ]
    .into_iter()
    .fold(Values::new(), |values, (metric, n)| {
        values.with(metric, Exact::whole(n))
    })
}

/// The fixed still sample spectrum's bars and peaks, the mockups' (`app.js`, `SAMPLE_SPECTRUM`).
pub const SAMPLE_BARS: [f64; COLUMNS] = [
    0.55, 0.8, 0.95, 0.7, 0.6, 0.5, 0.62, 0.45, 0.38, 0.42, 0.3, 0.22, 0.15, 0.08,
];
pub const SAMPLE_PEAKS: [f64; COLUMNS] = [
    0.7, 0.9, 1.0, 0.85, 0.7, 0.62, 0.7, 0.6, 0.5, 0.5, 0.42, 0.3, 0.22, 0.12,
];

/// The Music mode that draws the spectrum alone, and the Dancer's.
const SPECTRUM_ALONE: u16 = 13;
const DANCER: u16 = 23;

/// Whether `mode`'s picture follows the unit: it shows a temperature.
pub fn follows_unit(mode: Mode) -> bool {
    mode_info(mode)
        .metrics
        .iter()
        .any(|m| matches!(m, Metric::CpuTemperature | Metric::GpuTemperature))
}

/// What moves in a tile.
enum Motion {
    /// A value's strip: the whole grid in modes 1–12, rows 2–6 in modes 18–22.
    Strip(Marquee),
    /// Mode 13: the spectrum alone.
    Nothing,
    /// Modes 14–17.
    Emoji(EmojiLoop),
    /// Mode 23: the shared Dancer, or the still pose.
    Dancer,
}

struct Tile {
    mode: Mode,
    motion: Motion,
    /// What the tile draws above its spectrum: the last frame of its strip or loop.
    top: Frame,
    frame: Frame,
}

/// The pictures of all 23 tiles, and what moves them.
pub struct Live {
    unit: Unit,
    music: MusicSettings,
    values: Values,
    tiles: Vec<Tile>,
    /// The Music tiles' shared spectrum and the Dancer, while the sound is live.
    sound: Option<(Spectrum, Dancer)>,
}

/// The strip a mode shows for `values`: a value in modes 1–12 and 18–22.
fn strip(mode: Mode, values: &Values, unit: Unit) -> Picture {
    match mode_info(mode).kind {
        Kind::Music => value_picture(mode, values, unit).expect("modes 18–22 show a value"),
        _ => picture(mode, values, unit).expect("every mode is drawn"),
    }
}

fn motion_of(mode: Mode, values: &Values, unit: Unit) -> Motion {
    match mode.number() {
        SPECTRUM_ALONE => Motion::Nothing,
        14..=17 => Motion::Emoji(EmojiLoop::of(mode).expect("modes 14–17 have a loop")),
        DANCER => Motion::Dancer,
        _ => Motion::Strip(Marquee::new(strip(mode, values, unit))),
    }
}

impl Live {
    /// Every tile at home, showing what the cooler's first tick would: the picture at home, an
    /// emoji's first picture, the Dancer standing, the still sample below.
    pub fn new(unit: Unit, music: MusicSettings, values: Values) -> Live {
        let tiles = Mode::all()
            .map(|mode| Tile {
                mode,
                motion: motion_of(mode, &values, unit),
                top: Frame::blank(),
                frame: Frame::blank(),
            })
            .collect();
        let mut live = Live {
            unit,
            music,
            values,
            tiles,
            sound: None,
        };
        live.tick();
        live
    }

    /// `mode`'s picture now.
    ///
    /// # Panics
    ///
    /// When `mode` isn't one of the 23.
    pub fn frame(&self, mode: Mode) -> &Frame {
        &self
            .tiles
            .iter()
            .find(|tile| tile.mode == mode)
            .expect("a tile for every mode")
            .frame
    }

    /// Whether the sound is live: the Music tiles move with `Levels`.
    pub fn sound(&self) -> bool {
        self.sound.is_some()
    }

    /// One tick of 100 ms for every tile: a strip moves one column if it scrolls, an emoji loop
    /// goes on. Returns the modes whose picture changed.
    pub fn tick(&mut self) -> Vec<Mode> {
        for tile in &mut self.tiles {
            match &mut tile.motion {
                Motion::Strip(marquee) => tile.top = marquee.tick(),
                Motion::Emoji(emoji) => tile.top = emoji.tick(),
                Motion::Nothing | Motion::Dancer => {}
            }
        }
        self.recompose()
    }

    /// Every tile at home again, as when the window is shown: strips back to their home, the
    /// loops to their first picture, the spectrum and the Dancer to rest while the sound is live.
    /// Returns the modes whose picture changed.
    pub fn home(&mut self) -> Vec<Mode> {
        let (values, unit) = (&self.values, self.unit);
        for tile in &mut self.tiles {
            match &mut tile.motion {
                Motion::Strip(marquee) => marquee.reset(strip(tile.mode, values, unit)),
                Motion::Emoji(emoji) => emoji.reset(),
                Motion::Nothing | Motion::Dancer => {}
            }
        }
        if self.sound.is_some() {
            self.sound = Some((Spectrum::enter(), Dancer::new()));
        }
        self.tick()
    }

    /// The service's readings changed. A strip keeps its place as the cooler's does
    /// (`display.md`, Movement), and shows the new values at its next tick.
    pub fn set_values(&mut self, values: Values) {
        let unit = self.unit;
        for tile in &mut self.tiles {
            if let Motion::Strip(marquee) = &mut tile.motion {
                marquee.update(strip(tile.mode, &values, unit));
            }
        }
        self.values = values;
    }

    /// The temperature unit changed: the tiles that show a temperature restart at home in it, at
    /// once, as the cooler's picture does (`display.md`, Mode switches). Returns the modes whose
    /// picture changed.
    pub fn set_unit(&mut self, unit: Unit) -> Vec<Mode> {
        if unit == self.unit {
            return Vec::new();
        }
        self.unit = unit;
        let values = &self.values;
        for tile in &mut self.tiles {
            if follows_unit(tile.mode)
                && let Motion::Strip(marquee) = &mut tile.motion
            {
                marquee.reset(strip(tile.mode, values, unit));
                tile.top = marquee.tick();
            }
        }
        self.recompose()
    }

    /// The Music settings changed: the peak dots show or hide at once, and the falls apply from
    /// the next `Levels`. Returns the modes whose picture changed.
    pub fn set_music(&mut self, music: MusicSettings) -> Vec<Mode> {
        self.music = music;
        self.recompose()
    }

    /// The sound went live or stopped. Going live starts the spectrum at level 0 and the Dancer
    /// at rest, as entering a Music mode does on the cooler; stopping shows the still sample.
    /// Returns the modes whose picture changed.
    pub fn set_sound(&mut self, live: bool) -> Vec<Mode> {
        if live == self.sound.is_some() {
            return Vec::new();
        }
        self.sound = live.then(|| (Spectrum::enter(), Dancer::new()));
        self.recompose()
    }

    /// One tick's `Levels` from the service: the spectrum and the Dancer step by them, once, as
    /// the cooler's do. Ignored while the sound isn't live. Returns the modes whose picture
    /// changed.
    pub fn levels(&mut self, levels: &[f64; COLUMNS]) -> Vec<Mode> {
        let Some((spectrum, dancer)) = &mut self.sound else {
            return Vec::new();
        };
        spectrum.update(levels, &self.music);
        dancer.tick(levels);
        self.recompose()
    }

    /// Draws each tile from its top and the spectrum or the Dancer now; returns the modes whose
    /// frame differs from the last.
    fn recompose(&mut self) -> Vec<Mode> {
        let still = Spectrum::at(SAMPLE_BARS, SAMPLE_PEAKS);
        let mut changed = Vec::new();
        for tile in &mut self.tiles {
            let top = match (&tile.motion, &self.sound) {
                (Motion::Dancer, Some((_, dancer))) => dancer.pose().frame(),
                (Motion::Dancer, None) => Pose::STILL.frame(),
                _ => tile.top,
            };
            let frame = match spectrum_height(tile.mode) {
                Some(h) => {
                    let spectrum = self.sound.as_ref().map_or(&still, |(live, _)| live);
                    with_spectrum(&top, &spectrum.draw(h, self.music.peaks), h)
                }
                None => top,
            };
            if frame != tile.frame {
                tile.frame = frame;
                changed.push(tile.mode);
            }
        }
        changed
    }
}
