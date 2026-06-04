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
}
