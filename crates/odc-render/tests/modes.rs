//! The mode table and what this build draws (Phase 7 plan, Task 2; `docs/spec/display.md` and
//! `docs/spec/music.md`, Modes).

use odc_render::{Kind, Mode, Unit, Values, draws, mode_info, picture};
use odc_sensors::{Exact, Fan, Metric, Reading, Snapshot, Unavailable};

#[test]
fn the_table_lists_each_mode_s_metrics_top_first() {
    use Metric::{
        CpuPower, CpuTemperature, CpuUsage, GpuPower, GpuTemperature, GpuUsage, MemoryLoad,
    };
    let want: [(&[Metric], Kind); 23] = [
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
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[], Kind::Music),
        (&[CpuTemperature], Kind::Music),
        (&[GpuTemperature], Kind::Music),
        (&[CpuUsage], Kind::Music),
        (&[GpuUsage], Kind::Music),
        (&[MemoryLoad], Kind::Music),
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
    assert_eq!(Mode::new(24), None);
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
    let music_drawn: [u16; 8] = [13, 14, 15, 16, 17, 18, 19, 20];
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
    let snapshot = Snapshot::new(
        |metric| match metric {
            Metric::GpuUsage => Reading::Unavailable(Unavailable::NoDevice),
            Metric::CpuUsage => Reading::Unavailable(Unavailable::NotYet),
            other => Reading::Value(Exact::whole(match other {
                Metric::CpuTemperature => 57,
                Metric::CpuPower => 65,
                Metric::GpuTemperature => 38,
                Metric::GpuPower => 250,
                _ => 85,
            })),
        },
        Fan::NotChosen,
        Fan::NotChosen,
    );
    let values = Values::of(&snapshot);
    assert_eq!(values.get(Metric::CpuUsage), None);
    assert_eq!(values.get(Metric::GpuUsage), None);
    assert_eq!(values.get(Metric::MemoryLoad), Some(Exact::whole(85)));
    assert_eq!(values.get(Metric::CpuTemperature), Some(Exact::whole(57)));
    assert_eq!(values.get(Metric::GpuPower), Some(Exact::whole(250)));
}
