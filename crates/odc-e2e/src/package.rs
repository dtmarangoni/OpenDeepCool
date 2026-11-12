//! The package test (Phase 11 plan, Task 5): `odc-pkg-test`, built from the code being tested,
//! builds the Arch package from the committed HEAD in a rootless throwaway root, installs and
//! uninstalls it the AUR way and the GitHub way, and requires the root to end as it started. Each
//! of its checks becomes one of this run's. It runs only when named (`--only package`), since it
//! takes about ten minutes and downloads the crates the build needs.
//!
//! Without the `container` capability it fails with the fix: a package test needs a system to
//! install into, and has no fake (Task 5, Fake mode).

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};

use crate::app::scenario;

/// Builds `odc-pkg-test` with the profile this program was built with, next to it.
fn build() -> Result<std::path::PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(["build", "--quiet", "-p", "odc-pkg-test"])
        .current_dir(workspace());
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    let status = command
        .status()
        .map_err(|e| format!("cargo can't start: {e}"))?;
    if !status.success() {
        return Err(format!("building odc-pkg-test failed: {status}"));
    }
    std::env::current_exe()
        .map_err(|e| format!("this program's path: {e}"))?
        .parent()
        .map(|bin| bin.join("odc-pkg-test"))
        .ok_or_else(|| "this program has no folder".to_owned())
}

fn workspace() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn run(roots: &Roots, out_dir: &Path, summary: &mut Summary) {
    let missing = match probe(Capability::Container, roots) {
        Probe::Real => None,
        Probe::Absent { reason } => Some(reason),
        Probe::Unusable { reason, fix } => Some(format!("{reason}; fix: {fix}")),
    };
    if let Some(reason) = missing {
        summary.mode(
            Capability::Container,
            Mode::Failed {
                reason: reason.clone(),
            },
        );
        summary.check("package", Err(reason));
        return;
    }
    summary.mode(Capability::Container, Mode::Real);
    if let Err(e) = test(out_dir, summary) {
        summary.check("package", Err(e));
    }
}

fn test(out_dir: &Path, summary: &mut Summary) -> Result<(), String> {
    let program = build()?;
    let mut child = Command::new(&program)
        .arg("--repo")
        .arg(workspace())
        .arg("--work")
        .arg(out_dir.join("package"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("odc-pkg-test can't start: {e}"))?;
    let stdout = child.stdout.take().ok_or("odc-pkg-test's output")?;
    let mut reported = 0;
    let mut failed = false;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        println!("package: {line}");
        if let Some((name, result)) = scenario(&line) {
            failed |= result.is_err();
            summary.check(&format!("package_{name}"), result);
            reported += 1;
        }
    }
    let status = child.wait().map_err(|e| format!("odc-pkg-test: {e}"))?;
    if reported == 0 || (!status.success() && !failed) {
        return Err(format!(
            "odc-pkg-test ended ({status}) after {reported} checks"
        ));
    }
    Ok(())
}
