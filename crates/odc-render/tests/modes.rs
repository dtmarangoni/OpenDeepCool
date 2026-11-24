//! The mode table and what this build draws (Phase 7 plan, Task 2; `docs/spec/display.md` and
//! `docs/spec/music.md`, Modes).

use odc_render::{Kind, Mode, Unit, Values, draws, mode_info, picture};
use odc_sensors::{Exact, Metric, Reading, Snapshot, Unavailable};

#[test]
fn the_table_lists_each_mode_s_metrics_top_first() {
    use Metric::{
        CpuPower, CpuTemperature, CpuUsage, GpuPower, GpuTemperature, GpuUsage, MemoryLoad,
    };
    // Mode 17 is no mode (DeepCreative's Emoji 4, dropped on 2026-11-23): 25 modes in all.
    let want: [(&[Metric], Kind); 25] = [
        (&[CpuUsage], Kind::Load),
        (&[GpuUsage], Kind::Load),
        (&[MemoryLoad], Kind::Load),
        (&[CpuUsage, GpuUsage], Kind::Load),
        (&[CpuUsage, MemoryLoad], Kind::Load),
        (&[GpuUsage, MemoryLoad], Kind::Load),
        (&[CpuTemperature], Kind::Temperature),
        (&[GpuTemperature], Kind::Temperature),
        (&[CpuTemperature, GpuTemperature], Kind::Temperature),
        (&[CpuPower], Kind::Power),
        (&[GpuPower], Kind::Power),
        (&[CpuPower, GpuPower], Kind::Power),
        // 13, the spectrum alone, and 14–16, the three emoji over it.
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[CpuTemperature], Kind::Music),
        (&[GpuTemperature], Kind::Music),
        (&[CpuUsage], Kind::Music),
        (&[GpuUsage], Kind::Music),
        (&[MemoryLoad], Kind::Music),
        // 23, the Dancer, and 24–26, the three emoji alone.
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[], Kind::Music),
    ];
    let modes: Vec<Mode> = Mode::all().collect();
    assert_eq!(modes.len(), want.len());
    for (mode, (metrics, kind)) in modes.into_iter().zip(want) {
        let info = mode_info(mode);
        assert_eq!(
            (info.metrics, info.kind),
            (metrics, kind),
            "mode {}",
            mode.number()
        );
    }
    assert_eq!(Mode::new(0), None);
    assert_eq!(Mode::new(17), None, "no mode has the retired number");
    assert_eq!(Mode::new(27), None);
    assert!(Mode::new(26).is_some() && Mode::new(18).is_some());
    assert_eq!(Mode::all().count(), 25);
}

// A Music mode takes sound when it draws it: the spectrum, with or without an emoji or a value,
// and the Dancer; the three emoji alone, 24–26, take none (owner, 2026-11-23). No other mode does.
#[test]
fn only_the_music_modes_that_draw_the_sound_take_it() {
    for mode in Mode::all() {
        let n = mode.number();
        let wanted = n >= 13 && !(24..=26).contains(&n);
        assert_eq!(mode_info(mode).sound, wanted, "mode {n}");
        assert_eq!(
            mode_info(mode).kind == Kind::Music,
            n >= 13,
            "mode {n} is Music exactly from 13"
        );
    }
}

/// Every metric with its own non-zero value (AGENTS.md rule 5).
fn distinct() -> Values {
    Metric::ALL
        .into_iter()
        .zip(1..)
        .fold(Values::new(), |values, (metric, n)| {
            values.with(metric, Exact::whole(10 * n + 1))
        })
}

// owner, 2026-10-27 (solution review P8-03): for a mode that shows values, `draws` is true exactly
// where `picture` draws; a Music mode without values has no picture, and the engine draws it
// from its spectrum, loop or Dancer. Every mode from 1 to 12 is drawn (Phase 8), and of Music
// the modes Phase 9 has drawn so far, each in its own commit.
#[test]
fn draws_says_exactly_which_modes_picture_draws() {
    let values = distinct();
    let music_drawn: [u16; 13] = [13, 14, 15, 16, 18, 19, 20, 21, 22, 23, 24, 25, 26];
    for mode in Mode::all() {
        for unit in [Unit::Celsius, Unit::Fahrenheit] {
            let drawn_picture = picture(mode, &values, unit).is_ok();
            if mode_info(mode).metrics.is_empty() {
                assert!(!drawn_picture, "mode {} has no value", mode.number());
            } else {
                assert_eq!(draws(mode), drawn_picture, "mode {}", mode.number());
            }
        }
        let n = mode.number();
        assert_eq!(draws(mode), n <= 12 || music_drawn.contains(&n), "mode {n}");
    }
}

#[test]
fn a_snapshot_gives_each_metric_its_own_value_or_none() {
    let snapshot = Snapshot::new(|metric| match metric {
        Metric::GpuUsage => Reading::Unavailable(Unavailable::NoDevice),
        Metric::CpuUsage => Reading::Unavailable(Unavailable::NotYet),
        other => Reading::Value(Exact::whole(match other {
            Metric::CpuTemperature => 57,
            Metric::CpuPower => 65,
            Metric::GpuTemperature => 38,
            Metric::GpuPower => 250,
            _ => 85,
        })),
    });
    let values = Values::of(&snapshot);
    assert_eq!(values.get(Metric::CpuUsage), None);
    assert_eq!(values.get(Metric::GpuUsage), None);
    assert_eq!(values.get(Metric::MemoryLoad), Some(Exact::whole(85)));
    assert_eq!(values.get(Metric::CpuTemperature), Some(Exact::whole(57)));
    assert_eq!(values.get(Metric::GpuPower), Some(Exact::whole(250)));
}
