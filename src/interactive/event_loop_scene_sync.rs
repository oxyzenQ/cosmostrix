// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime scene synchronization for live-reload — extracted from
//! `event_loop.rs` to keep that file under the 800-LOC cap.
//!
//! Owns the `sync_base_cfg_with_runtime_scene()` helper: a pure function
//! that re-applies a scene's managed defaults (color, charset, speed,
//! density, rain_style) onto the base `CloudConfig` before
//! `rebuild_cloud_config` layers user config on top.
//!
//! Called from the live-reload path in `event_loop.rs` whenever the
//! runtime scene name diverges from `base_cfg.scene_name` (e.g. the user
//! pressed `x` to cycle scenes, then edited `config.toml` — the rebuild
//! must respect the new scene's defaults before applying config overrides).

use crate::CloudConfig;

/// Sync `base_cfg` with the runtime scene's managed defaults.
///
/// v50.0.0-beta.6 masterclass: called before `rebuild_cloud_config` so
/// config overrides layer on top of scene defaults (user wins: editing
/// `color` changes only color, not the whole scene profile).
///
/// Behavior:
/// 1. If `base_cfg.scene_name == scene_name`, returns immediately (no-op
///    — already synced). This is the common case on the first rebuild
///    after startup (scene hasn't changed).
/// 2. Updates `base_cfg.scene_name` to the new scene.
/// 3. Looks up the scene via `crate::scene::get_scene`. If not found
///    (custom scene deleted from config mid-session), returns without
///    applying defaults — the previous values persist.
/// 4. For each managed field the scene defines (`color`, `charset`,
///    `speed`, `density`, `rain_style`), overwrites the corresponding
///    `base_cfg` field. Fields the scene leaves `None` are NOT touched
///    (preserves user config values for those dimensions).
///
/// Charset handling: when the scene defines `charset`, both
/// `base_cfg.charset_preset` (the name) and `base_cfg.chars` (the
/// resolved glyph Vec) are updated — the latter via
/// `charset::build_chars` using `base_cfg.user_ranges` + `base_cfg.def_ascii`
/// so the user's custom ranges + ASCII-fallback flag are respected.
///
/// Parameters:
/// - `base_cfg`: mutable reference to the CloudConfig used as the rebuild
///   base (cloned from the startup `cfg` at the top of `run_interactive`).
/// - `scene_name`: the runtime scene name (from the event loop's
///   `scene_name` local, which is updated by `x`/`C`/ambient fires).
pub(super) fn sync_base_cfg_with_runtime_scene(base_cfg: &mut CloudConfig, scene_name: &str) {
    if base_cfg.scene_name == scene_name {
        return;
    }
    base_cfg.scene_name = scene_name.to_string();
    let Some(scene_info) = crate::scene::get_scene(scene_name) else {
        return;
    };
    let sc = scene_info.config;
    if let Some(color) = sc.color {
        if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
            base_cfg.color_scheme = scheme;
        }
    }
    if let Some(charset_name) = sc.charset {
        base_cfg.charset_preset = charset_name.to_string();
        if let Ok(charset) = crate::charset::charset_from_str(charset_name, base_cfg.def_ascii) {
            base_cfg.chars =
                crate::charset::build_chars(charset, &base_cfg.user_ranges, base_cfg.def_ascii);
        }
    }
    if let Some(speed) = sc.speed {
        base_cfg.speed = speed;
    }
    if let Some(density) = sc.density {
        base_cfg.base_density = density;
    }
    base_cfg.rain_style = sc.rain_style;
}
