//! The `odc-gpu-rank` program itself against a fake tree (Phase 6 plan, Task 9): its flags, the
//! file it writes, a failed query as a journal warning that doesn't fail the run, and the errors
//! that do. No GPU on NVIDIA's driver: asking it loads NVIDIA's library.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use odc_sensors::RANK_FILE;
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{amdgpu_dgpu, nouveau};

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("binary")
        .join(name)
}

fn run(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_odc-gpu-rank"))
        .args(args)
        .output()
        .expect("the program runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn it_ranks_the_tree_it_is_given_and_a_failed_query_only_warns() {
    let t = FakeTree::create(dir("ranks").join("tree")).expect("a fake tree");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    nouveau(&t, "0000:0e:00.0").expect("a GPU on nouveau");
    let state = dir("ranks").join("state");
    let _ = fs::remove_dir_all(&state);
    fs::create_dir_all(&state).expect("a state directory");
    let roots = t.roots();
    let output = run(&[
        Path::new("--state-dir"),
        &state,
        Path::new("--sys"),
        &roots.sysfs,
        Path::new("--dev"),
        &roots.dev,
    ]);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let file = fs::read_to_string(state.join(RANK_FILE)).expect("the file");
    assert!(
        file.contains(r#"{"pci": "0000:0a:00.0", "driver": "amdgpu", "kind": "unknown", "why": "query-failed", "error": ""#),
        "{file}"
    );
    assert!(
        file.contains(
            r#"{"pci": "0000:0e:00.0", "driver": "nouveau", "kind": "unknown", "why": "no-signal"}"#
        ),
        "{file}"
    );
    assert!(
        stderr.contains("<4>odc-gpu-rank: 0000:0a:00.0 (amdgpu): unknown: the query failed: "),
        "{stderr}"
    );
    assert!(
        stderr.contains("<6>odc-gpu-rank: 0000:0e:00.0 (nouveau): unknown"),
        "{stderr}"
    );
    assert!(
        stderr.contains("<6>odc-gpu-rank: 2 GPUs ranked in "),
        "{stderr}"
    );
}

#[test]
fn a_state_directory_it_cant_write_fails_the_run() {
    let t = FakeTree::create(dir("unwritable").join("tree")).expect("a fake tree");
    let missing = dir("unwritable").join("no-such-state");
    let _ = fs::remove_dir_all(&missing);
    let roots = t.roots();
    let output = run(&[
        Path::new("--state-dir"),
        &missing,
        Path::new("--sys"),
        &roots.sysfs,
        Path::new("--dev"),
        &roots.dev,
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).starts_with("<3>odc-gpu-rank: "));
}

#[test]
fn help_unknown_flags_and_missing_values() {
    let help = run(&[Path::new("--help")]);
    assert!(help.status.success());
    assert!(text(&help.stdout).starts_with("usage: odc-gpu-rank"));
    for (args, reason) in [
        (&["--fast"][..], "isn't an option"),
        (&["--state-dir"][..], "--state-dir needs a folder"),
        (&["--sys"][..], "--sys needs a folder"),
    ] {
        let paths: Vec<&Path> = args.iter().map(Path::new).collect();
        let output = run(&paths);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(text(&output.stderr).contains(reason), "{args:?}");
    }
}
