//! The GPU rank checks (Phase 6 plan, Task 9), one per GPU driver capability: the helper ranks
//! the real GPUs where their drivers are present and usable, and each absent driver's GPUs
//! through the fake kinds.
//! - Every PCI display controller gets a kind, and none is added.
//! - A driver with a documented signal must answer it, and one without is never asked.
//! - Two runs write the same file, which the display service reads back.
//!
//! No tool can say independently whether a real GPU is integrated, so that correctness rests on
//! the fakes, which answer each driver both ways. Nothing about any PC's GPUs is written here.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use odc_gpu_rank::fake::FakeKinds;
use odc_gpu_rank::{Answer, DriverKinds, KindSource, Signal, rank, write_ranking};
use odc_sensors::{PciAddr, RankedKind, Ranking, display_devices};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{amdgpu_apu, amdgpu_dgpu, i915, nouveau, nvidia_proprietary, xe};
use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};

const GPUS: [Capability; 5] = [
    Capability::GpuAmdgpu,
    Capability::GpuI915,
    Capability::GpuXe,
    Capability::GpuNvidia,
    Capability::GpuOther,
];

pub fn run(roots: &Roots, out_dir: &Path, summary: &mut Summary) {
    let probes: Vec<(Capability, Probe)> = GPUS.iter().map(|&c| (c, probe(c, roots))).collect();
    // One real run for every driver present, as the helper ranks them all at once.
    let real = probes.iter().any(|(_, p)| *p == Probe::Real).then(|| {
        twice(
            &roots.sysfs,
            &out_dir.join("gpu-rank/real"),
            [
                &mut DriverKinds::new(&roots.sysfs, &roots.dev),
                &mut DriverKinds::new(&roots.sysfs, &roots.dev),
            ],
        )
    });
    if let Some(result) = &real {
        let result = result.as_ref().map(|run| run.note.clone());
        summary.check("gpu_rank_real", result.map_err(Clone::clone));
    }
    for (capability, found) in probes {
        let name = format!("gpu_rank_{}", capability.name().trim_start_matches("gpu-"));
        let result = match found {
            Probe::Real => {
                summary.mode(capability, Mode::Real);
                match &real {
                    Some(Ok(run)) => per_driver(capability, &run.ranking),
                    Some(Err(e)) => Err(e.clone()),
                    None => Err("no real run".to_owned()),
                }
            }
            Probe::Absent { reason } => {
                summary.mode(capability, Mode::Fake { reason });
                fake(capability, &out_dir.join("gpu-rank").join(&name))
            }
            Probe::Unusable { reason, fix } => {
                let reason = format!("{reason}; fix: {fix}");
                summary.mode(
                    capability,
                    Mode::Failed {
                        reason: reason.clone(),
                    },
                );
                Err(reason)
            }
        };
        summary.check(&name, result);
    }
}

/// A ranking that two runs agreed on.
struct Run {
    ranking: Ranking,
    note: String,
}

/// Ranks the GPUs under `sys` twice, once with each source, and writes each ranking into a fresh
/// state directory under `dir` as the helper does. Both files must be the same, the display
/// service must read the file back, and every display controller must have a kind.
fn twice(sys: &Path, dir: &Path, sources: [&mut dyn KindSource; 2]) -> Result<Run, String> {
    let state = dir.join("state");
    let mut runs = Vec::new();
    for source in sources {
        let _ = fs::remove_dir_all(&state);
        fs::create_dir_all(&state).map_err(|e| format!("{}: {e}", state.display()))?;
        let ranking = rank(sys, source);
        let path =
            write_ranking(&state, &ranking).map_err(|e| format!("{}: {e}", state.display()))?;
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        runs.push((ranking, text));
    }
    let (ranking, text) = runs.swap_remove(0);
    if text != runs[0].1 {
        return Err(format!(
            "two runs wrote different files:\n{text}\n{}",
            runs[0].1
        ));
    }
    if Ranking::parse(&text) != Ok(ranking.clone()) {
        return Err(format!(
            "the display service reads the file otherwise:\n{text}"
        ));
    }
    let present: BTreeSet<PciAddr> = display_devices(sys)
        .into_iter()
        .map(|(pci, _)| pci)
        .collect();
    let ranked: BTreeSet<PciAddr> = ranking.gpus.iter().map(|gpu| gpu.pci).collect();
    if present != ranked {
        return Err(format!(
            "the display controllers {present:?} were ranked as {ranked:?}"
        ));
    }
    Ok(Run {
        note: format!("{} ranked, two runs alike", ranking.gpus.len()),
        ranking,
    })
}

