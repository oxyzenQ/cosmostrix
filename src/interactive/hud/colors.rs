// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD color helpers — extracted from `hud/mod.rs` to keep that file
//! under the 1500-LOC cap.
//!
//! Owns the chroma dragon gradient computation + hue-preserving
//! brightness boost for HUD row colors. Both functions are pure
//! (stateless, no Cloud coupling) and were the cleanest extraction
//! targets in the HUD subsystem.
//!
//! Re-exported from `hud/mod.rs` via `pub(crate) use` so the existing
//! `use super::*` glob in `hud/tests.rs` + `hud/tests_brighten.rs`
//! resolves them unchanged.

use crossterm::style::Color;

// HD-01 (HUD chroma dragon integration): the previous 4-stop
// `compute_rain_gradient` helper has been replaced by the 16-stop
// `compute_chroma_gradient_16` helper above. The old design paired 2 HUD
// rows per palette stop (dim/trail/mid/head × 2 = 8 rows); the new design
// gives each row its own palette stop, sweeping the full chroma dragon
// gradient top→bottom. This matches the border message's per-cell chroma
// sweep philosophy, applied per-LINE for text readability.
// v50 (2026-08-15): bumped from 8 → 9 stops after adding the `cid:`
// (commit id) line at row 8.
// v50 (2026-08-17): bumped from 9 → 16 stops to reserve rows 9-15 for
// the 7 owner-mandated HUD expansion metrics (scene / color / density /
// speed / endurance-health-score / effective-pressure / charset). The cid
// line shares the head stop (palette[n-1], brightest) with screensize —
// both are "definitive identity" lines that the owner reads to verify the
// build, so they earn the most prominent position.

/// HD-01 (HUD chroma dragon integration): compute an 22-stop chroma gradient
/// sweeping the active palette's full color range across all 22 HUD rows.
///
/// Each row `i ∈ [0..22]` samples `palette_colors` at interpolation
/// parameter `t = i / 17.0`, so row 0 (fps, top) → palette[0] and row 17
/// (cid, bottom) → palette[n-1]. This mirrors the border message's per-cell
/// clockwise sweep (`cloud/mod.rs::draw_message` BC-02) — applied per-LINE
/// instead of per-cell, because each HUD line is a distinct text block that
/// needs its own legible color.
///
/// ## Why 18 stops
/// The HUD renders exactly 18 rows: 8 metric rows (fps / target / rss /
/// cpu / p99 / dirty / vmode / mode) + 7 owner-mandated expansion rows
/// (scene / color / density / speed / ehs / dsty / charset) + cid +
/// screensize + build. The previous 16-stop design was bumped to 18 after
/// the `ehs:` + `dsty:` additions (v50 2026-08-17 audit). Each row gets
/// its own interpolated color stop — no two rows share the same color
/// unless the palette is shorter than 22 stops (interpolation handles
/// that case smoothly).
///
/// ## Brightness floor
/// `brighten_color` is applied AFTER interpolation to every stop. This
/// guarantees every HUD row is legible on a black background, even when
/// palette[0] is a near-black start stop — it gets boosted to neutral
/// grey RGB(120,120,120) when pure black, preserving readability without
/// losing the palette's hue identity for non-black stops.
///
/// Returns a fixed-size `[Color; 22]` array (no allocation, stack-only).
pub(crate) fn compute_chroma_gradient_22(palette_colors: &[Color]) -> [Color; 22] {
    let n = palette_colors.len();
    let mut out = [
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
    ];
    if n == 0 {
        return out;
    }
    // v50 (2026-08-17) HUD chroma gradient smoothness fix: replace the
    // previous discrete sampling `palette_colors[(t * last).round()]` with
    // `interpolate_palette_color(palette_colors, t)` — the same linear-
    // interpolation helper used by the border message gradient (C4 fix).
    //
    // The previous discrete sampling produced visible bands when the palette
    // had fewer stops than the HUD has rows (e.g. a 3-stop palette + 22 HUD
    // rows → 6+ rows sharing the same color block). The owner explicitly
    // flagged this category as inconsistent with the chroma dragon smoothness
    // mandate: "audit which color-processing sites are not yet using
    // chroma dragon interpolation optimally ... can look inconsistent
    // if not unified". Every visible color surface must route through
    // the chroma dragon pipeline for consistency.
    //
    // With interpolation, every HUD row gets a smoothly-varying color even
    // when the palette is small — matching the border message's smooth
    // gradient behavior and the chroma dragon's per-cell sweep philosophy.
    // The `brighten_color` floor (TARGET_V=200) is still applied AFTER
    // interpolation to guarantee readability on a black background.
    //
    // LTS stability: `interpolate_palette_color` is NaN/Inf-safe (returns
    // the first stop defensively), so a future bug in upstream palette
    // generation cannot crash the HUD or produce garbage colors.
    // v50.0.0-beta.6: divisor is now 21.0 (was 15.0) since the array has
    // 18 entries (indices 0-21). Row 0 → t=0.0, row 17 → t=1.0.
    for (i, slot) in out.iter_mut().enumerate() {
        let t = i as f32 / 21.0;
        let interpolated = crate::cloud::interpolate_palette_color(palette_colors, t);
        *slot = brighten_color(interpolated.unwrap_or(Color::DarkGrey));
    }
    out
}

