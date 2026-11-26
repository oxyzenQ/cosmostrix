// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

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
//! [charset-custom.zen]
//! set = "|"
//! ```
//!
//! - `set` — the literal string of characters to use as the rain glyph
//!   pool. Order is preserved (the renderer shuffles them at runtime, so
//!   order does not affect on-screen appearance, only what's in the pool).
//!   ANY single-width glyph is legal — including the config metacharacters
//!   `[`, `]`, `#`, `=` (quote the value; bug #19) — so a single-glyph
//!   pool like `set = "["` is valid.
//! - Whitespace inside `set` (except ASCII space) is skipped. Space (` `)
//!   is a valid single-width character and is kept when it is not a
//!   leading/trailing edge (edges are trimmed as accidental padding).
//! - Control characters (C0/C1) are rejected with a clear error.
//! - Wide / zero-width characters (emoji, CJK fullwidth, combining marks)
//!   are SKIPPED with a warning — the value is accepted, the offending
//!   codepoints are dropped from the pool. If nothing usable remains
//!   (e.g. the pool was a single wide char) the value is rejected with
//!   "no usable single-width characters". The renderer is column-based
//!   and assumes one cell per glyph — wide chars corrupt alignment. This is
//!   the Cosmic Dragon principle: no emoji, no wide chars, ever. It is a
//!   permanent design choice, not a limitation to be lifted later.
//! - There are no escape sequences: a LONE double-quote glyph is not
//!   expressible (`"` is the string delimiter at the config layer and
//!   gets stripped); quotes in the MIDDLE of a pool are kept.
//! - Maximum length: 256 characters. Longer values are rejected so the
//!   rain glyph pool does not become a memory hog.
//!
//! # Loading
//!
//! Two equivalent ways to activate a custom charset:
//!
//! 1. CLI: `cosmostrix --charset zen` (when `[charset-custom.zen]` exists
//!    in config.toml, the custom block takes precedence over any built-in
//!    preset with the same name).
//! 2. Config: `charset = "zen"` in config.toml.
//!
//! Lookup is case-insensitive: `[charset-custom.zen]` is matched by
//! `charset = "zen"`, `--charset zen`, etc.
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
pub(crate) const CHARSET_CUSTOM_MAX_LEN: usize = 256;

/// v50.0.0-beta.6 LTS, NIGHT-hunt-39 (2026-09-13), NIGHT-hunt-40
/// (2026-09-13, owner mandate): maximum number of custom charset
/// blocks accepted in a single config.toml. Aligned with colors-custom,
/// scene-custom AND the ambient entry count (all four namespaces use
/// the owner's min-1/max-24 entry policy). Bounds the BTreeMap size +
/// iteration cost in `collect_charset_custom`; 24 blocks is far beyond
/// any realistic use case (built-in charsets are ~25).
pub(crate) const CHARSET_CUSTOM_MAX_BLOCKS: usize = 24;

/// v50.0.0-beta.6 LTS: maximum length of a custom charset block name.
/// Bounds BTreeMap key allocation. 64 chars is generous (built-in names
/// are ≤16 chars like "cyberpunk"); longer names are likely typos.
pub(crate) const CHARSET_CUSTOM_MAX_NAME_LEN: usize = 64;

/// A parsed custom charset definition — a flat list of single-width chars.
#[derive(Debug, Clone, Default)]
pub(crate) struct CharsetCustomDef {
    /// The validated character pool, in config-declared order.
    pub chars: Vec<char>,
    /// The block name exactly as written in config.toml (original case),
    /// for user-facing display. Map keys stay lowercase so lookups remain
    /// case-insensitive; this field keeps `--list-charsets`, the
    /// "expected one of" error list and the shadow-warning faithful to
    /// the config source. NIGHT-cafe-1 (2026-11-25): `[charset-custom.T1]`
    /// used to surface as "t1" everywhere its name was printed.
    pub display_name: String,
}

