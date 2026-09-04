// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `radar` — sonar sweep reveal from the top-left anchor.
//!
//! The first follow-up masterclass candidate from the post-cascade
//! expansion (see `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md`
//! §3.E — spatial/positional family, currently empty). The 10 shipped
//! styles are all time-paced (typewriter/engrave/hologram/glitch/
//! scorch/cascade) or instant (fade/instant/words/slide); radar is
//! the first SPATIAL style: a sonar sweep rotates clockwise from the
//! top-left corner anchor, and each content cell reveals when the
//! sweep beam crosses its angle from the anchor.
//!
//! ## Reveal math (stateless)
//!
//! Each content cell is mapped to a 2D position `(x, y)` where `x` is
//! the cell's column index (content_idx) and `y` is the cell's row
//! (always 0 on a 1-line overlay — the default). The angle from the
//! top-left anchor `(0, 0)` to `(x, y)` is `atan2(y + 1, x + 1)` (the
//! `+1` avoids the degenerate `atan2(0, 0)` at the anchor itself).
//!
//! On a 1-line overlay, `y = 0` so every cell sits on the horizontal
//! axis — `atan2(1, x + 1)`. The angle decreases as `x` increases
//! (cell 0 = atan2(1, 1) = 45°, cell 10 = atan2(1, 11) ≈ 5°). The
//! sweep rotates clockwise from 90° (top) to 0° (right), so cells
//! reveal in REVERSE order (right-to-left) on a 1-line overlay —
//! distinct from typewriter (left-to-right) and cascade (left-to-right).
//!
//! On a multi-line overlay, the angle encodes the cell's diagonal
//! distance from the anchor — cells near the diagonal reveal first,
//! cells far from the diagonal (top-right or bottom-left) reveal
//! last. The visual reads as "sonar pings radiating outward from the
//! top-left corner" — a spatial sweep, not a temporal one.
//!
//! ## Sweep beam + ping boost
//!
//! The sweep beam covers a small angular window
//! (`RADAR_BEAM_WIDTH_RAD` = 18° = ~0.314 rad). When the sweep angle
//! crosses a cell's angle, the cell enters a ping phase over
//! `RADAR_PING_MS` (200 ms): the glyph starts at a dim factor
//! (`RADAR_PING_DIM` = 0.50), spikes to `RADAR_PING_PEAK` (1.4 —
//! the head boost, same > 1.0 path as engrave/scorch) at the ping
//! midpoint, then settles to 1.0. The ping is the "sonar echo"
//! — the cell flashes as the beam passes, then dims to settle.
//!
//! ## Why no slide_rows / glyph_override / tint
//!
//! Radar is purely brightness-modulated — the spatial sweep is the
//! reveal math, not a positional or color animation. The `factor`
//! field alone carries the ping curve. This keeps radar in the
//! "stateless, zero API extension" bucket (same as hologram's
//! reveal math — only hologram's scanline pass needed the sidecar).
//!
//! ## --no-effects contract
//!
//! Radar has NO particle sidecar — the ping boost is the reveal math,
//! not a cosmetic overlay. So `--no-effects` does NOT gate anything
//! in this style (same contract as glitch/cascade).
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::{index_fraction, index_pacing, lagged_border, CellReveal};

// ── Reveal math constants (stateless) ───────────────────────────────────────

/// Sweep rotation duration. The sonar beam completes one full sweep
/// (90° → 0° on a 1-line overlay, or 180° → 0° on multi-line) over
/// this window. 1500 ms = ~1.5 s for the full sweep — slower than
/// cascade's 1.2 s (60 ms × 20 chars) because radar is a deliberate,
/// scanning feel, not a fast waterfall.
pub(crate) const RADAR_SWEEP_MS: usize = 1500;

/// Sweep angular range (radians). The beam starts at PI/2 (90°, top)
/// and rotates clockwise to 0 (right). On a 1-line overlay, every
/// cell's angle is in (0, PI/4] (atan2(1, x+1) for x in 0..N), so the
/// full sweep covers them. On multi-line, cells can have angles up to
/// PI/2 (directly below the anchor), so the sweep covers PI/2 → 0.
pub(crate) const RADAR_SWEEP_START_RAD: f32 = std::f32::consts::FRAC_PI_2;

