// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-6 scene-bundle contract (v80.0.0-alpha.1, 2026-09-03).
//!
//! Owner bug: editing `scene = cinematic` into config on a
//! `cosmostrix --scene cosmos -C ascii -c neon-green` run applied only the
//! numeric fields of the new scene (fps/speed/density/glitch) while the
//! CLI-locked color/charset stuck — a half-applied scene. The contract:
//! a PRESENT `scene` key is a runtime trigger; the scene's complete
//! bundle outranks the CLI locks (same precedence the ambient apply path
//! and the scene-custom block already use). The CLI lock survives as the
//! FALLBACK layer when the key is commented back out.

use std::collections::HashMap;

use super::rebuild_cloud_config;
use super::tests::minimal_cloud_config;

/// The old contract (pre-HUNT-6: "CLI --speed wins over scene default")
/// produced the half-applied scene. Now the scene's managed defaults beat
/// the CLI locks whenever the `scene` key is present at runtime; an
/// explicit `speed`/`density`/`fps`/`glitch-level` config key (applied
/// AFTER the scene block) still outranks the scene default.
#[test]
fn rebuild_scene_key_applies_scene_speed_over_cli_lock_at_runtime() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "matrix".to_string());
    let mut base = minimal_cloud_config();
    base.cli_explicit.speed = true;
    base.cli_explicit.scene = false;
    base.speed = 25.0;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.speed, 18.0,
        "runtime scene key applies the scene's speed default over the CLI lock"
    );
}

/// The owner's exact repro shape — CLI `-c neon-green -C ascii` + config
/// `scene = cinematic`: the scene's own color AND charset must apply
/// (energy-zen + zen), not just the numeric fields.
#[test]
fn rebuild_scene_key_applies_full_bundle_over_cli_color_and_charset_locks() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cinematic".to_string());
    let mut base = minimal_cloud_config();
    base.cli_explicit.color = true;
    base.cli_explicit.charset = true;
    base.cli_explicit.colors_custom = true;
    base.color_scheme = crate::runtime::ColorScheme::NeonGreen;
    base.charset_preset = "ascii".to_string();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::EnergyZen,
        "the config scene key must beat the CLI -c lock at runtime (complete bundle)"
    );
    assert_eq!(
        new.charset_preset, "zen",
        "the config scene key must beat the CLI -C lock at runtime (complete bundle)"
    );
}

/// Layering guard: the config's OWN `color`/`charset` keys (applied BEFORE
/// the scene block) still outrank the scene's defaults — an explicit user
/// key is a stronger intent than the scene default.
#[test]
fn rebuild_explicit_color_and_charset_keys_beat_scene_defaults() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cinematic".to_string());
    cfg.insert("color".to_string(), "snow".to_string());
    cfg.insert("charset".to_string(), "retro".to_string());
    let base = minimal_cloud_config();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::Snow,
        "the explicit color key must beat the scene's color default"
    );
    assert_eq!(
        new.charset_preset, "retro",
        "the explicit charset key must beat the scene's charset default"
    );
}
