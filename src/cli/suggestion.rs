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
/// 1. The "tip:" line and the "Did you mean?" line ALWAYS agree on which
///    flag to suggest (no more `--clr` -> tip says `color-bg` but
///    Did-you-mean says `color` disagreement).
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
