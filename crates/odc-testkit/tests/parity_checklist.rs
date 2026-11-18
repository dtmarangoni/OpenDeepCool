//! The parity checklist's OpenDeepCool column (Phase 11 plan, Task 9; AC5): every row says how
//! OpenDeepCool meets it, and every `test` row names the tests that prove it, so a renamed or
//! deleted test, or a row left without one, fails here instead of going unnoticed.
//!
//! A row's last cell is one of:
//! - `test: [`name`](../../path/to/file.rs), …`, optionally followed by `; ours: ID, ID` for a row
//!   that is DeepCreative's rule in part, whose OpenDeepCool half is in those rows;
//! - `replaced: ID, ID`, DeepCreative's behaviour only, whose OpenDeepCool rule is in the rows
//!   named (or, for a row about something OpenDeepCool doesn't do, a link into the spec);
//! - `device`, the cooler's own behaviour, seen by the owner in Phase 3.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const CHECKLIST: &str = "docs/spec/parity-checklist.md";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn is_id(word: &str) -> bool {
    let letters = word.bytes().take_while(u8::is_ascii_uppercase).count();
    let digits = &word[letters..];
    (1..=2).contains(&letters) && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The rows: ID and last cell, in order. A table row is one whose first cell is an ID.
fn rows(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 3 || !cells[0].is_empty() || !is_id(cells[1]) {
            continue;
        }
        // Leading and trailing empty cells, and ID, Mode, Item, Spec, Evidence, Owner check and
        // OpenDeepCool between them.
        if cells.len() != 9 {
            return Err(format!("{}: {} cells, not 7", cells[1], cells.len() - 2));
        }
        rows.push((cells[1].to_owned(), cells[7].to_owned()));
    }
    Ok(rows)
}

/// The `[`name`](path)` links of a cell, in order.
fn links(cell: &str) -> Vec<(&str, &str)> {
    let mut found = Vec::new();
    let mut rest = cell;
    while let Some(start) = rest.find("[`") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("`](") else { break };
        let name = &rest[..end];
        rest = &rest[end + 3..];
        let Some(close) = rest.find(')') else { break };
        found.push((name, &rest[..close]));
        rest = &rest[close + 1..];
    }
    found
}

/// Whether `text` declares a function called `name`.
fn declares(text: &str, name: &str) -> bool {
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    text.match_indices(&format!("fn {name}"))
        .any(|(at, found)| {
            let before = text[..at].bytes().next_back();
            let after = text.as_bytes().get(at + found.len()).copied();
            !before.is_some_and(word) && !after.is_some_and(word)
        })
}

/// Everything wrong with the OpenDeepCool column, with links resolved from `docs/spec` of `root`.
fn problems(text: &str, root: &Path) -> Vec<String> {
    let rows = match rows(text) {
        Ok(rows) => rows,
        Err(e) => return vec![e],
    };
    let mut kinds = BTreeMap::new();
    let mut out = Vec::new();
    for (id, cell) in &rows {
        if kinds.insert(id.as_str(), cell.as_str()).is_some() {
            out.push(format!("{id}: twice"));
        }
    }
    let is_test = |id: &str| kinds.get(id).is_some_and(|cell| cell.starts_with("test: "));
    let spec = root.join("docs/spec");
    for (id, cell) in &rows {
        if let Some(tests) = cell.strip_prefix("test: ") {
            let (tests, ours) = match tests.split_once("; ours:") {
                Some((tests, ours)) => (tests, Some(ours)),
                None => (tests, None),
            };
            let named = links(tests);
            if named.is_empty() {
                out.push(format!("{id}: `test` names no test"));
            }
            for (name, path) in named {
                match fs::read_to_string(spec.join(path)) {
                    Err(e) => out.push(format!("{id}: {path}: {e}")),
                    Ok(source) if !declares(&source, name) => {
                        out.push(format!("{id}: no fn {name} in {path}"));
                    }
                    Ok(_) => {}
                }
            }
            for word in ours.into_iter().flat_map(|o| o.split([',', ' '])) {
                if !word.is_empty() && !is_test(word) {
                    out.push(format!("{id}: ours: {word} isn't a test row"));
                }
            }
        } else if let Some(replaced) = cell.strip_prefix("replaced:") {
            // The ID words; a row about something OpenDeepCool doesn't do points at the spec
            // instead.
            let ids: Vec<&str> = replaced
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| is_id(w))
                .collect();
            for word in &ids {
                if !is_test(word) {
                    out.push(format!("{id}: replaced by {word}, which isn't a test row"));
                }
            }
            if ids.is_empty() {
                let target = replaced
                    .split("](")
                    .nth(1)
                    .and_then(|t| t.split([')', '#']).next());
                match target {
                    Some(file) if spec.join(file).is_file() => {}
                    other => out.push(format!("{id}: replaced by nothing: {other:?}")),
                }
            }
        } else if cell != "device" {
            out.push(format!("{id}: {cell:?} isn't test, replaced or device"));
        }
    }
    out
}