/// Boost a color's brightness while preserving its hue, so the HUD
/// follows the rain's actual color scheme instead of washing out to grey.
///
/// ## Why hue-preserving scaling (not white blend)
/// The previous implementation blended 35% source + 65% white, which
/// desaturated every color toward grey — a green rain produced a
/// grey-green HUD, an amber rain produced a washed-out amber. The user
/// explicitly flagged this: "HUD metrics colors too grey should be
/// dynamic follow the rain not hardcoded grey".
///
/// The new implementation uses HSV-style value scaling:
/// 1. Convert any Color variant to RGB via `palette::color_to_rgb`
///    (so AnsiValue + named colors also get processed — previously
///    they were returned as-is, which meant a 256-color palette stayed
///    at its native brightness even when too dim to read).
/// 2. Find the max channel (V in HSV).
/// 3. If V >= TARGET_V, the color is already bright enough — return
///    as-is to preserve the rain's vivid hue.
/// 4. If V < TARGET_V and V > 0, scale all channels by TARGET_V / V.
///    This preserves the hue ratio between channels — a dark green
///    RGB(0,50,0) becomes RGB(0,200,0), not a washed-out grey-green.
/// 5. If V == 0 (pure black), fall back to a neutral dim grey.
///    Scaling zero gives zero, so we need an explicit fallback.
///
/// TARGET_V = 200 ensures readability on a black background without
/// oversaturating. A vivid RGB(0,255,0) green is returned unchanged;
/// a dim RGB(0,80,0) green is boosted to RGB(0,200,0).
pub(crate) fn brighten_color(color: Color) -> Color {
    let (r, g, b) = crate::palette::color_to_rgb(color);
    const TARGET_V: u32 = 200;
    let max = r.max(g).max(b) as u32;
    if max >= TARGET_V {
        // Already bright enough — preserve the rain's vivid hue.
        Color::Rgb { r, g, b }
    } else if max == 0 {
        // Pure black — scaling zero gives zero, so fall back to a
        // neutral dim grey. This is the only case where we don't
        // preserve hue (there's no hue to preserve).
        Color::Rgb {
            r: 120,
            g: 120,
            b: 120,
        }
    } else {
        // Scale all channels by TARGET_V / max to boost brightness
        // while preserving the hue ratio between channels.
        // Uses integer math: scale = TARGET_V * 100 / max, then
        // (channel * scale) / 100. Min(255) guards against overflow
        // when the source channel is close to max but max < TARGET_V.
        //
        // SAFETY: max > 0 here because the `else if max == 0` branch
        // above caught the zero case. The debug_assert documents this
        // invariant for readers and catches logic regressions in dev
        // builds.
        debug_assert!(max > 0, "max must be > 0 here; zero case handled above");
        let scale = TARGET_V * 100 / max;
        Color::Rgb {
            r: ((r as u32 * scale) / 100).min(255) as u8,
            g: ((g as u32 * scale) / 100).min(255) as u8,
            b: ((b as u32 * scale) / 100).min(255) as u8,
        }
    }
}
