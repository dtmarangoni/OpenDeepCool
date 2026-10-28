//! Running git, without a shell.

use std::process::Command;

/// Runs git and returns its standard output, or the command and its error.
pub fn git(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub fn git_text(args: &[&str]) -> Result<String, String> {
    git(args).map(|out| String::from_utf8_lossy(&out).into_owned())
}

/// The fields of a `-z` listing: NUL-separated, the empty last one dropped.
pub fn nul_fields(out: &[u8]) -> Vec<String> {
    out.split(|&b| b == 0)
        .filter(|f| !f.is_empty())
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect()
}

/// The paths git treats as binary in a `--numstat -z` listing, where a binary file's line counts
/// are `-`. Renames are off, so each record is `<added>\t<deleted>\t<path>`.
pub fn binary_paths(numstat: &[u8]) -> Vec<String> {
    nul_fields(numstat)
        .iter()
        .filter_map(|record| {
            let mut parts = record.splitn(3, '\t');
            let (added, deleted, path) = (parts.next()?, parts.next()?, parts.next()?);
            (added == "-" && deleted == "-").then(|| path.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numstat_marks_binary_files_with_dashes() {
        let listing = b"3\t1\tsrc/a.rs\x00-\t-\timg.png\x000\t0\tempty.txt\x00";
        assert_eq!(binary_paths(listing), ["img.png"]);
    }
}