/// Beam angular width. 18° ≈ 0.314 rad. Cells within this window of
/// the current sweep angle get the ping boost; cells outside are
/// either not-yet-revealed or settled. Wider beam = more cells ping
/// simultaneously (less spatial resolution); narrower beam = fewer
/// cells ping (more scanning feel). 18° is the sonar-typical beam
/// width — wide enough to read as a sweep, narrow enough to feel
/// directional.
pub(crate) const RADAR_BEAM_WIDTH_RAD: f32 = 0.314_159_27;

/// Ping phase duration. After the beam crosses a cell's angle, the
/// cell enters a 200 ms ping: dim → peak → settle. 200 ms = ~12
/// frames at 60 FPS, enough to read as a "flash" without strobing.
pub(crate) const RADAR_PING_MS: usize = 200;

/// Ping starting brightness (dim). 0.50 = the glyph starts at 50%
/// brightness when the beam first touches it. Lower than cascade's
/// 0.40 because the ping SPIKES upward (to RADAR_PING_PEAK), so the
/// dim start gives the spike more visual range.
pub(crate) const RADAR_PING_DIM: f32 = 0.50;

/// Ping peak brightness (head boost). 1.4 = the glyph flashes to 140%
/// brightness at the ping midpoint, then settles to 1.0. Same > 1.0
/// path as engrave (2x hot head) and scorch (1.5x ember head) — the
/// renderer clamps downstream. The peak is the "sonar echo" — the
/// cell flashes as the beam passes.
pub(crate) const RADAR_PING_PEAK: f32 = 1.40;

// ── Reveal math (stateless) ────────────────────────────────────────────────

/// Compute a cell's angle from the top-left anchor (0, 0).
///
/// `content_idx` is the cell's column index (x). The row (y) is always
/// 0 on a 1-line overlay (the default), but the math is general: the
/// `+1` offset on both axes avoids the degenerate `atan2(0, 0)` at
/// the anchor itself, and gives cell 0 a meaningful 45° starting
/// angle on a 1-line overlay.
///
/// Returns the angle in radians, in `(0, PI/2]`. Cells closer to the
/// anchor (small x) have larger angles (closer to 90°); cells farther
/// from the anchor (large x) have smaller angles (closer to 0°).
#[inline]
fn cell_angle(content_idx: usize) -> f32 {
    // y = 0 (1-line overlay default). The +1 avoids atan2(0, 0).
    // atan2(1, x + 1): cell 0 = atan2(1, 1) = 45° (PI/4);
    // cell 10 = atan2(1, 11) ≈ 5° (0.087 rad).
    let x = (content_idx as f32) + 1.0;
    let y = 1.0_f32;
    y.atan2(x)
}

/// Compute the current sweep angle from elapsed time.
///
/// The sweep starts at `RADAR_SWEEP_START_RAD` (PI/2 = 90°, top) and
/// rotates clockwise to 0 (right) over `RADAR_SWEEP_MS`. After the
/// sweep completes, the angle stays at 0 (every cell has been pinged).
///
/// `elapsed_ms = None` means "no timeline" → return 0 (sweep complete,
/// every cell settled — the bench/edge path).
#[inline]
fn sweep_angle(elapsed_ms: Option<usize>) -> f32 {
    let Some(ms) = elapsed_ms else {
        return 0.0;
    };
    let progress = (ms as f32 / RADAR_SWEEP_MS as f32).clamp(0.0, 1.0);
    RADAR_SWEEP_START_RAD * (1.0 - progress)
}

