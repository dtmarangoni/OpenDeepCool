//! What the guard checks in an identity, a message and a file (AGENTS.md rules 6 and 10): the
//! Phase 2 checks, the identity git will record, and binary files (owner, 2026-10-27; solution
//! review P2-01 to P2-03).

use std::fs;
use std::path::Path;

/// Folders whose files never go into the repository.
const LOCAL_ONLY: [&str; 4] = [
    "docs/reference/vendor/",
    "docs/local/",
    "research/",
    "captures/",
];

/// Installers, DLLs, Electron bundles, V8 bytecode, native modules, firmware, PDFs and USB
/// captures. `.bin` stays (P2-06).
const BLOCKED_EXTENSIONS: [&str; 13] = [
    "exe", "dll", "sys", "msi", "asar", "jsc", "node", "pak", "7z", "bin", "pdf", "pcap", "pcapng",
];

const MAX_BYTES: usize = 2 * 1024 * 1024;

/// Binary files the guard lets through: none yet (P2-03).
const BINARY_ALLOWED: [&str; 0] = [];

const NO_REPLY: &str = "@users.noreply.github.com";

/// DeepCreative code signatures, built from parts so this file doesn't hold them whole.
fn code_markers() -> [String; 3] {
    [
        ["__vite", "Preload"].concat(),
        ["defineStore(\"lp", "Store\""].concat(),
        ["bytecode", "-loader"].concat(),
    ]
}

/// The sensitive values, lower-case, from the git-ignored `docs/local/machine.md`, between
/// `<!-- guard:sensitive-begin -->` and `<!-- guard:sensitive-end -->`. Empty when the file is
/// absent, as on a fresh clone elsewhere.
#[derive(Clone, Debug, Default)]
pub struct Sensitive {
    values: Vec<String>,
}