/// Collect every `[charset-custom.<name>]` block from the parsed config
/// HashMap and return them as a name → def map.
///
/// Only the `set` field is recognized. Unknown fields under
/// `[charset-custom.<name>]` are silently skipped — they are filtered
/// upstream by `is_known_key()` in `configfile.rs`, so they never reach
/// this function.
///
/// Names are normalized to lowercase for case-insensitive matching; the
/// original-case spelling is preserved in `CharsetCustomDef::display_name`
/// for every user-facing surface (NIGHT-cafe-1).
///
/// v50.0.0-beta.6 LTS: bounded by `CHARSET_CUSTOM_MAX_BLOCKS` (24,
/// NIGHT-hunt-39, NIGHT-hunt-40) and `CHARSET_CUSTOM_MAX_NAME_LEN` (64 chars)
/// to prevent config typos from bloating memory or stalling startup. Since
/// NIGHT-depthtest-3 the length cap is fronted by a HARD validation error
/// (`validate_charset_custom_name_len`), so an oversized name can no
/// longer pass `--testconf` silently; since NIGHT-hunt-37 the block count
/// is a hard error too (`validate_charset_custom_block_count`).
#[must_use]
pub(crate) fn collect_charset_custom(
    cfg: &HashMap<String, String>,
) -> BTreeMap<String, CharsetCustomDef> {
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
        // v50.0.0-beta.6 LTS: skip oversized names early (before
        // to_ascii_lowercase allocates). 64 chars is generous.
        if name.len() > CHARSET_CUSTOM_MAX_NAME_LEN {
            continue;
        }
        // NIGHT-cafe-1: keep the original-case block name for display
        // before the key is normalized. Same last-wins semantics as
        // `chars` for pathological duplicate case-variant blocks.
        let display_name = name.to_string();
        let name = name.to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        // v50.0.0-beta.6 LTS: skip if we already hit the block cap.
        if out.len() >= CHARSET_CUSTOM_MAX_BLOCKS && !out.contains_key(&name) {
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
        def.display_name = display_name;
    }

    out
}

