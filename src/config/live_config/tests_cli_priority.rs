// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Z-master-1-v2 regression tests: killer-features (colors-custom /
//! charset-custom / scene-custom) live-reload priority contract.
//!
//! Extracted from `live_config/tests.rs` to keep that file under the
//! 800-LOC hard cap (see `src/RULES_LOC.md`). Uses the same
//! `minimal_cloud_config` / `cfg2base` helpers via `super::tests`.

use super::tests::{cfg2base, minimal_cloud_config};
use super::*;

// ── Z-master-1-v2: killer-features live-reload priority hardening ──────

/// (Z1-1): `--speed` CLI explicit must survive a live-reload that
/// re-applies the active scene-custom block's speed field. Previously only
/// fps was gated (FPS-F4); speed/density/color/charset/glitch silently
/// overrode the CLI flag on every config edit.
#[test]
fn rebuild_scene_custom_speed_field_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.speed".to_string(),
        "40".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.speed = 12.0;
    base.cli_explicit.speed = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(new.speed, 12.0, "CLI --speed must win over the block field");
}

/// (Z1-1): same contract for the block's density field.
#[test]
fn rebuild_scene_custom_density_field_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.density".to_string(),
        "2.5".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.density = 0.75;
    base.cli_explicit.density = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.density, 0.75,
        "CLI --density must win over the block field"
    );
}

/// (Z1-1): same contract for the block's color field.
#[test]
fn rebuild_scene_custom_color_field_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.color".to_string(),
        "green".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.cli_explicit.color = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::NeonPurple,
        "CLI --color must win over the block field"
    );
}

/// (Z1-1): same contract for the block's charset field.
#[test]
fn rebuild_scene_custom_charset_field_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.charset".to_string(),
        "retro".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.cli_explicit.charset = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.charset_preset, "binary",
        "CLI --charset must win over the block field"
    );
}

/// (Z1-1): same contract for the block's glitch-level field (preset values
/// must not be re-derived over an explicit CLI flag).
#[test]
fn rebuild_scene_custom_glitch_field_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.glitch-level".to_string(),
        "intense".to_string(),
    );
    let mut base = cfg2base();
    base.cli_explicit.glitch_level = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.glitch_level,
        crate::config::GlitchLevel::None,
        "CLI --glitch-level must win over the block field"
    );
    assert!(!new.glitch_enabled);
}

/// (Z1-1): same contract for the block's colors-custom field — an explicit
/// `--color` flag must keep the block from swapping in a custom palette.
#[test]
fn rebuild_scene_custom_colors_custom_field_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert("colors-custom.p1.bg".to_string(), "#0a0a0a".to_string());
    cfg.insert(
        "colors-custom.p1.rain".to_string(),
        "#00ff41,#00b32d,#005c17".to_string(),
    );
    cfg.insert(
        "scene-custom.test-scene.colors-custom".to_string(),
        "p1".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.custom_palette = None;
    base.custom_palette_name = None;
    base.cli_explicit.color = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.custom_palette.is_none(),
        "explicit --color must block the block's colors-custom field"
    );
}

/// (Z1-2): conflict determinism — when a block defines BOTH `color` and
/// `colors-custom`, startup keeps the scheme and never loads the palette
/// (`apply_profile_overrides` skips colors-custom when color is present).
/// The live-reload field layer must match: no palette, scheme applied.
#[test]
fn rebuild_scene_custom_block_color_beats_colors_custom() {
    let mut cfg = HashMap::new();
    cfg.insert("colors-custom.p1.bg".to_string(), "#0a0a0a".to_string());
    cfg.insert(
        "colors-custom.p1.rain".to_string(),
        "#00ff41,#00b32d,#005c17".to_string(),
    );
    cfg.insert(
        "scene-custom.test-scene.color".to_string(),
        "green".to_string(),
    );
    cfg.insert(
        "scene-custom.test-scene.colors-custom".to_string(),
        "p1".to_string(),
    );
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::Green,
        "block color field must apply"
    );
    assert!(
        new.custom_palette.is_none(),
        "block colors-custom must be skipped when color is present (startup parity)"
    );
}

