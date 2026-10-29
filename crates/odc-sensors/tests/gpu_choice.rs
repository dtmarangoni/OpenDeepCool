//! The GPU list, its ranking file and the choice of the GPU the display reads, over fake trees
//! (Phase 6 plan, Task 7; `docs/spec/sensors.md`, GPU choice).

use std::fs;
use std::path::PathBuf;

use odc_sensors::{
    Gpu, GpuKind, PciAddr, RANK_FILE, Ranked, RankedKind, Ranking, Unknown, choose, gpus,
    rank_is_stale, review_ranking,
};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{amdgpu_apu, amdgpu_dgpu, i915, nouveau, nvidia_proprietary, xe};

fn tree(name: &str) -> FakeTree {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("gpu_choice")
        .join(name);
    FakeTree::create(root).expect("a fake tree")
}

fn sys(tree: &FakeTree) -> PathBuf {
    tree.root().join("sys")
}

fn pci(text: &str) -> PciAddr {
    text.parse().expect("a PCI address")
}

const CARD: &str = "0000:0a:00.0";
const CARD2: &str = "0000:0b:00.0";
const IGPU: &str = "0000:00:02.0";
const APU: &str = "0000:0c:00.0";

fn dedicated(at: &str, driver: &str) -> Ranked {
    Ranked {
        pci: pci(at),
        driver: driver.into(),
        kind: RankedKind::Dedicated {
            signal: "the driver's own flag".into(),
        },
    }
}

fn integrated(at: &str, driver: &str) -> Ranked {
    Ranked {
        pci: pci(at),
        driver: driver.into(),
        kind: RankedKind::Integrated {
            signal: "the driver's own flag".into(),
        },
    }
}

/// The GPUs' addresses in list order, and the chosen one's.
fn order_and_choice(list: &[Gpu], saved: Option<PciAddr>) -> (Vec<String>, Option<String>) {
    let order = list.iter().map(|gpu| gpu.pci.to_string()).collect();
    let chosen = choose(list, saved).map(|gpu| gpu.pci.to_string());
    (order, chosen)
}

#[test]
fn one_gpu_of_any_kind_is_chosen_and_the_list_has_one_entry() {
    let t = tree("one");
    nouveau(&t, CARD).expect("nouveau");
    let list = gpus(&sys(&t), None);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, GpuKind::Unknown(Unknown::NotRanked));
    assert_eq!(list[0].driver, "nouveau");
    assert_eq!(
        order_and_choice(&list, None),
        (vec![CARD.to_owned()], Some(CARD.to_owned()))
    );
}

#[test]
fn a_dedicated_card_comes_before_integrated_graphics() {
    let t = tree("card-and-igpu");
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
    i915(&t, IGPU, 44_000, 9_100_000).expect("igpu");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD, "amdgpu"), integrated(IGPU, "i915")],
    };
    let list = gpus(&sys(&t), Some(&ranking));
    // The integrated GPU has the lower address, but the card still comes first.
    assert_eq!(
        order_and_choice(&list, None),
        (
            vec![CARD.to_owned(), IGPU.to_owned()],
            Some(CARD.to_owned())
        )
    );
}

#[test]
fn of_two_dedicated_cards_the_lower_address_wins() {
    let t = tree("two-cards");
    xe(&t, CARD2, 47_000, 7_300_000).expect("xe");
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("amdgpu");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD2, "xe"), dedicated(CARD, "amdgpu")],
    };
    let list = gpus(&sys(&t), Some(&ranking));
    assert_eq!(
        order_and_choice(&list, None),
        (
            vec![CARD.to_owned(), CARD2.to_owned()],
            Some(CARD.to_owned())
        )
    );
}

