// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Auto-promote decision for the forgiving TOML parser.
//!
//! Extracted from `configfile.rs` to keep the parser under the 800-line
//! project LOC cap. This module owns the single decision: when a key
//! nested under a `[section]` header is unknown in scope, should the
//! parser silently re-home it to root scope (auto-promote) or surface
//! it as an `unknown_key` rejection?
//!
//! ## History
//!
//! v50.0.0-beta.6 FATAL FIX: stopped auto-promoting ANY key inside a
//! custom block (`charset-custom.*`, `colors-custom.*`, `scene-custom.*`)
//! to prevent silent side-effects like `color = green` inside
//! `[charset-custom.quantum]` changing the global color scheme.
//!
//! v80.0.0-beta.1 RELAXATION (owner bug report): the FATAL FIX was too
//! strict — it also blocked legitimate top-level NAMESPACED keys (e.g.
//! `ambient.01-50`, `colors-custom.X.Y`, `charset-custom.X.Y`) that the
//! user accidentally nested under the previous `[section]` header. The
//! owner hit this after uncommenting the template's
//! `[charset-custom.cyberpunk_2077]` block: the subsequent
//! `ambient.<HH-MM>` flat keys in the template got nested, producing
//! `unknown key(s): 'charset-custom.cyberpunk_2077.ambient.01-50'`.
//!
//! The fix: keep the SCALAR-key typo guard (a `color` / `bold` / `speed`
//! inside a custom block is still rejected as a typo'd field name) but
//! allow auto-promote of NAMESPACED keys (those containing a dot). A
//! dot-bearing key is unambiguously a top-level namespace prefix, not
//! a custom-block field name.

use crate::configfile::is_known_key;

/// Decide whether a nested key should be auto-promoted to root scope.
///
/// Returns `true` when ALL of the following hold:
///   1. We are inside a `[section]` header (current_section is non-empty).
///   2. The bare key (without the section prefix) is itself a recognized
///      top-level key (`is_known_key(&key)` returns `true`).
///   3. EITHER we are NOT inside a custom block, OR the bare key is
///      namespaced (contains a dot — e.g. `ambient.01-50`,
///      `colors-custom.X.Y`, `color.tune.bold`).
///
/// Returns `false` otherwise — the caller should push the full dotted
/// key to `unknown_keys` so `config_hints` can attach a targeted error
/// message.
///
/// # Arguments
///
/// * `current_section` — the active `[section]` header text (lowercased,
///   empty string when at root scope).
/// * `bare_key` — the un-prefixed key the user wrote (the LHS of
///   `key = value`, already lowercased by the caller).
#[inline]
#[must_use]
pub(crate) fn should_auto_promote(current_section: &str, bare_key: &str) -> bool {
    if current_section.is_empty() {
        return false;
    }
    if !is_known_key(bare_key) {
        return false;
    }
    let is_custom_block = current_section.starts_with("charset-custom.")
        || current_section.starts_with("colors-custom.")
        || current_section.starts_with("scene-custom.");
    // A namespaced key carries a top-level prefix dot, e.g.
    // `ambient.01-50`, `color.tune.bold`, `colors-custom.X.Y`,
    // `charset-custom.X.Y`, `scene-custom.X.Y`. Scalar keys like
    // `color`, `bold`, `speed` have no dot — they are field-name typos
    // when written inside a custom block.
    let bare_key_is_namespaced = bare_key.contains('.');
    !is_custom_block || bare_key_is_namespaced
}
