// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI suggestion ENGINE — the value-suggestion core.
//!
//! Owns the shared edit-distance machinery (`edit_distance`,
//! `closest_value_match`) that sibling modules use to build
//! closest-match lookups over domain-specific candidate lists
//! (colors, scenes, charsets, enum values, custom block names).
//! Presentation (the canonical "tip:" line format) lives in
//! `cli/ux.rs` — the CLI UX contract module.
//!
//! History (v100.0.0-nightly.1, 2026-09-04): this file previously
//! also carried `extract_clap_suggestion`, a string-parser that
//! scraped clap's rendered "tip:" line to re-append a duplicate tip
//! in main.rs. Deleted — clap's own render already prints the tip
//! exactly once, styled white via the `valids` entry in
//! `clap_styles()`. Structured tests for that contract now live in
//! `tests/clap_suggestion.rs` and `cli/ux.rs`.

// ─────────────────────────────────────────────────────────────────────────────
// Value suggestion (v80.0.0-beta.1 Z-master-1B did-you-mean audit)
// ─────────────────────────────────────────────────────────────────────────────

/// Levenshtein edit distance (shared engine; the copies in `cli/mod.rs`
/// and `config_hints/mod.rs` predate this consolidation point).
///
/// Public so sibling modules can build their own closest-match lookups
/// over domain-specific candidate lists (colors, scenes, charsets,
/// enum values, custom block names).
pub(crate) fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Closest candidate within edit distance 2 (case-insensitive), or `None`.
///
/// This is the same policy `closest_color_name` has used since bug #13:
/// distance ≤ 2 catches typos (transposition = distance 2 in plain
/// Levenshtein, single insertion/deletion/substitution = distance 1)
/// without suggesting unrelated values. Ties resolve to the FIRST
/// candidate at the best distance (deterministic given a stable
/// candidate order).
///
/// Returns an owned `String` so callers can mix `&'static str` and
/// runtime-collected `String` candidates uniformly.
pub(crate) fn closest_value_match(input: &str, candidates: &[&str]) -> Option<String> {
    let input_lower = input.trim().to_ascii_lowercase();
    if input_lower.is_empty() {
        return None;
    }
    let mut best: Option<(String, usize)> = None;
    for candidate in candidates {
        let dist = edit_distance(&input_lower, candidate);
        if dist > 2 {
            continue;
        }
        match &best {
            None => best = Some(((*candidate).to_string(), dist)),
            Some((_, d)) if dist < *d => best = Some(((*candidate).to_string(), dist)),
            _ => {}
        }
    }
    best.map(|(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_value_match_catches_single_typos() {
        let candidates = ["none", "subtle", "default", "intense"];
        assert_eq!(
            closest_value_match("sutble", &candidates),
            Some("subtle".to_string())
        );
        assert_eq!(
            closest_value_match("Defaul", &candidates),
            Some("default".to_string())
        );
    }

    #[test]
    fn closest_value_match_ignores_distant_values() {
        let candidates = ["small", "normal", "large"];
        assert_eq!(closest_value_match("extravagant", &candidates), None);
        assert_eq!(closest_value_match("", &candidates), None);
    }

    #[test]
    fn closest_value_match_is_case_insensitive() {
        let candidates = ["cinematic", "matrix", "monolith"];
        assert_eq!(
            closest_value_match("CINEMATC", &candidates),
            Some("cinematic".to_string())
        );
    }

    #[test]
    fn closest_value_match_prefers_nearest() {
        // "binari" is distance 1 from "binary" and distance ≥ 3 from
        // everything else in the charset list.
        let candidates = ["auto", "matrix", "binary", "katakana"];
        assert_eq!(
            closest_value_match("binari", &candidates),
            Some("binary".to_string())
        );
    }

    #[test]
    fn edit_distance_zero_for_identical() {
        assert_eq!(edit_distance("scene", "scene"), 0);
        assert_eq!(edit_distance("scene", "scnee"), 2);
    }
}
