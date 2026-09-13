// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-37: custom-block header completeness validation.
//!
//! Owner mandate (2026-09-13): "sections custom blocks should complete
//! don't missing one or overloads/duplicate, and the value should not
//! empty, each custom name max is 64 char". The key-level validators
//! (scene-custom completeness, the colors-custom load contract,
//! charset-custom value checks) only ever see blocks that produced at
//! least ONE key — a block that opens with a header and defines
//! nothing (every field line commented out or missing) is invisible
//! to all of them. The parser now records every custom-block header
//! in `ParsedConfig::custom_block_headers`, and this validator asks
//! the missing question directly: does the block have its required
//! fields?
//!
//! Contract (uniform rejection, all three surfaces — `--testconf`,
//! startup, live-reload watcher):
//! - `[charset-custom.<name>]` must define `set` (an empty `set`
//!   value is already a hard error in the value layer).
//! - `[colors-custom.<name>]` must define BOTH `bg` and `rain` (the
//!   deprecated `stops` alias satisfies the rain slot; the
//!   rain+stops overload is rejected by `colors_custom::strictness`).
//! - `[scene-custom.<name>]` must define at least one field — the
//!   seven-dimension completeness validator owns the per-field
//!   reporting once a block has keys, so a header-only scene block
//!   is reported here as missing all seven dimensions.
//! - The block name must be non-empty, valid
//!   (`[A-Za-z0-9_-]` — the shared `is_valid_custom_name` rule), and
//!   at most 64 chars (the NIGHT-depthtest-3 limit; a header-only
//!   oversized block was invisible to the key-scanning name-length
//!   validators).
//!
//! A block written as flat dotted keys at root (no `[section]`
//! header) needs no header to be complete — it is outside this
//! validator's scope (the key-level validators own it).

use std::collections::HashMap;

use crate::charset_custom::CHARSET_CUSTOM_MAX_NAME_LEN;
use crate::colors_custom::COLORS_CUSTOM_MAX_NAME_LEN;
use crate::configfile::is_valid_custom_name;
use crate::scene_custom::SCENE_CUSTOM_MAX_NAME_LEN;

/// Validate every recorded custom-block header against the config
/// map: name shape first, then field completeness. Headers arrive
/// sorted from `ParsedConfig::custom_block_headers_from`, so the
/// first reported block is deterministic across hash seeds.
///
/// Returns `None` when every header passes (or none exist), `Some(msg)`
/// carrying the first violation.
#[must_use]
pub(crate) fn validate_custom_block_headers(
    headers: &[String],
    cfg: &HashMap<String, String>,
) -> Option<String> {
    if headers.is_empty() {
        return None;
    }
    for header in headers {
        let (namespace, name) = match header.split_once('.') {
            Some((ns, rest)) => (ns, rest),
            None => (header.as_str(), ""),
        };
        if let Some(msg) = validate_block_name(namespace, name) {
            return Some(msg);
        }
        match namespace {
            "charset-custom" => {
                if !cfg.contains_key(&format!("charset-custom.{name}.set")) {
                    return Some(format!(
                        "charset-custom '{name}': block is incomplete — the required field 'set' is missing (a [charset-custom.<name>] block must define set = \"...\")"
                    ));
                }
            }
            "colors-custom" => {
                let has_bg = cfg.contains_key(&format!("colors-custom.{name}.bg"));
                let has_rain = cfg.contains_key(&format!("colors-custom.{name}.rain"))
                    || cfg.contains_key(&format!("colors-custom.{name}.stops"));
                if !has_bg || !has_rain {
                    let mut missing: Vec<&str> = Vec::new();
                    if !has_bg {
                        missing.push("bg");
                    }
                    if !has_rain {
                        missing.push("rain");
                    }
                    return Some(format!(
                        "colors-custom '{name}': block is incomplete — missing {} (a [colors-custom.<name>] block must define BOTH bg and rain)",
                        missing.join(" and ")
                    ));
                }
            }
            "scene-custom" => {
                let prefix = format!("scene-custom.{name}.");
                if !cfg.keys().any(|k| k.starts_with(&prefix)) {
                    // Header-only scene block: every dimension is
                    // missing. Once a block has at least one key, the
                    // seven-dimension completeness validator
                    // (scene_custom) owns the per-field reporting.
                    return Some(format!(
                        "scene-custom '{name}' is incomplete: missing {} — a [scene-custom.<name>] block must be COMPLETELY filled; incomplete blocks are rejected",
                        crate::scene_custom::scene_custom_required_fields_hint()
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

/// Shared name-shape checks for a custom-block header: non-empty,
/// valid characters, at most the namespace's 64-char limit (all three
/// namespaces share 64 — the per-namespace constant is still named
/// so the contract stays explicit if one ever diverges).
fn validate_block_name(namespace: &str, name: &str) -> Option<String> {
    if name.is_empty() {
        return Some(format!(
            "malformed custom block header '[{namespace}]' — the block name is missing (expected [{namespace}.<name>])"
        ));
    }
    let max = match namespace {
        "charset-custom" => CHARSET_CUSTOM_MAX_NAME_LEN,
        "colors-custom" => COLORS_CUSTOM_MAX_NAME_LEN,
        "scene-custom" => SCENE_CUSTOM_MAX_NAME_LEN,
        _ => return None,
    };
    let chars = name.chars().count();
    if chars > max {
        return Some(format!(
            "custom block name '{name}' is {chars} chars — over the {max}-char limit, so the block can never load (shorten the name)"
        ));
    }
    if !is_valid_custom_name(name) {
        return Some(format!(
            "invalid custom block name '{name}' in [{namespace}.{name}] (allowed: letters, digits, '-', '_')"
        ));
    }
    None
}

#[cfg(test)]
#[path = "../../test/testconf/tests_custom_block_headers.rs"]
mod tests;
