// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-depthtest-2 startup diagnostics extracted from
//! `config_apply.rs` (800-LOC cap, same pattern as
//! `config_apply_scene_glitch.rs`). Two layers:

//! 1. `startup_read_error` — an EXPLICIT `--config <path>` that cannot
//!    be read (missing, unreadable, oversized) is a hard error.
//!    Previously the load silently returned an empty map and
//!    cosmostrix ran on pure defaults — the user asked for a specific
//!    file, and a typo in the stem or the extension case produced a
//!    fully-silent wrong run. The default-path load is exempt by
//!    design: a missing ~/.config config is a normal first run and
//!    legitimately falls back (user-defaults -> /etc -> built-in
//!    defaults).
//! 2. `startup_duplicate_error` — duplicate keys and duplicate
//!    [section] headers. Real TOML rejects both with a hard parse
//!    error; the forgiving parser used to merge silently (sections)
//!    or take the last writer silently (keys) — a duplicate
//!    [scene-custom.x] / [colors-custom.x] / [charset-custom.x]
//!    block was indistinguishable from a single edited one, and a
//!    duplicate key value flip was invisible. Rejects in lockstep
//!    with --testconf and the live-reload watcher (the
//!    S-master-HUNT-2 uniform-rejection contract).

use crate::configfile::ParsedConfig;

use std::path::Path;

/// Layer 0: hard error when an explicit `--config` override could not
/// be read. `None` for the default-path load (fallback semantics
/// apply there by design) or when the read succeeded.
pub(crate) fn startup_read_error(
    config_override: Option<&Path>,
    parsed: &ParsedConfig,
) -> Option<String> {
    let read_err = parsed.read_error.as_ref()?;
    let configured = config_override
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(format!(
        "error: --config '{configured}': {read_err}\n  \
         An explicit --config path must point to an existing, readable TOML file.\n  \
         The default-path lookup (with /etc fallback) only applies when --config is\n  \
         omitted — check the path with: cosmostrix --config-path"
    ))
}

/// Layer 1: hard error on malformed lines (stray text without
/// 'key = value' syntax; first 3 reported).
pub(crate) fn startup_malformed_error(parsed: &ParsedConfig) -> Option<String> {
    if parsed.malformed_lines.is_empty() {
        return None;
    }
    let lines: Vec<&str> = parsed
        .malformed_lines
        .iter()
        .take(3)
        .map(String::as_str)
        .collect();
    Some(format!(
        "error: invalid config — malformed line(s): '{}' (expected 'key = value' syntax)\n\n  Fix the error above, or run 'cosmostrix --testconf' for details.",
        lines.join(", ")
    ))
}

/// Layer 2: hard error on unknown keys (likely typos; first 3
/// reported). Appends the targeted "did you mean" hints for
/// structural TOML mistakes (e.g. bold under [color.tune]) — the
/// depth-test fix from the pre-extraction inline code, unchanged.
pub(crate) fn startup_unknown_error(parsed: &ParsedConfig) -> Option<String> {
    if parsed.unknown_keys.is_empty() {
        return None;
    }
    let keys: Vec<&str> = parsed
        .unknown_keys
        .iter()
        .take(3)
        .map(String::as_str)
        .collect();
    let hints = crate::config_hints::format_hints_block(&parsed.unknown_keys);
    Some(format!(
        "error: invalid config — unknown key(s): '{}' (run 'cosmostrix --testconf' for known keys){hints}\n\n  Fix the error above, or run 'cosmostrix --testconf' for details.",
        keys.join(", ")
    ))
}

/// Layer 1.5: hard error on duplicate key / duplicate [section]
/// definitions (first 3 of each kind reported, mirroring the
/// malformed-line and unknown-key layers).
pub(crate) fn startup_duplicate_error(parsed: &ParsedConfig) -> Option<String> {
    if parsed.duplicate_keys.is_empty() && parsed.duplicate_sections.is_empty() {
        return None;
    }
    let mut dup_report: Vec<String> = Vec::new();
    for section in parsed.duplicate_sections.iter().take(3) {
        dup_report.push(format!("section [{section}] defined more than once"));
    }
    for key in parsed.duplicate_keys.iter().take(3) {
        dup_report.push(format!(
            "key '{key}' defined more than once (last value wins — remove the earlier line)"
        ));
    }
    Some(format!(
        "error: invalid config — duplicate definition(s): {}\n  \
         TOML does not allow redefining a key or a [section] — delete the duplicate line(s)/header(s).\n\n  \
         Fix the error above, or run 'cosmostrix --testconf' for details.",
        dup_report.join("; ")
    ))
}
