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
//! 2. **`scene-custom.<name>.adaptive-custom.<HH-MM>.<…>`** — the user
//!    wrote `[scene-custom.hacker-mode.adaptive-custom.10-00]` which the
//!    parser dutifully treats as a 5-segment dotted key. v30 (2026-08-05):
//!    the `adaptive-custom.*` key namespace was eliminated at commit
//!    `07b44b5` along with the atmosphere engine subsystem, so the hint
//!    now tells the user to REMOVE these entries (not move them to root
//!    scope, which was the original v25.7 advice).
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
pub(crate) fn suggest_for_unknown_key(key: &str) -> Option<String> {
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
    //
    // v30 (2026-08-05, atmosphere elimination): the entire `adaptive-custom.*`
    // key namespace was eliminated at commit 07b44b5 along with the atmosphere
    // engine subsystem. The hint now tells the user to REMOVE these keys,
    // not move them to root scope (which was the original v25.7 advice when
    // the keys were still live).
    if key.starts_with("scene-custom.") {
        let segments: Vec<&str> = key.split('.').collect();
        // segments[0] == "scene-custom", segments[1] == <name>,
        // any later segment == "adaptive-custom" → mis-nested.
        if segments.len() > 2 && segments.iter().skip(2).any(|s| *s == "adaptive-custom") {
            return Some(format!(
                "'{key}': 'adaptive-custom.*' keys have been removed (atmosphere engine \
                 eliminated at commit 07b44b5, 2026-08-05). Remove this entry from your \
                 config.toml. Historical design spec: \
                 docs/archive/specs/ATMOSPHERE_ENGINE.md"
            ));
        }
        // Pattern 2b: ambient nested under [scene-custom.<name>]. Same
        // mis-nesting pattern as adaptive-custom — the user wrote
        // `[scene-custom.hacker-mode.ambient.10-00]` and the parser
        // produced `scene-custom.hacker-mode.ambient.10-00.color`. Ambient
        // keys belong at the root scope (`ambient.<HH-MM>`), not nested
        // under scene-custom blocks.
        if segments.len() > 2 && segments.iter().skip(2).any(|s| *s == "ambient") {
            return Some(format!(
                "'{key}': 'ambient.*' keys are top-level — they cannot be nested under \
                 [scene-custom.<name>]. Move the entry out of the [scene-custom.{name}] section \
                 and write it at the file root as: ambient.<HH-MM> = <color>, <scene>, ...",
                name = segments.get(1).copied().unwrap_or("<name>")
            ));
        }
    }

    // Pattern 3 (v25.10 / bug #8): invalid colors-custom field. Triggered
    // by `colors-custom.<name>.<field>` where `<field>` is not one of the
    // three accepted values (`bg`, `rain`, `stops`). Previously this surfaced
    // as a generic "unknown key (likely typo)" with no hint about which
    // fields are valid — common user mistake is writing `head`/`body`/`tail`
    // (which belong to built-in palette internals, NOT colors-custom blocks)
    // or `background` (removed alias — use `bg`), or attempting nested
    // sub-tables like `colors-custom.foo.normal.red` (not supported — use
    // flat `bg`/`rain` fields only).
    if let Some(rest) = key.strip_prefix("colors-custom.") {
        // rest looks like "<name>.<field>" or "<name>.<sub>.<...>".
        // Split off the name; everything after is the "field" portion.
        if let Some((name, field)) = rest.split_once('.') {
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                // Defensive guard: if the field is one of the three valid
                // ones, the key wouldn't have reached unknown_keys in normal
                // flow — but be paranoid and don't emit a misleading hint.
                if !is_valid_colors_custom_field_str(field) {
                    return Some(colors_custom_field_hint(key, field));
                }
            }
        }
    }

    // Pattern 5 (Phase 5 closure P1-#6): snake_case top-level key. The config
    // surface is kebab-case (color-bg, monolith-size, auto-color-drift, etc.).
    // Users coming from Rust struct-field naming often write snake_case by
    // accident (color_bg, monolith_size). The edit distance from `color_bg`
    // to `color-bg` is 1 (replace `_` with `-`), so pattern 4 WOULD catch it,
    // but the generic "did you mean" message doesn't explain WHY. This pattern
    // emits a clearer message naming the kebab-case convention.
    if !key.contains('.') && key.contains('_') {
        let kebab = key.replace('_', "-");
        if is_top_level_user_key(&kebab) {
            return Some(format!(
                "'{key}': config.toml uses kebab-case (dashes), not snake_case (underscores). \
                 Write it as: '{kebab}' = <value>"
            ));
        }
    }

    // Pattern 6 (Phase 5 closure P1-#8): density-map at top-level. The
    // `density-map` key is only valid inside [profile.<name>] or
    // [scene-custom.<name>] sections. Users who write it at the top level
    // get a generic "unknown key" error with no explanation that it's a
    // section-only field. This pattern emits a targeted move hint.
    if key == "density-map" || key == "density_map" {
        return Some(format!(
            "'{key}': density-map is a section-only field — it is NOT valid at the top level. \
             Move it inside a [profile.<name>] or [scene-custom.<name>] block: \
             e.g. [scene-custom.foo]\n    density-map = \"0.5,0.3,0.2\""
        ));
    }

    // Pattern 4 (v25.11 / bug #13): top-level key typo. If the unknown key
    // is a simple word (no dots) that is edit-distance ≤ 2 from a known
    // top-level USER_CONFIG_KEYS entry, suggest the closest match. This
    // catches common typos like `collor` → `color`, `speeed` → `speed`,
    // `densit` → `density`, `charaset` → `charset`, etc.
    //
    // Only triggered for keys WITHOUT dots — dotted keys are handled by
    // the patterns above (color.tune.*, scene-custom.*, colors-custom.*).
    // A dotted key like `collor.tune.brightness` would NOT trigger this
    // (it would fall through to None), which is correct because the user
    // likely mis-nested the entire section, not just typo'd the prefix.
    if !key.contains('.') && !key.is_empty() {
        if let Some(suggestion) = closest_top_level_key(key) {
            return Some(format!(
                "'{key}': unknown key (likely typo). Did you mean '{suggestion}'? \
                 Run 'cosmostrix --testconf' to see all valid config keys."
            ));
        }
    }

    None
}

