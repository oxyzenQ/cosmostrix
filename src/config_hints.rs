// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! "Did you mean …" hints for unknown config keys.
//!
//! The TOML parser in [`crate::configfile`] classifies any key not matching
//! a known pattern as `unknown_keys`. Previously the only follow-up was a
//! generic `(run 'cosmostrix --testconf' for known keys)` line, which
//! doesn't help when the user has nested a key under the wrong section
//! header (a structural TOML mistake, not a typo).
//!
//! This module pattern-matches two real world user-error cases observed
//! during the v25.6 depth test and returns a targeted hint explaining
//! WHERE the key should live:
//!
//! 1. **`color.tune.bold`** — `bold` is a top-level key (values 0/1/2),
//!    not a `[color.tune]` field. The `[color.tune]` section only accepts
//!    `brightness | saturation | head | body | tail`.
//!
//! 2. **`scene-custom.<name>.adaptive-custom.<HH-MM>.<…>`** —
//!    `adaptive-custom.HH-MM` is a TOP-LEVEL key (e.g.
//!    `adaptive-custom.10-00 = cosmos, monolith, speed=15`). The user
//!    wrote `[scene-custom.hacker-mode.adaptive-custom.10-00]` which the
//!    parser dutifully treats as a 5-segment dotted key — none of which
//!    match any known pattern.
//!
//! Hints are opt-in: callers (live reload, `--testconf`, startup
//! validation) only append a `hint:` line when [`suggest_for_unknown_key`]
//! returns `Some`. Keys with no recognized pattern get the original
//! generic "run --testconf" message unchanged.

use crate::configfile::USER_CONFIG_KEYS;

/// Returns a targeted "did you mean" hint for known-bad key patterns, or
/// `None` for keys with no recognized structural mistake.
///
/// The hint is a single-line string (no leading newline) suitable for
/// appending after the existing error message. Callers are responsible
/// for any indent prefix (e.g. `"  hint: "`).
#[must_use]
pub fn suggest_for_unknown_key(key: &str) -> Option<String> {
    // Pattern 1: a top-level key accidentally nested under [color.tune].
    // Triggered by `color.tune.<suffix>` where `<suffix>` is a recognized
    // top-level USER_CONFIG_KEYS entry (e.g. `color.tune.bold`).
    if let Some(suffix) = key.strip_prefix("color.tune.") {
        if !suffix.is_empty() && is_top_level_user_key(suffix) {
            let mut hint = format!(
                "'{key}': '{suffix}' is a top-level key, not a [color.tune] field. \
                 Move it out of [color.tune] — write it at the file root as: {suffix} = <value>"
            );
            // `bold` is doubly wrong: wrong location AND wrong value type
            // (it's a 0/1/2 enum, not a boolean). Call this out so the user
            // doesn't fix the location and re-trigger an error with `bold = true`.
            if suffix == "bold" {
                hint.push_str(" (values: 0=off, 1=random default, 2=all — not booleans)");
            }
            return Some(hint);
        }
    }

    // Pattern 2: adaptive-custom nested under [scene-custom.<name>].
    // Triggered by any key whose dotted segments contain "adaptive-custom"
    // after a "scene-custom." prefix. The parser produces these when the
    // user writes `[scene-custom.hacker-mode.adaptive-custom.10-00]` and
    // then `color = cosmos` underneath — yielding the full dotted key
    // `scene-custom.hacker-mode.adaptive-custom.10-00.color`.
    if key.starts_with("scene-custom.") {
        let segments: Vec<&str> = key.split('.').collect();
        // segments[0] == "scene-custom", segments[1] == <name>,
        // any later segment == "adaptive-custom" → mis-nested.
        if segments.len() > 2 && segments.iter().skip(2).any(|s| *s == "adaptive-custom") {
            return Some(format!(
                "'{key}': 'adaptive-custom.HH-MM' is a top-level key, not a [scene-custom.<name>] field. \
                 Move it out of [scene-custom.<name>] — write it at the file root as: \
                 adaptive-custom.10-00 = <color>, <scene>, [key=value, ...]"
            ));
        }
    }

    None
}