/// Per-cell reveal: sonar sweep from the top-left anchor.
///
/// Pure function of `(content_idx, elapsed_ms, reveal_count)` — no
/// per-frame state, no per-cell bookkeeping in `Cloud`. The cell's
/// angle from the anchor determines when the sweep beam crosses it;
/// the ping phase follows.
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    if content_idx >= reveal_count {
        return CellReveal::hidden();
    }
    let Some(ms) = elapsed_ms else {
        // No timeline (bench/edge): settled immediately.
        return CellReveal::settled();
    };

    let angle = cell_angle(content_idx);
    let sweep = sweep_angle(Some(ms));

    // The sweep rotates from PI/2 → 0. A cell is "crossed" when the
    // sweep angle drops to within RADAR_BEAM_WIDTH_RAD of the cell's
    // angle. Before crossing: hidden. After crossing + ping window:
    // settled. During the ping window: dim → peak → settle.
    let sweep_ahead = sweep - angle; // > 0 = sweep hasn't reached cell yet

    if sweep_ahead > RADAR_BEAM_WIDTH_RAD {
        // Sweep hasn't reached this cell yet: hidden.
        return CellReveal::hidden();
    }

    // The sweep crossed the cell at some past instant. Compute the
    // ping age: time since the sweep angle was exactly at the cell's
    // angle (the center of the beam window).
    //
    // sweep(t) = RADAR_SWEEP_START_RAD * (1 - t / RADAR_SWEEP_MS)
    // sweep = angle  =>  t_cross = RADAR_SWEEP_MS * (1 - angle / RADAR_SWEEP_START_RAD)
    let t_cross_ms = (RADAR_SWEEP_MS as f32 * (1.0 - angle / RADAR_SWEEP_START_RAD)) as usize;
    let ping_age = ms.saturating_sub(t_cross_ms);

    if ping_age >= RADAR_PING_MS {
        // Ping complete: settled at full brightness.
        CellReveal::settled()
    } else {
        // Ping phase: dim → peak → settle over RADAR_PING_MS.
        // The ping curve is a triangle: ramp from RADAR_PING_DIM to
        // RADAR_PING_PEAK over the first half, then from RADAR_PING_PEAK
        // to 1.0 over the second half. This gives the "echo flash"
        // shape — a quick spike then settle.
        let progress = ping_age as f32 / RADAR_PING_MS as f32;
        let factor = if progress < 0.5 {
            // First half: dim → peak.
            let p = progress * 2.0;
            RADAR_PING_DIM + (RADAR_PING_PEAK - RADAR_PING_DIM) * p
        } else {
            // Second half: peak → settle.
            let p = (progress - 0.5) * 2.0;
            RADAR_PING_PEAK + (1.0 - RADAR_PING_PEAK) * p
        };
        CellReveal {
            visible: true,
            factor,
            slide_rows: 0,
            glyph_override: None,
            tint: None,
        }
    }
}

/// Index budget: cells revealed per ~75 ms of sweep (the sweep covers
/// PI/2 rad over RADAR_SWEEP_MS, so the per-cell reveal time depends
/// on the cell's angle — but the budget gate uses a linear approximation
/// so the renderer's `content_idx < reveal_count` check works). The
/// `.max(1)` floor mirrors the pre-v80.0.0-beta.1 contract (first cell
/// at t=0).
///
/// The linear approximation: assume cells are evenly spaced in angle
/// (they're not — atan2 is non-linear — but the approximation is close
/// enough for the budget gate, which only needs to cap eligibility).
/// The average per-cell time is RADAR_SWEEP_MS / total_text.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    // The sweep covers PI/2 rad over RADAR_SWEEP_MS. Cells are at
    // angles atan2(1, x+1) for x in 0..total_text. The angular span
    // per cell varies, but the budget gate uses a linear time pacing
    // (cell_idx * (RADAR_SWEEP_MS / total_text)) as the eligibility
    // cap — the actual reveal happens when the sweep crosses the
    // cell's angle (see `reveal`), not at this linear time.
    let per_cell_ms = (RADAR_SWEEP_MS / total_text.max(1)).max(1);
    index_pacing(per_cell_ms, elapsed_ms, total_text)
}

/// Border lags text (t^1.5) — the pre-v80.0.0-beta.1 cinematic curve.
pub(super) fn border_progress(text_progress: f32) -> f32 {
    lagged_border(text_progress)
}

/// Text progress: revealed-cell fraction.
pub(super) fn text_progress(reveal_count: usize, total_text: usize) -> f32 {
    index_fraction(reveal_count, total_text)
}

#[cfg(test)]
mod tests {
    use super::super::{content_reveal, index_reveal_count, MsgFillStyle};
    use super::*;

