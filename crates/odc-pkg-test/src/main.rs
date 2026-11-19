//! `odc-pkg-test`: the package test (Phase 11 plan, Task 5), run by `odc-e2e --only package`.
//!
//! It builds a rootless throwaway Arch root, takes a snapshot of it, builds the release
//! candidate there the AUR way and the GitHub way, as a normal build user with `makepkg -si`,
//! checks what each install leaves, and requires each uninstall to leave the root as the
//! snapshot was, apart from a short, fixed list. Along the way: two builds byte for byte, the
//! Start at boot program, an administrator's changes under OpenDeepCool's names, an upgrade,
//! and signed tags.
//!
//! Each check prints one line, `ok <name> <seconds> <note>` or `FAILED <name> <seconds> <why>`,
//! as the settings app's driver does, and the program ends with a failure if any check failed.
//! Nothing on the host changes: the root is under `--work`, and every command that changes it
//! runs inside it.

mod candidate;
mod root;
mod run;
mod snapshot;

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: odc-pkg-test --repo <dir> --work <dir> [--keep] [--export <dir>]

  --repo <dir>    the repository, whose committed HEAD is the release candidate
  --work <dir>    a folder for the throwaway root and its builds, emptied first
  --keep          leave the root in place afterwards, to look into it
  --export <dir>  copy the packages this run built, release 1 and the upgrade's release 2, out
                  of the root into <dir>: the exact packages it validated

Run by `odc-e2e --only package`. Two internal forms run inside the root's user
namespace: --snapshot <root> <out> [--skip <path>]... and --remove <dir>.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--snapshot") => inside_snapshot(&args[1..]),
        Some("--remove") => inside_remove(&args[1..]),
        _ => test(&args),
    }
}

fn test(args: &[String]) -> ExitCode {
    let mut repo = None;
    let mut work = None;
    let mut keep = false;
    let mut export = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => repo = args.next().map(PathBuf::from),
            "--work" => work = args.next().map(PathBuf::from),
            "--keep" => keep = true,
            "--export" => export = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("odc-pkg-test: {arg:?} isn't an option\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(repo), Some(work)) = (repo, work) else {
        eprintln!("odc-pkg-test: --repo and --work are needed\n\n{USAGE}");
        return ExitCode::from(2);
    };
    let absolute = |path: PathBuf| -> PathBuf {
        if path.is_absolute() {
            path
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    };
    if run::run(
        &absolute(repo),
        &absolute(work),
        keep,
        export.map(absolute).as_deref(),
    ) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `--snapshot <root> <out> [--skip <path>]...`, inside the user namespace.
fn inside_snapshot(args: &[String]) -> ExitCode {
    let [root, out, rest @ ..] = args else {
        eprintln!("--snapshot <root> <out> [--skip <path>]...");
        return ExitCode::from(2);
    };
    let mut skip = Vec::new();
    let mut rest = rest.iter();
    while let Some(flag) = rest.next() {
        match (flag.as_str(), rest.next()) {
            ("--skip", Some(path)) => skip.push(path.as_str()),
            _ => {
                eprintln!("--snapshot: {flag:?} isn't an option");
                return ExitCode::from(2);
            }
        }
    }
    match snapshot::take(&PathBuf::from(root), &skip) {
        Ok(snap) => match fs::write(out, snapshot::render(&snap)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{out}: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// `--remove <dir>`, inside the user namespace.
fn inside_remove(args: &[String]) -> ExitCode {
    let [dir] = args else {
        eprintln!("--remove <dir>");
        return ExitCode::from(2);
    };
    match fs::remove_dir_all(dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{dir}: {e}");
            ExitCode::FAILURE
        }
    }
}
