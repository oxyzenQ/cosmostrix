// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI arg canonicalization — extracted from `main.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns the `canonicalize_runtime_args()` helper: normalizes the
//! `-c`/`--color` argument to its canonical theme name (e.g. `green`
//! -> `green`, `grn` -> `green`) so config + verbose output use the
//! canonical form regardless of user input.
//!
//! Re-exported from `main.rs` via `pub(crate) use` so all existing
//! call sites continue to resolve unchanged.

use crate::config::Args;

/// Canonicalize the `--color` argument to its canonical theme name.
///
/// Skips canonicalization when `-c`/`--color` points to a custom
/// palette (not a built-in theme name). Custom names have no canonical
/// form — they are user-defined identifiers loaded from
/// `[colors-custom.<name>]` config blocks.
///
/// For built-in themes, the `theme::canonical_name_for_input` lookup
/// resolves aliases (`grn` -> `green`, `cyber` -> `cyberpunk`, etc.)
/// so the canonical name flows through to verbose output, config
/// diff traces, and the `--dump-config` report.
pub(crate) fn canonicalize_runtime_args(args: &mut Args) {
    // Skip canonicalization when -c/--color points to a custom palette
    // (not a built-in theme name). Custom names have no canonical form.
    if crate::colors_custom::is_colors_custom_name(
        &crate::configfile::load_config_file(args.config.as_deref()),
        &args.color,
    ) {
        return;
    }
    if let Some(canonical) = crate::theme::canonical_name_for_input(&args.color) {
        args.color = canonical.to_string();
    }
}