/// (Z1-2): conflict determinism — `charset` beats `charset-custom` inside a
/// block (mirrors the startup skip rule in `apply_profile_overrides`).
#[test]
fn rebuild_scene_custom_block_charset_beats_charset_custom() {
    let mut cfg = HashMap::new();
    cfg.insert("charset-custom.zen2.set".to_string(), "ABCDEF".to_string());
    cfg.insert(
        "scene-custom.test-scene.charset".to_string(),
        "retro".to_string(),
    );
    cfg.insert(
        "scene-custom.test-scene.charset-custom".to_string(),
        "zen2".to_string(),
    );
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.charset_preset, "retro",
        "block charset field must win over charset-custom (startup parity)"
    );
    assert_eq!(
        new.chars,
        crate::charset::build_chars(
            crate::charset::Charset::BOXDRAW,
            &new.user_ranges,
            new.def_ascii
        ),
        "chars must come from the charset field, not charset-custom"
    );
}

/// (Z1-3): base-scene inheritance at live-reload must respect CLI flags
/// (mirrors startup `apply_base_scene_to_args` is_explicit checks and the
/// FPS-F4 gate). `--speed` must survive the base-scene defaults re-apply.
#[test]
fn rebuild_base_scene_speed_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.base-scene".to_string(),
        "cinematic".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.speed = 20.0;
    base.cli_explicit.speed = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.speed, 20.0,
        "CLI --speed must win over base-scene default (cinematic speed = 9.0)"
    );
}

/// (Z1-3): base-scene color default must not override an explicit CLI
/// color while the block re-applies.
#[test]
fn rebuild_base_scene_color_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.base-scene".to_string(),
        "cinematic".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.cli_explicit.color = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::NeonPurple,
        "CLI --color must win over base-scene default color"
    );
}

/// (Z1-3): base-scene glitch preset must respect an explicit CLI
/// glitch-level.
#[test]
fn rebuild_base_scene_glitch_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.base-scene".to_string(),
        "cinematic".to_string(),
    );
    let mut base = cfg2base();
    base.cli_explicit.glitch_level = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.glitch_level,
        crate::config::GlitchLevel::None,
        "CLI --glitch-level must win over base-scene glitch preset"
    );
    assert!(!new.glitch_enabled);
}

/// (Z1-4): switching the config `scene` key away from a palette-owning
/// custom scene must clear the stale custom palette — create_cloud applies
/// the palette after the scheme, so a lingering palette made the scene
/// switch a visual no-op for color.
#[test]
fn rebuild_scene_switch_clears_stale_custom_palette() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cinematic".to_string());
    let mut base = minimal_cloud_config();
    base.custom_palette = Some(crate::palette::Palette {
        colors: vec![crossterm::style::Color::Rgb {
            r: 0,
            g: 255,
            b: 65,
        }],
        bg: None,
    });
    base.custom_palette_name = Some("p1".to_string());
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.custom_palette.is_none(),
        "stale custom palette must be cleared on scene switch"
    );
    assert!(new.custom_palette_name.is_none());
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::EnergyZen,
        "cinematic's energy-zen scheme must actually take effect"
    );
}

// ── Z-master-2-v2: CLI intent preservation for config keys ────────────

/// (Z2-1, v80.0.0-beta.1 rewrite): config `bold` key PRESENT wins over the CLI
/// `--bold` lock at runtime (temporal precedence). The CLI value is the
/// fallback when the key is commented out — pinned in
/// tests_cli_fallback.rs (`fallback_bold_key_absent_keeps_cli_lock`).
#[test]
fn rebuild_bold_key_overrides_cli_lock_when_present() {
    let mut cfg = HashMap::new();
    cfg.insert("bold".to_string(), "2".to_string());
    let mut base = minimal_cloud_config();
    base.bold_mode = crate::runtime::BoldMode::Off;
    base.cli_explicit.bold = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.bold_mode,
        crate::runtime::BoldMode::All,
        "config bold key present must override the CLI --bold lock (v80.0.0-beta.1)"
    );
}

