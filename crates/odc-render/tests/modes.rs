//! The mode table and what this build draws (Phase 7 plan, Task 2; `docs/spec/display.md` and
//! `docs/spec/music.md`, Modes).

use odc_render::{Kind, Mode, Unit, Values, mode_info, picture};
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

// Music is every mode from 13 on (17 is none), the emoji alone included: they move only while
// sound plays (owner, 2026-11-23), so every Music mode takes sound.
#[test]
fn every_music_mode_is_from_13_on() {
    for mode in Mode::all() {
        let n = mode.number();
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

// A mode has a picture exactly when it shows values; a Music mode without values has none, and
// the engine draws it from its spectrum, loop or Dancer.
#[test]
fn picture_draws_exactly_the_modes_that_show_values() {
    let values = distinct();
    for mode in Mode::all() {
        for unit in [Unit::Celsius, Unit::Fahrenheit] {
            assert_eq!(
                picture(mode, &values, unit).is_some(),
                !mode_info(mode).metrics.is_empty(),
                "mode {}",
                mode.number()
            );
        }
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