#[test]
fn integrated_graphics_come_before_a_gpu_whose_driver_has_no_signal() {
    let t = tree("apu-and-nouveau");
    nouveau(&t, CARD).expect("nouveau");
    amdgpu_apu(&t, APU, 48_000, 12, 9_000_000).expect("apu");
    let ranking = Ranking {
        gpus: vec![
            Ranked {
                pci: pci(CARD),
                driver: "nouveau".into(),
                kind: RankedKind::NoSignal,
            },
            integrated(APU, "amdgpu"),
        ],
    };
    let list = gpus(&sys(&t), Some(&ranking));
    assert_eq!(
        order_and_choice(&list, None),
        (vec![APU.to_owned(), CARD.to_owned()], Some(APU.to_owned()))
    );
    assert_eq!(list[1].kind, GpuKind::Unknown(Unknown::NoSignal));
}

#[test]
fn a_failed_kind_query_ranks_after_integrated_and_says_its_error() {
    let t = tree("failed-query");
    nvidia_proprietary(&t, CARD).expect("nvidia");
    i915(&t, IGPU, 44_000, 9_100_000).expect("igpu");
    let error = "cuInit: CUDA_ERROR_NO_DEVICE";
    let ranking = Ranking {
        gpus: vec![
            Ranked {
                pci: pci(CARD),
                driver: "nvidia".into(),
                kind: RankedKind::QueryFailed {
                    error: error.into(),
                },
            },
            integrated(IGPU, "i915"),
        ],
    };
    let list = gpus(&sys(&t), Some(&ranking));
    assert_eq!(choose(&list, None).map(|g| g.driver.as_str()), Some("i915"));
    let nvidia = list.iter().find(|g| g.driver == "nvidia").expect("nvidia");
    assert_eq!(
        nvidia.kind,
        GpuKind::Unknown(Unknown::QueryFailed {
            error: error.into()
        })
    );
    assert_ne!(nvidia.kind, GpuKind::Unknown(Unknown::NoSignal));
}

#[test]
fn the_saved_gpu_wins_when_present_and_the_best_ranked_stands_in_when_it_isnt() {
    let t = tree("saved");
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
    i915(&t, IGPU, 44_000, 9_100_000).expect("igpu");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD, "amdgpu"), integrated(IGPU, "i915")],
    };
    let list = gpus(&sys(&t), Some(&ranking));
    assert_eq!(
        order_and_choice(&list, Some(pci(IGPU))).1.as_deref(),
        Some(IGPU)
    );
    // A saved GPU that's gone: the best ranked stands in, and the saved choice itself is the
    // caller's to keep, untouched.
    let gone = pci("0000:0d:00.0");
    assert_eq!(order_and_choice(&list, Some(gone)).1.as_deref(), Some(CARD));
}

#[test]
fn a_ranking_is_stale_when_the_gpus_changed_and_the_user_hasnt_chosen() {
    let t = tree("stale");
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
    i915(&t, IGPU, 44_000, 9_100_000).expect("igpu");
    let same = Ranking {
        gpus: vec![dedicated(CARD, "amdgpu"), integrated(IGPU, "i915")],
    };
    let older = Ranking {
        gpus: vec![integrated(IGPU, "i915")],
    };
    let list = gpus(&sys(&t), Some(&older));
    assert!(rank_is_stale(&list, &older, false));
    assert!(!rank_is_stale(&list, &older, true));
    assert!(!rank_is_stale(&gpus(&sys(&t), Some(&same)), &same, false));
    // Once the user has chosen, the new card stays unknown, ranked last.
    let card = list
        .iter()
        .find(|g| g.driver == "amdgpu")
        .expect("the card");
    assert_eq!(card.kind, GpuKind::Unknown(Unknown::NotRanked));
    assert_eq!(list.last().map(|g| g.driver.as_str()), Some("amdgpu"));
}

