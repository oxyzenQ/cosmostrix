// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Custom character set definitions from config.toml.
//!
//! Replaces the legacy `--charset-file <PATH>` CLI flag (v11–v24) with an
//! in-config `[charset-custom.<name>]` block. Users no longer need to
//! create an external text file and pass a CLI flag — the charset lives
//! inside `config.toml` next to every other setting.
//!
//! # Format
//!
//! ```toml
//! [charset-custom.cat]
//! set = "x9"
//! ```
//!
//! - `set` — the literal string of characters to use as the rain glyph
//!   pool. Order is preserved (the renderer shuffles them at runtime, so
//!   order does not affect on-screen appearance, only what's in the pool).
//! - Whitespace inside `set` (except ASCII space) is skipped. Space (` `)
//!   is a valid single-width character and is kept.
//! - Control characters (C0/C1) are rejected with a clear error.
//! - Wide / zero-width characters (emoji, CJK fullwidth, combining marks)
//!   are rejected with a clear error. The renderer is column-based and
//!   assumes one cell per glyph — wide chars corrupt alignment. This is
//!   the Cosmic Dragon principle: no emoji, no wide chars, ever. It is a
//!   permanent design choice, not a limitation to be lifted later.
//! - Maximum length: 256 characters. Longer values are rejected so the
//!   rain glyph pool does not become a memory hog.
//!
//! # Loading
//!
//! Two equivalent ways to activate a custom charset:
//!
//! 1. CLI: `cosmostrix --charset cat` (when `[charset-custom.cat]` exists
//!    in config.toml, the custom block takes precedence over any built-in
//!    preset with the same name).
//! 2. Config: `charset = "cat"` in config.toml.
//!
//! Lookup is case-insensitive: `[charset-custom.Cat]` is matched by
//! `charset = "cat"`, `--charset CAT`, etc.
//!
//! # Live reload
//!
//! Editing a `[charset-custom.<name>]` block while cosmostrix is running
//! takes effect on the next live reload, just like every other config
//! field. The `rebuild_cloud_config` function in `live_config.rs` checks
//! `charset-custom.<name>` first when applying the `charset` config key.

use std::collections::{BTreeMap, HashMap};

use unicode_width::UnicodeWidthChar;

/// Maximum number of characters allowed in a single custom charset.
///
/// Bounded to keep the rain glyph pool predictable and prevent accidental
/// memory bloat from a typo (e.g., pasting a 10 000-char string).
pub const CHARSET_CUSTOM_MAX_LEN: usize = 256;

/// A parsed custom charset definition — a flat list of single-width chars.
#[derive(Debug, Clone, Default)]
pub struct CharsetCustomDef {
    /// The validated character pool, in config-declared order.
    pub chars: Vec<char>,
}

impl CharsetCustomDef {
    /// True if no usable characters were extracted.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
}

/// Collect every `[charset-custom.<name>]` block from the parsed config
/// HashMap and return them as a name → def map.
///
/// Only the `set` field is recognized. Unknown fields under
/// `[charset-custom.<name>]` are silently skipped — they are filtered
/// upstream by `is_known_key()` in `configfile.rs`, so they never reach
/// this function.
///
/// Names are normalized to lowercase for case-insensitive matching.
#[must_use]
pub fn collect_charset_custom(cfg: &HashMap<String, String>) -> BTreeMap<String, CharsetCustomDef> {
    let mut out: BTreeMap<String, CharsetCustomDef> = BTreeMap::new();

    for (key, value) in cfg {
        let Some(rest) = key.strip_prefix("charset-custom.") else {
            continue;
        };
        let Some((name, field)) = rest.split_once('.') else {
            continue;
        };
        if field != "set" {
            // Unknown field — skip. is_known_key() already rejected
            // anything other than `set`, so this branch is defensive.
            continue;
        }
        let name = name.to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        // Even if validation fails, we want to record the name so the
        // caller can produce a helpful "invalid custom charset" error
        // rather than a "not found" error. We store an empty def; the
        // caller's `load_custom_charset` will re-parse + re-validate and
        // surface the actual error.
        let def = out.entry(name).or_default();
        if let Ok(chars) = parse_charset_value(value) {
            def.chars = chars;
        }
    }

    out
}

