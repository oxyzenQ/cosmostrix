// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI suggestion extraction — extracted from `main.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns the `extract_clap_suggestion()` helper: a pure string-parsing
//! function that reads clap's OWN "tip:" line from its error output
//! and returns the first suggested flag name (without the `--` prefix).
//!
//! Re-exported from `main.rs` via `pub(crate) use` so all existing
//! call sites continue to resolve unchanged.

/// Extract clap's OWN suggested flag from its error string.
///
/// When clap's `suggestions` feature is enabled and the user types an
/// unknown flag that is close to a known one, clap appends a line:
///
///   ```text
///   tip: a similar argument exists: '--no-effects'
///   ```
///
/// (or the plural form `tip: some similar arguments exist: '--a', '--b'`).
///
/// This function parses that line and returns the FIRST suggested flag
/// name (without the `--` prefix). By reusing clap's own suggestion
/// instead of maintaining a separate `KNOWN_LONG_FLAGS` list + Levenshtein
/// engine, we guarantee:
///
/// 1. The "tip:" line from clap and our `format_argument_suggestion`
///    line ALWAYS agree on which flag to suggest (no more `--clr` ->
///    tip says `color-bg` but our line says `color` disagreement).
/// 2. No hand-maintained flag list to drift when flags are renamed
///    (the v50.0.0-beta.7 `--disable-effects` -> `--no-effects` rename
///    missed `KNOWN_LONG_FLAGS`, which was the root cause of this bug).
/// 3. Zero duplicate-engine maintenance overhead.
///
/// Returns `None` when clap did not find a close match (no "tip:" line
/// in the error string).
pub(crate) fn extract_clap_suggestion(err_str: &str) -> Option<String> {
    // Clap renders the tip line in two forms:
    //   singular: "tip: a similar argument exists: '--FLAG'"
    //   plural:   "tip: some similar arguments exist: '--FLAG1', '--FLAG2'"
    // Both contain the pattern: '-- followed by the flag name and a closing '
    // We find the FIRST occurrence after "tip:" to extract the primary suggestion.
    let tip_marker = "tip:";
    let tip_pos = err_str.find(tip_marker)?;
    let after_tip = &err_str[tip_pos..];
    // Find the first '--FLAG' pattern (clap always wraps flag names in single
    // quotes with the -- prefix inside the quote).
    let quote_flag_marker = "'--";
    let flag_start = after_tip.find(quote_flag_marker)? + quote_flag_marker.len();
    let rest = &after_tip[flag_start..];
    let flag_end = rest.find('\'')?;
    Some(rest[..flag_end].to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Value suggestion (v51 Z-master-1B did-you-mean audit)
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

/// Format a VALUE suggestion as a consistent "tip:" line.
///
/// Returns `\n  tip: a similar value exists: '<value>'`. This is the
/// canonical format for all enum/value typo suggestions (colors,
/// scenes, charsets, glitch-level, msg-fill-style, etc.) —
/// replacing the legacy `Did you mean '<value>'?` format that was
/// scattered across 14+ files with no consistency.
///
/// For FLAG suggestions (unknown `--foo` flags), the format is
/// `tip: a similar argument exists: '--flag'` — but those are
/// rendered inline via `eprintln!` with ANSI color wrappers in
/// `main.rs` and `argv_expand.rs`, so no helper is needed there.
pub(crate) fn format_value_suggestion(suggestion: &str) -> String {
    format!("\n  tip: a similar value exists: '{suggestion}'")
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