/// Build a multi-line hint block for a list of unknown keys.
///
/// Returns an empty `String` when no key has a recognized pattern, so
/// callers can unconditionally append the result without producing
/// trailing whitespace. Each hint line is prefixed with `"\n  hint: "`
/// for inline display under the existing error message.
///
/// Only the first 3 unknown keys are inspected, matching the truncation
/// used by the existing error formatters in `live_config.rs` and
/// `config_apply.rs` (which `take(3)` before joining).
#[must_use]
pub fn format_hints_block(keys: &[String]) -> String {
    let mut block = String::new();
    for k in keys.iter().take(3) {
        if let Some(hint) = suggest_for_unknown_key(k) {
            block.push_str("\n  hint: ");
            block.push_str(&hint);
        }
    }
    block
}

/// Returns `true` if `candidate` is a recognized top-level user-config key.
/// Used to distinguish `color.tune.bold` (real key, wrong location) from
/// `color.tune.foobar` (genuine typo, no useful hint).
fn is_top_level_user_key(candidate: &str) -> bool {
    USER_CONFIG_KEYS.contains(&candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── suggest_for_unknown_key: color.tune.bold ───────────────────────────

    #[test]
    fn color_tune_bold_returns_hint_mentioning_top_level() {
        let hint = suggest_for_unknown_key("color.tune.bold").expect("expected hint");
        assert!(
            hint.contains("top-level"),
            "hint should mention top-level: {hint}"
        );
        assert!(
            hint.contains("bold"),
            "hint should mention the key name: {hint}"
        );
        assert!(
            hint.contains("[color.tune]"),
            "hint should mention the wrong section: {hint}"
        );
    }

    #[test]
    fn color_tune_bold_hint_warns_about_value_type() {
        // `bold` is a 0/1/2 enum, not a boolean. The hint must call this
        // out so the user doesn't fix the location and then write `bold = true`.
        let hint = suggest_for_unknown_key("color.tune.bold").unwrap();
        assert!(
            hint.contains("0=off") && hint.contains("not booleans"),
            "hint should explain the value type: {hint}"
        );
    }

    #[test]
    fn color_tune_other_top_level_keys_also_get_hints() {
        // Any top-level key nested under [color.tune] gets the structural
        // hint. The user might write [color.tune] and then `speed = 10`
        // (producing color.tune.speed) — same mistake, different key.
        let hint = suggest_for_unknown_key("color.tune.speed").expect("expected hint");
        assert!(hint.contains("top-level"));
        assert!(hint.contains("speed"));
        // `speed` is not `bold`, so no value-type warning.
        assert!(!hint.contains("not booleans"));
    }

    #[test]
    fn color_tune_recognized_field_returns_none() {
        // `color.tune.brightness` is a VALID key — it should never reach
        // suggest_for_unknown_key in practice (the parser accepts it), but
        // if it does, we must NOT produce a misleading hint.
        assert_eq!(suggest_for_unknown_key("color.tune.brightness"), None);
        assert_eq!(suggest_for_unknown_key("color.tune.saturation"), None);
        assert_eq!(suggest_for_unknown_key("color.tune.head"), None);
    }

    #[test]
    fn color_tune_genuine_typo_returns_none() {
        // `color.tune.foobar` is not a top-level key AND not a recognized
        // color.tune field. No useful structural hint — caller falls back
        // to the generic "run --testconf" message.
        assert_eq!(suggest_for_unknown_key("color.tune.foobar"), None);
    }

    // ── suggest_for_unknown_key: scene-custom.*.adaptive-custom.* ─────────

    #[test]
    fn scene_custom_with_nested_adaptive_custom_returns_hint() {
        // The exact key produced by the parser when the user writes
        // [scene-custom.hacker-mode.adaptive-custom.10-00] then color = cosmos.
        let key = "scene-custom.hacker-mode.adaptive-custom.10-00.color";
        let hint = suggest_for_unknown_key(key).expect("expected hint");
        assert!(
            hint.contains("top-level"),
            "hint should mention top-level: {hint}"
        );
        assert!(
            hint.contains("adaptive-custom.HH-MM"),
            "hint should show the correct format: {hint}"
        );
        assert!(
            hint.contains("[scene-custom"),
            "hint should reference the wrong section: {hint}"
        );
    }

    #[test]
    fn scene_custom_with_nested_adaptive_custom_no_trailing_field() {
        // Edge case: section header itself (no field after) — the parser
        // doesn't add these to unknown_keys (section headers aren't
        // validated), but if it ever does, the hint should still fire.
        let key = "scene-custom.hacker-mode.adaptive-custom.10-00";
        let hint = suggest_for_unknown_key(key).expect("expected hint");
        assert!(hint.contains("top-level"));
    }

    #[test]
    fn scene_custom_normal_field_returns_none() {
        // `scene-custom.hacker-mode.color` is a VALID key — no hint.
        assert_eq!(
            suggest_for_unknown_key("scene-custom.hacker-mode.color"),
            None
        );
        assert_eq!(
            suggest_for_unknown_key("scene-custom.hacker-mode.speed"),
            None
        );
    }

    #[test]
    fn adaptive_custom_at_top_level_returns_none() {
        // `adaptive-custom.10-00` at file root is a VALID key — no hint.
        assert_eq!(suggest_for_unknown_key("adaptive-custom.10-00"), None);
    }

    // ── suggest_for_unknown_key: generic typos ────────────────────────────

    #[test]
    fn generic_typo_returns_none() {
        // Genuine typos have no structural hint to give.
        assert_eq!(suggest_for_unknown_key("colro"), None);
        assert_eq!(suggest_for_unknown_key("colour"), None);
        assert_eq!(
            suggest_for_unknown_key("scene-custom.hacker-mode.totally-fake-field"),
            None
        );
    }

    // ── format_hints_block ────────────────────────────────────────────────

    #[test]
    fn format_hints_block_empty_for_no_hints() {
        let keys = vec!["colro".to_string(), "colour".to_string()];
        assert_eq!(format_hints_block(&keys), "");
    }

    #[test]
    fn format_hints_block_includes_hint_lines_for_known_patterns() {
        let keys = vec!["color.tune.bold".to_string()];
        let block = format_hints_block(&keys);
        assert!(
            block.starts_with("\n  hint: "),
            "block should start with newline + indent: {block:?}"
        );
        assert!(block.contains("top-level"));
    }

    #[test]
    fn format_hints_block_only_includes_first_three_keys() {
        // Match the take(3) truncation used by the existing error formatters.
        let keys = vec![
            "color.tune.bold".to_string(),
            "color.tune.speed".to_string(),
            "scene-custom.hacker-mode.adaptive-custom.10-00.color".to_string(),
            "color.tune.fps".to_string(), // would produce a 4th hint — must be skipped
        ];
        let block = format_hints_block(&keys);
        // Count hint lines — should be exactly 3.
        let hint_count = block.matches("\n  hint: ").count();
        assert_eq!(hint_count, 3, "should only inspect first 3 keys: {block}");
    }

    #[test]
    fn format_hints_block_mixed_keys_only_emits_for_known_patterns() {
        let keys = vec![
            "colro".to_string(),           // no hint
            "color.tune.bold".to_string(), // hint
            "colour".to_string(),          // no hint
        ];
        let block = format_hints_block(&keys);
        let hint_count = block.matches("\n  hint: ").count();
        assert_eq!(
            hint_count, 1,
            "only the recognized pattern should produce a hint"
        );
        assert!(block.contains("color.tune.bold"));
    }

    // ── Integration: parse_config_text produces the expected unknown keys ─

    #[test]
    fn parse_color_tune_section_with_bold_produces_unknown_key() {
        // Verify the parser actually feeds the right key into unknown_keys
        // when the user writes [color.tune] then bold = true. This is the
        // exact pattern from the v25.6 depth test.
        let parsed = crate::configfile::parse_config_text("[color.tune]\nbold = true\n");
        assert!(
            parsed.unknown_keys.contains(&"color.tune.bold".to_string()),
            "expected color.tune.bold in unknown_keys, got: {:?}",
            parsed.unknown_keys
        );
        // Hint should fire on the produced key.
        assert!(suggest_for_unknown_key("color.tune.bold").is_some());
    }

    #[test]
    fn parse_scene_custom_section_with_nested_adaptive_custom_produces_unknown_key() {
        // Verify the parser feeds the right key when the user writes
        // [scene-custom.hacker-mode.adaptive-custom.10-00] then color = cosmos.
        let content = "[scene-custom.hacker-mode.adaptive-custom.10-00]\ncolor = cosmos\n";
        let parsed = crate::configfile::parse_config_text(content);
        assert!(
            !parsed.unknown_keys.is_empty(),
            "expected unknown keys for nested adaptive-custom, got: {:?}",
            parsed.unknown_keys
        );
        let full_key = "scene-custom.hacker-mode.adaptive-custom.10-00.color";
        assert!(
            parsed.unknown_keys.contains(&full_key.to_string()),
            "expected {full_key} in unknown_keys, got: {:?}",
            parsed.unknown_keys
        );
        assert!(suggest_for_unknown_key(full_key).is_some());
    }
}