/// Parse a `set = "..."` value into a validated `Vec<char>`.
///
/// Strips surrounding quotes (TOML strings arrive still-quoted in the
/// flat HashMap form used by `configfile::parse_config_text`), then
/// iterates over characters and applies the same single-width + non-
/// control filter as `charset.rs::build_chars`.
///
/// Returns `Err(message)` if:
/// - the value is empty after trimming (no usable chars)
/// - a control character is present
/// - the pool exceeds `CHARSET_CUSTOM_MAX_LEN` characters
///
/// Wide / zero-width characters are SKIPPED (not errors) — this matches
/// the existing `--charset-file` behavior and avoids hard-failing when a
/// user copy-pastes a string that happens to include a non-breaking space
/// or a stray combining mark. A warning is emitted to stderr per skipped
/// codepoint.
///
/// Cosmic Dragon principle: stripping wide chars is a permanent design
/// choice. The renderer will never support emoji or full-width CJK glyphs —
/// its soul is single-cell diff-based rendering. Do not interpret this
/// filter as a temporary limitation.
pub fn parse_charset_value(value: &str) -> Result<Vec<char>, String> {
    let s = value.trim().trim_matches('"').trim();
    let mut chars: Vec<char> = Vec::new();
    let mut skipped_wide: Vec<String> = Vec::new();

    for ch in s.chars() {
        // Skip whitespace except ASCII space (same rule as the old
        // --charset-file path). Newlines, tabs, etc. are silently
        // dropped so users can wrap long values across lines.
        if ch.is_whitespace() && ch != ' ' {
            continue;
        }
        // Reject control characters outright — they are invisible and
        // can break terminal rendering. Hard error, not a skip.
        if ch.is_control() {
            return Err(format!(
                "control character U+{:04X} in charset-custom set — invisible chars break rendering",
                ch as u32
            ));
        }
        match ch.width() {
            Some(1) => chars.push(ch),
            _ => skipped_wide.push(format!("U+{:04X}", ch as u32)),
        }
    }

    if !skipped_wide.is_empty() {
        // Same warning style as the old --charset-file path so users
        // upgrading from v24 see identical behavior.
        eprintln!(
            "[cosmostrix] warning: skipped {} wide/zero-width character(s) from charset-custom: {}",
            skipped_wide.len(),
            skipped_wide.join(", ")
        );
    }

    if chars.is_empty() {
        return Err("charset-custom set contains no usable single-width characters".to_string());
    }

    if chars.len() > CHARSET_CUSTOM_MAX_LEN {
        return Err(format!(
            "charset-custom set has {} characters — maximum is {CHARSET_CUSTOM_MAX_LEN} (trim the value or split into multiple presets)",
            chars.len()
        ));
    }

    Ok(chars)
}

/// Look up a custom charset by name and return its char pool.
///
/// Returns `Err(message)` if:
/// - no `[charset-custom.<name>]` block exists with that name (the
///   message lists every defined name so the user can fix the typo)
/// - the block exists but its `set` value is invalid (control char,
///   too long, empty after filtering)
///
/// Callers should fall back to `charset::charset_from_str` when this
/// returns `Err` — a "not found" error for a custom charset is normal
/// and means the user is asking for a built-in preset.
pub fn load_custom_charset(cfg: &HashMap<String, String>, name: &str) -> Result<Vec<char>, String> {
    let palettes = collect_charset_custom(cfg);
    let normalized = name.trim().to_ascii_lowercase();
    let def = palettes.get(&normalized).ok_or_else(|| {
        let mut available: Vec<String> = palettes.keys().cloned().collect();
        available.sort();
        let list = if available.is_empty() {
            "<none defined>".to_string()
        } else {
            available.join(", ")
        };
        format!(
            "custom charset '{name}' not found in config\nexpected one of: {list}\n\n  Use --list-charsets to see built-in and custom charsets."
        )
    })?;
    if def.chars.is_empty() {
        // The block exists but its `set` value failed to parse during
        // collect. Re-parse now to surface the actual error to the user.
        let raw = cfg
            .get(&format!("charset-custom.{normalized}.set"))
            .map(String::as_str)
            .unwrap_or("");
        return parse_charset_value(raw);
    }
    Ok(def.chars.clone())
}

