// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! "tip: a similar value exists" hints for unknown config keys.
//!
//! The TOML parser in [`crate::configfile`] classifies any key not matching
//! a known pattern as `unknown_keys`. Previously the only follow-up was a
//! generic `(run 'cosmostrix --testconf' for known keys)` line, which
//! doesn't help when the user has nested a key under the wrong section
//! header (a structural TOML mistake, not a typo).
//!
//! This module pattern-matches two real world user-error cases observed
//! during the depth test and returns a targeted hint explaining
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
//!    scope, which was the original advice).
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
    // Pattern 0: legacy [profile.<name>] block key. The profile system
    // was removed in v14 — these keys are inert and must be renamed to
    // scene-custom.<name>.<field> to take effect.
    if let Some(rest) = key.strip_prefix("profile.") {
        return Some(format!(
            "'{key}': [profile.<name>] blocks are inert (removed in v14). \
             Rename the prefix to scene-custom: scene-custom.{rest}"
        ));
    }

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
    // not move them to root scope (which was the original advice when
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
        // Pattern 2c: bare `async` nested under [scene-custom.<name>].
        // The field name is `async-mode` (matching the top-level key).
        // Users who write `async = true` inside a custom scene block get a
        // generic "unknown key" — this pattern tells them to use `async-mode`.
        if segments.len() == 3 && segments[2] == "async" {
            return Some(format!(
                "'{key}': inside [scene-custom.<name>] blocks, \
                 the field name is 'async-mode' (not 'async'). \
                 Write: async-mode = true",
            ));
        }
    }

    // Pattern 3 (bug #8): invalid colors-custom field. Triggered
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
    // surface is kebab-case (color-bg, monolith-size, crystal-dragon, etc.).
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
    // `density-map` key is only valid inside [scene-custom.<name>]
    // sections. Users who write it at the top level get a generic "unknown
    // key" error with no explanation that it's a section-only field. This
    // pattern emits a targeted move hint.
    if key == "density-map" || key == "density_map" {
        return Some(format!(
            "'{key}': density-map is a section-only field — it is NOT valid at the top level. \
             Move it inside a [scene-custom.<name>] block: \
             e.g. [scene-custom.foo]\n    density-map = \"0.5,0.3,0.2\""
        ));
    }

    // Pattern 4 (bug #13): top-level key typo. If the unknown key
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
                "'{key}': unknown key (likely typo){}\n                 Run 'cosmostrix --testconf' to see all valid config keys.",
                crate::cli::suggestion::format_value_suggestion(suggestion)
            ));
        }
    }

    // Pattern 6 (LTS audit 2026-08-19): removed v15-era `auto-color-drift`
    // config key. v15 silently ignored unknown keys (a bug); v50 strict-
    // rejects them. Users upgrading from v15 → v50 who still have the old
    // `auto-color-drift` key in their config.toml get a targeted hint
    // pointing them to the canonical v50 replacement: `crystal-dragon`.
    //
    // The hint also covers common variants users might write:
    //   - auto-color-drift (kebab-case, the original v15 spelling)
    //   - auto_color_drift (snake_case, common Rust-ism)
    //   - autocolordrift  (no separator)
    //   - auto-drift      (shortened form)
    //
    // Without this hint, users get a generic "unknown key (likely typo)"
    // message that doesn't explain WHY the key was rejected or WHAT to use
    // instead — confusing for users migrating from v15.
    let lower = key.to_ascii_lowercase();
    let is_auto_color_drift_variant = lower == "auto-color-drift"
        || lower == "auto_color_drift"
        || lower == "autocolordrift"
        || lower == "auto-drift"
        || lower == "auto_drift"
        || lower == "autodrift";
    if is_auto_color_drift_variant {
        return Some(format!(
            "'{key}': removed v15-era config key. The auto color drift feature \
             was renamed to 'crystal-dragon' in v50 (point-based temperature \
             group system, replaces the old pattern-based drift). Replace \
             '{key} = <value>' with 'crystal-dragon = false' (or 'true' to enable)."
        ));
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
             Example: [colors-custom.<name>]\n  bg = \"#0a0a12\"\n  rain = [\"#1a0033\", \"#4d0080\", \"#9933ff\", \"#cc66ff\", \"#e6b3ff\", \"#f2ccff\", \"#ffffff\"]"
        );
    }
    // Removed alias — point users to the canonical name.
    if field == "background" {
        return format!(
            "'{key}': 'background' was removed — use 'bg' instead. \
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
         Example: [colors-custom.<name>]\n  bg = \"#0a0a12\"\n  rain = [\"#1a0033\", \"#4d0080\", \"#9933ff\", \"#cc66ff\", \"#e6b3ff\", \"#f2ccff\", \"#ffffff\"]"
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
mod tests;
