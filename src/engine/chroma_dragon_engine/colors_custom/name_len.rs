// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-depthtest-3: oversized colors-custom block-name validation —
//! extracted from `colors_custom.rs` to keep that file under the
//! 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! The collector drops names longer than
//! [`super::COLORS_CUSTOM_MAX_NAME_LEN`], which made a perfectly valid
//! palette block invisible to `--testconf` (PASS) and `--list-colors`
//! (never listed) — the same silent-skip blind spot the owner hit on
//! scene-custom. `validate_colors_custom_blocks` front-loads this
//! pre-scan so every surface that calls it (`--testconf`, startup,
//! live-reload watcher) rejects oversized names in lockstep.

use std::collections::HashMap;

use super::COLORS_CUSTOM_MAX_NAME_LEN;

/// Reject `colors-custom.<name>` blocks whose name exceeds
/// [`COLORS_CUSTOM_MAX_NAME_LEN`] (64 chars). Only keys shaped like
/// the collector accepts are scanned (`colors-custom.<name>.<bg
/// |rain|stops>`); unknown fields keep their existing unknown-key
/// error path. Sorted + deduplicated for a deterministic
/// first-error contract across hash seeds.
pub(super) fn validate_colors_custom_name_len(cfg: &HashMap<String, String>) -> Option<String> {
    let mut oversized: Vec<&str> = cfg
        .keys()
        .filter_map(|key| {
            let rest = key.strip_prefix("colors-custom.")?;
            let (name, field) = rest.split_once('.')?;
            let known_field = matches!(field, "bg" | "rain" | "stops");
            (name.len() > COLORS_CUSTOM_MAX_NAME_LEN && known_field).then_some(name)
        })
        .collect();
    oversized.sort_unstable();
    oversized.dedup();
    let name = *oversized.first()?;
    let mut msg = format!(
        "colors-custom name '{}...' is {} chars — exceeds the {}-char name limit; oversized-name blocks are dropped before validation and listing, so the palette would be invisible to --testconf and --list-colors. Shorten the name.",
        name.chars().take(24).collect::<String>(),
        name.chars().count(),
        COLORS_CUSTOM_MAX_NAME_LEN
    );
    if oversized.len() > 1 {
        msg.push_str(&format!(" (+{} more oversized names)", oversized.len() - 1));
    }
    Some(msg)
}

#[cfg(test)]
mod tests {
    use super::super::validate_colors_custom_blocks;
    use super::COLORS_CUSTOM_MAX_NAME_LEN;
    use std::collections::HashMap;

    #[test]
    fn block_validation_rejects_oversized_name() {
        // A valid 2-stop palette under a 65+-char name used to pass
        // --testconf silently (the collector dropped the block before
        // the validator iterated it). The raw-key pre-scan must flag
        // it with the 64-char limit.
        let mut cfg = HashMap::new();
        let long_name = "x".repeat(COLORS_CUSTOM_MAX_NAME_LEN + 1);
        cfg.insert(
            format!("colors-custom.{long_name}.rain"),
            "#000000, #ffffff".to_string(),
        );
        let err = validate_colors_custom_blocks(&cfg).expect("must reject");
        assert!(
            err.contains("exceeds the 64-char name limit"),
            "must name the limit: {err}"
        );
    }

    #[test]
    fn block_validation_accepts_boundary_64_char_name() {
        let mut cfg = HashMap::new();
        let name = "a".repeat(COLORS_CUSTOM_MAX_NAME_LEN);
        cfg.insert(
            format!("colors-custom.{name}.rain"),
            "#000000, #ffffff".to_string(),
        );
        assert!(
            validate_colors_custom_blocks(&cfg).is_none(),
            "a 64-char name with a valid palette must pass"
        );
    }
}