/// Find the closest match in `USER_CONFIG_KEYS` to `input` using edit distance.
/// Returns `Some(suggestion)` if the best match has edit distance ≤ 2, or
/// `None` if no key is close enough (avoiding false positives for keys that
/// are genuinely unrelated).
#[must_use]
fn closest_top_level_key(input: &str) -> Option<&'static str> {
    let input_lower = input.to_ascii_lowercase();
    let mut best: Option<(&'static str, usize)> = None;
    for &candidate in USER_CONFIG_KEYS.iter() {
        let dist = edit_distance(&input_lower, candidate);
        // Only accept if distance ≤ 2 AND the candidate is at least 3 chars
        // (avoiding false matches for very short keys like `fps`).
        if dist <= 2 && candidate.len() >= 3 {
            match best {
                None => best = Some((candidate, dist)),
                Some((_, best_dist)) if dist < best_dist => best = Some((candidate, dist)),
                _ => {}
            }
        }
    }
    best.map(|(s, _)| s)
}

/// Compute Levenshtein edit distance between two strings.
/// Used for "did you mean" typo suggestions.
fn edit_distance(a: &str, b: &str) -> usize {
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

/// Returns `true` if `field` is a recognized colors-custom field name.
/// Mirrors `configfile::is_valid_colors_custom_field` (which is private).
/// Kept in sync via tests in this module.
#[inline]
fn is_valid_colors_custom_field_str(field: &str) -> bool {
    matches!(field, "bg" | "rain" | "stops")
}

/// Build a targeted hint for an invalid `colors-custom.<name>.<field>` key.
///
/// `field` is everything after the second `.` (e.g. `head`, `body`,
/// `normal.red`, `background`). The hint explains the valid fields and
/// calls out common confusions (`head`/`body`/`tail` belong to built-in
/// palette internals; `background` was removed; nested sub-tables not
/// supported).
fn colors_custom_field_hint(key: &str, field: &str) -> String {
    // Common confusion: user thinks colors-custom supports the same
    // head/body/tail triple as the built-in palette spec.
    if field == "head" || field == "body" || field == "tail" {
        return format!(
            "'{key}': '{field}' is not a colors-custom field. \
             The 'head'/'body'/'tail' triple belongs to built-in palette internals and is not \
             user-configurable. Valid fields: bg (background color), rain (gradient stops). \
             Example: [colors-custom.<name>]\n  bg = \"#0a0a12\"\n  rain = [\"#1a0033\", \"#9933ff\", \"#ffffff\"]"
        );
    }
    // Removed alias — point users to the canonical name.
    if field == "background" {
        return format!(
            "'{key}': 'background' was removed in v25.10 — use 'bg' instead. \
             Example: bg = \"#0a0a12\""
        );
    }
    // Nested sub-table attempt (e.g. colors-custom.foo.normal.red).
    // `field` here contains a dot — e.g. "normal.red" or "bright.green".
    if field.contains('.') {
        let top = field.split('.').next().unwrap_or(field);
        return format!(
            "'{key}': nested sub-table '{top}' is not supported under [colors-custom.<name>]. \
             colors-custom only accepts flat 'bg' and 'rain' fields — there is no 'normal'/'bright' \
             sub-table. Use the top-level color tune ([color.tune]) for per-segment brightness \
             adjustments, or define a separate colors-custom palette for each variant."
        );
    }
    // Generic invalid field — list the valid ones.
    format!(
        "'{key}': '{field}' is not a recognized colors-custom field. \
         Valid fields: bg (background color), rain (gradient stops), \
         stops (deprecated alias for 'rain'). \
         Example: [colors-custom.<name>]\n  bg = \"#0a0a12\"\n  rain = [\"#1a0033\", \"#9933ff\", \"#ffffff\"]"
    )
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
pub(crate) fn format_hints_block(keys: &[String]) -> String {
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
        // v30 (2026-08-05): the hint now says these keys have been REMOVED
        // (atmosphere engine eliminated), not "move to root scope".
        let key = "scene-custom.hacker-mode.adaptive-custom.10-00.color";
        let hint = suggest_for_unknown_key(key).expect("expected hint");
        assert!(
            hint.contains("removed"),
            "hint should mention removed: {hint}"
        );
        assert!(
            hint.contains("07b44b5"),
            "hint should reference the elimination commit: {hint}"
        );
        assert!(
            hint.contains("Remove this entry"),
            "hint should tell user to remove the entry: {hint}"
        );
    }

    #[test]
    fn scene_custom_with_nested_adaptive_custom_no_trailing_field() {
        // Edge case: section header itself (no field after) — the parser
        // doesn't add these to unknown_keys (section headers aren't
        // validated), but if it ever does, the hint should still fire.
        let key = "scene-custom.hacker-mode.adaptive-custom.10-00";
        let hint = suggest_for_unknown_key(key).expect("expected hint");
        assert!(hint.contains("removed"));
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
        // v30 (2026-08-05, atmosphere elimination): `adaptive-custom.10-00`
        // at file root is no longer a valid key — it would be caught by
        // is_known_key("adaptive-custom") returning false (the bare key is
        // still in USER_CONFIG_KEYS for the validate_field_value migration
        // message, but `adaptive-custom.10-00` has the HH-MM suffix and is
        // not in USER_CONFIG_KEYS). The hint function only handles the
        // scene-custom nested pattern; the root-scope `adaptive-custom.10-00`
        // case is handled by the unknown_keys + validate_field_value path,
        // not by suggest_for_unknown_key. So this returns None here.
        assert_eq!(suggest_for_unknown_key("adaptive-custom.10-00"), None);
    }

    // ── suggest_for_unknown_key: generic typos ────────────────────────────

    #[test]
    fn generic_typo_returns_none() {
        // v25.11: 'colro' and 'colour' now match 'color' (edit distance ≤ 2)
        // so they DO get a did-you-mean hint. Use genuinely unrelated keys
        // that are edit-distance > 2 from any known key.
        assert_eq!(suggest_for_unknown_key("xyzqwerty"), None);
        assert_eq!(suggest_for_unknown_key("zzzzzzzzz"), None);
        assert_eq!(
            suggest_for_unknown_key("scene-custom.hacker-mode.totally-fake-field"),
            None
        );
    }

    // ── format_hints_block ────────────────────────────────────────────────

    #[test]
    fn format_hints_block_empty_for_no_hints() {
        // v25.11: use keys that are NOT close to any known key (edit distance > 2).
        // 'colro'/'colour' now match 'color' and would produce hints.
        let keys = vec!["xyzqwerty".to_string(), "zzzzzzzzz".to_string()];
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
        // v25.11: 'colro'/'colour' now match 'color' and produce hints.
        // Use genuinely unrelated keys for the no-hint cases.
        let keys = vec![
            "xyzqwerty".to_string(),       // no hint (edit dist > 2 from all keys)
            "color.tune.bold".to_string(), // hint (structural pattern)
            "zzzzzzzzz".to_string(),       // no hint
        ];
        let block = format_hints_block(&keys);
        let hint_count = block.matches("\n  hint: ").count();
        assert_eq!(
            hint_count, 1,
            "only the recognized pattern should produce a hint: {block}"
        );
        assert!(block.contains("color.tune.bold"));
    }

    // ── Integration: parse_config_text promotes mis-nested top-level keys ─

    #[test]
    fn parse_color_tune_section_with_bold_promotes_to_root() {
        // v25.7: [color.tune] + `bold = true` no longer lands in unknown_keys.
        // The parser auto-promotes `bold` to root scope (it's a known top-level
        // key) and records the promotion. The hint function still fires when
        // explicitly given the would-be-nested form, so users who see the
        // promotion notice in --testconf can understand what happened.
        let parsed = crate::configfile::parse_config_text("[color.tune]\nbold = true\n");
        assert!(
            parsed.unknown_keys.is_empty(),
            "expected no unknown keys (auto-promoted), got: {:?}",
            parsed.unknown_keys
        );
        assert!(
            parsed
                .promoted_keys
                .iter()
                .any(|(from, to)| from == "color.tune.bold" && to == "bold"),
            "expected color.tune.bold -> bold promotion, got: {:?}",
            parsed.promoted_keys
        );
        // Hint still works on the would-be-nested form (for testconf display).
        assert!(suggest_for_unknown_key("color.tune.bold").is_some());
    }

    #[test]
    fn parse_scene_custom_section_with_nested_adaptive_custom_promotes_to_root() {
        // v25.7: [scene-custom.hacker-mode.adaptive-custom.10-00] + color = cosmos
        // — both `adaptive-custom.10-00` (segment of the section path) AND
        // `color` (the flat key under it) are recognized top-level keys, so
        // the auto-promote fires for `color` (the flat key). No unknown_keys.
        // The hint function still fires when given the would-be-nested form
        // (e.g. for --testconf display when the user explicitly wrote the
        // mis-nested form without a flat key underneath).
        let content = "[scene-custom.hacker-mode.adaptive-custom.10-00]\ncolor = cosmos\n";
        let parsed = crate::configfile::parse_config_text(content);
        assert!(
            parsed.unknown_keys.is_empty(),
            "expected no unknown keys (color auto-promoted to root), got: {:?}",
            parsed.unknown_keys
        );
        // `color = cosmos` was promoted to root scope.
        assert_eq!(
            parsed.values.get("color").map(String::as_str),
            Some("cosmos")
        );
        // Hint still fires on the would-be-nested form (for testconf display).
        let full_key = "scene-custom.hacker-mode.adaptive-custom.10-00.color";
        assert!(suggest_for_unknown_key(full_key).is_some());
    }

    // ── Pattern 3 (v25.10 / bug #8): invalid colors-custom fields ──────────

    #[test]
    fn colors_custom_head_field_returns_hint() {
        // `head` belongs to built-in palette internals, NOT colors-custom.
        let hint = suggest_for_unknown_key("colors-custom.foo.head").expect("expected hint");
        assert!(hint.contains("head"), "hint mentions the field: {hint}");
        assert!(hint.contains("bg"), "hint lists bg as valid: {hint}");
        assert!(hint.contains("rain"), "hint lists rain as valid: {hint}");
        assert!(
            hint.contains("palette internals"),
            "hint explains head/body/tail belong to palette internals: {hint}"
        );
    }

    #[test]
    fn colors_custom_body_and_tail_get_same_hint_as_head() {
        for field in &["body", "tail"] {
            let key = format!("colors-custom.foo.{field}");
            let hint = suggest_for_unknown_key(&key).expect("expected hint");
            assert!(hint.contains("palette internals"), "field {field}: {hint}");
        }
    }

    #[test]
    fn colors_custom_background_field_returns_removal_hint() {
        // `background` was an undocumented alias removed in v25.10.
        let hint = suggest_for_unknown_key("colors-custom.foo.background").expect("expected hint");
        assert!(
            hint.contains("removed"),
            "hint should mention removal: {hint}"
        );
        assert!(hint.contains("bg"), "hint should suggest bg: {hint}");
    }

    #[test]
    fn colors_custom_nested_normal_subtable_returns_hint() {
        // colors-custom.foo.normal.red — user thought normal/bright subtables existed.
        let hint = suggest_for_unknown_key("colors-custom.foo.normal.red").expect("expected hint");
        assert!(hint.contains("nested"), "hint mentions nested: {hint}");
        assert!(
            hint.contains("normal"),
            "hint mentions the sub-table: {hint}"
        );
        assert!(
            hint.contains("color.tune"),
            "hint points to color.tune for per-segment adjustments: {hint}"
        );
    }

    #[test]
    fn colors_custom_nested_bright_subtable_returns_hint() {
        let hint =
            suggest_for_unknown_key("colors-custom.foo.bright.green").expect("expected hint");
        assert!(hint.contains("nested"));
        assert!(hint.contains("bright"));
    }

    #[test]
    fn colors_custom_generic_invalid_field_returns_hint_listing_valid_fields() {
        let hint = suggest_for_unknown_key("colors-custom.foo.color").expect("expected hint");
        assert!(hint.contains("color"), "hint mentions the field: {hint}");
        assert!(hint.contains("bg"));
        assert!(hint.contains("rain"));
        assert!(
            hint.contains("stops"),
            "hint should also mention the deprecated alias: {hint}"
        );
    }

    #[test]
    fn colors_custom_valid_fields_return_none() {
        // Defensive guard: valid keys should never produce a hint, even if
        // a caller accidentally invokes suggest_for_unknown_key on them.
        assert_eq!(suggest_for_unknown_key("colors-custom.foo.bg"), None);
        assert_eq!(suggest_for_unknown_key("colors-custom.foo.rain"), None);
        assert_eq!(suggest_for_unknown_key("colors-custom.foo.stops"), None);
    }

    #[test]
    fn colors_custom_invalid_name_does_not_trigger_hint() {
        // If the name segment is invalid (e.g. contains a dot or non-alnum
        // char), the key doesn't match the colors-custom.<name>.<field>
        // pattern — no hint. The caller falls back to the generic message.
        // `colors-custom.foo.bar.baz.qux` — name="foo", field="bar.baz.qux"
        // (contains dots). The hint should still fire (field is invalid).
        let hint = suggest_for_unknown_key("colors-custom.foo.bar.baz.qux");
        assert!(hint.is_some(), "multi-dot field should still get a hint");
    }

    #[test]
    fn parse_colors_custom_head_lands_in_unknown_keys_with_hint() {
        // End-to-end: user writes head=#fff, gets unknown key + hint.
        let parsed = crate::configfile::parse_config_text("[colors-custom.foo]\nhead = \"#fff\"\n");
        assert!(
            parsed
                .unknown_keys
                .contains(&"colors-custom.foo.head".to_string()),
            "head should be in unknown_keys: {:?}",
            parsed.unknown_keys
        );
        let hint = suggest_for_unknown_key("colors-custom.foo.head");
        assert!(hint.is_some(), "hint should fire for head");
    }

    #[test]
    fn parse_colors_custom_stops_accepted_not_in_unknown_keys() {
        // v25.10: stops is now a deprecated alias for rain — accepted.
        let parsed =
            crate::configfile::parse_config_text("[colors-custom.foo]\nstops = \"#ff0000\"\n");
        assert!(
            parsed.unknown_keys.is_empty(),
            "stops should NOT be in unknown_keys: {:?}",
            parsed.unknown_keys
        );
        assert!(parsed.values.contains_key("colors-custom.foo.stops"));
    }

    #[test]
    fn parse_colors_custom_background_lands_in_unknown_keys_with_hint() {
        // v25.10: background alias removed — surfaces as unknown key.
        let parsed =
            crate::configfile::parse_config_text("[colors-custom.foo]\nbackground = \"#fff\"\n");
        assert!(
            parsed
                .unknown_keys
                .contains(&"colors-custom.foo.background".to_string()),
            "background should be in unknown_keys: {:?}",
            parsed.unknown_keys
        );
        let hint = suggest_for_unknown_key("colors-custom.foo.background");
        assert!(hint.is_some(), "hint should fire for background");
    }

    // ── v25.11 (bug #13): top-level key typo "did you mean" hints ──

    #[test]
    fn typo_collor_suggests_color() {
        let hint = suggest_for_unknown_key("collor");
        assert!(hint.is_some(), "should suggest for 'collor'");
        let h = hint.unwrap();
        assert!(h.contains("color"), "hint should suggest 'color': {h}");
        assert!(h.contains("Did you mean"), "should be a did-you-mean: {h}");
    }

    #[test]
    fn typo_speeed_suggests_speed() {
        let hint = suggest_for_unknown_key("speeed");
        assert!(hint.is_some(), "should suggest for 'speeed'");
        assert!(
            hint.unwrap().contains("speed"),
            "hint should suggest 'speed'"
        );
    }

    #[test]
    fn typo_densit_suggests_density() {
        let hint = suggest_for_unknown_key("densit");
        assert!(hint.is_some(), "should suggest for 'densit'");
        assert!(
            hint.unwrap().contains("density"),
            "hint should suggest 'density'"
        );
    }

    #[test]
    fn typo_charaset_suggests_charset() {
        let hint = suggest_for_unknown_key("charaset");
        assert!(hint.is_some(), "should suggest for 'charaset'");
        assert!(
            hint.unwrap().contains("charset"),
            "hint should suggest 'charset'"
        );
    }

    #[test]
    fn typo_glitchlevel_suggests_glitch_level() {
        let hint = suggest_for_unknown_key("glitchlevel");
        assert!(hint.is_some(), "should suggest for 'glitchlevel'");
        assert!(
            hint.unwrap().contains("glitch-level"),
            "hint should suggest 'glitch-level'"
        );
    }

    #[test]
    fn typo_with_dot_does_not_trigger_top_level_suggestion() {
        // Dotted keys should NOT trigger the top-level typo suggestion —
        // they're handled by the structural patterns above, or they're
        // genuinely unknown dotted keys.
        let hint = suggest_for_unknown_key("collor.tune");
        assert!(
            hint.is_none(),
            "dotted key should not get top-level typo hint"
        );
    }

    #[test]
    fn completely_unrelated_key_gets_no_suggestion() {
        // A key that's edit-distance > 2 from any known key should get None.
        let hint = suggest_for_unknown_key("xyzqwrstuv");
        assert!(
            hint.is_none(),
            "unrelated key should not get a false suggestion"
        );
    }

    #[test]
    fn edit_distance_basic_cases() {
        assert_eq!(edit_distance("color", "color"), 0);
        assert_eq!(edit_distance("collor", "color"), 1); // extra 'l'
        assert_eq!(edit_distance("speeed", "speed"), 1); // extra 'e'
        assert_eq!(edit_distance("charaset", "charset"), 1); // swap a/e
        assert_eq!(edit_distance("density", "densit"), 1); // missing 'y'
        assert_eq!(edit_distance("abc", "xyz"), 3); // completely different
        assert_eq!(edit_distance("", "color"), 5); // empty to non-empty
        assert_eq!(edit_distance("color", ""), 5); // non-empty to empty
    }

    // ── Phase 5 closure (P1-#6): snake_case → kebab-case hint ──

    #[test]
    fn snake_case_color_bg_suggests_kebab_case() {
        let hint = suggest_for_unknown_key("color_bg").expect("expected hint");
        assert!(
            hint.contains("kebab-case"),
            "hint should mention kebab-case: {hint}"
        );
        assert!(
            hint.contains("color-bg"),
            "hint should suggest the correct kebab-case form: {hint}"
        );
    }

    #[test]
    fn snake_case_monolith_size_suggests_kebab_case() {
        let hint = suggest_for_unknown_key("monolith_size").expect("expected hint");
        assert!(
            hint.contains("monolith-size"),
            "hint should suggest kebab-case: {hint}"
        );
    }

    #[test]
    fn snake_case_auto_color_drift_suggests_kebab_case() {
        let hint = suggest_for_unknown_key("auto_color_drift").expect("expected hint");
        assert!(hint.contains("auto-color-drift"));
    }

    #[test]
    fn random_underscore_key_without_kebab_match_gets_no_snake_hint() {
        // `foo_bar` is not a snake_case form of any USER_CONFIG_KEYS entry,
        // so the snake_case hint should NOT fire (falls through to pattern 4
        // typo check, which also won't match because edit distance is too far).
        let hint = suggest_for_unknown_key("foo_bar");
        assert!(
            hint.is_none(),
            "unrelated underscore key should get no hint"
        );
    }

    // ── Phase 5 closure (P1-#8): density-map top-level hint ──

    #[test]
    fn density_map_at_top_level_suggests_section_move() {
        let hint = suggest_for_unknown_key("density-map").expect("expected hint");
        assert!(
            hint.contains("section-only"),
            "hint should mention section-only: {hint}"
        );
        assert!(
            hint.contains("[profile.") || hint.contains("[scene-custom."),
            "hint should mention target sections: {hint}"
        );
    }

    #[test]
    fn density_map_snake_case_also_gets_section_hint() {
        let hint = suggest_for_unknown_key("density_map").expect("expected hint");
        // Even though it has an underscore, pattern 6 fires BEFORE pattern 5
        // (snake_case check) because density-map is a special section-only field.
        assert!(hint.contains("section-only"));
        assert!(hint.contains("density-map")); // canonical form in the hint
    }
}