/// Parse a `set = "..."` value into a validated `Vec<char>`.
///
/// Values normally arrive UNQUOTED from the flat HashMap produced by
/// `configfile::parse_config_text` (the config layer already strips the
/// outer double quotes). The `trim_matches('"')` here is deliberate
/// defense-in-depth for direct callers that hand over still-quoted
/// values — it strips ONE OR MORE leading/trailing double quotes before
/// the per-character single-width + non-control filter (the same filter
/// as `charset.rs::build_chars`).
///
/// Returns `Err(message)` if:
/// - the value is empty after trimming (no usable chars)
/// - a control character is present
/// - the pool exceeds `CHARSET_CUSTOM_MAX_LEN` characters
///
/// Wide / zero-width characters are SKIPPED (not errors) — this matches
/// the old `--charset-file` path's behavior (flag removed in v25) and
/// avoids hard-failing when a
/// user copy-pastes a string that happens to include a non-breaking space
/// or a stray combining mark. A warning is emitted to stderr per skipped
/// codepoint.
///
/// Cosmic Dragon principle: stripping wide chars is a permanent design
/// choice. The renderer will never support emoji or full-width CJK glyphs —
/// its soul is single-cell diff-based rendering. Do not interpret this
/// filter as a temporary limitation.
pub(crate) fn parse_charset_value(value: &str) -> Result<Vec<char>, String> {
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
        // v80.0.0-beta.1 defining-features hardening: routed through warn_runtime_or_now —
        // collect_charset_custom re-runs on every scene change and live
        // reload, so this warning fires mid-rain and must buffer (AB-10)
        // instead of eprintln-ing into the alt screen.
        crate::output::warn_runtime_or_now(&format!(
            "skipped {} wide/zero-width character(s) from charset-custom: {}",
            skipped_wide.len(),
            skipped_wide.join(", ")
        ));
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
pub(crate) fn load_custom_charset(
    cfg: &HashMap<String, String>,
    name: &str,
) -> Result<Vec<char>, String> {
    let palettes = collect_charset_custom(cfg);
    let normalized = name.trim().to_ascii_lowercase();
    let def = palettes.get(&normalized).ok_or_else(|| {
        // NIGHT-cafe-1: list the block names exactly as the config spells
        // them (original case). Order stays deterministic — the BTreeMap
        // iterates in lowercase-key order regardless of HashMap seed.
        let mut available: Vec<String> = palettes
            .iter()
            .map(|(key, d)| {
                if d.display_name.is_empty() {
                    key.clone()
                } else {
                    d.display_name.clone()
                }
            })
            .collect();
        available.sort_by_key(|a| a.to_ascii_lowercase());
        let list = if available.is_empty() {
            "<none defined>".to_string()
        } else {
            available.join(", ")
        };
        // NIGHT-depthtest-3: a >64-char name can never match a block
        // (the collector drops oversized names) — surface the limit
        // instead of a bare "not found" that reads as a missing block.
        let limit_note = if normalized.len() > CHARSET_CUSTOM_MAX_NAME_LEN {
            format!(
                "\n  note: the name is {} chars — over the {CHARSET_CUSTOM_MAX_NAME_LEN}-char limit, so no [charset-custom.<name>] block can ever define it",
                normalized.chars().count()
            )
        } else {
            String::new()
        };
        format!(
            "custom charset '{name}' not found in config{limit_note}\nexpected one of: {list}\n\n  Use --list-charsets to see built-in and custom charsets."
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
///
/// v50.0.0-beta.6 Option D: when a custom charset shadows a builtin
/// preset with the same name, emits a warning so the user knows the
/// custom block won. Custom always wins (owner mandate).
/// Display name for a custom charset block, from the parser-level
/// original-case header record.
///
/// NIGHT-dragon-fly-2 (2026-11-26): the lookup key stays lowercase
/// everywhere (the matching contract: `-C t1` loads
/// `[charset-custom.T1]`), but the name carried into runtime display
/// state — the HUD `chr:` line, the startup verbose line, session
/// reports — must keep the block's original spelling, exactly like the
/// listing surfaces since NIGHT-cafe-1. Without this, a block spelled
/// `[charset-custom.Quantum]` rendered the HUD as `quantum` although
/// `--list-charsets` correctly showed `Quantum`.
///
/// Rides the memoized config load (no extra disk read; the startup
/// caller just resolved the same file). Returns `None` when no
/// `[charset-custom.<normalized>]` block exists (builtin name or no
/// config) so the caller keeps its normalized name unchanged.
#[must_use]
pub(crate) fn original_case_custom_charset_name(
    config_path: Option<&std::path::Path>,
    normalized: &str,
) -> Option<String> {
    if normalized.is_empty() {
        return None;
    }
    let key = format!("charset-custom.{normalized}");
    crate::configfile::load_config_file_full(config_path)
        .original_custom_name_map()
        .get(&key)
        .cloned()
}

#[must_use]
pub(crate) fn load_custom_charset_if_matches(
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
    let chars = load_custom_charset(cfg, name).ok()?;
    // v50.0.0-beta.6 Option D: warn if this custom name also matches a
    // builtin preset. Custom wins (already returned below), but the user
    // must be informed so silent shadowing never happens. NIGHT-cafe-1
    // note: this line still quotes the LOWERCASED lookup key — the
    // parser-level original-case record does not reach this deep (it is
    // threaded only to the listing surfaces); matching is
    // case-insensitive either way.
    if crate::charset::charset_from_str(name, false).is_ok() {
        let builtin_desc = "builtin preset (see --list-charsets)";
        let custom_desc = format!(
            "{} char(s) from [charset-custom.{}]",
            chars.len(),
            normalized
        );
        crate::output::warn_name_collision("charset", name, builtin_desc, &custom_desc);
    }
    Some(chars)
}

/// Validate a `charset-custom.<name>.set` value for use by `--testconf`
/// and `validate_config_strictly`. Returns `Some(error_message)` on
/// failure, `None` on success.
///
/// This is the entry point `testconf.rs` calls during strict validation.
/// It does NOT mutate any state — pure validation.
#[must_use]
pub(crate) fn validate_charset_custom_value(value: &str) -> Option<String> {
    parse_charset_value(value).err()
}

/// NIGHT-depthtest-3 (owner repro 2026-09-11): reject
/// `charset-custom.<name>` blocks whose name exceeds
/// [`CHARSET_CUSTOM_MAX_NAME_LEN`] (64 chars).
///
/// The collector drops oversized names, which made a valid
/// `[charset-custom.<65+-char name>]` block invisible to `--testconf`
/// (PASS) and `--list-charsets` (never listed) — the same blind-spot
/// class the owner hit on scene-custom. Called from BOTH shared
/// entry points (`testconf::run` and `validate_config_strictly`) so
/// `--testconf`, startup, and the live-reload watcher reject in
/// lockstep. Only `charset-custom.<name>.set` keys are scanned
/// (the `set` field is the whole schema); other shapes keep their
/// existing unknown-key error paths. Sorted + deduplicated: the
/// reported name is deterministic across hash seeds.
#[must_use]
pub(crate) fn validate_charset_custom_name_len(cfg: &HashMap<String, String>) -> Option<String> {
    let mut oversized: Vec<&str> = cfg
        .keys()
        .filter_map(|key| {
            let rest = key.strip_prefix("charset-custom.")?;
            let (name, field) = rest.split_once('.')?;
            (field == "set" && name.len() > CHARSET_CUSTOM_MAX_NAME_LEN).then_some(name)
        })
        .collect();
    oversized.sort_unstable();
    oversized.dedup();
    let name = *oversized.first()?;
    let mut msg = format!(
        "charset-custom name '{}...' is {} chars — exceeds the {}-char name limit; oversized-name blocks are dropped before validation and listing, so the charset would be invisible to --testconf and --list-charsets. Shorten the name.",
        name.chars().take(24).collect::<String>(),
        name.chars().count(),
        CHARSET_CUSTOM_MAX_NAME_LEN
    );
    if oversized.len() > 1 {
        msg.push_str(&format!(" (+{} more oversized names)", oversized.len() - 1));
    }
    Some(msg)
}

/// NIGHT-hunt-37 (owner mandate 2026-09-13): the number of distinct
/// charset-custom blocks must not exceed [`CHARSET_CUSTOM_MAX_BLOCKS`].
/// The collector silently dropped blocks beyond the cap (which ones
/// survived was unspecified — HashMap iteration order); validation now
/// rejects the config so the loss is loud on all three surfaces
/// (`--testconf`, startup, live-reload watcher — the same entry points
/// that call `validate_charset_custom_name_len`). BTreeSet iteration
/// is sorted — the verdict does not depend on the hash seed.
#[must_use]
pub(crate) fn validate_charset_custom_block_count(cfg: &HashMap<String, String>) -> Option<String> {
    let names: std::collections::BTreeSet<&str> = cfg
        .keys()
        .filter_map(|key| {
            let rest = key.strip_prefix("charset-custom.")?;
            let (name, _field) = rest.split_once('.')?;
            Some(name)
        })
        .collect();
    if names.len() > CHARSET_CUSTOM_MAX_BLOCKS {
        return Some(format!(
            "charset-custom: {} blocks defined — maximum is {CHARSET_CUSTOM_MAX_BLOCKS} (remove unused charsets or split the config)",
            names.len()
        ));
    }
    None
}

/// Startup charset resolution — the v25 block moved out of main.rs
/// (night-dragon-fly-2, with the test extraction riding the same LOC
/// cap): normalize the CLI name, load a matching `[charset-custom.*]`
/// pool, swap the display name to the block's original-case spelling,
/// or fall through to the builtin table. Returns the display name plus
/// the glyph pool. Matching stays lowercase-keyed; the display swap is
/// `original_case_custom_charset_name`'s job, and every later lookup
/// (custom loader, live-reload applier) normalizes internally, so the
/// swapped name is display-only.
pub(crate) fn resolve_startup_charset(
    cli_charset: &str,
    config_path: Option<&std::path::Path>,
    def_ascii: bool,
    verbose: bool,
    user_ranges: &[(char, char)],
) -> (String, Vec<char>) {
    let mut charset_preset = crate::cli::normalize_charset_preset_name(cli_charset);
    let cfg = crate::configfile::load_config_file(config_path);
    if let Some(custom_chars) = load_custom_charset_if_matches(&cfg, &charset_preset) {
        if let Some(original) = original_case_custom_charset_name(config_path, &charset_preset) {
            charset_preset = original;
        }
        if verbose {
            crate::output::eprintln_verbose_raw(&format!(
                "charset: {} (custom, {} chars)",
                charset_preset,
                custom_chars.len()
            ));
        }
        return (charset_preset, custom_chars);
    }
    let charset = match crate::charset::charset_from_str(cli_charset, def_ascii) {
        Ok(c) => c,
        Err(e) => crate::cli::ux::die_input(e),
    };
    (
        charset_preset,
        crate::charset::build_chars(charset, user_ranges, def_ascii),
    )
}

#[path = "../../test/scene_charset_custom/tests.rs"]
#[cfg(test)]
pub(crate) mod tests;
