// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-beta.1 msg-fill-style live-reload tests, extracted from
//! `tests.rs` to keep that file under the 800-LOC hard cap
//! (see `src/RULES_LOC.md`).
//!
//! Covers:
//! - Each shipped style (typewriter / fade / words / slide /
//!   instant / engrave / hologram / glitch / scorch / cascade)
//!   live-reloads via the `msg-fill-style` config key.
//! - The config surface is case-insensitive (mirrors every other
//!   enum key).
//! - An invalid value soft-fails (logged, style unchanged — same
//!   policy as intro-color live reload).
//! - When the key is absent (commented out), the startup style is
//!   preserved (enums have no reset-on-comment semantics).
//! - CLI `-mfs` explicit wins over config on live reload (priority
//!   contract: CLI > config.toml).

use std::collections::HashMap;

use super::rebuild_cloud_config;
use super::tests::minimal_cloud_config;

/// The engrave style (v80.0.0-beta.1 follow-up) live-reloads exactly like the
/// other styles — the spark sidecar arms on the next style change via
/// `set_msg_fill_style`.
#[test]
fn rebuild_applies_msg_fill_style_engrave() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "engrave".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Engrave,
        "config msg-fill-style=engrave must be applied on live reload"
    );
}

/// The hologram style (post-engrave follow-up) live-reloads exactly
/// like the other styles. Hologram is fully stateless (no sidecar
/// to arm), so the test only needs to assert the enum variant — the
/// scanline pass self-gates on the next draw_message frame.
#[test]
fn rebuild_applies_msg_fill_style_hologram() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "hologram".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Hologram,
        "config msg-fill-style=hologram must be applied on live reload"
    );
}

/// The glitch style (post-hologram follow-up) live-reloads exactly
/// like the other styles. Glitch is fully stateless (no sidecar;
/// the glyph substitution is part of the reveal math itself, not a
/// cosmetic overlay), so the test only needs to assert the enum
/// variant — the wrong-glyph substitution kicks in on the next
/// draw_message frame.
#[test]
fn rebuild_applies_msg_fill_style_glitch() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "glitch".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Glitch,
        "config msg-fill-style=glitch must be applied on live reload"
    );
}

/// The scorch style (post-glitch follow-up) live-reloads exactly
/// like the other styles. Scorch adds a 16-slot smoke sidecar
/// (armed on the next style change via `set_msg_fill_style`) and
/// extends `CellReveal` with a `tint` field (the color-shifting API
/// surface). The test only needs to assert the enum variant — the
/// smoke pool arms and the tint kicks in on the next draw_message
/// frame.
#[test]
fn rebuild_applies_msg_fill_style_scorch() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "scorch".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Scorch,
        "config msg-fill-style=scorch must be applied on live reload"
    );
}

/// The cascade style (post-scorch follow-up) live-reloads exactly
/// like the other styles. Cascade is fully stateless (no sidecar;
/// reuses the signed `slide_rows` field for drop-from-above). The
/// test only needs to assert the enum variant — the drop animation
/// kicks in on the next draw_message frame.
#[test]
fn rebuild_applies_msg_fill_style_cascade() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "cascade".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Cascade,
        "config msg-fill-style=cascade must be applied on live reload"
    );
}

/// Editing `msg-fill-style` in config.toml mid-run must switch the
/// reveal style on the next rebuild (no restart needed).
#[test]
fn rebuild_applies_msg_fill_style_from_config() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "slide".to_string());
    let base = minimal_cloud_config();
    assert_eq!(
        base.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Typewriter,
        "baseline must start at typewriter"
    );
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Slide,
        "config msg-fill-style=slide must be applied on live reload"
    );
}

/// The config surface is case-insensitive (mirrors every other enum
/// key: intro, glitch-level, monolith-size).
#[test]
fn rebuild_msg_fill_style_config_is_case_insensitive() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "Fade".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Fade
    );
}

/// An invalid style value soft-fails: logged, style unchanged (same
/// policy as intro-color live reload — don't crash a running session).
#[test]
fn rebuild_msg_fill_style_invalid_soft_fails() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "scanner".to_string());
    let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Typewriter,
        "invalid msg-fill-style must keep the previous style (soft-fail)"
    );
}

/// When the key is absent (commented out), the startup style is
/// preserved — enums have no reset-on-comment semantics.
#[test]
fn rebuild_msg_fill_style_absent_keeps_startup_value() {
    let mut base = minimal_cloud_config();
    base.msg_fill_style = crate::msg_fill_style::MsgFillStyle::Slide;
    let cfg = HashMap::new();
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Slide,
        "absent msg-fill-style key must preserve the startup style"
    );
}

/// v80.0.0-beta.1 (owner contract): config `msg-fill-style` key PRESENT wins over
/// the CLI `-mfs` lock at runtime (temporal precedence). The CLI style
/// returns when the key is commented back out (base keeps the locked
/// startup style — pinned in tests_cli_fallback.rs).
#[test]
fn rebuild_msg_fill_style_key_overrides_cli_lock_when_present() {
    let mut cfg = HashMap::new();
    cfg.insert("msg-fill-style".to_string(), "fade".to_string());
    let mut base = minimal_cloud_config();
    // Simulate the user running `cosmostrix -mfs slide`: the CLI flag is
    // recorded as explicit, and the style is set to Slide.
    base.cli_explicit.msg_fill_style = true;
    base.msg_fill_style = crate::msg_fill_style::MsgFillStyle::Slide;
    let new = rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        new.msg_fill_style,
        crate::msg_fill_style::MsgFillStyle::Fade,
        "config msg-fill-style key present must override the CLI -mfs lock (v80.0.0-beta.1)"
    );
}
