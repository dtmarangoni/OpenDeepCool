//! `odc-mockups <out-dir>`: writes the settings app's design mockups into `<out-dir>`, normally
//! `docs/design/mockups`. A state's change updates `odc_mockups::pages` and the design first, then
//! the mockups are written again; `cargo test -p odc-mockups` fails when the committed pages
//! differ from what this writes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(out), None) = (args.next(), args.next()) else {
        eprintln!("usage: odc-mockups <out-dir>");
        return ExitCode::from(2);
    };
    match write(&PathBuf::from(out)) {
        Ok(n) => {
            println!("wrote {n}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("odc-mockups: {e}");
            ExitCode::FAILURE
        }
    }
}

fn write(out: &Path) -> Result<usize, String> {
    let read = |name: &str| {
        let path = Path::new(odc_mockups::SOURCES).join(name);
        fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    };
    let (css, js) = (read("app.css")?, read("app.js")?);
    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let pages = odc_mockups::render(&css, &js);
    for (file, html) in &pages {
        let path = out.join(file);
        fs::write(&path, html).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(pages.len())
}
