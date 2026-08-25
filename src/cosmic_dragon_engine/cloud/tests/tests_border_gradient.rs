// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Border message chroma dragon gradient tests.
//!
//! v50 (2026-08-17): regression tests for the smooth border gradient fix.
//! The owner reported visible "gaps" between palette stops in the
//! `--message` border sweep — e.g. a white→red sweep showed a white block
//! then a red block with no in-between, instead of the smooth white →
//! semi-red → red gradient the rain color already produced. Root cause:
//! the previous implementation rounded `t * (n-1)` to the nearest integer
//! palette index and picked that discrete stop. The fix uses
//! `interpolate_palette_color` to linearly blend between adjacent stops
//! using the fractional remainder.
//!
//! These tests verify the helper produces interpolated colors at non-
//! integer `t` values (no gaps), preserves palette-identity stops at
//! integer boundaries, and handles edge cases (empty palette, single
//! stop, NaN/Inf, out-of-range `t`) defensively without panicking.
//!
//! ## blend_toward_rgb rounding convention
//!
//! `crate::chroma_dragon_engine::legacy::blend_toward_rgb` uses integer math with a
//! `+128` rounding offset (half-up convention):
//! `out = src + (tgt - src) * wf / 256` where `wf = (factor * 256) as i32`
//! and `+128` is added before the divide to round half-up.
//!
//! This means exact 50% blends between adjacent stops may produce values
//! ±1 from the theoretical midpoint due to truncation toward zero on
//! negative deltas. The expected values in these tests are computed
//! directly from the formula (not from theoretical midpoint math).

use crossterm::style::Color;

use super::interpolate_palette_color;

#[test]
fn empty_palette_returns_none() {
    // An empty palette slice is a degenerate case (Mono mode usually
    // skips the gradient entirely), but the helper must NOT panic —
    // returns `None` so the caller falls back to `content_fg`.
    let palette: Vec<Color> = vec![];
    assert_eq!(interpolate_palette_color(&palette, 0.0), None);
    assert_eq!(interpolate_palette_color(&palette, 0.5), None);
    assert_eq!(interpolate_palette_color(&palette, 1.0), None);
}

#[test]
fn single_stop_palette_returns_that_stop_for_any_t() {
    // A one-stop palette has nothing to interpolate between — every `t`
    // returns the same stop. Important for tiny custom palettes.
    let palette = vec![Color::Rgb {
        r: 100,
        g: 200,
        b: 50,
    }];
    assert_eq!(
        interpolate_palette_color(&palette, 0.0),
        Some(Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        })
    );
    assert_eq!(
        interpolate_palette_color(&palette, 0.5),
        Some(Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        })
    );
    assert_eq!(
        interpolate_palette_color(&palette, 1.0),
        Some(Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        })
    );
}

#[test]
fn integer_t_returns_exact_palette_stop_no_interpolation() {
    // At integer boundaries (t=0.0, t=1/n, t=2/n, ..., t=1.0), the helper
    // must return the exact palette stop — no interpolation. This preserves
    // palette-identity stops so the chroma dragon's anchor stops are
    // unchanged. A regression here would mean even integer stops are being
    // blended, which would shift the palette's identity hues.
    let palette = vec![
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 0: white
        Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        }, // idx 1: mid-grey
        Color::Rgb { r: 0, g: 0, b: 0 }, // idx 2: black
    ];
    // t = 0.0 → palette[0]
    assert_eq!(
        interpolate_palette_color(&palette, 0.0),
        Some(Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        })
    );
    // t = 1/2 = 0.5 → palette[1]
    assert_eq!(
        interpolate_palette_color(&palette, 0.5),
        Some(Color::Rgb {
            r: 128,
            g: 128,
            b: 128
        })
    );
    // t = 1.0 → palette[2] (last)
    assert_eq!(
        interpolate_palette_color(&palette, 1.0),
        Some(Color::Rgb { r: 0, g: 0, b: 0 })
    );
}