#[test]
fn every_row_of_the_checklist_says_how_opendeepcool_meets_it_and_every_test_it_names_exists() {
    let text = fs::read_to_string(repo().join(CHECKLIST)).expect("the checklist");
    let found = problems(&text, &repo());
    assert!(found.is_empty(), "{}", found.join("\n"));
    let rows = rows(&text).expect("the rows");
    // 89 rows until row S9, the cooler's fans and pump, left with the feature (Phase 11a).
    assert!(rows.len() >= 88, "{} rows", rows.len());
    let tests = rows.iter().filter(|(_, c)| c.starts_with("test: ")).count();
    assert!(tests >= 80, "{tests} test rows");
}

/// The checker finds what it is for: each of these damaged copies of a real row fails.
#[test]
fn the_checker_fails_a_row_with_no_test_a_missing_fn_a_missing_file_or_a_bad_pointer() {
    let text = fs::read_to_string(repo().join(CHECKLIST)).expect("the checklist");
    let root = repo();
    assert!(problems(&text, &root).is_empty());
    let d11 = text
        .lines()
        .find(|l| l.starts_with("| D11 |"))
        .expect("row D11");
    let first = links(d11)[0];
    let (name, path) = first;
    let damaged = |from: &str, to: &str| {
        assert!(text.contains(from), "{from}");
        problems(&text.replacen(from, to, 1), &root)
    };
    // The same, inside row D11 only.
    let damaged_d11 = |from: &str, to: &str| {
        assert!(d11.contains(from), "{from}");
        problems(&text.replacen(d11, &d11.replacen(from, to, 1), 1), &root)
    };
    // A renamed test.
    let found = damaged_d11(
        &format!("[`{name}`]({path})"),
        &format!("[`{name}_renamed`]({path})"),
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("D11: no fn"), "{found:?}");
    // A moved file.
    let found = damaged_d11(
        &format!("[`{name}`]({path})"),
        &format!("[`{name}`]({path}.gone)"),
    );
    assert!(
        found
            .iter()
            .any(|f| f.starts_with("D11: ") && f.contains(".gone")),
        "{found:?}"
    );
    // The old, bare kinds and a kind nobody defined.
    for cell in ["| test |", "| pending |", "| test; ours: D36 |"] {
        let line = d11
            .rsplit_once("| test:")
            .map(|(head, _)| head)
            .expect("a test row");
        let broken = text.replacen(d11, &format!("{line}{cell}"), 1);
        assert!(!problems(&broken, &root).is_empty(), "{cell}");
    }
    // A pointer at a row that is not a test row, or at none.
    let found = damaged("replaced: D39 |", "replaced: P8 |");
    assert!(
        found
            .iter()
            .any(|f| f.contains("P8, which isn't a test row")),
        "{found:?}"
    );
    let found = damaged("replaced: D39 |", "replaced: D99 |");
    assert!(found.iter().any(|f| f.contains("D99")), "{found:?}");
    let found = damaged("replaced: D39 |", "replaced: nothing |");
    assert!(
        found.iter().any(|f| f.contains("replaced by nothing")),
        "{found:?}"
    );
    // An `ours` row that doesn't exist.
    let found = damaged("; ours: D36, D37 |", "; ours: D36, D77 |");
    assert!(found.iter().any(|f| f.contains("D77")), "{found:?}");
}

#[test]
fn a_function_is_found_by_its_whole_name_only() {
    assert!(declares("#[test]\nfn the_name() {}", "the_name"));
    assert!(declares("pub fn the_name(", "the_name"));
    assert!(!declares("fn the_name_longer() {}", "the_name"));
    assert!(!declares("fn a_the_name() {}", "the_name"));
    assert!(!declares("// the_name", "the_name"));
}