    #[test]
    fn radar_reveals_progressively_as_sweep_rotates() {
        // The sweep covers PI/2 → 0 over RADAR_SWEEP_MS (1500 ms).
        // Cell 0 is at angle PI/4 (atan2(1, 1)). The sweep reaches
        // PI/4 at t_cross = RADAR_SWEEP_MS * (1 - (PI/4) / (PI/2))
        //                  = 1500 * (1 - 0.5) = 750 ms.
        // Before 750 ms (minus beam width): cell 0 hidden.
        // After 750 ms + RADAR_PING_MS (200 ms) = 950 ms: cell 0 settled.
        let total = 10;
        // At 0 ms: sweep at PI/2, no cell crossed yet (cell 0 angle is
        // PI/4, sweep is at PI/2, sweep_ahead = PI/4 > beam width).
        // But the .max(1) floor in reveal_budget means reveal_count = 1
        // at t=0, so cell 0 IS eligible — and the sweep hasn't reached
        // it, so it's hidden.
        let count = index_reveal_count(MsgFillStyle::Radar, Some(0), total);
        assert_eq!(count, 1, "max(1) floor: first cell at t=0");
        let r = content_reveal(MsgFillStyle::Radar, 0, 1, Some(0), 1, 1.0);
        assert!(
            !r.visible,
            "cell 0 must be hidden at t=0 (sweep hasn't reached it)"
        );

        // At 950 ms (cell 0 ping complete): settled.
        let r = content_reveal(MsgFillStyle::Radar, 0, 1, Some(950), 10, 1.0);
        assert!(r.visible, "cell 0 must be visible at 950ms (ping complete)");
        assert!(
            (r.factor - 1.0).abs() < 1e-6,
            "cell 0 factor must be 1.0 at 950ms (settled)"
        );
    }

    #[test]
    fn radar_settles_after_sweep_completes() {
        // At large elapsed (sweep at 0, all cells crossed + ping complete):
        // every cell settled.
        let r = content_reveal(MsgFillStyle::Radar, 5, 1, Some(60_000), 10, 1.0);
        assert!(r.visible);
        assert!(
            (r.factor - 1.0).abs() < 1e-6,
            "factor must be 1.0 after sweep"
        );
        assert_eq!(r.slide_rows, 0);
        assert!(r.tint.is_none());
        assert!(r.glyph_override.is_none());
    }

