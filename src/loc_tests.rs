// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    const MAX_RUST_LOC: usize = 1000;

    #[test]
    fn rust_source_files_stay_under_line_cap() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rust_files(&src, &mut files);

        let mut oversized = Vec::new();
        for path in files {
            let text = fs::read_to_string(&path).expect("read rust source");
            let lines = text.lines().count();
            if lines > MAX_RUST_LOC {
                oversized.push(format!("{} ({lines} lines)", path.display()));
            }
        }

        assert!(
            oversized.is_empty(),
            "Rust files must stay under {MAX_RUST_LOC} LOC:\n{}",
            oversized.join("\n")
        );
    }

    fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read source dir") {
            let entry = entry.expect("read source entry");
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    // ── Canonical Identity / Metadata Tests ──

    /// Scan all .rs and .toml files for lowercase `oxyzenq` (wrong casing).
    /// User-facing source and metadata must use the canonical `oxyzenQ`.
    #[test]
    fn no_lowercase_oxyzenq_in_source_or_toml() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut violations = Vec::new();

        scan_dir_for_lowercase_oxyzenq(root, &mut violations);

        assert!(
            violations.is_empty(),
            "lowercase 'oxyzenq' found in tracked files — use 'oxyzenQ':\n{}",
            violations.join("\n")
        );
    }

    fn scan_dir_for_lowercase_oxyzenq(dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("read entry");
            let path = entry.path();
            if path.is_dir() {
                // Skip .git and target directories
                if path
                    .file_name()
                    .is_some_and(|n| n == ".git" || n == "target")
                {
                    continue;
                }
                scan_dir_for_lowercase_oxyzenq(&path, out);
            } else {
                // Skip this guard file itself (it contains the string in assertions)
                if path.file_name().is_some_and(|n| n == "loc_tests.rs") {
                    continue;
                }
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext, "rs" | "toml" | "md" | "sh") {
                    if let Ok(text) = fs::read_to_string(&path) {
                        for (i, line) in text.lines().enumerate() {
                            let trimmed = line.trim();
                            // Skip SPDX copyright headers
                            if trimmed.starts_with("// Copyright") {
                                continue;
                            }
                            // Flag lines containing lowercase "oxyzenq"
                            // that do NOT also contain the canonical "oxyzenQ".
                            // This catches pure-lowercase references while
                            // allowing lines with the correct casing.
                            if trimmed.contains("oxyzenq") && !trimmed.contains("oxyzenQ") {
                                out.push(format!("{}:{}: {}", path.display(), i + 1, trimmed));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Verify runtime version report uses canonical author and source URL.
    #[test]
    fn version_report_uses_canonical_author_and_source() {
        let report = crate::info::version_report();
        assert!(
            report.contains("rezky_nightky (oxyzenQ)"),
            "version_report must contain canonical author 'rezky_nightky (oxyzenQ)', got:\n{report}"
        );
        assert!(
            report.contains("https://github.com/oxyzenQ/cosmostrix"),
            "version_report must contain canonical source URL, got:\n{report}"
        );
    }

    /// Verify update URLs use canonical repo owner.
    #[test]
    fn update_urls_use_canonical_repo_owner() {
        assert!(
            crate::update::CANONICAL_GITHUB_API_URL.contains("oxyzenQ"),
            "GITHUB_API_URL must contain canonical oxyzenQ"
        );
        assert!(
            crate::update::CANONICAL_RELEASES_URL.contains("oxyzenQ"),
            "RELEASES_URL must contain canonical oxyzenQ"
        );
        assert!(
            !crate::update::CANONICAL_GITHUB_API_URL.contains("oxyzenq"),
            "GITHUB_API_URL must not contain lowercase oxyzenq"
        );
    }

    /// Verify Cargo.toml repository field uses canonical URL.
    #[test]
    fn cargo_toml_uses_canonical_repository() {
        let cargo_toml =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
                .expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("https://github.com/oxyzenQ/cosmostrix"),
            "Cargo.toml repository must use canonical oxyzenQ"
        );
        assert!(
            !cargo_toml.contains("github.com/oxyzenq"),
            "Cargo.toml must not contain lowercase oxyzenq"
        );
    }
}
