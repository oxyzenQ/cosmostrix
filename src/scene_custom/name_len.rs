// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-depthtest-3: oversized scene-custom block-name validation —
//! extracted from `scene_custom/mod.rs` to keep that file under the
//! 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owner repro (2026-09-11): a complete `[scene-custom.<name>]` block
//! with a 65+-char name passed `--testconf` and never appeared in
//! `--list-scenes`. Root cause: the collector caps names at
//! [`SCENE_CUSTOM_MAX_NAME_LEN`] to bound BTreeMap keys, but the old
//! flow only SKIPPED the entry — the block became invisible to the
//! completeness validator (which iterates the collected map), the
//! listing printers, and the scene reference resolution, while the
//! raw-key reference scan in `field_validation` blessed it. The
//! silent skip is now fronted by a hard validation error on every
//! surface that calls `validate_scene_custom_completeness`:
//! `--testconf` (exit 2), startup (exit 2), and the live-reload
//! watcher (reject).

use std::collections::HashMap;

use super::{SCENE_CUSTOM_FIELDS, SCENE_CUSTOM_MAX_NAME_LEN, SCENE_CUSTOM_NAMESPACE};

/// Reject `scene-custom.<name>` blocks whose name exceeds
/// [`SCENE_CUSTOM_MAX_NAME_LEN`] (64 chars). Called as a pre-scan by
/// [`super::validate_scene_custom_completeness`] before the
/// completeness loop, so the length error names the block first.
///
/// Only keys shaped like the collector accepts are scanned
/// (`scene-custom.<name>.<known-field>`); malformed keys and unknown
/// fields keep their existing error paths. Sorted, deduplicated
/// iteration: the reported name is deterministic across hash seeds.
pub(super) fn validate_scene_custom_name_len(cfg: &HashMap<String, String>) -> Result<(), String> {
    let mut oversized: Vec<&str> = cfg
        .keys()
        .filter_map(|key| {
            // SCENE_CUSTOM_NAMESPACE is "scene-custom" WITHOUT the
            // separator dot — strip it explicitly so the extracted
            // name is not born with a leading dot (the collector's
            // split_once() equivalent).
            let rest = key.strip_prefix(SCENE_CUSTOM_NAMESPACE)?;
            let rest = rest.strip_prefix('.')?;
            let (name, field) = rest.rsplit_once('.')?;
            (name.len() > SCENE_CUSTOM_MAX_NAME_LEN && SCENE_CUSTOM_FIELDS.contains(&field))
                .then_some(name)
        })
        .collect();
    oversized.sort_unstable();
    oversized.dedup();
    let Some(&name) = oversized.first() else {
        return Ok(());
    };
    let mut msg = format!(
        "scene-custom name '{}' is {} chars — exceeds the {}-char name limit; oversized-name blocks are dropped before validation and listing, so the block would be invisible to --testconf and --list-scenes. Shorten the name.",
        truncate_name_for_error(name),
        name.chars().count(),
        SCENE_CUSTOM_MAX_NAME_LEN
    );
    if oversized.len() > 1 {
        msg.push_str(&format!(" (+{} more oversized names)", oversized.len() - 1));
    }
    Err(msg)
}

/// Render an over-limit block name for an error message without
/// flooding the terminal: names at or under the display cap are shown
/// verbatim; longer ones keep the first 24 chars plus an ellipsis.
/// Pure display — the char count is reported separately by the caller.
/// Shared by the block validator above and the CLI lookup-name error
/// in [`super::apply_scene_custom_layer`].
pub(super) fn truncate_name_for_error(name: &str) -> String {
    const NAME_ERROR_DISPLAY_CAP: usize = 24;
    if name.chars().count() <= NAME_ERROR_DISPLAY_CAP {
        name.to_string()
    } else {
        let head: String = name.chars().take(NAME_ERROR_DISPLAY_CAP).collect();
        format!("{head}...")
    }
}