    #[test]
    fn radar_settles_without_timeline() {
        // No timeline (bench/edge): settled immediately.
        let r = content_reveal(MsgFillStyle::Radar, 0, 1, None, 10, 1.0);
        assert!(r.visible);
        assert_eq!(r.slide_rows, 0);
        assert!((r.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn radar_ping_curve_goes_dim_to_peak_to_settle() {
        // Cell 0 crosses at t_cross = 750 ms (see test above).
        // Ping phase: 750..950 ms.
        // At 750 ms (ping_age=0): factor = RADAR_PING_DIM (0.50).
        // At 850 ms (ping_age=100, progress=0.5): factor = RADAR_PING_PEAK (1.40).
        // At 950 ms (ping_age=200): settled at 1.0.
        let dim = content_reveal(MsgFillStyle::Radar, 0, 1, Some(750), 10, 1.0);
        assert!(dim.visible);
        assert!(
            (dim.factor - RADAR_PING_DIM).abs() < 1e-3,
            "factor at ping start ({}) must be RADAR_PING_DIM ({})",
            dim.factor,
            RADAR_PING_DIM
        );

        let peak = content_reveal(MsgFillStyle::Radar, 0, 1, Some(850), 10, 1.0);
        assert!(peak.visible);
        assert!(
            (peak.factor - RADAR_PING_PEAK).abs() < 1e-3,
            "factor at ping midpoint ({}) must be RADAR_PING_PEAK ({})",
            peak.factor,
            RADAR_PING_PEAK
        );

        let settled = content_reveal(MsgFillStyle::Radar, 0, 1, Some(950), 10, 1.0);
        assert!(settled.visible);
        assert!(
            (settled.factor - 1.0).abs() < 1e-6,
            "factor at ping end ({}) must be 1.0 (settled)",
            settled.factor
        );
    }

    #[test]
    fn radar_hidden_until_sweep_reaches_cell() {
        // Cell 5 is at angle atan2(1, 6) ≈ 0.165 rad (9.46°).
        // t_cross = 1500 * (1 - 0.165 / (PI/2)) ≈ 1342 ms.
        // The beam window is RADAR_BEAM_WIDTH_RAD = 0.314 rad (~18°),
        // so the cell becomes visible when sweep - cell_angle <= beam_width.
        // sweep(t) = PI/2 * (1 - t/1500). Solve for sweep = cell_angle + beam_width:
        //   PI/2 * (1 - t/1500) = 0.165 + 0.314 = 0.479
        //   t = 1500 * (1 - 0.479 / (PI/2)) ≈ 1043 ms.
        // So cell 5 is hidden before ~1043 ms (sweep too far ahead).
        let t_cross =
            (RADAR_SWEEP_MS as f32 * (1.0 - cell_angle(5) / RADAR_SWEEP_START_RAD)) as usize;
        // Pick a time well before the beam window opens (t_cross - 400
        // is comfortably before the beam touches cell 5).
        let before = content_reveal(
            MsgFillStyle::Radar,
            5,
            1,
            Some(t_cross.saturating_sub(400)),
            10,
            1.0,
        );
        assert!(
            !before.visible,
            "cell 5 must be hidden well before sweep reaches it"
        );

        // At crossing (ping start):
        let at_cross = content_reveal(MsgFillStyle::Radar, 5, 1, Some(t_cross), 10, 1.0);
        assert!(
            at_cross.visible,
            "cell 5 must be visible at t_cross (ping start)"
        );
    }

    #[test]
    fn radar_hidden_outside_reveal_budget() {
        // Cells beyond reveal_count are always hidden.
        let r = content_reveal(MsgFillStyle::Radar, 10, 1, Some(60_000), 5, 1.0);
        assert!(!r.visible, "cell 10 must be hidden when reveal_count == 5");
    }

    #[test]
    fn radar_factor_never_exceeds_ping_peak() {
        // The ping curve peaks at RADAR_PING_PEAK (1.40) at the midpoint.
        // Sample every 10 ms across the ping window and verify the
        // factor never exceeds RADAR_PING_PEAK + epsilon (clamping
        // happens downstream, but the math should not overshoot).
        let t_cross =
            (RADAR_SWEEP_MS as f32 * (1.0 - cell_angle(0) / RADAR_SWEEP_START_RAD)) as usize;
        for ping_age in 0..=RADAR_PING_MS {
            let ms = t_cross + ping_age;
            let r = content_reveal(MsgFillStyle::Radar, 0, 1, Some(ms), 10, 1.0);
            if r.visible {
                assert!(
                    r.factor <= RADAR_PING_PEAK + 1e-3,
                    "factor {} at ping_age={} must not exceed RADAR_PING_PEAK ({})",
                    r.factor,
                    ping_age,
                    RADAR_PING_PEAK
                );
            }
        }
    }

    #[test]
    fn radar_cell_angle_decreases_with_column_index() {
        // Cells farther from the anchor (larger x) have smaller angles
        // (closer to 0 = right). This is the spatial property that makes
        // radar reveal right-to-left on a 1-line overlay (cells at small
        // angles are crossed LATER by the clockwise sweep).
        let angle_0 = cell_angle(0);
        let angle_5 = cell_angle(5);
        let angle_10 = cell_angle(10);
        assert!(
            angle_0 > angle_5,
            "cell 0 angle ({}) > cell 5 angle ({})",
            angle_0,
            angle_5
        );
        assert!(
            angle_5 > angle_10,
            "cell 5 angle ({}) > cell 10 angle ({})",
            angle_5,
            angle_10
        );
    }

    #[test]
    fn radar_constants_hold_research_doc_contract() {
        // Lock the values called out in the doc comment so a future
        // round can't drift them silently.
        assert_eq!(RADAR_SWEEP_MS, 1500);
        assert_eq!(RADAR_SWEEP_START_RAD, std::f32::consts::FRAC_PI_2);
        assert!((RADAR_BEAM_WIDTH_RAD - 0.314_159_27).abs() < 1e-3);
        assert_eq!(RADAR_PING_MS, 200);
        assert!((RADAR_PING_DIM - 0.50).abs() < 1e-6);
        assert!((RADAR_PING_PEAK - 1.40).abs() < 1e-6);
    }
}
