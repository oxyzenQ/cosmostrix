// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD color helpers — extracted from `hud/mod.rs` to keep that file
//! under the 800-LOC cap.
//!
//! Owns the chroma dragon gradient computation + hue-preserving
//! brightness boost for HUD metric colors. Both functions are pure
//! (stateless, no Cloud coupling) and were the cleanest extraction
//! targets in the HUD subsystem.
//!
//! Re-exported from `hud/mod.rs` via `pub(crate) use` so the existing
//! `use super::*` glob in `hud/tests.rs` + `hud/tests_brighten.rs`
//! resolves them unchanged.

use crossterm::style::Color;

use super::hud_init::HUD_VISUAL_ORDER;

// HD-01 (HUD chroma dragon integration): the previous 4-stop
// `compute_rain_gradient` helper was replaced by the per-metric-stop
// gradient. The design gives each metric slot its own palette stop,
// sweeping the full chroma dragon gradient through the panel's VISUAL
// order (v80.0.0-beta.3, branch hud-scifi-dashboard).
// v50 (2026-08-15): bumped to 9 stops after adding the `cid:` line.
// v50 (2026-08-17): bumped to 16 stops for the 7 owner-mandated HUD
// expansion metrics.
// v50.0.0-beta.7: bumped to 22 stops after the Option C expansion.
// Z-master-1X round 5: bumped to 24 stops after adding dcel + tcel.
// v80.0.0-beta.3 (owner-approved Option B panel): still 24 stops, but
// the t-parameter now follows the metric's VISUAL slot in the
// bottom-center panel (HUD_VISUAL_ORDER) instead of the flat 24-row
// stack: the header pair (fps/tgt) + footer (screensize) are bright
// caps (t=1.0) and the 21 grid-body slots sweep t=0.0 → t=1.0.

/// HD-01 (HUD chroma dragon integration): compute the 24-stop chroma
/// gradient mapped through the panel's VISUAL slot order — one palette
/// stop per metric, positioned where the metric actually renders.
///
/// v80.0.0-beta.3 (branch hud-scifi-dashboard, owner-approved Option B):
/// the HUD renders as a bottom-center 3-column grid panel. The t
/// parameter follows `HUD_VISUAL_ORDER` (hud_init.rs):
///
/// - visual slots 0-1 (header strip: fps, tgt) → t = 1.0 — the BRIGHT
///   head. "FPS on top" as a bright hero strip is part of the
///   owner-approved Option B mock (the doc's "bright FPS header strip").
/// - visual slots 2-22 (grid body, 7 rows × 3 cells) → t sweeps
///   (v-2)/20.0 from 0.0 (first grid row: ehs/prs/scn — dim tail) to
///   1.0 (last grid row: rss/cid/up — bright head). The rain-aesthetic
///   dim→bright orientation survives inside the grid body.
/// - visual slot 23 (footer strip: screensize) → t = 1.0 — BRIGHT,
///   matching the header as the panel's closing anchor.
///
/// The result reads as bright caps (header + footer + corners + `▼`
/// accent, which all render in the t=1.0 stop) closing a gradient hull
/// — the "space capsule" silhouette of the approved Option B mock.
///
/// ## Why one color per metric (not per panel ROW)
/// Each grid cell is a distinct mini text block; giving each metric its
/// own interpolated stop extends the message border's per-cell sweep
/// philosophy (BC-02) to text cells, and it keeps the 24-metric color
/// identities stable across layout changes (indices are the metric
/// identities — see `cached_lines` in mod.rs).
///
/// ## Brightness floor
/// `brighten_color` is applied AFTER interpolation to every stop. This
/// guarantees every metric cell is legible on a black background, even
/// when palette[0] is a near-black start stop — it gets boosted to
/// neutral grey RGB(120,120,120) when pure black, preserving readability
/// without losing the palette's hue identity for non-black stops.
///
/// Returns a fixed-size `[Color; 24]` array (no allocation, stack-only),
/// indexed by METRIC index (same indexing as `cached_lines`).
pub(crate) fn compute_chroma_gradient_panel(palette_colors: &[Color]) -> [Color; 24] {
    let n = palette_colors.len();
    let mut out = std::array::from_fn(|_| Color::DarkGrey);
    if n == 0 {
        return out;
    }
    // v50 (2026-08-17) smoothness + v80.0.0-beta.3 panel mapping: every
    // metric's t lands in `interpolate_palette_color` (the same linear-
    // interpolation helper the border message gradient uses — the C5
    // fix that eliminated visible bands on small palettes). The t
    // schedule per visual slot v:
    //   v ∈ {0, 1, 23} (header/footer caps) → t = 1.0 (bright head)
    //   v ∈ 2..=22      (grid body)        → t = (v - 2) / 20.0
    // VISUAL slots map to metric indices via HUD_VISUAL_ORDER, so the
    // result stays indexed by metric while following VISUAL position.
    for (v, &metric) in HUD_VISUAL_ORDER.iter().enumerate() {
        let t = match v {
            0 | 1 | 23 => 1.0,
            _ => (v as f32 - 2.0) / 20.0,
        };
        let interpolated = crate::cloud::interpolate_palette_color(palette_colors, t);
        out[metric] = brighten_color(interpolated.unwrap_or(Color::DarkGrey));
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