/// (Z2-1, v80.0.0-beta.1 rewrite): config `shading-mode` key PRESENT wins over
/// the CLI lock at runtime; the CLI value is the fallback on absence.
#[test]
fn rebuild_shading_mode_key_overrides_cli_lock_when_present() {
    let mut cfg = HashMap::new();
    cfg.insert("shading-mode".to_string(), "1".to_string());
    let mut base = minimal_cloud_config();
    base.shading_mode = crate::runtime::ShadingMode::Random;
    base.cli_explicit.shading_mode = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.shading_mode,
        crate::runtime::ShadingMode::DistanceFromHead,
        "config shading-mode key present must override the CLI lock (v80.0.0-beta.1)"
    );
}

/// (Z2-1, v80.0.0-beta.1 rewrite): config `color-bg` key PRESENT wins over the
/// CLI lock at runtime; the CLI value is the fallback on absence.
#[test]
fn rebuild_color_bg_key_overrides_cli_lock_when_present() {
    let mut cfg = HashMap::new();
    cfg.insert("color-bg".to_string(), "black".to_string());
    let mut base = minimal_cloud_config();
    base.default_bg = true;
    base.cli_explicit.color_bg = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        !new.default_bg,
        "config color-bg key present must override the CLI --color-bg lock (v80.0.0-beta.1)"
    );
}

/// (Z2-2, v80.0.0-beta.1 rewrite): a config `color` key switching to a builtin
/// now CLEARS a CLI-owned custom palette (`--colors-custom`) — the key
/// is the most recent user intent. The palette RETURNS when the key is
/// commented back out (base carries the locked palette) — pinned in
/// tests_cli_fallback.rs (`fallback_color_key_absent_restores_cli_palette`).
/// Startup still checks `--colors-custom` FIRST (CLI > config at
/// startup — the temporal inversion is runtime-only).
#[test]
fn rebuild_color_key_clears_cli_colors_custom_palette_when_present() {
    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "snow".to_string());
    let mut base = minimal_cloud_config();
    base.custom_palette = Some(crate::palette::Palette {
        colors: vec![crossterm::style::Color::Rgb {
            r: 0,
            g: 255,
            b: 65,
        }],
        bg: None,
    });
    base.custom_palette_name = Some("p1".to_string());
    base.cli_explicit.colors_custom = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert!(
        new.custom_palette.is_none(),
        "config color key present must clear the CLI-owned palette (v80.0.0-beta.1)"
    );
    assert_eq!(new.color_scheme, crate::runtime::ColorScheme::Snow);
}

/// (Z2-3, v80.0.0-beta.1 rewrite): a config `scene` key PRESENT now replaces the
/// CLI-selected custom scene (`--scene-custom`) — the key is the most
/// recent user intent. The custom scene RETURNS when the key is
/// commented back out (base.scene_custom_name is never cleared — pinned
/// in tests_cli_fallback.rs,
/// `fallback_scene_key_absent_restores_cli_scene_custom`).
#[test]
fn rebuild_scene_key_replaces_cli_scene_custom_when_present() {
    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cinematic".to_string());
    let mut base = minimal_cloud_config();
    base.cli_explicit.scene_custom = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.scene_name, "cinematic",
        "config scene key present must replace the CLI --scene-custom selection (v80.0.0-beta.1)"
    );
    assert_eq!(
        new.scene_custom_name, None,
        "switching to a builtin scene clears the custom-scene tracker (startup parity)"
    );
    assert_eq!(
        new.color_scheme,
        crate::runtime::ColorScheme::EnergyZen,
        "cinematic's energy-zen default applies"
    );
}

