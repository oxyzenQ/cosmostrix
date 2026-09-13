// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-37: colors-custom strictness contract — extracted from
//! `colors_custom.rs` (and extending it) to keep that file under the
//! 800-LOC hard cap (see `src/RULES_LOC.md`), following `name_len.rs`.
//!
//! Owner mandate (2026-09-13): custom blocks must be COMPLETE and
//! STRICT. For colors-custom that means three rules the pre-hunt-37
//! layers did not enforce:
//!
//! 1. Rain-stop ceiling: the documented limit was 64 with a SILENT
//!    collector cap (the collector truncated the overflow with a
//!    buffered warning, so the owner's 67-stop repro ran without any
//!    error on every surface). The OKLab engine resamples every
//!    palette to `COLORS_CUSTOM_PALETTE_STEPS` (9) perceptual
//!    samples, so stops beyond 9 are provably discarded input. The
//!    ceiling is now 9 and it is a HARD error on every surface
//!    (startup, --testconf, live-reload watcher), never a cap.
//! 2. Field overload: `rain` and the deprecated `stops` alias in the
//!    same block used to be CONCATENATED (both pushed into
//!    `palette.rain`), an overloaded block by the owner's
//!    definition. Now a hard error — define exactly one of them.
//! 3. Block-count ceiling: the collector silently dropped blocks
//!    beyond `COLORS_CUSTOM_MAX_BLOCKS` (the last silent-skip left
//!    after NIGHT-depthtest-3 fixed the name-length skip). Now a
//!    hard error, so a large pasted config fails loudly instead of
//!    losing blocks invisibly.
//!
//! All three checks are value/structure-level validation: they run at
//! config-parse time on all three validation surfaces and add zero
//! per-frame work (the chroma pipeline math is untouched — see the
//! chroma KEY.md UNLOCK entry for this round).

use std::collections::{BTreeSet, HashMap};

use super::{COLORS_CUSTOM_MAX_BLOCKS, COLORS_CUSTOM_MAX_RAIN_STOPS};

/// Split a raw `rain`/`stops` value into its stop entries.
///
/// Accepts both storage formats the config parser produces:
/// - TOML array: `["#1a0033", "#4d0080", ...]` (multi-line arrays are
///   already joined to one line by `parse_config_text`)
/// - CSV string: `"#1a0033, #4d0080"`
///
/// Entries are trimmed, quote-stripped and empty-filtered — the exact
/// normalization the collector applies before `parse_hex_color`, so
/// the validator's stop count can never drift from what the runtime
/// actually parses (the F-23-1 duplicated-predicate lesson: the
/// pre-hunt-37 code carried three hand-written twins of this split —
/// the collector's, `--testconf`'s value checker, and the array
/// parser — any of which could have drifted). Re-exported by
/// `colors_custom.rs` so every consumer shares this one definition.
pub(crate) fn split_rain_stop_entries(value: &str) -> Vec<&str> {
    let s = value.trim();
    let s = s.strip_prefix('[').unwrap_or(s);
    let s = s.strip_suffix(']').unwrap_or(s);
    s.split(',')
        .map(|e| e.trim().trim_matches('"').trim())
        .filter(|e| !e.is_empty())
        .collect()
}

/// NIGHT-hunt-37 rule 1 + 2: rain-stop ceiling and rain/stops
/// overload, asked of the RAW config values.
///
/// The count must be taken from the source strings, not the collected
/// `CustomPaletteDef` — the collector caps at
/// `COLORS_CUSTOM_MAX_RAIN_STOPS` before `to_palette` ever sees the
/// full list, so a collected-def count would silently re-bless the
/// exact overflow this validator exists to reject.
///
/// Returns `None` when no colors-custom block violates the contract.
/// Keys are iterated sorted — the first reported block is
/// deterministic across hash seeds (the same contract as every
/// validator in this layer).
pub(super) fn validate_rain_stop_contract(cfg: &HashMap<String, String>) -> Option<String> {
    // (block name, field, raw value) for every rain/stops key.
    let mut rain_like: Vec<(&str, &str, &str)> = Vec::new();
    for (key, value) in cfg {
        let Some(rest) = key.strip_prefix("colors-custom.") else {
            continue;
        };
        let Some((name, field)) = rest.split_once('.') else {
            continue;
        };
        if field == "rain" || field == "stops" {
            rain_like.push((name, field, value.as_str()));
        }
    }
    if rain_like.is_empty() {
        return None;
    }
    rain_like.sort();

    // Rule 1: per-key stop count against the ceiling. A count taken
    // here sees the OWNER's input verbatim (e.g. the 67-stop repro),
    // before any truncation could hide it.
    for (name, field, value) in &rain_like {
        let count = split_rain_stop_entries(value).len();
        if count > COLORS_CUSTOM_MAX_RAIN_STOPS {
            return Some(format!(
                "colors-custom.{name}.{field}: {count} rain stops — maximum is {COLORS_CUSTOM_MAX_RAIN_STOPS} (the OKLab engine resamples every palette to 9 perceptual samples, so stops beyond that are discarded input; trim the list)"
            ));
        }
    }

    // Rule 2: `rain` and the deprecated `stops` alias in the SAME
    // block — both would be concatenated into `palette.rain` by the
    // collector, an overloaded definition. Exactly one must be set.
    let names: BTreeSet<&str> = rain_like.iter().map(|(name, _, _)| *name).collect();
    for name in names {
        let has_rain = rain_like.iter().any(|(n, f, _)| *n == name && *f == "rain");
        let has_stops = rain_like
            .iter()
            .any(|(n, f, _)| *n == name && *f == "stops");
        if has_rain && has_stops {
            return Some(format!(
                "colors-custom.{name}: both 'rain' and the deprecated 'stops' alias are set — define exactly one (prefer 'rain')"
            ));
        }
    }
    None
}

/// NIGHT-hunt-37 rule 3: the number of distinct colors-custom blocks
/// must not exceed `COLORS_CUSTOM_MAX_BLOCKS`. The collector silently
/// dropped blocks beyond the cap (which ones survived was unspecified
/// — HashMap iteration order); validation now rejects the config so
/// the loss is loud. BTreeSet iteration is sorted — the verdict does
/// not depend on the hash seed.
pub(super) fn validate_block_count(cfg: &HashMap<String, String>) -> Option<String> {
    let names: BTreeSet<&str> = cfg
        .keys()
        .filter_map(|key| key.strip_prefix("colors-custom."))
        .filter_map(|rest| rest.split_once('.').map(|(name, _)| name))
        .collect();
    if names.len() > COLORS_CUSTOM_MAX_BLOCKS {
        return Some(format!(
            "colors-custom: {} blocks defined — maximum is {COLORS_CUSTOM_MAX_BLOCKS} (remove unused palettes or split the config)",
            names.len()
        ));
    }
    None
}

#[cfg(test)]
#[path = "../../../../test/engine/chroma_dragon_engine/colors_custom/tests_strictness.rs"]
mod tests_strictness;
