// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! charset_custom tests, extracted from the inline `mod tests { ... }`
//! block (night-dragon-fly-2) — the file crossed the src/RULES_LOC.md
//! hard 800-line cap when the original-case display swap landed, and
//! the safepath precedent (`#[path = "../../test/safepath/tests.rs"]`)
//! is the sanctioned extraction pattern: same `use super::*` access to
//! the parent's private items, zero code motion beyond the move.

use super::*;

#[test]
fn parse_empty_string_rejects() {
    assert!(parse_charset_value("").is_err());
    assert!(parse_charset_value("   ").is_err());
    assert!(parse_charset_value("\"\"").is_err());
}

#[test]
fn parse_plain_chars_succeeds() {
    let v = parse_charset_value("abc").unwrap();
    assert_eq!(v, vec!['a', 'b', 'c']);
}

#[test]
fn parse_quoted_string_succeeds() {
    let v = parse_charset_value("\"|\"").unwrap();
    assert_eq!(v, vec!['|']);
}

#[test]
fn parse_skips_internal_whitespace_except_space() {
    // Tab and newline are dropped; space is kept (it is a valid
    // single-width character and may be intentional, e.g. for
    // visual gap effects in the rain).
    let v = parse_charset_value("a\tb\nc d").unwrap();
    assert_eq!(v, vec!['a', 'b', 'c', ' ', 'd']);
}

#[test]
fn parse_rejects_control_char() {
    // U+0007 (BEL) is a control character — must hard-fail.
    let s = "ab\u{0007}cd";
    let err = parse_charset_value(s).unwrap_err();
    assert!(err.contains("control character"), "got: {err}");
    assert!(err.contains("U+0007"), "got: {err}");
}

#[test]
fn parse_skips_wide_chars_with_warning() {
    // CJK fullwidth '猫' (U+732B) is wide — should be skipped, not error.
    let v = parse_charset_value("a猫b").unwrap();
    assert_eq!(v, vec!['a', 'b']);
}

#[test]
fn parse_rejects_too_long() {
    // 257 'x' chars — over the 256 cap.
    let s = "x".repeat(CHARSET_CUSTOM_MAX_LEN + 1);
    let err = parse_charset_value(&s).unwrap_err();
    assert!(err.contains("maximum"), "got: {err}");
    assert!(
        err.contains(&CHARSET_CUSTOM_MAX_LEN.to_string()),
        "got: {err}"
    );
}

#[test]
fn parse_accepts_exactly_max_len() {
    let s = "x".repeat(CHARSET_CUSTOM_MAX_LEN);
    let v = parse_charset_value(&s).unwrap();
    assert_eq!(v.len(), CHARSET_CUSTOM_MAX_LEN);
}

#[test]
fn collect_finds_single_block() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.zen.set".to_string(), "|".to_string());
    let map = collect_charset_custom(&cfg);
    assert!(map.contains_key("zen"));
    assert_eq!(map["zen"].chars, vec!['|']);
}

#[test]
fn collect_is_case_insensitive_on_name() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.MySet.set".to_string(), "ab".to_string());
    let map = collect_charset_custom(&cfg);
    assert!(map.contains_key("myset"));
}

#[test]
fn collect_preserves_display_name_case() {
    // NIGHT-cafe-1: user-facing surfaces must show the block name
    // exactly as the config spells it, while the map key stays
    // lowercase for case-insensitive lookup.
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.T1.set".to_string(), "ab".to_string());
    let map = collect_charset_custom(&cfg);
    assert!(map.contains_key("t1"));
    assert_eq!(map["t1"].display_name, "T1");
}

#[test]
fn load_custom_charset_error_lists_original_case() {
    // NIGHT-cafe-1: the "expected one of" list quotes the config's
    // own spelling, not the lowercased key.
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.T1.set".to_string(), "ab".to_string());
    let err = load_custom_charset(&cfg, "missing").unwrap_err();
    assert!(err.contains("T1"), "got: {err}");
}

#[test]
fn collect_ignores_unknown_fields() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.zen.set".to_string(), "ab".to_string());
    // Unknown field — should be skipped (and is_known_key in
    // configfile.rs would have already rejected it as unknown).
    cfg.insert(
        "charset-custom.zen.unknownfield".to_string(),
        "ignored".to_string(),
    );
    let map = collect_charset_custom(&cfg);
    assert_eq!(map["zen"].chars, vec!['a', 'b']);
}

