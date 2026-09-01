// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v51.1 CLI-locked fallback contract (owner directive, 2026-09-01).
//!
//! Owner repro: `cosmostrix -v -s -C minimal --scene crystal-dragon
//! -mfs words` + runtime config.toml edit `# scene = cinematic` →
//! `scene = cinematic` (live-reload works — good) → back to
//! `# scene = cinematic` (engine STAYED on cinematic — premature logic).
//!
//! The masterclass contract:
//!
//! ```text
//! Startup:  CLI > config.toml > scene defaults > built-in defaults
//! Runtime:  config key > CLI lock > scene defaults > built-in defaults
//! ```
//!
//! At runtime an explicit config key overrides the CLI flag (the file edit
//! is the most recent user intent) — but the CLI value stays LOCKED
//! underneath: when the key is commented out / removed, the engine falls
//! back to the locked startup value. The tests below pin the fallback for
//! every key family (scene e2e lives in
//! interactive::event_loop_scene_sync::tests — it needs the
//! restore/sync helpers; these tests pin the rebuild-level behavior).
//!
//! Uses the same `minimal_cloud_config` helper via `super::tests`.

use super::tests::minimal_cloud_config;
use super::*;

/// Sequence fallback: fps key present wins, key absent reverts to the
/// locked startup value (the base is never contaminated — fps is not a
/// scene-sync-managed field, so no restore is needed).
#[test]
fn fallback_fps_key_absent_keeps_cli_lock() {
    let mut base = minimal_cloud_config();
    base.target_fps = 30.0; // CLI --fps 30 (locked)
    base.cli_explicit.fps = true;

    let mut cfg = HashMap::new();
    cfg.insert("fps".to_string(), "60".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert_eq!(with_key.target_fps, 60.0, "config fps key present wins");

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert_eq!(
        after_comment_out.target_fps, 30.0,
        "commenting the fps key out must fall back to the CLI-locked value"
    );
}

/// Sequence fallback for `bold`: config key present wins, absent reverts
/// to the locked startup bold mode.
#[test]
fn fallback_bold_key_absent_keeps_cli_lock() {
    let mut base = minimal_cloud_config();
    base.bold_mode = crate::runtime::BoldMode::Off; // CLI --bold 0
    base.cli_explicit.bold = true;

    let mut cfg = HashMap::new();
    cfg.insert("bold".to_string(), "2".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert_eq!(with_key.bold_mode, crate::runtime::BoldMode::All);

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert_eq!(
        after_comment_out.bold_mode,
        crate::runtime::BoldMode::Off,
        "commenting the bold key out must fall back to the CLI-locked value"
    );
}

/// Sequence fallback for a CLI-owned custom palette (`--colors-custom`):
/// a config `color` key clears it (present wins), commenting the key back
/// out restores the locked palette — base carries it for the whole session.
#[test]
fn fallback_color_key_absent_restores_cli_palette() {
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

    let mut cfg = HashMap::new();
    cfg.insert("color".to_string(), "snow".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert!(
        with_key.custom_palette.is_none(),
        "config color key clears the palette"
    );

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert!(
        after_comment_out.custom_palette.is_some(),
        "commenting the color key out must restore the CLI-locked palette"
    );
    assert_eq!(after_comment_out.custom_palette_name.as_deref(), Some("p1"));
}

/// Sequence fallback for `--scene-custom`: a config `scene` key replaces
/// the selection (present wins), commenting the key back out restores it —
/// base.scene_custom_name is never cleared, so the tail block re-applies.
#[test]
fn fallback_scene_key_absent_restores_cli_scene_custom() {
    let mut base = minimal_cloud_config();
    // minimal_cloud_config() already models a CLI-locked custom scene:
    // scene_name = "test-scene" + scene_custom_name = Some("test-scene").
    base.cli_explicit.scene_custom = true;

    let mut cfg = HashMap::new();
    cfg.insert("scene".to_string(), "cinematic".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert_eq!(with_key.scene_name, "cinematic");
    assert_eq!(with_key.scene_custom_name, None);

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert_eq!(
        after_comment_out.scene_name, "test-scene",
        "commenting the scene key out must restore the CLI-locked custom scene"
    );
    assert_eq!(
        after_comment_out.scene_custom_name.as_deref(),
        Some("test-scene"),
        "the custom-scene tracker must survive so the field layer re-applies"
    );
}

/// Sequence fallback for a CLI `-m` message: a config `message-border`
/// key overrides it (present wins), commenting the key back out keeps the
/// CLI message (NOT the default fallback — the lock wins the fallback).
#[test]
fn fallback_message_key_absent_keeps_cli_lock() {
    let mut base = minimal_cloud_config();
    base.message = Some("from-cli".to_string());
    base.message_border = false;
    base.cli_explicit.message = true;

    let mut cfg = HashMap::new();
    cfg.insert("message-border".to_string(), "from-config".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert_eq!(with_key.message.as_deref(), Some("from-config"));

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert_eq!(
        after_comment_out.message.as_deref(),
        Some("from-cli"),
        "commenting the message key out must fall back to the CLI -m value"
    );
    assert!(
        !after_comment_out.message_border,
        "the CLI -m border choice (false) returns with it"
    );
}

/// Sequence fallback for `--msg-mode false`: a config `msg-mode` key
/// overrides it (present wins), commenting the key back out restores the
/// CLI lock.
#[test]
fn fallback_msg_mode_key_absent_keeps_cli_lock() {
    let mut base = minimal_cloud_config();
    base.msg_mode = false; // CLI --msg-mode false
    base.cli_explicit.msg_mode = true;

    let mut cfg = HashMap::new();
    cfg.insert("msg-mode".to_string(), "true".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert!(with_key.msg_mode, "config msg-mode key present wins");

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert!(
        !after_comment_out.msg_mode,
        "commenting the msg-mode key out must fall back to the CLI-locked value"
    );
}

/// Sequence fallback for `-mfs`/`--msg-fill-style` (the owner's command
/// locks `words`): a config key overrides it, commenting the key back out
/// restores the locked startup style.
#[test]
fn fallback_msg_fill_style_key_absent_keeps_cli_lock() {
    let mut base = minimal_cloud_config();
    base.msg_fill_style = crate::msg_fill_style::MsgFillStyle::Slide; // CLI -mfs slide
    base.cli_explicit.msg_fill_style = true;

    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "fade".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        with_key.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Fade
    );

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert_eq!(
        after_comment_out.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Slide,
        "commenting the msg-fill-style key out must fall back to the CLI lock"
    );
}

/// Sequence fallback for `--color-tune`: tune keys present win, commenting
/// the whole [color.tune] block out keeps the locked CLI tune (does NOT
/// reset to identity — the alpha.7 reset only applies with no CLI lock,
/// see tests_rejection_msg.rs).
#[test]
fn fallback_color_tune_keys_absent_keep_cli_lock() {
    let mut base = minimal_cloud_config();
    base.color_tune = crate::color_tune::ColorTune {
        brightness: 2.0,
        ..crate::color_tune::ColorTune::IDENTITY
    };
    base.cli_explicit.color_tune = true;

    let mut cfg = HashMap::new();
    cfg.insert("color.tune.brightness".to_string(), "0.5".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        with_key.color_tune.brightness, 0.5,
        "config tune key present wins"
    );

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert_eq!(
        after_comment_out.color_tune.brightness, 2.0,
        "commenting the [color.tune] block out must fall back to the CLI-locked tune"
    );
}

/// Sequence fallback for `--charset` (the owner's command locks
/// `-C minimal`): a config `charset` key overrides it, commenting the key
/// back out restores the locked startup charset (name + glyph vec).
#[test]
fn fallback_charset_key_absent_keeps_cli_lock() {
    let mut base = minimal_cloud_config();
    base.charset_preset = "minimal".to_string(); // CLI -C minimal
    base.cli_explicit.charset = true;

    let mut cfg = HashMap::new();
    cfg.insert("charset".to_string(), "retro".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        with_key.charset_preset, "retro",
        "config charset key present wins"
    );

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert_eq!(
        after_comment_out.charset_preset, "minimal",
        "commenting the charset key out must fall back to the CLI-locked preset"
    );
    assert_eq!(
        after_comment_out.chars.len(),
        base.chars.len(),
        "the locked glyph vec returns with the preset"
    );
}

/// Sequence fallback for `--crystal-dragon` / `--power-dragon` /
/// `--async-mode` booleans (one representative of the uniform
/// key-present-wins / key-absent-locked pattern).
#[test]
fn fallback_dragon_and_async_keys_absent_keep_cli_lock() {
    let mut base = minimal_cloud_config();
    base.crystal_dragon = true; // CLI --crystal-dragon
    base.cli_explicit.crystal_dragon = true;
    base.power_dragon = false; // CLI --power-dragon false
    base.cli_explicit.power_dragon = true;
    base.async_mode = false; // CLI --async-mode false
    base.cli_explicit.async_mode = true;

    let mut cfg = HashMap::new();
    cfg.insert("crystal-dragon".to_string(), "false".to_string());
    cfg.insert("power-dragon".to_string(), "true".to_string());
    cfg.insert("async-mode".to_string(), "true".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert!(
        !with_key.crystal_dragon,
        "config crystal-dragon key present wins"
    );
    assert!(
        with_key.power_dragon,
        "config power-dragon key present wins"
    );
    assert!(with_key.async_mode, "config async-mode key present wins");

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert!(
        after_comment_out.crystal_dragon,
        "commenting the crystal-dragon key out must fall back to the CLI lock"
    );
    assert!(
        !after_comment_out.power_dragon,
        "commenting the power-dragon key out must fall back to the CLI lock"
    );
    assert!(
        !after_comment_out.async_mode,
        "commenting the async-mode key out must fall back to the CLI lock"
    );
}

/// Startup effective values (config@startup, no CLI flag) are ALSO part of
/// the locked layer: commenting a key the engine started with is a no-op
/// (the locked value equals the startup value) — the fallback layer is
/// uniform, not CLI-only.
#[test]
fn fallback_layer_is_the_startup_effective_not_just_cli() {
    let mut base = minimal_cloud_config();
    base.target_fps = 144.0; // config@startup value (no CLI flag)
    base.cli_explicit.fps = false;

    let mut cfg = HashMap::new();
    cfg.insert("fps".to_string(), "60".to_string());
    let with_key = rebuild_cloud_config(&base, &cfg);
    assert_eq!(with_key.target_fps, 60.0);

    let no_key = HashMap::new();
    let after_comment_out = rebuild_cloud_config(&base, &no_key);
    assert_eq!(
        after_comment_out.target_fps, 144.0,
        "no-CLI runs fall back to the startup-effective value (config@startup)"
    );
}