#[test]
fn non_integer_t_interpolates_between_adjacent_stops() {
    // THE OWNER REGRESSION TEST: at non-integer `t`, the helper must
    // produce an interpolated color (not pick a discrete stop). This
    // eliminates the visible "gap" the owner reported between palette
    // stops. Test palette: white → red (so a 50% interpolation produces
    // a salmon-pink — the "semi-red" the owner wants visible between
    // pure white and pure red).
    //
    // blend_toward_rgb rounding: out = src + (tgt - src) * wf / 256
    // where wf = (factor * 256) as i32, plus a +128 rounding offset.
    // Truncation toward zero on negative deltas means exact 50% blends
    // produce values ±1 from the theoretical midpoint.
    let palette = vec![
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 0: white
        Color::Rgb { r: 255, g: 0, b: 0 }, // idx 1: red
    ];
    // t = 0.5 → scaled_t = 0.5, pos = 0, frac = 0.5.
    // wf = (0.5 * 256) as i32 = 128.
    // R: 255 + (255-255)*128/256 + 128/256 = 255 + 0 + 0 = 255.
    // G: 255 + (0-255)*128/256 + 128/256 = 255 + (-32512)/256 = 255 + (-127) = 128.
    // B: same as G = 128.
    // Result: RGB(255, 128, 128) — salmon-pink "semi-red".
    let interpolated = interpolate_palette_color(&palette, 0.5);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 255,
            g: 128,
            b: 128
        }),
        "t=0.5 between white and red must produce salmon-pink RGB(255,128,128), \
         not discrete white or red block"
    );
    // t = 0.25 → scaled_t = 0.25, pos = 0, frac = 0.25.
    // wf = (0.25 * 256) as i32 = 64.
    // G: 255 + (0-255)*64/256 + 128/256 = 255 + (-16192)/256 = 255 + (-63) = 192.
    let interpolated = interpolate_palette_color(&palette, 0.25);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 255,
            g: 192,
            b: 192
        }),
        "t=0.25 between white and red must produce a lighter salmon-pink"
    );
    // t = 0.75 → scaled_t = 0.75, pos = 0, frac = 0.75.
    // wf = (0.75 * 256) as i32 = 192.
    // G: 255 + (0-255)*192/256 + 128/256 = 255 + (-48832)/256 = 255 + (-190) = 65.
    let interpolated = interpolate_palette_color(&palette, 0.75);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 255,
            g: 65,
            b: 65
        }),
        "t=0.75 between white and red must produce a deeper salmon-pink"
    );
}

#[test]
fn three_stop_palette_interpolates_across_two_segments() {
    // With 3 stops, the helper interpolates across segment [0,1] for
    // t ∈ (0, 0.5), and segment [1, 2] for t ∈ (0.5, 1.0). At t = 0.5
    // (the boundary), it returns palette[1] exactly.
    //
    // Test palette: white → grey → black. Linear blend between adjacent
    // stops is exact (no chroma surprises) so we can assert precise
    // values derived from the blend_toward_rgb formula.
    let palette = vec![
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 0: white
        Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        }, // idx 1: mid-grey
        Color::Rgb { r: 0, g: 0, b: 0 }, // idx 2: black
    ];
    // t = 0.25 → scaled_t = 0.5, pos = 0, frac = 0.5.
    // wf = 128. R: 255 + (128-255)*128/256 + 128/256 = 255 + (-16128)/256
    // = 255 + (-63) = 192 (truncation toward zero, -16128/256 = -63).
    let interpolated = interpolate_palette_color(&palette, 0.25);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 192,
            g: 192,
            b: 192
        })
    );
    // t = 0.75 → scaled_t = 1.5, pos = 1, frac = 0.5.
    // wf = 128. R: 128 + (0-128)*128/256 + 128/256 = 128 + (-16256)/256
    // = 128 + (-63) = 65 (truncation toward zero, -16256/256 = -63).
    let interpolated = interpolate_palette_color(&palette, 0.75);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 65,
            g: 65,
            b: 65
        })
    );
}