#[test]
fn collect_handles_multiple_blocks() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.a.set".to_string(), "12".to_string());
    cfg.insert("charset-custom.b.set".to_string(), "xy".to_string());
    let map = collect_charset_custom(&cfg);
    assert_eq!(map.len(), 2);
    assert_eq!(map["a"].chars, vec!['1', '2']);
    assert_eq!(map["b"].chars, vec!['x', 'y']);
}

#[test]
fn load_custom_charset_found() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.zen.set".to_string(), "|".to_string());
    let v = load_custom_charset(&cfg, "zen").unwrap();
    assert_eq!(v, vec!['|']);
}

#[test]
fn load_custom_charset_case_insensitive() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.zen.set".to_string(), "|".to_string());
    let v = load_custom_charset(&cfg, "zen").unwrap();
    assert_eq!(v, vec!['|']);
}

#[test]
fn load_custom_charset_not_found_lists_available() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.alpha.set".to_string(), "ab".to_string());
    cfg.insert("charset-custom.beta.set".to_string(), "cd".to_string());
    let err = load_custom_charset(&cfg, "gamma").unwrap_err();
    assert!(err.contains("not found"), "got: {err}");
    assert!(err.contains("alpha"), "got: {err}");
    assert!(err.contains("beta"), "got: {err}");
}

#[test]
fn load_custom_charset_if_matches_returns_some_when_present() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.zen.set".to_string(), "|".to_string());
    let v = load_custom_charset_if_matches(&cfg, "zen");
    assert!(v.is_some());
    assert_eq!(v.unwrap(), vec!['|']);
}

#[test]
fn load_custom_charset_if_matches_returns_none_when_absent() {
    let cfg = HashMap::new();
    assert!(load_custom_charset_if_matches(&cfg, "zen").is_none());
}

#[test]
fn load_custom_charset_if_matches_returns_none_for_empty_name() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom..set".to_string(), "|".to_string());
    assert!(load_custom_charset_if_matches(&cfg, "").is_none());
}

#[test]
fn validate_charset_custom_value_ok() {
    assert!(validate_charset_custom_value("abc").is_none());
    assert!(validate_charset_custom_value("\"|\"").is_none());
}

#[test]
fn validate_charset_custom_value_rejects_empty() {
    assert!(validate_charset_custom_value("").is_some());
    assert!(validate_charset_custom_value("\"\"").is_some());
}

#[test]
fn validate_charset_custom_value_rejects_too_long() {
    let s = "x".repeat(CHARSET_CUSTOM_MAX_LEN + 1);
    assert!(validate_charset_custom_value(&s).is_some());
}

#[test]
fn validate_charset_custom_value_rejects_control_char() {
    assert!(validate_charset_custom_value("a\u{0007}b").is_some());
}

// ── v50.0.0-beta.6 LTS: bounds enforcement tests ─────────────────

#[test]
fn collect_caps_total_blocks_at_max() {
    // A config with >CHARSET_CUSTOM_MAX_BLOCKS blocks keeps at most
    // MAX_BLOCKS entries (which ones survive is unspecified — HashMap
    // iteration order), never allocating unbounded.
    let mut cfg = HashMap::new();
    for i in 0..(CHARSET_CUSTOM_MAX_BLOCKS + 10) {
        cfg.insert(format!("charset-custom.charset{i}.set"), "ab".to_string());
    }
    let map = collect_charset_custom(&cfg);
    assert!(
        map.len() <= CHARSET_CUSTOM_MAX_BLOCKS,
        "total blocks must be capped at {}, got {}",
        CHARSET_CUSTOM_MAX_BLOCKS,
        map.len()
    );
}

#[test]
fn collect_skips_oversized_names() {
    // A name longer than CHARSET_CUSTOM_MAX_NAME_LEN should be
    // silently skipped (no allocation, no BTreeMap entry).
    // NIGHT-depthtest-3: the COLLECTOR still skips (this contract);
    // the config gate now hard-errors via
    // validate_charset_custom_name_len (see its tests).
    let mut cfg = HashMap::new();
    let long_name = "x".repeat(CHARSET_CUSTOM_MAX_NAME_LEN + 1);
    cfg.insert(format!("charset-custom.{long_name}.set"), "ab".to_string());
    let map = collect_charset_custom(&cfg);
    assert!(
        map.is_empty(),
        "oversized name must be skipped, got {} entries",
        map.len()
    );
}