#[test]
fn the_service_deletes_a_stale_ranking_only_while_the_user_hasnt_chosen() {
    let t = tree("review-stale");
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
    i915(&t, IGPU, 44_000, 9_100_000).expect("igpu");
    let older = Ranking {
        gpus: vec![integrated(IGPU, "i915")],
    };
    let file = t
        .file(format!("state/{RANK_FILE}"), older.render())
        .expect("a ranking");

    let kept = review_ranking(&sys(&t), &file, true);
    assert_eq!((kept.ranking.as_ref(), kept.deleted), (Some(&older), None));
    assert!(file.exists());

    let review = review_ranking(&sys(&t), &file, false);
    assert_eq!(review.ranking.as_ref(), Some(&older));
    let why = review.deleted.expect("deleted");
    assert!(why.contains("the GPUs changed"), "{why}");
    assert!(!file.exists());
    // No file at all: nothing to delete, and every GPU is unknown.
    assert_eq!(review_ranking(&sys(&t), &file, false).ranking, None);
}

#[test]
fn a_broken_ranking_file_is_deleted_whether_or_not_the_user_has_chosen() {
    let good = Ranking {
        gpus: vec![dedicated(CARD, "amdgpu"), integrated(IGPU, "i915")],
    }
    .render();
    let cases = [
        ("truncated", good[..good.len() / 2].to_owned()),
        (
            "unknown kind",
            good.replace("\"dedicated\"", "\"discrete\""),
        ),
        ("malformed address", good.replace(CARD, "0000:0g:00.0")),
        ("unknown key", good.replace("\"driver\"", "\"vendor\"")),
        ("twice", good.replace(IGPU, CARD)),
        (
            "other version",
            good.replace("\"version\": 1", "\"version\": 2"),
        ),
        (
            "no signal",
            good.replacen(", \"signal\": \"the driver's own flag\"", "", 1),
        ),
        ("too big", format!("{good}{}", " ".repeat(70_000))),
    ];
    for (name, text) in cases {
        for user_has_chosen in [false, true] {
            let t = tree("broken");
            amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
            i915(&t, IGPU, 44_000, 9_100_000).expect("igpu");
            let file = t.file(format!("state/{RANK_FILE}"), &text).expect("a file");
            let review = review_ranking(&sys(&t), &file, user_has_chosen);
            assert_eq!(review.ranking, None, "{name}");
            let why = review
                .deleted
                .unwrap_or_else(|| panic!("{name}: not deleted"));
            assert!(why.contains("broken"), "{name}: {why}");
            assert!(!file.exists(), "{name}");
            let list = gpus(&sys(&t), review.ranking.as_ref());
            assert!(
                list.iter()
                    .all(|g| g.kind == GpuKind::Unknown(Unknown::NotRanked)),
                "{name}"
            );
        }
    }
}

#[test]
fn a_ranking_file_that_is_a_link_is_removed_never_followed() {
    let t = tree("link");
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
    let target = t
        .file(
            "elsewhere/real.json",
            Ranking {
                gpus: vec![dedicated(CARD, "amdgpu")],
            }
            .render(),
        )
        .expect("a file");
    let link = t
        .symlink(format!("state/{RANK_FILE}"), "../elsewhere/real.json")
        .expect("a link");
    let review = review_ranking(&sys(&t), &link, false);
    assert_eq!(review.ranking, None);
    assert!(review.deleted.is_some());
    assert!(fs::symlink_metadata(&link).is_err(), "the link is gone");
    assert!(target.exists(), "what it pointed to stays");
}

#[test]
fn a_ranking_renders_and_parses_back_whatever_its_texts_hold() {
    let ranking = Ranking {
        gpus: vec![
            dedicated(CARD, "amdgpu"),
            integrated(IGPU, "i915"),
            Ranked {
                pci: pci(APU),
                driver: "nouveau".into(),
                kind: RankedKind::NoSignal,
            },
            Ranked {
                pci: pci("10000:e0:1f.7"),
                driver: "nvidia".into(),
                kind: RankedKind::QueryFailed {
                    error: "line one\n\"quoted\" \\ é\u{1}".into(),
                },
            },
        ],
    };
    assert_eq!(Ranking::parse(&ranking.render()), Ok(ranking));
    assert_eq!(
        Ranking::parse(&Ranking::default().render()),
        Ok(Ranking::default())
    );
}