/// (Z2-1): scene-custom block `bold` field must respect `--bold`.
#[test]
fn rebuild_scene_custom_bold_field_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert("scene-custom.test-scene.bold".to_string(), "2".to_string());
    let mut base = minimal_cloud_config();
    base.bold_mode = crate::runtime::BoldMode::Off;
    base.cli_explicit.bold = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.bold_mode,
        crate::runtime::BoldMode::Off,
        "CLI --bold must win over the block's bold field"
    );
}

/// (Z2-1): scene-custom block `shading-mode` field must respect
/// `--shading-mode`.
#[test]
fn rebuild_scene_custom_shading_field_respects_cli_explicit() {
    let mut cfg = HashMap::new();
    cfg.insert(
        "scene-custom.test-scene.shading-mode".to_string(),
        "1".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.shading_mode = crate::runtime::ShadingMode::Random;
    base.cli_explicit.shading_mode = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.shading_mode,
        crate::runtime::ShadingMode::Random,
        "CLI --shading-mode must win over the block's shading-mode field"
    );
}

/// (Z2-1): scene-custom block `colors-custom` field must respect
/// `--colors-custom` (the CLI-owned palette is not replaced by the block's
/// palette reference on re-apply).
#[test]
fn rebuild_scene_custom_colors_custom_field_respects_colors_custom_flag() {
    let mut cfg = HashMap::new();
    cfg.insert("colors-custom.p1.bg".to_string(), "#0a0a0a".to_string());
    cfg.insert(
        "colors-custom.p1.rain".to_string(),
        "#00ff41,#00b32d,#005c17".to_string(),
    );
    cfg.insert("colors-custom.p2.bg".to_string(), "#000022".to_string());
    cfg.insert(
        "colors-custom.p2.rain".to_string(),
        "#41b6ff,#2d81b6,#17532d".to_string(),
    );
    cfg.insert(
        "scene-custom.test-scene.colors-custom".to_string(),
        "p2".to_string(),
    );
    let mut base = minimal_cloud_config();
    base.custom_palette = Some(crate::palette::Palette {
        colors: vec![crossterm::style::Color::Rgb {
            r: 0,
            g: 255,
            b: 65,
        }],
        bg: None,
    });
    base.custom_palette_name = Some("p1".to_string());
    base.cli_explicit.colors_custom = true;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.custom_palette_name.as_deref(),
        Some("p1"),
        "CLI --colors-custom palette must win over the block's palette reference"
    );
}

/// (Z2-4): build_cli_explicit must track the five new flags from real
/// argv (bold, shading-mode, color-bg, colors-custom, scene-custom).
#[test]
fn build_cli_explicit_tracks_z_master_2_v2_flags() {
    use crate::config::Args;
    let matches = <Args as clap::CommandFactory>::command()
        .try_get_matches_from([
            "cosmostrix",
            "--bold",
            "0",
            "--shading-mode",
            "1",
            "--color-bg",
            "black",
            "--colors-custom",
            "p1",
            "--scene-custom",
            "my-scene",
        ])
        .unwrap();
    let (_, cli) = crate::cli::cli_explicit::build_cli_explicit(&matches);
    assert!(cli.bold, "--bold must be tracked");
    assert!(cli.shading_mode, "--shading-mode must be tracked");
    assert!(cli.color_bg, "--color-bg must be tracked");
    assert!(cli.colors_custom, "--colors-custom must be tracked");
    assert!(cli.scene_custom, "--scene-custom must be tracked");
}

/// (Z2-4): the five new flags must NOT be flagged explicit when absent
/// from argv (default clap value sources must not leak into the tracker).
#[test]
fn build_cli_explicit_absent_flags_stay_false() {
    use crate::config::Args;
    let matches = <Args as clap::CommandFactory>::command()
        .try_get_matches_from(["cosmostrix"])
        .unwrap();
    let (_, cli) = crate::cli::cli_explicit::build_cli_explicit(&matches);
    assert!(!cli.bold);
    assert!(!cli.shading_mode);
    assert!(!cli.color_bg);
    assert!(!cli.colors_custom);
    assert!(!cli.scene_custom);
}