#[test]
fn out_of_range_t_clamps_to_endpoints() {
    // The helper clamps `t` to [0.0, 1.0] before computing scaled_t.
    // t < 0 → returns palette[0] (first stop).
    // t > 1 → returns palette[n-1] (last stop).
    // This matches the visual semantics — t represents the parametric
    // position around the border box perimeter, which is naturally
    // bounded [0, 1].
    let palette = vec![
        Color::Rgb { r: 255, g: 0, b: 0 }, // idx 0: red (first)
        Color::Rgb { r: 0, g: 255, b: 0 }, // idx 1: green
        Color::Rgb { r: 0, g: 0, b: 255 }, // idx 2: blue (last)
    ];
    // t = -0.5 → clamps to 0.0 → palette[0]
    assert_eq!(
        interpolate_palette_color(&palette, -0.5),
        Some(Color::Rgb { r: 255, g: 0, b: 0 })
    );
    // t = 1.5 → clamps to 1.0 → palette[2]
    assert_eq!(
        interpolate_palette_color(&palette, 1.5),
        Some(Color::Rgb { r: 0, g: 0, b: 255 })
    );
}

#[test]
fn nan_t_falls_back_to_first_stop_defensive() {
    // NaN `t` must NOT panic — returns the first stop defensively.
    // The owner mandate for HUD metric stability extends to all runtime
    // math; a NaN t (e.g. from a 0/0 division upstream) would propagate
    // as a NaN color otherwise and could crash the renderer.
    let palette = vec![
        Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        }, // idx 0: first
        Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        }, // idx 1
    ];
    assert_eq!(
        interpolate_palette_color(&palette, f32::NAN),
        Some(Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }),
        "NaN t must fall back to first stop, not panic"
    );
    assert_eq!(
        interpolate_palette_color(&palette, f32::INFINITY),
        Some(Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }),
        "+Inf t must fall back to first stop, not panic"
    );
    assert_eq!(
        interpolate_palette_color(&palette, f32::NEG_INFINITY),
        Some(Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }),
        "-Inf t must fall back to first stop, not panic"
    );
}

