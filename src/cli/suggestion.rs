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

// ─────────────────────────────────────────────────────────────────────────────
// Case-insensitive flag suggestion (v100.0.0-nightly.1, owner `--LIS` report)
// ─────────────────────────────────────────────────────────────────────────────

/// Jaro similarity (clap's own flag-suggestion metric), case-insensitive.
///
/// Mirrors `strsim::jaro` faithfully — clap 4.5's `did_you_mean` engine
/// scores flag candidates with plain Jaro at a > 0.7 confidence
/// threshold (they dropped Jaro-Winkler in GH #4660: strsim's
/// implementation scores common prefixes >= 10 as perfect matches).
/// The only delta here: both strings are LOWERCASED first, so a
/// case-variant prefix of a known flag still scores — case-sensitive
/// Jaro("LIS", "list-scenes") = 0.0 (no char matches at all) while
/// `jaro_ci` = 3 matches, 0 transpositions = 0.757 > 0.7.
///
/// For already-lowercase inputs the score is IDENTICAL to clap's own
/// engine (ASCII flags are lowercase), which is what makes the
/// fallback in `cli::ux::enrich_unknown_arg_suggestion` safe: it only
/// fires when clap found nothing, and can never suggest a flag clap
/// itself would have rejected for the same lowercase input.
///
/// Algorithm (strsim::generic_jaro): greedy in-window matching with a
/// `max(len_a, len_b) / 2 - 1` (saturating) window, transpositions
/// counted via the last-matched index, score
/// `(m/|a| + m/|b| + (m - t)/m) / 3`.
pub(crate) fn jaro_ci(a: &str, b: &str) -> f64 {
    let lower = |s: &str| -> Vec<char> { s.chars().flat_map(char::to_lowercase).collect() };
    let a = lower(a);
    let b = lower(b);
    let (a_len, b_len) = (a.len(), b.len());
    if a_len == 0 && b_len == 0 {
        return 1.0;
    }
    if a_len == 0 || b_len == 0 {
        return 0.0;
    }
    if a_len == 1 && b_len == 1 {
        return if a[0] == b[0] { 1.0 } else { 0.0 };
    }
    let search_range = (a_len.max(b_len) / 2).saturating_sub(1);
    let mut b_consumed = vec![false; b_len];
    let mut matches = 0.0_f64;
    let mut transpositions = 0.0_f64;
    let mut b_match_index = 0_usize;
    for (i, &a_elem) in a.iter().enumerate() {
        let min_bound = i.saturating_sub(search_range);
        let max_bound = (b_len - 1).min(i + search_range);
        if min_bound > max_bound {
            continue;
        }
        for (j, &b_elem) in b.iter().enumerate() {
            if min_bound <= j && j <= max_bound && a_elem == b_elem && !b_consumed[j] {
                b_consumed[j] = true;
                matches += 1.0;
                if j < b_match_index {
                    transpositions += 1.0;
                }
                b_match_index = j;
                break;
            }
        }
    }
    if matches == 0.0 {
        0.0
    } else {
        (1.0 / 3.0)
            * ((matches / a_len as f64)
                + (matches / b_len as f64)
                + ((matches - transpositions) / matches))
    }
}

