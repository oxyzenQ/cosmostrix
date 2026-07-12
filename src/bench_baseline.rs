// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Baseline comparison: compare current benchmark against a saved baseline.
//!
//! Phase 7 of DeepSeek benchmark restructuring plan.
//!
//! Usage:
//!   cosmostrix --benchmark --json --save-baseline v13.5.0.json
//!   cosmostrix --benchmark --json --compare-baseline v13.4.0.json
//!
//! Comparison flags regressions (>5% FPS drop) and improvements (>5% FPS gain).

use std::collections::HashMap;

/// Metrics to compare with regression/improvement thresholds.
/// Key = JSON field name, (label, regression_threshold_pct, improvement_threshold_pct)
const COMPARE_METRICS: &[(&str, &str, f64)] = &[
    ("avg_fps", "Avg FPS", 5.0),
    ("peak_fps", "Peak FPS", 5.0),
    ("p99_frame_time_ms", "p99 frame time", 10.0),
    ("avg_frame_time_ms", "Avg frame time", 10.0),
    ("dirty_cells_per_frame", "Dirty cells/frame", 20.0),
    ("total_ns_per_cell", "ns/cell", 10.0),
    ("avg_cpu_percent", "Avg CPU%", 15.0),
];

/// Save benchmark JSON to a file.
pub fn save_baseline(path: &str, json: &str) -> Result<(), String> {
    std::fs::write(path, json).map_err(|e| format!("error: cannot save baseline to '{path}': {e}"))
}

/// Compare current benchmark JSON against a saved baseline JSON.
/// Prints a comparison table to stdout.
pub fn compare_with_baseline(baseline_path: &str, current_json: &str) -> Result<(), String> {
    let baseline_text = std::fs::read_to_string(baseline_path)
        .map_err(|e| format!("error: cannot read baseline '{baseline_path}': {e}"))?;

    let baseline = parse_json_flat(&baseline_text);
    let current = parse_json_flat(current_json);

    println!();
    println!("BASELINE COMPARISON");
    println!("───────────────────");
    println!("  baseline: {baseline_path}");

    // Check if screen sizes match
    let base_cols = baseline.get("cols").or_else(|| baseline.get("config.cols"));
    let base_lines = baseline
        .get("lines")
        .or_else(|| baseline.get("config.lines"));
    let curr_cols = current.get("cols").or_else(|| current.get("config.cols"));
    let curr_lines = current.get("lines").or_else(|| current.get("config.lines"));
    if base_cols.is_some_and(|bc| curr_cols.is_some_and(|cc| bc != cc))
        || base_lines.is_some_and(|bl| curr_lines.is_some_and(|cl| bl != cl))
    {
        println!(
            "  ⚠ WARNING: screen sizes differ (baseline {}x{}, current {}x{})",
            base_cols.unwrap_or(&"?".to_string()),
            base_lines.unwrap_or(&"?".to_string()),
            curr_cols.unwrap_or(&"?".to_string()),
            curr_lines.unwrap_or(&"?".to_string())
        );
        println!("  Comparison may not be meaningful.");
    }
    println!();

    let mut regressions = 0;
    let mut improvements = 0;

    println!(
        "  {:<25} {:>12} {:>12} {:>10} {:>8}",
        "Metric", "Baseline", "Current", "Delta", "Status"
    );
    println!(
        "  {:<25} {:>12} {:>12} {:>10} {:>8}",
        "──────", "────────", "───────", "─────", "──────"
    );

    for (key, label, threshold) in COMPARE_METRICS {
        let base_val = baseline.get(*key).and_then(|v| v.parse::<f64>().ok());
        let curr_val = current.get(*key).and_then(|v| v.parse::<f64>().ok());

        match (base_val, curr_val) {
            (Some(bv), Some(cv)) => {
                let pct = if bv != 0.0 {
                    ((cv - bv) / bv) * 100.0
                } else {
                    0.0
                };

                let better = if is_lower_better(key) {
                    pct < 0.0
                } else {
                    pct > 0.0
                };
                let status = if pct.abs() < *threshold {
                    "OK"
                } else if better {
                    "✓ BETTER"
                } else {
                    "✗ WORSE"
                };

                if status.contains("WORSE") {
                    regressions += 1;
                } else if status.contains("BETTER") {
                    improvements += 1;
                }

                println!(
                    "  {:<25} {:>12} {:>12} {:>+9.1}% {:>8}",
                    label,
                    crate::humanize::humanize_f64(bv),
                    crate::humanize::humanize_f64(cv),
                    pct,
                    status
                );
            }
            _ => {
                println!(
                    "  {:<25} {:>12} {:>12} {:>10} {:>8}",
                    label, "—", "—", "—", "N/A"
                );
            }
        }
    }

    println!();
    if regressions > 0 {
        println!("  ⚠ {regressions} REGRESSION(S) detected!");
    } else if improvements > 0 {
        println!("  ✓ {improvements} improvement(s) detected. No regressions.");
    } else {
        println!("  ✓ No significant changes detected.");
    }
    println!();

    Ok(())
}

/// For some metrics, lower is better (frame time, RSS, CPU%, ns/cell).
/// For others, higher is better (FPS).
fn is_lower_better(key: &str) -> bool {
    matches!(
        key,
        "p99_frame_time_ms"
            | "avg_frame_time_ms"
            | "total_ns_per_cell"
            | "peak_rss"
            | "avg_cpu_percent"
    )
}