impl Sensitive {
    pub fn load(repo: &Path) -> Result<Sensitive, String> {
        let path = repo.join("docs/local/machine.md");
        match fs::read_to_string(&path) {
            Ok(text) => Ok(Sensitive::parse(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Sensitive::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn parse(text: &str) -> Sensitive {
        let values = text
            .lines()
            .skip_while(|l| !l.contains("<!-- guard:sensitive-begin -->"))
            .skip(1)
            .take_while(|l| !l.contains("<!-- guard:sensitive-end -->"))
            .map(|l| {
                // A bullet, `-` or `*`, counts only when whitespace follows it.
                let l = l.trim_start();
                let mut chars = l.chars();
                let bullet = matches!(chars.next(), Some('-' | '*'))
                    && chars.next().is_some_and(char::is_whitespace);
                let l = if bullet { l[1..].trim_start() } else { l };
                l.replace('`', "").trim_end().to_lowercase()
            })
            .filter(|v| !v.is_empty())
            .collect();
        Sensitive { values }
    }

    /// Whether the list is empty, as on a clone without `docs/local/`.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The values, for a check that the list was read whole.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// The first listed value the text holds, letter case ignored.
    pub fn hit(&self, text: &str) -> Option<&str> {
        let lower = text.to_lowercase();
        self.values
            .iter()
            .find(|v| lower.contains(v.as_str()))
            .map(String::as_str)
    }
}

/// An identity as git records it, `Name <email> …`: the no-reply address, and no listed value.
pub fn check_identity(who: &str, ident: &str, sensitive: &Sensitive) -> Vec<String> {
    let mut problems = Vec::new();
    let email = ident
        .split_once('<')
        .and_then(|(_, rest)| rest.split_once('>'))
        .map_or("", |(email, _)| email);
    if !email.ends_with(NO_REPLY) {
        problems.push(format!(
            "{who} {email:?} isn't the GitHub no-reply address (AGENTS.md rule 10)"
        ));
    }
    if sensitive.hit(ident).is_some() {
        problems.push(format!("{who} contains a sensitive value"));
    }
    problems
}

/// A commit or tag message: no listed value, comment lines aside.
pub fn check_message(what: &str, message: &str, sensitive: &Sensitive) -> Vec<String> {
    let text: String = message
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    match sensitive.hit(&text) {
        Some(hit) => vec![format!("{what} contains a sensitive value (\"{hit}\")")],
        None => Vec::new(),
    }
}

/// A file about to be committed or pushed.
pub fn check_file(path: &str, content: &[u8], binary: bool, sensitive: &Sensitive) -> Vec<String> {
    let lower = path.to_lowercase();
    if LOCAL_ONLY.iter().any(|dir| path.starts_with(dir)) {
        return vec![format!("{path}: a local-only folder")];
    }
    if lower
        .rsplit_once('.')
        .is_some_and(|(_, ext)| BLOCKED_EXTENSIONS.contains(&ext))
    {
        return vec![format!("{path}: a binary or vendor file type")];
    }
    if content.len() > MAX_BYTES {
        return vec![format!(
            "{path}: {} bytes, over the {MAX_BYTES}-byte limit",
            content.len()
        )];
    }
    if binary && !BINARY_ALLOWED.contains(&path) {
        return vec![format!("{path}: a binary file (P2-03)")];
    }
    let mut problems = Vec::new();
    let text = String::from_utf8_lossy(content);
    // Markdown may quote these names.
    if !lower.ends_with(".md") && code_markers().iter().any(|m| text.contains(m.as_str())) {
        problems.push(format!("{path}: looks like DeepCreative code"));
    }
    if let Some(hit) = sensitive.hit(&text) {
        problems.push(format!("{path} contains a sensitive value (\"{hit}\")"));
    }
    problems
}

/// The paths whose change runs the Rust checks: code, packaging, the frames and the contract the
/// tests read, the guard and CI (owner, 2026-10-27; solution review P5-08).
pub fn runs_rust_checks(path: &str) -> bool {
    const PREFIXES: [&str; 7] = [
        "crates/",
        "tools/",
        "packaging/",
        "docs/spec/frames/",
        ".githooks/",
        ".github/",
        "docs/spec/dbus.md",
    ];
    path == "Cargo.toml" || path == "Cargo.lock" || PREFIXES.iter().any(|p| path.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sensitive() -> Sensitive {
        Sensitive::parse(
            "# notes\n- not listed\n<!-- guard:sensitive-begin -->\n\n- SecretSerial123\n- `Dev Realname`\n<!-- guard:sensitive-end -->\n- not listed either\n",
        )
    }

    #[test]
    fn the_list_is_read_between_the_markers() {
        let s = sensitive();
        assert_eq!(s.len(), 2);
        assert_eq!(s.hit("x SECRETSERIAL123 y"), Some("secretserial123"));
        assert_eq!(s.hit("dev realname"), Some("dev realname"));
        assert_eq!(s.hit("not listed"), None);
        assert_eq!(Sensitive::parse("no markers here").len(), 0);
    }

    #[test]
    fn identities_need_the_no_reply_address_and_no_listed_value() {
        let s = sensitive();
        let good = "Dev <1+dev@users.noreply.github.com> 1700000000 +0000";
        assert!(check_identity("author", good, &s).is_empty());
        let wrong = check_identity("author", "Dev <dev@example.com> 1700000000 +0000", &s);
        assert!(wrong[0].contains("isn't the GitHub no-reply address"));
        let named = check_identity(
            "committer",
            "Dev Realname <1+d@users.noreply.github.com>",
            &s,
        );
        assert!(named[0].contains("sensitive value"));
    }

    #[test]
    fn files_are_refused_for_each_phase_2_reason_and_binaries() {
        let s = sensitive();
        let refused = [
            ("docs/local/notes.txt", &b"x"[..], false, "local-only"),
            ("research/x.txt", b"x", false, "local-only"),
            ("tools/app.EXE", b"x", false, "vendor file type"),
            ("firmware.bin", b"x", false, "vendor file type"),
            ("img.png", b"\x89PNG\0", true, "a binary file"),
            (
                "notes.txt",
                b"serial: secretserial123",
                false,
                "sensitive value",
            ),
        ];
        for (path, content, binary, reason) in refused {
            let problems = check_file(path, content, binary, &s);
            assert!(
                problems.iter().any(|p| p.contains(reason)),
                "{path}: {problems:?}"
            );
        }
        let big = vec![b'a'; MAX_BYTES + 1];
        assert!(check_file("big.txt", &big, false, &s)[0].contains("limit"));
        let marker = code_markers()[0].clone();
        assert!(check_file("app.js", marker.as_bytes(), false, &s)[0].contains("DeepCreative"));
        assert!(check_file("notes.md", marker.as_bytes(), false, &s).is_empty());
        assert!(check_file("src/main.rs", b"fn main() {}", false, &s).is_empty());
    }

    #[test]
    fn messages_are_checked_without_their_comment_lines() {
        let s = sensitive();
        assert!(check_message("the message", "# SecretSerial123\nfix", &s).is_empty());
        assert!(!check_message("the message", "fix SecretSerial123", &s).is_empty());
    }

    #[test]
    fn code_frames_contract_guard_and_ci_run_the_rust_checks_and_docs_dont() {
        for path in [
            "Cargo.toml",
            "Cargo.lock",
            "crates/odc-protocol/src/lib.rs",
            "tools/guard/src/main.rs",
            "packaging/udev/70-opendeepcool.rules",
            "docs/spec/frames/mode-001.txt",
            "docs/spec/dbus.md",
            ".githooks/pre-commit",
            ".github/workflows/ci.yml",
        ] {
            assert!(runs_rust_checks(path), "{path}");
        }
        for path in [
            "AGENTS.md",
            "docs/prp/plans/x.plan.md",
            "docs/spec/display.md",
            "README.md",
        ] {
            assert!(!runs_rust_checks(path), "{path}");
        }
    }
}