/// Closest long-flag name by case-insensitive Jaro (clap's > 0.7
/// confidence threshold), or `None`. Input and candidates are BARE
/// names (no leading dashes).
///
/// Used ONLY as the fallback when clap's own case-sensitive engine
/// found nothing (see `cli::ux::enrich_unknown_arg_suggestion`):
/// clap suggests `--lis` -> `--list-scenes` but stays silent for
/// `--LIS`, and the rescue must not add noise clap would not make.
/// Ties resolve to the LAST candidate in declaration order — clap's
/// own `did_you_mean` inserts equal-confidence candidates in
/// iteration order and pops the last, so the lowercase twin of a
/// rescued typo suggests the same flag clap would have suggested
/// (`--LIS` and `--lis` both point at `--list-scenes`).
pub(crate) fn closest_long_flag_ci(input: &str, candidates: &[&str]) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut best: Option<(String, f64)> = None;
    for candidate in candidates {
        let confidence = jaro_ci(trimmed, candidate);
        if confidence <= 0.7 {
            continue;
        }
        match &best {
            // `>=` not `>`: later candidates win ties, mirroring
            // clap's ascending-sort-then-pop tie-break.
            None => best = Some(((*candidate).to_string(), confidence)),
            Some((_, c)) if confidence >= *c => {
                best = Some(((*candidate).to_string(), confidence));
            }
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

    // ── jaro_ci / closest_long_flag_ci (owner `--LIS` report) ───────────

    #[test]
    fn jaro_ci_matches_strsim_jaro_for_lowercase() {
        // Hand-computed against strsim::jaro semantics: "lis" vs
        // "list-scenes" = 3 matches / 0 transpositions / window 4 ->
        // (m/|a| + m/|b| + m/m) / 3 = (1 + 3/11 + 1) / 3.
        assert!((jaro_ci("lis", "list-scenes") - (1.0 + 3.0 / 11.0 + 1.0) / 3.0).abs() < 1e-9);
    }

    #[test]
    fn jaro_ci_rescues_case_variants_case_sensitive_jaro_misses() {
        // Case-sensitive Jaro("LIS", "list-scenes") = 0.0 (no char
        // matches); the case-insensitive pass = the lowercase score.
        assert!(jaro_ci("LIS", "list-scenes") > 0.7);
        assert_eq!(jaro_ci("HELPSS", "help"), jaro_ci("helpss", "help"));
        assert!(jaro_ci("HELPSS", "help") > 0.7);
    }

    #[test]
    fn jaro_ci_zero_for_unrelated_and_short() {
        assert_eq!(jaro_ci("zzzzqqqq", "color"), 0.0);
        assert_eq!(jaro_ci("x", "verbose"), 0.0);
        assert_eq!(jaro_ci("", "help"), 0.0);
        assert_eq!(jaro_ci("x", "x"), 1.0);
        assert_eq!(jaro_ci("x", "y"), 0.0);
    }

    #[test]
    fn closest_long_flag_ci_suggests_list_scenes_for_lis() {
        // Candidates in REAL declaration order (list-scenes declared
        // after list-colors in Args): the 0.7576 tie between the two
        // must break to list-scenes, matching what clap itself
        // suggests for the lowercase twin `--lis`.
        let candidates = [
            "help",
            "verbose",
            "scene",
            "list-charsets",
            "list-colors",
            "list-scenes",
        ];
        assert_eq!(
            closest_long_flag_ci("LIS", &candidates),
            Some("list-scenes".to_string())
        );
        // Lowercase input scores identically — same suggestion clap
        // itself would make for --lis.
        assert_eq!(
            closest_long_flag_ci("lis", &candidates),
            Some("list-scenes".to_string())
        );
    }

    #[test]
    fn closest_long_flag_ci_tie_breaks_to_last_candidate() {
        // Two exact ties ("lis" vs both 11-char flags = 0.7576): the
        // LAST candidate in declaration order wins, mirroring clap's
        // ascending-sort-then-pop engine.
        let candidates = ["list-colors", "list-scenes"];
        assert_eq!(
            closest_long_flag_ci("lis", &candidates),
            Some("list-scenes".to_string())
        );
        let reversed = ["list-scenes", "list-colors"];
        assert_eq!(
            closest_long_flag_ci("lis", &reversed),
            Some("list-colors".to_string())
        );
    }

    #[test]
    fn closest_long_flag_ci_stays_silent_for_distant_input() {
        let candidates = ["help", "verbose", "scene", "list-scenes", "no-effects"];
        assert_eq!(closest_long_flag_ci("zzzzqqqq", &candidates), None);
        assert_eq!(closest_long_flag_ci("x", &candidates), None);
        assert_eq!(closest_long_flag_ci("", &candidates), None);
        assert_eq!(closest_long_flag_ci("  ", &candidates), None);
    }

    #[test]
    fn closest_long_flag_ci_prefers_nearest_candidate() {
        // "scnee" scores 0.933 against "scene" and 0.783 against
        // "scena" — the nearer candidate must win (highest confidence,
        // not declaration order).
        let candidates = ["scena", "scene"];
        let got = closest_long_flag_ci("scnee", &candidates);
        assert_eq!(got, Some("scene".to_string()));
    }
}
