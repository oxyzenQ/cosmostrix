// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-beta.2 custom-name parity + fps-intent + CLI-priority tests.
//! Extracted from `config_apply_tests/mod.rs` to keep that source file
//! under the 800-LOC cap (see `src/RULES_LOC.md`).

#![cfg(test)]

use super::args_with_config;

// ── v80.0.0-beta.2 fps user-intent tracking ──────────────────────────────
// Owner bug (2026-09-02): an explicit `fps = 60` from config.toml or a
// scene-custom block was silently stomped by the dynamic default (144 on
// high-perf terminals) because `fps_user_set = args.fps != 60.0` cannot
// distinguish an explicit 60 from the clap default. The tracker records
// the config-layer intent; main.rs consults it before applying the
// dynamic default. These tests are label-agnostic (assert is_some, not
// the exact source) because the tracker is process-global and tests run
// in parallel — any other fps-setting test writes it too.

#[test]
fn config_fps_60_records_explicit_intent() {
    let args = args_with_config("fps = 60\n", &[]);
    assert_eq!(args.fps, 60.0);
    assert!(
        crate::fps_explicit_source().is_some(),
        "an explicit config fps (including exactly 60) must record intent \
         so the dynamic default cannot stomp it"
    );
}

#[test]
fn scene_custom_fps_60_records_explicit_intent() {
    let config = "[scene-custom.cp77]\nbase-scene = \"storm\"\nfps = 60\n";
    let args = args_with_config(config, &["--scene-custom", "cp77"]);
    assert_eq!(args.fps, 60.0);
    assert!(
        crate::fps_explicit_source().is_some(),
        "a scene-custom block fps (including exactly 60) must record \
         intent — the owner's cp77 showed tgt: 144 on a high-refresh HUD"
    );
}

#[test]
fn scene_custom_fps_value_applies_over_base_scene() {
    // The block's own fps (60) must win over the base-scene's (storm 120)
    // — the base layer applies first, the field layer second.
    let config = "[scene-custom.cp77]\nbase-scene = \"storm\"\nfps = 60\n";
    let args = args_with_config(config, &["--scene-custom", "cp77"]);
    assert_eq!(args.fps, 60.0);
}

// ── v80.0.0-beta.2 CLI priority (Z1-1 startup parity) ────────────────────

#[test]
fn cli_color_wins_over_scene_custom_colors_custom() {
    // An explicit `-c cosmos` must survive a scene-custom block whose
    // colors-custom references a custom palette — the live-reload path
    // (apply_scene_custom_field_to_cloud_config) has always gated on
    // cli_explicit.color; the startup path documented the same layering
    // but never implemented the gate.
    let config =
        "[scene-custom.cp77]\nbase-scene = \"storm\"\ncolors-custom = \"cyberpunk_2077\"\n\
[colors-custom.cyberpunk_2077]\nbg = \"#0a0a12\"\nrain = \"#00fff7,#ff003c\"\n";
    let args = args_with_config(config, &["--scene-custom", "cp77", "-c", "cosmos"]);
    assert!(
        args.colors_custom.is_none(),
        "explicit -c/--color must block the scene-custom block's palette reference"
    );
    assert_eq!(args.color, "cosmos");
}

#[test]
fn scene_custom_colors_custom_applies_without_cli_color() {
    // Without an explicit -c, the block's palette reference flows into
    // args.colors_custom (the pre-beta.2 behavior — unchanged).
    let config =
        "[scene-custom.cp77]\nbase-scene = \"storm\"\ncolors-custom = \"cyberpunk_2077\"\n\
[colors-custom.cyberpunk_2077]\nbg = \"#0a0a12\"\nrain = \"#00fff7,#ff003c\"\n";
    let args = args_with_config(config, &["--scene-custom", "cp77"]);
    assert_eq!(
        args.colors_custom.as_deref(),
        Some("cyberpunk_2077"),
        "block palette reference applies when no CLI color is explicit"
    );
}

// ── v80.0.0-beta.2 custom-name config acceptance (owner fatal bug) ───────

#[test]
fn config_scene_custom_name_is_accepted_at_startup_apply() {
    // `scene = hacker-mode` in config.toml must resolve (the fatal
    // "unknown scene" startup error blocked every launch, including CLI
    // overrides, while the runtime path accepted the name).
    // v80.0.0-beta.2: the block is a complete seven-dimension profile
    // (rain first — NIGHT-research-5)
    // (base-scene removed; completeness is required).
    let config = "scene = \"hacker-mode\"\n[scene-custom.hacker-mode]\n\
color = \"green\"\ncharset = \"hacker\"\nfps = 60\nspeed = 28\n\
density = 1.2\nglitch-level = \"intense\"\n";
    let args = args_with_config(config, &[]);
    assert_eq!(args.scene.as_deref(), Some("hacker-mode"));
    assert_eq!(args.scene_custom.as_deref(), Some("hacker-mode"));
}

#[test]
fn config_color_custom_name_is_accepted_at_startup_apply() {
    // `color = test` referencing [colors-custom.test] must resolve —
    // charset had this acceptance since v25; color was asymmetrically
    // rejected with a misleading hint (owner report 2026-09-02).
    let config =
        "color = \"test\"\n[colors-custom.test]\nbg = \"#0a0a0a\"\nrain = \"#1a0033,#ffffff\"\n";
    let args = args_with_config(config, &[]);
    assert_eq!(args.color, "test");
}