#[test]
fn adjacent_cells_produce_distinct_colors_no_gaps() {
    // THE OWNER REGRESSION TEST (visual gap elimination): with a small
    // palette and many border cells, consecutive cells must produce
    // DISTINCT colors — not the same discrete palette stop repeated.
    // This is what eliminates the visible "gap" the owner reported.
    //
    // Test setup: 3-stop palette (red/green/blue), 10 cells (so t steps
    // of 1/9 ≈ 0.111). Without interpolation, cells 0/1/2/3 would all
    // be palette[0] (red), cells 4/5 would be palette[1] (green), cells
    // 6/7/8/9 would be palette[2] (blue) — only 3 distinct colors.
    // With interpolation, every cell gets a slightly different color.
    let palette = vec![
        Color::Rgb { r: 255, g: 0, b: 0 }, // idx 0: red
        Color::Rgb { r: 0, g: 255, b: 0 }, // idx 1: green
        Color::Rgb { r: 0, g: 0, b: 255 }, // idx 2: blue
    ];
    let total_cells = 10usize;
    let mut colors: Vec<Color> = Vec::with_capacity(total_cells);
    for i in 0..total_cells {
        let t = i as f32 / (total_cells - 1) as f32;
        colors.push(interpolate_palette_color(&palette, t).unwrap());
    }
    // Count distinct colors — with interpolation we expect at least 5
    // distinct values (the 3 palette stops + interpolated intermediates).
    // The old discrete-sampling implementation would have produced only
    // 3 distinct values (one per palette stop).
    let distinct_count = {
        let mut unique: Vec<Color> = colors.clone();
        unique.dedup();
        unique.len()
    };
    assert!(
        distinct_count >= 5,
        "interpolated border must produce >= 5 distinct colors across 10 cells \
         (got {distinct_count}) — the old discrete-sampling implementation would \
         have produced only 3 (one per palette stop), causing the visible gap"
    );
    // Also assert that NO two adjacent cells share the same color when
    // the palette has 3+ stops and there are 5+ cells — interpolation
    // guarantees monotonic transitions.
    for window in colors.windows(2) {
        assert_ne!(
            window[0], window[1],
            "adjacent border cells must NOT share the same color — that's the \
             visible gap the owner reported"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// RAIN_BORDER_TOUCH_GLOW (Option C+D) tests.
//
// Verifies the touch-detection helper and the cached top-border geometry.
// See `docs/research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md` for the design.
// ─────────────────────────────────────────────────────────────────────────

use std::time::Instant;

use crate::cloud::Cloud;
use crate::cosmic_dragon_engine::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
use crate::frame::Frame;
use crate::rain_style::RainStyle;
// LTS polish (2026-08-26): import the Mono-mode `make_cloud` helper for the
// empty-palette panic-safety regression test below.
use super::make_cloud;

/// Helper: build a colored (non-Mono) Cloud so palette.colors is populated
/// and the touch-glow effect actually has colors to blend with. The default
/// `make_cloud()` uses `ColorMode::Mono`, which leaves `palette.colors`
/// empty and the touch-glow blending path early-returns — fine for
/// production (Mono mode is intentionally colorless) but unsuitable for
/// testing the touch-glow visual effect.
fn make_cloud_colored() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud
}

/// After `set_message("hello")` + `set_message_border(true)`, the cached
/// `message_top_line` must point at the top row of the overlay box (not
/// `u16::MAX` sentinel), and `message_left_col`/`message_right_col` must
/// span the box's horizontal extent. The droplet advance loop relies on
/// these to fire touch events.
#[test]
fn reset_message_caches_top_border_geometry_when_bordered() {
    let mut cloud = make_cloud_colored();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    assert_ne!(
        cloud.message_top_line,
        u16::MAX,
        "message_top_line must NOT be u16::MAX sentinel when a bordered overlay is active"
    );
    assert!(
        cloud.message_left_col < cloud.message_right_col,
        "left_col ({}) must be < right_col ({}) for the overlay span",
        cloud.message_left_col,
        cloud.message_right_col
    );
    // Top border row must be strictly inside the terminal (overlay is
    // centered, so start_line ≥ 1 for a non-trivial message on a 10-line
    // terminal).
    assert!(
        cloud.message_top_line < cloud.lines,
        "top_line ({}) must be inside the terminal (lines={})",
        cloud.message_top_line,
        cloud.lines
    );
}

/// When no border is active (`-m` instead of `-mb`), the cached geometry
/// must remain at sentinel values so the touch-detection helper's early
/// return fires — zero cost on the hot path.
#[test]
fn reset_message_top_line_sentinel_when_unbordered() {
    let mut cloud = make_cloud_colored();
    cloud.set_message("hello");
    // No set_message_border(true) — overlay is content-only.
    cloud.set_message_border(false);

    assert_eq!(
        cloud.message_top_line,
        u16::MAX,
        "top_line must be u16::MAX sentinel when no bordered overlay is active"
    );
}

/// A transition (`prev < top && hp >= top`) for a column inside the
/// overlay's horizontal span MUST push exactly one BorderPulse. The
/// pulse's `head_rgb` must match the current palette's last-stop color
/// (dynamic, not static white) — owner insight: "warna bukan hanya putih
/// tapi dinamis".
#[test]
fn detect_border_touch_pushes_pulse_on_transition() {
    let mut cloud = make_cloud_colored();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    let top = cloud.message_top_line;
    let col = cloud.message_left_col; // inside the overlay span
    let now = Instant::now();

    // Transition: prev < top, hp >= top.
    cloud.detect_border_touch(col, top.saturating_sub(1), top, now);

    assert_eq!(
        cloud.border_pulses.len(),
        1,
        "exactly one BorderPulse must be pushed on transition"
    );

    // Verify dynamic color: pulse head_rgb == palette's last stop.
    let pulse = cloud.border_pulses[0];
    let expected_head_rgb = cloud
        .palette
        .colors
        .last()
        .copied()
        .and_then(crate::palette::decode_color)
        .unwrap_or((255, 255, 255));
    assert_eq!(
        pulse.head_rgb, expected_head_rgb,
        "pulse head_rgb must come from the active palette's last stop (dynamic)"
    );
    // Touched column must match the droplet's bound_col.
    assert_eq!(pulse.col, col);
    // Birth time must be the call's `now`.
    assert_eq!(pulse.birth, now);
}

/// No transition (prev >= top) — head was already at or below the top
/// border last frame. Must NOT push a pulse (avoids continuous-trigger
/// while a droplet sits at the border).
#[test]
fn detect_border_touch_no_pulse_when_no_transition() {
    let mut cloud = make_cloud_colored();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    let top = cloud.message_top_line;
    let col = cloud.message_left_col;
    let now = Instant::now();

    // prev == top: head was already at the top border. Not a transition.
    cloud.detect_border_touch(col, top, top, now);
    assert!(
        cloud.border_pulses.is_empty(),
        "no pulse must be pushed when prev_head_put_line >= top"
    );

    // prev > top: head was already past the border. Not a transition.
    cloud.detect_border_touch(col, top.saturating_add(1), top, now);
    assert!(
        cloud.border_pulses.is_empty(),
        "no pulse must be pushed when prev_head_put_line > top"
    );
}

/// A column outside the overlay's horizontal span must NOT fire a touch,
/// even on a valid transition — the droplet misses the overlay entirely.
#[test]
fn detect_border_touch_no_pulse_when_col_outside() {
    let mut cloud = make_cloud_colored();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    let top = cloud.message_top_line;
    let now = Instant::now();

    // Column to the LEFT of the overlay.
    let col_left = cloud.message_left_col.saturating_sub(1);
    cloud.detect_border_touch(col_left, top.saturating_sub(1), top, now);
    assert!(
        cloud.border_pulses.is_empty(),
        "no pulse must be pushed for a column to the left of the overlay"
    );

    // Column at the RIGHT edge (exclusive) — not inside.
    let col_right = cloud.message_right_col;
    cloud.detect_border_touch(col_right, top.saturating_sub(1), top, now);
    assert!(
        cloud.border_pulses.is_empty(),
        "no pulse must be pushed for a column at-or-past the right edge of the overlay"
    );
}

/// After a pulse is pushed, draw_message must consume it: the touched
/// border cell's fg color blends toward head_rgb (not the natural
/// gradient color). This is the "appears white matching the rain head"
/// effect the owner requested.
#[test]
fn draw_message_blends_touched_cell_toward_head_rgb() {
    let mut cloud = make_cloud_colored();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    let top = cloud.message_top_line;
    let col = cloud.message_left_col;
    let now = Instant::now();

    // Trigger a touch.
    cloud.detect_border_touch(col, top.saturating_sub(1), top, now);

    // Render one frame — draw_message consumes the pulse and blends the
    // touched cell toward head_rgb. The pulse envelope is at peak (1.0)
    // immediately after touch (smoothstep: env=1.0 at t=0).
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.rain_at(&mut frame, now);

    // Locate the touched MsgChr to find its position in the frame.
    let touched = cloud
        .message
        .iter()
        .find(|mc| mc.line == top && mc.col == col)
        .expect("touched cell must exist in self.message");

    let cell = frame
        .get(touched.col, touched.line)
        .expect("touched cell must be rendered in the frame");

    // The natural (un-touched) border color is the chroma gradient
    // result for this cell. With a pulse at peak (envelope=1.0), the
    // rendered color must be the head_rgb itself (full blend toward).
    let expected_head_rgb = cloud
        .palette
        .colors
        .last()
        .copied()
        .and_then(crate::palette::decode_color)
        .unwrap_or((255, 255, 255));

    let cell_rgb = cell
        .fg
        .and_then(crate::palette::decode_color)
        .unwrap_or((0, 0, 0));

    assert_eq!(
        cell_rgb, expected_head_rgb,
        "touched border cell must render at full head_rgb at pulse peak (envelope=1.0); \
         got {:?}, expected {:?}",
        cell_rgb, expected_head_rgb
    );
}

/// Pulse expiry: after `BORDER_TOUCH_PULSE_LIFETIME_MS`, the pulse must
/// be drained from `self.border_pulses` and the touched border cell
/// returns to its natural gradient color.
#[test]
fn pulse_expires_after_lifetime() {
    use std::time::Duration;

    let mut cloud = make_cloud_colored();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    let top = cloud.message_top_line;
    let col = cloud.message_left_col;
    let t0 = Instant::now();

    // Trigger a touch.
    cloud.detect_border_touch(col, top.saturating_sub(1), top, t0);
    assert_eq!(cloud.border_pulses.len(), 1);

    // Advance time past the pulse lifetime.
    let lifetime_ms = crate::chroma_dragon_engine::tuning::BORDER_TOUCH_PULSE_LIFETIME_MS;
    let past = t0 + Duration::from_millis(lifetime_ms as u64 + 100);

    // Render — draw_message drains expired pulses.
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.rain_at(&mut frame, past);

    assert!(
        cloud.border_pulses.is_empty(),
        "pulse must be drained after BORDER_TOUCH_PULSE_LIFETIME_MS ({} ms) + grace",
        lifetime_ms
    );
}

/// LTS polish (2026-08-26): a second touch to the **same** cell while a
/// pulse is still alive MUST refresh the existing entry (re-arm `birth`,
/// re-snapshot `head_rgb`) instead of pushing a duplicate. This bounds
/// `self.border_pulses.len() <= self.message.len()` regardless of touch
/// density — defensive against multi-droplet-per-column scenarios where N
/// droplets could otherwise stack N redundant pulses for one `msg_idx`.
///
/// Owner spec: "kalo hujan mengenainya lagi muncul lagi" — re-touch
/// re-fires the glow. The dedup-by-`msg_idx` refresh implements exactly
/// this: the cell keeps glowing, but the lifetime clock resets to the
/// newest touch. The owner sees a sustained glow under continuous touch,
/// not a stack of decaying copies.
#[test]
fn detect_border_touch_dedup_refresh_on_re_touch() {
    let mut cloud = make_cloud_colored();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    let top = cloud.message_top_line;
    let col = cloud.message_left_col;
    let t0 = Instant::now();

    // First touch.
    cloud.detect_border_touch(col, top.saturating_sub(1), top, t0);
    assert_eq!(
        cloud.border_pulses.len(),
        1,
        "first touch must push exactly one pulse"
    );
    let first_pulse = cloud.border_pulses[0];
    let first_birth = first_pulse.birth;

    // Second touch to the same cell at a later instant.
    let t1 = t0 + std::time::Duration::from_millis(50);
    cloud.detect_border_touch(col, top.saturating_sub(1), top, t1);

    // LTS bound: pool size must NOT grow.
    assert_eq!(
        cloud.border_pulses.len(),
        1,
        "second touch to the same msg_idx must refresh in place, not push a duplicate"
    );

    // The pulse's birth must be updated to the later instant.
    let refreshed_pulse = cloud.border_pulses[0];
    assert!(
        refreshed_pulse.birth > first_birth,
        "refreshed pulse birth ({:?}) must be later than the first ({:?})",
        refreshed_pulse.birth,
        first_birth
    );
    assert_eq!(
        refreshed_pulse.msg_idx, first_pulse.msg_idx,
        "refreshed pulse must target the same msg_idx as the first"
    );
}

/// LTS polish (2026-08-26): `detect_border_touch` must not panic when the
/// active palette has zero colors (Mono mode, or a misconfigured `rain =
/// []` config). The `.last().copied().and_then(decode_color).unwrap_or`
/// chain in the touch path falls back to `(255, 255, 255)` — this test
/// pins the fallback so a future "simplification" to `.last().unwrap()`
/// cannot sneak through.
///
/// Pre-LTS audit (DeepSeek review) flagged this as a theoretical panic
/// risk on empty palettes; verification showed the code was already safe,
/// but this test guards against regression.
#[test]
fn detect_border_touch_no_panic_on_empty_palette() {
    // Mono mode => palette.colors is empty.
    let mut cloud = make_cloud();
    cloud.set_message("hello");
    cloud.set_message_border(true);

    let top = cloud.message_top_line;
    let col = cloud.message_left_col;
    let now = Instant::now();

    // Must not panic, and must push a pulse with the white fallback.
    cloud.detect_border_touch(col, top.saturating_sub(1), top, now);

    assert_eq!(
        cloud.border_pulses.len(),
        1,
        "touch must still push a pulse even with an empty palette"
    );
    assert_eq!(
        cloud.border_pulses[0].head_rgb,
        (255, 255, 255),
        "empty palette must fall back to pure-white head_rgb"
    );
}