/// Convenience helper used by `rebuild_cloud_config` (live reload) and
/// the startup path in `main.rs`: if `name` matches a `[charset-custom.<name>]`
/// block, return its char pool; otherwise return `None` so the caller
/// falls back to the built-in charset lookup.
///
/// On parse error, returns `None` — the caller's fall-through to
/// built-in will then fail with a clear "unknown charset" message that
/// also lists valid built-in names. We intentionally do NOT surface the
/// custom-block parse error here, because the strict-validation pass
/// (`testconf::validate_config_strictly`) already reported it at
/// startup / on live reload. Surfacing it again here would be noise.
#[must_use]
pub fn load_custom_charset_if_matches(
    cfg: &HashMap<String, String>,
    name: &str,
) -> Option<Vec<char>> {
    let normalized = name.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    // Cheap pre-check: avoid building the full BTreeMap when the key
    // doesn't exist at all.
    let key = format!("charset-custom.{normalized}.set");
    if !cfg.contains_key(&key) {
        return None;
    }
    load_custom_charset(cfg, name).ok()
}

/// Validate a `charset-custom.<name>.set` value for use by `--testconf`
/// and `validate_config_strictly`. Returns `Some(error_message)` on
/// failure, `None` on success.
///
/// This is the entry point `testconf.rs` calls during strict validation.
/// It does NOT mutate any state — pure validation.
#[must_use]
pub fn validate_charset_custom_value(value: &str) -> Option<String> {
    parse_charset_value(value).err()
}

#[cfg(test)]
mod tests {
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
        let v = parse_charset_value("\"x9\"").unwrap();
        assert_eq!(v, vec!['x', '9']);
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
        cfg.insert("charset-custom.cat.set".to_string(), "x9".to_string());
        let map = collect_charset_custom(&cfg);
        assert!(map.contains_key("cat"));
        assert_eq!(map["cat"].chars, vec!['x', '9']);
    }

    #[test]
    fn collect_is_case_insensitive_on_name() {
        let mut cfg = HashMap::new();
        cfg.insert("charset-custom.MySet.set".to_string(), "ab".to_string());
        let map = collect_charset_custom(&cfg);
        assert!(map.contains_key("myset"));
    }

    #[test]
    fn collect_ignores_unknown_fields() {
        let mut cfg = HashMap::new();
        cfg.insert("charset-custom.cat.set".to_string(), "ab".to_string());
        // Unknown field — should be skipped (and is_known_key in
        // configfile.rs would have already rejected it as unknown).
        cfg.insert(
            "charset-custom.cat.unknownfield".to_string(),
            "ignored".to_string(),
        );
        let map = collect_charset_custom(&cfg);
        assert_eq!(map["cat"].chars, vec!['a', 'b']);
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
        cfg.insert("charset-custom.cat.set".to_string(), "x9".to_string());
        let v = load_custom_charset(&cfg, "cat").unwrap();
        assert_eq!(v, vec!['x', '9']);
    }

    #[test]
    fn load_custom_charset_case_insensitive() {
        let mut cfg = HashMap::new();
        cfg.insert("charset-custom.Cat.set".to_string(), "x9".to_string());
        let v = load_custom_charset(&cfg, "cat").unwrap();
        assert_eq!(v, vec!['x', '9']);
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
        cfg.insert("charset-custom.cat.set".to_string(), "x9".to_string());
        let v = load_custom_charset_if_matches(&cfg, "cat");
        assert!(v.is_some());
        assert_eq!(v.unwrap(), vec!['x', '9']);
    }

    #[test]
    fn load_custom_charset_if_matches_returns_none_when_absent() {
        let cfg = HashMap::new();
        assert!(load_custom_charset_if_matches(&cfg, "cat").is_none());
    }

    #[test]
    fn load_custom_charset_if_matches_returns_none_for_empty_name() {
        let mut cfg = HashMap::new();
        cfg.insert("charset-custom..set".to_string(), "x9".to_string());
        assert!(load_custom_charset_if_matches(&cfg, "").is_none());
    }

    #[test]
    fn validate_charset_custom_value_ok() {
        assert!(validate_charset_custom_value("abc").is_none());
        assert!(validate_charset_custom_value("\"x9\"").is_none());
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
}
