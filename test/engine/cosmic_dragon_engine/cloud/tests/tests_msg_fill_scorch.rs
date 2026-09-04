// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Render-level tests for the v80.0.0-beta.1 msg-fill-style `scorch` style
//! (post-glitch follow-up — see
//! `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §3.C).
//!
//! Split into its own file (mirroring `tests_msg_fill_hologram.rs` /
//! `tests_msg_fill_glitch.rs`) to keep `tests_msg_fill_style.rs`
//! under the 800-LOC hard cap (see `src/RULES_LOC.md`). The shared
//! helpers (`make_cloud_colored`, `set_message_elapsed`,
//! `visible_content_cells`) are imported from the parent
//! msg-fill-style test file.
//!
//! The scorch style keeps the REVEAL math stateless (ember tint +
//! factor curve 1.5 → 0.8 → 1.0 over 400 ms) and adds a bounded
//! 16-slot smoke sidecar (slow upward gray '░' puffs, 700 ms
//! lifetime) rendered inside `draw_message` at the END (same
//! pattern as engrave sparks). These tests mirror the engrave
//! acceptance ritual: pacing, tint presence/absence, smoke
//! presence/absence, `--no-effects` gating, r-restart re-arm.

use std::time::{Duration, Instant};

use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
// Shared helpers from the parent msg-fill-style test file.
use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed, visible_content_cells};

/// Count smoke glyphs ('░', U+2591 LIGHT SHADE) anywhere in the
/// frame. The overlay itself never emits this glyph outside the
/// scorch smoke pass, so any hit is a smoke particle.
fn scorch_smoke_glyphs(frame: &Frame) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    for y in 0..frame.height {
        for x in 0..frame.width {
            if let Some(cell) = frame.get(x, y) {
                if cell.ch == '░' {
                    out.push((x, y));
                }
            }
        }
    }
    out
}

#[test]
fn scorch_reveals_progressively_like_typewriter_pacing() {
    // Same 80 ms/char pacing as typewriter/engrave/hologram/glitch:
    // "hello world" = 10 content chars; 160 ms → 2 chars revealed.
    let mut cloud = make_cloud_colored(MsgFillStyle::Scorch);
    set_message_elapsed(&mut cloud, "hello world", 160);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        2,
        "scorch at 160ms must show exactly 2 chars (80ms/char), got {}",
        visible.len()
    );
    let chars: String = visible.iter().map(|(_, _, c)| *c).collect();
    assert_eq!(
        chars, "he",
        "first two chars of 'hello world' must be visible"
    );
}

#[test]
fn scorch_head_burns_hot_with_ember_tint() {
    // Cell 0 at age 0: factor = 1.0 + SCORCH_HEAD_BOOST = 1.5.
    // The rendered color must be blended toward the ember RGB
    // (255, 100, 30) by blend 1.0 (full ember — palette color
    // fully replaced). Using a dim custom palette so the boost
    // does not clamp at 255 and the blend is observable.
    use crate::rain_style::RainStyle;
    use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
    let mut cloud = super::super::Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(30, 12);
    cloud.set_msg_fill_style(MsgFillStyle::Scorch);
    // Dim green palette so the ember blend is clearly observable
    // (green (0, 255, 0) blended toward ember (255, 100, 30) by 1.0
    // = ember; by 0.0 = green; by 0.5 = muddy midpoint).
    cloud.set_palette(
        None,
        crate::palette::Palette {
            colors: vec![crossterm::style::Color::Rgb { r: 0, g: 128, b: 0 }],
            bg: None,
        },
    );
    set_message_elapsed(&mut cloud, "hi", 0);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    // Find the first visible content cell and check its color is
    // the ember RGB (full blend at age 0).
    let mc = cloud
        .message
        .iter()
        .find(|mc| !mc.is_border && mc.val != ' ')
        .expect("message must have content cells");
    let cell = frame
        .get(mc.col, mc.line)
        .expect("first content cell must be drawn");
    assert_eq!(cell.ch, 'h', "first content char must be 'h'");
    if let crossterm::style::Color::Rgb { r, g, b } = cell.fg.expect("fg must be set") {
        // Full ember blend at age 0: the palette green (0, 128, 0)
        // blended toward ember (255, 100, 30) by 1.0 = ember.
        assert!(r > 200, "ember R must be near 255 at full blend (got {r})");
        assert!(
            g > 50 && g < 150,
            "ember G must be near 100 at full blend (got {g})"
        );
        assert!(b < 80, "ember B must be near 30 at full blend (got {b})");
    } else {
        panic!("fg must be Color::Rgb for scorch ember tint");
    }
}

#[test]
fn scorch_settles_to_palette_color_after_cool_window() {
    // At age >= SCORCH_COOL_MS (400 ms): tint = None, factor = 1.0.
    // The rendered color must be the palette color (no ember tint).
    // Use --no-effects to suppress the smoke sidecar (which would
    // otherwise spawn a puff on the first draw and potentially
    // overlap a content cell, breaking visible_content_cells).
    let mut cloud = make_cloud_colored(MsgFillStyle::Scorch);
    cloud.set_effects_enabled(false);
    set_message_elapsed(&mut cloud, "hi", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        2,
        "scorch at 10s must show both chars (settled)"
    );
    // No smoke (--no-effects).
    assert!(
        scorch_smoke_glyphs(&frame).is_empty(),
        "scorch smoke must be suppressed under --no-effects"
    );
}