/// The real run's GPUs on `capability`'s driver: each with a signal answered it, and each on a
/// driver without one wasn't asked.
fn per_driver(capability: Capability, ranking: &Ranking) -> Result<String, String> {
    let mut seen = 0;
    for gpu in &ranking.gpus {
        let ours = match capability.gpu_driver() {
            Some(driver) => gpu.driver == driver,
            None => Signal::for_driver(&gpu.driver).is_none(),
        };
        if !ours {
            continue;
        }
        seen += 1;
        match (&gpu.kind, capability.gpu_driver()) {
            (RankedKind::Dedicated { .. } | RankedKind::Integrated { .. }, Some(_))
            | (RankedKind::NoSignal, None) => {}
            (RankedKind::QueryFailed { error }, _) => {
                return Err(format!("{}: the kind query failed: {error}", gpu.pci));
            }
            (other, _) => return Err(format!("{}: {other:?}", gpu.pci)),
        }
    }
    if seen == 0 {
        return Err("the probe found the driver, the helper didn't".to_owned());
    }
    Ok(format!("{seen} real, each with its kind"))
}

/// A fake tree for `capability`'s driver: a dedicated and an integrated GPU on it, which the fake
/// kinds answer that way; for drivers without a signal, a GPU on nouveau, which is never asked.
fn fake(capability: Capability, dir: &Path) -> Result<String, String> {
    let tree = FakeTree::create(dir.join("tree")).map_err(|e| e.to_string())?;
    let (dedicated, integrated) = ("0000:0a:00.0", "0000:00:02.0");
    let built = match capability {
        Capability::GpuAmdgpu => amdgpu_dgpu(&tree, dedicated, 41_000, 17, 23_000_000)
            .and(amdgpu_apu(&tree, integrated, 45_000, 9, 11_000_000)),
        Capability::GpuI915 => i915(&tree, dedicated, 52_000, 7_000_000)
            .and(i915(&tree, integrated, 48_000, 3_000_000)),
        Capability::GpuXe => {
            xe(&tree, dedicated, 63_000, 9_000_000).and(xe(&tree, integrated, 58_000, 4_000_000))
        }
        Capability::GpuNvidia => {
            nvidia_proprietary(&tree, dedicated).and(nvidia_proprietary(&tree, integrated))
        }
        _ => nouveau(&tree, "0000:0e:00.0"),
    };
    built.map_err(|e| e.to_string())?;
    let answers = [
        (dedicated, Ok(Answer::Dedicated)),
        (integrated, Ok(Answer::Integrated)),
    ];
    let (mut first, mut second) = (FakeKinds::new(&answers), FakeKinds::new(&answers));
    let run = twice(&tree.roots().sysfs, dir, [&mut first, &mut second])?;
    let asked: Vec<(Signal, PciAddr)> = [first.asked(), second.asked()].concat();
    let Some(driver) = capability.gpu_driver() else {
        let kinds: Vec<&RankedKind> = run.ranking.gpus.iter().map(|gpu| &gpu.kind).collect();
        if kinds != [&RankedKind::NoSignal] || !asked.is_empty() {
            return Err(format!("nouveau: {kinds:?}, asked {asked:?}"));
        }
        return Ok("nouveau: unknown, never asked".to_owned());
    };
    let signal = Signal::for_driver(driver).ok_or("a driver without a signal")?;
    for (pci, kind) in [
        (
            dedicated,
            RankedKind::Dedicated {
                signal: signal.describe(Answer::Dedicated),
            },
        ),
        (
            integrated,
            RankedKind::Integrated {
                signal: signal.describe(Answer::Integrated),
            },
        ),
    ] {
        match run
            .ranking
            .gpus
            .iter()
            .find(|gpu| gpu.pci.to_string() == pci)
        {
            Some(gpu) if gpu.kind == kind => {}
            other => return Err(format!("{pci}: {other:?}, not {kind:?}")),
        }
    }
    if asked.len() != 4 || asked.iter().any(|(s, _)| *s != signal) {
        return Err(format!("asked {asked:?}"));
    }
    Ok(format!("{driver}: dedicated and integrated, by {signal:?}"))
}