/// Simple flat JSON parser: extracts "key": numeric_value pairs.
/// Handles nested objects by flattening with dot notation.
/// This is a minimal parser — not a full JSON parser.
fn parse_json_flat(json: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    // Simple approach: find all "key": value patterns
    let bytes = json.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for "key": patterns
        if bytes[i] == b'"' {
            let key_start = i + 1;
            let mut j = key_start;
            while j < bytes.len() && bytes[j] != b'"' {
                if bytes[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            if j < bytes.len() {
                let key = &json[key_start..j];
                let mut k = j + 1;
                // Skip whitespace
                while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k < bytes.len() && bytes[k] == b':' {
                    k += 1;
                    // Skip whitespace
                    while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                        k += 1;
                    }
                    // Extract value
                    if k < bytes.len() {
                        let (val, end) = extract_value(json, k);
                        if !val.is_empty() {
                            map.insert(key.to_string(), val);
                        }
                        i = end;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    map
}

/// Extract a JSON value starting at position `start`. Returns (value_string, end_position).
/// For nested objects/arrays, returns position right after the opening brace
/// (not skipping the content — the main loop will scan through the keys inside).
fn extract_value(json: &str, start: usize) -> (String, usize) {
    let bytes = json.as_bytes();
    if start >= bytes.len() {
        return (String::new(), start);
    }

    match bytes[start] {
        b'"' => {
            // String value
            let mut end = start + 1;
            while end < bytes.len() && bytes[end] != b'"' {
                if bytes[end] == b'\\' {
                    end += 1;
                }
                end += 1;
            }
            (json[start + 1..end.min(bytes.len())].to_string(), end + 1)
        }
        b'{' | b'[' => {
            // Nested object/array — don't skip it. Return position after
            // the opening brace so the main loop scans through the keys inside.
            (String::new(), start + 1)
        }
        _ => {
            // Number or boolean or null
            let mut end = start;
            while end < bytes.len()
                && !bytes[end].is_ascii_whitespace()
                && bytes[end] != b','
                && bytes[end] != b'}'
                && bytes[end] != b']'
            {
                end += 1;
            }
            (json[start..end].to_string(), end)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for the silent-no-op bug where `--save-baseline`
    /// was ignored in text mode (without `--json`). The wiring fix lives
    /// in bench.rs, but this test guards the public save→compare round-trip
    /// contract so a future refactor can't silently break it again.
    ///
    /// Bug history: v13.5.0 — user ran
    ///   `cosmostrix --benchmark --save-baseline v13.5.0-final.json`
    /// without `--json`, the file was never written, and the subsequent
    /// `--compare-baseline v13.5.0-final.json` failed with
    /// "No such file or directory".
    #[test]
    fn save_then_compare_baseline_round_trip() {
        // Synthesize a minimal benchmark JSON with the fields compare_with_baseline reads.
        let baseline_json = r#"{
            "avg_fps": 37000.0,
            "peak_fps": 58000.0,
            "p99_frame_time_ms": 0.032,
            "avg_frame_time_ms": 0.027,
            "dirty_cells_per_frame": 362.0,
            "total_ns_per_cell": 74.0,
            "avg_cpu_percent": 95.0,
            "cols": 120,
            "lines": 40
        }"#;

        let tmp = std::env::temp_dir().join(format!(
            "cosmostrix-baseline-roundtrip-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        // save_baseline must write the file.
        save_baseline(tmp.to_str().unwrap(), baseline_json)
            .expect("save_baseline must succeed for a writable temp path");
        assert!(tmp.exists(), "baseline file must exist after save_baseline");

        // compare_with_baseline must be able to read it back without error.
        // (We don't care about the printed comparison here — only that
        // reading + parsing doesn't fail. Any regression that breaks the
        // save→compare path will surface as an Err here.)
        let current_json = r#"{
            "avg_fps": 38000.0,
            "peak_fps": 57000.0,
            "p99_frame_time_ms": 0.033,
            "avg_frame_time_ms": 0.026,
            "dirty_cells_per_frame": 361.0,
            "total_ns_per_cell": 73.0,
            "avg_cpu_percent": 98.0,
            "cols": 120,
            "lines": 40
        }"#;
        let result = compare_with_baseline(tmp.to_str().unwrap(), current_json);
        assert!(
            result.is_ok(),
            "compare_with_baseline must succeed: {:?}",
            result
        );

        std::fs::remove_file(&tmp).ok();
    }

    /// `save_baseline` must return an Err (not panic, not silently succeed)
    /// when the target path is unwritable. This guards against future
    /// "swallow the error" regressions.
    #[test]
    fn save_baseline_returns_err_on_unwritable_path() {
        // A path inside a nonexistent directory is unwritable.
        let bad_path = "/nonexistent_dir_for_cosmostrix_test/baseline.json";
        let result = save_baseline(bad_path, "{}");
        assert!(result.is_err(), "save_baseline must Err on unwritable path");
        assert!(
            result.unwrap_err().contains("cannot save baseline"),
            "error message must explain what failed"
        );
    }

    /// `compare_with_baseline` must return an Err when the baseline file
    /// doesn't exist — and the error message must mention the path so the
    /// user can diagnose. This is the exact error the user hit.
    #[test]
    fn compare_baseline_returns_err_on_missing_file() {
        let missing = "/nonexistent_dir_for_cosmostrix_test/missing-baseline.json";
        let result = compare_with_baseline(missing, "{}");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("cannot read baseline"), "got: {err}");
        assert!(
            err.contains(missing),
            "error must mention the path; got: {err}"
        );
    }
}