// ── (bug #19) single-glyph depth stress: validation layer ──────
// Owner mandate 2026-08-30: stress-test ANY single glyph so the
// quoted-bracket parser bug class never returns. The parser layer
// battery lives in configfile_tests/bug19.rs; these tests lock the
// charset-custom validation layer on top of it.

#[test]
fn parse_single_special_char_glyphs_are_valid_pools() {
    // Every config-syntax metachar and common punctuation is a legal
    // single-width glyph once the parser hands the value over
    // verbatim (bug #19). A pool of exactly one such char must load.
    for ch in [
        '[', ']', '{', '}', '(', ')', '<', '>', '#', '=', ',', '\'', '\\', '|', '/', '?', '!', '@',
        '$', '%', '^', '&', '*', '+', '-', '_', '~', '`', ':', ';', '.', '∇', '∀', '∂',
    ] {
        let v = parse_charset_value(&ch.to_string())
            .unwrap_or_else(|e| panic!("glyph {ch:?} must be a valid single-glyph pool: {e}"));
        assert_eq!(v, vec![ch], "glyph {ch:?} must survive verbatim");
    }
}

#[test]
fn parse_lone_double_quote_is_not_expressible() {
    // Documented syntax corner (no escape sequences): a lone `"` is
    // the string delimiter at the config layer and is stripped away,
    // leaving an empty pool. Locked here so any future change is a
    // conscious decision (e.g. adding escape support), not an
    // accident.
    assert!(parse_charset_value("\"").is_err());
}

#[test]
fn parse_mid_pool_double_quote_is_kept() {
    // Quotes in the MIDDLE of a pool are ordinary glyphs.
    let v = parse_charset_value("a\"b").unwrap();
    assert_eq!(v, vec!['a', '"', 'b']);
}

#[test]
fn parse_edge_spaces_trimmed_inner_space_kept() {
    // Leading/trailing spaces are trimmed (accidental padding);
    // inner spaces are real glyphs (documented: space is a valid
    // single-width character). A whitespace-only pool collapses to
    // empty and is rejected.
    assert_eq!(parse_charset_value(" a b ").unwrap(), vec!['a', ' ', 'b']);
    assert!(parse_charset_value("   ").is_err());
}

#[test]
fn single_wide_or_zero_width_glyph_gives_clear_error() {
    // A pool whose ONLY glyph is wide (CJK) or zero-width (ZWSP) has
    // nothing usable left after the skip filter — the error must say
    // so. Control chars stay hard errors.
    assert!(
        parse_charset_value("漢")
            .unwrap_err()
            .contains("no usable single-width characters")
    );
    assert!(
        parse_charset_value("\u{200B}")
            .unwrap_err()
            .contains("no usable single-width characters")
    );
    assert!(
        parse_charset_value("\u{0007}")
            .unwrap_err()
            .contains("control character")
    );
}
// ── NIGHT-depthtest-3: oversized-name hard validation ──────────

#[test]
fn name_len_validation_rejects_oversized_name() {
    // A valid set under a 65+-char name used to pass --testconf
    // silently (the collector dropped the block before any
    // validation saw it). The raw-key scan must flag it.
    let mut cfg = HashMap::new();
    let long_name = "x".repeat(CHARSET_CUSTOM_MAX_NAME_LEN + 1);
    cfg.insert(format!("charset-custom.{long_name}.set"), "01".to_string());
    let err = validate_charset_custom_name_len(&cfg).expect("must reject");
    assert!(
        err.contains("exceeds the 64-char name limit"),
        "must name the limit: {err}"
    );
}

#[test]
fn name_len_validation_accepts_boundary_64_char_name() {
    let mut cfg = HashMap::new();
    let name = "a".repeat(CHARSET_CUSTOM_MAX_NAME_LEN);
    cfg.insert(format!("charset-custom.{name}.set"), "01".to_string());
    assert!(
        validate_charset_custom_name_len(&cfg).is_none(),
        "a 64-char name with a valid set must pass"
    );
}

#[test]
fn name_len_validation_ignores_unknown_fields() {
    // Only `charset-custom.<name>.set` keys are scanned; other
    // shapes belong to the unknown-key path.
    let mut cfg = HashMap::new();
    let long_name = "x".repeat(CHARSET_CUSTOM_MAX_NAME_LEN + 1);
    cfg.insert(
        format!("charset-custom.{long_name}.bogus"),
        "01".to_string(),
    );
    assert!(
        validate_charset_custom_name_len(&cfg).is_none(),
        "unknown fields are not the length gate's job"
    );
}