#[test]
fn scorch_smoke_visible_during_reveal() {
    // During the reveal (elapsed < total reveal time), the scorch
    // head is advancing and smoke puffs must be spawning. Scan a
    // range of elapsed values to find one with active smoke —
    // probabilistically guaranteed (head advances every 80 ms,
    // smoke lives 700 ms, so any elapsed in 100..2000 has smoke).
    let mut cloud = make_cloud_colored(MsgFillStyle::Scorch);
    let mut found_smoke = None;
    for ms in (100..3000).step_by(100) {
        set_message_elapsed(&mut cloud, "wake up, neo", ms);
        let mut frame = Frame::new(30, 12, cloud.palette.bg);
        cloud.draw_message(&mut frame, Instant::now());
        let smoke = scorch_smoke_glyphs(&frame);
        if !smoke.is_empty() {
            found_smoke = Some(smoke.len());
            break;
        }
    }
    assert!(
        found_smoke.is_some(),
        "scorch must produce smoke puffs during reveal (scanned 100..3000 ms step 100)"
    );
}

#[test]
fn scorch_smoke_respects_no_effects() {
    // PERF-4: --no-effects must suppress the smoke sidecar
    // (same contract as engrave sparks and hologram scanline).
    // The reveal math itself runs unchanged — text still burns in
    // with ember tint; only the smoke puffs are gated.
    let mut cloud = make_cloud_colored(MsgFillStyle::Scorch);
    cloud.set_effects_enabled(false);
    set_message_elapsed(&mut cloud, "wake up, neo", 500);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let smoke = scorch_smoke_glyphs(&frame);
    assert!(
        smoke.is_empty(),
        "--no-effects must suppress the scorch smoke (got {} '░' glyphs)",
        smoke.len()
    );
    // The reveal math must still run — the first chars of the
    // message must be visible (the ember tint is NOT gated by
    // --no-effects, only the smoke is).
    let visible = visible_content_cells(&frame, &cloud);
    assert!(
        !visible.is_empty(),
        "--no-effects must NOT suppress the reveal math itself"
    );
}

#[test]
fn scorch_fires_one_smoke_puff_per_newly_revealed_char() {
    // First draw at small elapsed: head at char 0 → exactly one
    // puff of SCORCH_SMOKE_PER_HEAD (1) smoke particles.
    let mut cloud = make_cloud_colored(MsgFillStyle::Scorch);
    set_message_elapsed(&mut cloud, "hello world", 10);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        cloud.scorch.active_count, 1,
        "first draw must fire exactly SCORCH_SMOKE_PER_HEAD (1) smoke particle"
    );
    assert!(!scorch_smoke_glyphs(&frame).is_empty(), "smoke must render");

    // Same head again (elapsed still 10ms, head_idx unchanged) →
    // no new puff.
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        cloud.scorch.active_count, 1,
        "a stationary head must not spawn additional puffs"
    );

    // Advance the reveal timeline to move the head to char 1
    // (elapsed >= 160ms so reveal_count = 160/80 = 2, head_idx = 1).
    // Set message_start_time directly (bypass set_message which
    // would reset the smoke pool) so the puff from draw 1 is still
    // active.
    cloud.message_start_time = Some(Instant::now() - Duration::from_millis(160));
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        cloud.scorch.active_count, 2,
        "a moved head must fire exactly one additional puff (old puff still active)"
    );
    assert!(cloud.scorch.active_count <= crate::msg_fill_style::scorch::SCORCH_SMOKE_POOL_SIZE);
}

#[test]
fn scorch_smoke_expires_and_stops_when_reveal_completes() {
    // Elapsed far past the reveal: head parks on the last char and
    // fires its (single) puff on the first draw, then never again.
    let mut cloud = make_cloud_colored(MsgFillStyle::Scorch);
    set_message_elapsed(&mut cloud, "hi", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(cloud.scorch.active_count, 1);

    // Same head again → no new puff.
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(cloud.scorch.active_count, 1);

    // S-master-HUNT-21/22: sim_age accumulates the real-time dt
    // (33ms steps stay under the anti-teleport cap).
    // Loop draw_message in 33ms steps until smoke expires (sim_age >= 700ms).
    let mut t = Instant::now();
    while cloud.scorch.active_count > 0 {
        t += Duration::from_millis(33);
        let mut frame = Frame::new(30, 12, cloud.palette.bg);
        cloud.draw_message(&mut frame, t);
    }
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, t);
    assert_eq!(
        cloud.scorch.active_count, 0,
        "expired smoke must deactivate with no respawn from a parked head"
    );
    assert!(
        scorch_smoke_glyphs(&frame).is_empty(),
        "no smoke glyphs after expiry"
    );
}

#[test]
fn scorch_reveal_rearms_fresh_puffs_after_typewriter_restart() {
    // r-restart (restart_message_typewriter) rewinds the
    // timeline; the movement detector must re-arm so the fresh
    // reveal's first char fires its puff again.
    let mut cloud = make_cloud_colored(MsgFillStyle::Scorch);
    set_message_elapsed(&mut cloud, "hi", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(cloud.scorch.active_count, 1);

    cloud.restart_message_typewriter();
    // The restart sets start = now; immediately after, elapsed is
    // ~10 ms, head at char 0 again.
    set_message_elapsed(&mut cloud, "hi", 10);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        cloud.scorch.active_count, 1,
        "restarted reveal must re-fire the first-char puff"
    );
}
