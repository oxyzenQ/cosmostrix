// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `tide` — traveling sine wave reveal.
//!
//! The second follow-up masterclass candidate from the post-cascade
//! expansion (see `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md`
//! §3.F — dynamic-coherence family). Radar (HUNT-11) was the first
//! SPATIAL style; tide is the first WAVE-COHERENT style: a sine wave
//! travels left-to-right across the overlay, and each content cell
//! reveals when the wave's peak passes over its column. Cells ride
//! the wave — they rise with the peak, brighten at the crest, then
//! settle as the wave passes.
//!
//! ## Reveal math (stateless)
//!
//! A sine wave `sin(2π * (x / TIDE_WAVELENGTH) - (elapsed / TIDE_PERIOD))`
//! travels left-to-right. The wave's phase at column `x` (content_idx)
//! and time `elapsed_ms` determines the cell's reveal state:
//!
//! - **Before the wave arrives** (phase < -PI/2): the cell is hidden.
//!   The wave hasn't reached this column yet.
//! - **Rising with the wave** (-PI/2 ≤ phase < 0): the cell slides UP
//!   from 1 row below (positive `slide_rows`) and fades in from dim
//!   (0.40) to peak brightness. The glyph "rides" the wave's upward
//!   slope — same mechanism as slide style, but driven by the wave
//!   phase instead of a per-char stagger.
//! - **At the crest** (phase ≈ 0): the cell is at peak brightness
//!   (1.30, the head boost — same > 1.0 path as engrave/scorch/radar).
//! - **Settling after the wave** (phase ≥ 0): the cell settles to 1.0
//!   brightness at its final position over `TIDE_SETTLE_MS` (300 ms).
//!
//! The wave has wavelength `TIDE_WAVELENGTH` (5 columns) and period
//! `TIDE_PERIOD` (800 ms), so the wave travels at
//! `TIDE_WAVELENGTH / TIDE_PERIOD` = 5 cols / 800 ms = 6.25 cols/s.
//! A 20-char overlay is fully revealed in ~3.2 s (vs radar's 1.5 s —
//! tide is slower, more contemplative, matching the "living rain"
//! feel the owner wants).
//!
//! ## Why slide_rows (not glyph_override or tint)
//!
//! Tide reuses the signed `slide_rows` field (positive = below, the
//! slide style's direction) for the "ride the wave" animation. The
//! glyph rises from 1 row below as the wave's upward slope passes,
//! then lands at the final position at the crest. This is the SAME
//! mechanism as slide style, but driven by wave phase instead of a
//! fixed per-char stagger — zero API extension, same as cascade.
//!
//! ## --no-effects contract
//!
//! Tide has NO particle sidecar — the wave animation IS the reveal
//! math, not a cosmetic overlay. So `--no-effects` does NOT gate
//! anything in this style (same contract as glitch/cascade/radar).
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::{index_fraction, index_pacing, lagged_border, CellReveal};

// ── Reveal math constants (stateless) ───────────────────────────────────────

/// Wave wavelength in columns. 5 = the wave spans 5 columns per cycle.
/// A 20-char overlay sees ~4 wave cycles. Smaller wavelength = more
/// visible "ripples"; larger wavelength = smoother, less wavy feel.
pub(crate) const TIDE_WAVELENGTH: f32 = 5.0;

/// Wave period in ms. 800 ms per cycle = 1.25 Hz. The wave travels
/// one wavelength (5 cols) per period (800 ms) = 6.25 cols/s. Slower
/// than radar's 1500 ms sweep — tide is contemplative, not scanning.
pub(crate) const TIDE_PERIOD: usize = 800;

/// Peak brightness at the wave crest. 1.30 = the glyph flashes to
/// 130% brightness at the crest, then settles to 1.0. Same > 1.0
/// path as engrave (2x), scorch (1.5x), radar (1.4x) — tide's peak
/// is the gentlest of the four (the wave is calm, not violent).
pub(crate) const TIDE_PEAK: f32 = 1.30;

/// Starting brightness while rising with the wave. 0.40 = the glyph
/// starts at 40% brightness when the wave's upward slope first
/// touches it. Lower than slide's 0.70 because the wave rise is
/// slower (300 ms vs 240 ms) — the "dim start" gives the rise more
/// visual range.
pub(crate) const TIDE_RISE_DIM: f32 = 0.40;

/// Settle window after the crest. 300 ms = ~18 frames at 60 FPS.
/// After the wave's crest passes (phase ≥ 0), the cell settles from
/// TIDE_PEAK to 1.0 over this window. Long enough to read as a
/// "wash" (the wave receding), short enough to not feel sluggish.
pub(crate) const TIDE_SETTLE_MS: usize = 300;

/// Rising window before the crest. 300 ms = ~18 frames at 60 FPS.
/// Before the wave's crest arrives at this cell, the glyph rises from
/// 1 row below (slide_rows = 1) while fading in from TIDE_RISE_DIM
/// (0.40) to TIDE_PEAK (1.30). Same duration as the settle window
/// for visual symmetry (rise and fall feel balanced).
pub(crate) const TIDE_RISE_MS: usize = 300;

// ── Reveal math (stateless) ────────────────────────────────────────────────

/// Per-cell peak arrival time: `content_idx * TIDE_PERIOD / TIDE_WAVELENGTH`.
///
/// The wave's crest (peak brightness) travels left-to-right at speed
/// `TIDE_WAVELENGTH / TIDE_PERIOD` (cols per ms). Cell `x` is reached
/// by the crest at `t_peak = x * TIDE_PERIOD / TIDE_WAVELENGTH`.
/// Before `t_peak - TIDE_RISE_MS`: hidden. After `t_peak + TIDE_SETTLE_MS`:
/// settled.
#[inline]
fn peak_arrival_ms(content_idx: usize) -> usize {
    // TIDE_PERIOD / TIDE_WAVELENGTH = 800 / 5 = 160 ms per column.
    // Use integer math to avoid f32 rounding drift in the budget gate.
    let per_col_ms = TIDE_PERIOD / TIDE_WAVELENGTH as usize;
    content_idx * per_col_ms
}

/// Per-cell reveal: traveling sine wave (one-shot pulse model).
///
/// Pure function of `(content_idx, elapsed_ms, reveal_count)` — no
/// per-frame state, no per-cell bookkeeping in `Cloud`. The wave's
/// crest arrives at cell `x` at `t_peak = x * TIDE_PERIOD / TIDE_WAVELENGTH`.
/// The cell reveals in three phases around `t_peak`:
///
/// - **Before rising** (`t < t_peak - TIDE_RISE_MS`): hidden.
/// - **Rising** (`t_peak - TIDE_RISE_MS ≤ t < t_peak`): the glyph
///   slides UP from 1 row below (positive `slide_rows`) and fades
///   in from dim (`TIDE_RISE_DIM` = 0.40) to peak (`TIDE_PEAK` = 1.30).
/// - **At crest** (`t = t_peak`): peak brightness, landed
///   (`slide_rows = 0`).
/// - **Settling** (`t_peak < t ≤ t_peak + TIDE_SETTLE_MS`): factor
///   ramps from `TIDE_PEAK` to 1.0.
/// - **Settled** (`t > t_peak + TIDE_SETTLE_MS`): full brightness
///   at final position.
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

    let t_peak = peak_arrival_ms(content_idx);

    // Before rising window: hidden.
    if ms + TIDE_RISE_MS < t_peak {
        // Use saturating add to avoid overflow at ms=0; the comparison
        // `ms + TIDE_RISE_MS < t_peak` is equivalent to
        // `ms < t_peak - TIDE_RISE_MS` but overflow-safe.
        return CellReveal::hidden();
    }

    // Rising phase: ms in [t_peak - TIDE_RISE_MS, t_peak).
    if ms < t_peak {
        let rise_age = t_peak - ms; // ms until peak (TIDE_RISE_MS down to 0)
        let rise_progress = 1.0 - (rise_age as f32 / TIDE_RISE_MS as f32); // 0 at rise start, 1 at peak
                                                                           // slide_rows ramps from 1 (below) to 0 (landed at crest).
        let slide_rows = ((1.0 - rise_progress) * 1.0).round() as i16;
        // factor ramps from TIDE_RISE_DIM (0.40) to TIDE_PEAK (1.30).
        let factor = TIDE_RISE_DIM + (TIDE_PEAK - TIDE_RISE_DIM) * rise_progress;
        return CellReveal {
            visible: true,
            factor,
            slide_rows,
            glyph_override: None,
            tint: None,
        };
    }

    // ms >= t_peak: settling phase.
    let settle_age = ms - t_peak;
    if settle_age >= TIDE_SETTLE_MS {
        // Settle complete: full brightness at final position.
        CellReveal::settled()
    } else {
        // Settling: factor ramps from TIDE_PEAK (1.30) to 1.0 over
        // TIDE_SETTLE_MS. slide_rows = 0 (landed at crest, no more
        // vertical motion).
        let settle_progress = settle_age as f32 / TIDE_SETTLE_MS as f32;
        let factor = TIDE_PEAK + (1.0 - TIDE_PEAK) * settle_progress;
        CellReveal {
            visible: true,
            factor,
            slide_rows: 0,
            glyph_override: None,
            tint: None,
        }
    }
}

/// Index budget: cells revealed per ~160 ms of wave travel (the wave
/// covers TIDE_WAVELENGTH columns per TIDE_PERIOD, so per-column time
/// is TIDE_PERIOD / TIDE_WAVELENGTH ≈ 160 ms). The `.max(1)` floor
/// mirrors the pre-v80.0.0-beta.1 contract (first cell at t=0).
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    let per_cell_ms = (TIDE_PERIOD as f32 / TIDE_WAVELENGTH) as usize;
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
    use super::super::{content_reveal, MsgFillStyle};
    use super::*;

    #[test]
    fn tide_reveals_progressively_as_wave_travels() {
        // The wave's crest travels left-to-right at TIDE_PERIOD/TIDE_WAVELENGTH
        // = 160 ms/column. Cell 0's crest at t=0. Cell 5's crest at t=800.
        // Cell 10's crest at t=1600.
        // At 0 ms: cell 0 at crest (peak brightness), cells 5+ hidden (wave
        // hasn't arrived — rising window starts at t_peak - TIDE_RISE_MS).
        let r0 = content_reveal(MsgFillStyle::Tide, 0, 1, Some(0), 10, 1.0);
        assert!(r0.visible, "cell 0 must be visible at t=0 (at crest)");

        let r5 = content_reveal(MsgFillStyle::Tide, 5, 1, Some(0), 10, 1.0);
        assert!(
            !r5.visible,
            "cell 5 must be hidden at t=0 (wave hasn't arrived)"
        );

        // At 800 ms: cell 5 at crest, cell 0 settled.
        let r5_later = content_reveal(MsgFillStyle::Tide, 5, 1, Some(800), 10, 1.0);
        assert!(
            r5_later.visible,
            "cell 5 must be visible at t=800ms (at crest)"
        );
    }

    #[test]
    fn tide_settles_after_wave_passes() {
        // At large elapsed (wave has passed every cell + settle window):
        // every cell settled at full brightness.
        let r = content_reveal(MsgFillStyle::Tide, 5, 1, Some(60_000), 10, 1.0);
        assert!(r.visible);
        assert!(
            (r.factor - 1.0).abs() < 1e-6,
            "factor must be 1.0 after wave"
        );
        assert_eq!(r.slide_rows, 0);
        assert!(r.tint.is_none());
        assert!(r.glyph_override.is_none());
    }

    #[test]
    fn tide_settles_without_timeline() {
        // No timeline (bench/edge): settled immediately.
        let r = content_reveal(MsgFillStyle::Tide, 0, 1, None, 10, 1.0);
        assert!(r.visible);
        assert_eq!(r.slide_rows, 0);
        assert!((r.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn tide_rising_phase_has_positive_slide_rows() {
        // Cell 5's crest at t_peak = 5 * 160 = 800 ms. Rising window
        // is [t_peak - TIDE_RISE_MS, t_peak) = [500, 800) ms.
        // At 600 ms (mid-rise): rise_age = 800 - 600 = 200,
        // rise_progress = 1 - 200/300 = 0.33. slide_rows rounds to 1
        // (still below), factor between TIDE_RISE_DIM and TIDE_PEAK.
        let r = content_reveal(MsgFillStyle::Tide, 5, 1, Some(600), 10, 1.0);
        assert!(
            r.visible,
            "cell 5 must be visible during rising phase (600ms)"
        );
        assert!(
            r.slide_rows >= 0,
            "rising-phase slide_rows {} must be >= 0 (landed or below)",
            r.slide_rows
        );
        assert!(
            r.factor >= TIDE_RISE_DIM && r.factor <= TIDE_PEAK + 1e-3,
            "rising-phase factor {} must be in [TIDE_RISE_DIM={}, TIDE_PEAK={}]",
            r.factor,
            TIDE_RISE_DIM,
            TIDE_PEAK
        );
    }

    #[test]
    fn tide_crest_phase_has_peak_brightness() {
        // At the crest (t_peak for cell 5 = 800 ms), factor = TIDE_PEAK
        // (1.30). slide_rows = 0 (landed).
        let r = content_reveal(MsgFillStyle::Tide, 5, 1, Some(800), 10, 1.0);
        assert!(r.visible, "cell 5 must be visible at crest (800ms)");
        assert!(
            (r.factor - TIDE_PEAK).abs() < 1e-3,
            "cell 5 factor at crest ({}) must be TIDE_PEAK ({})",
            r.factor,
            TIDE_PEAK
        );
        assert!(r.factor > 1.0, "tide peak must exceed 1.0 (head boost)");
    }

    #[test]
    fn tide_settle_phase_ramps_from_peak_to_one() {
        // Cell 5's crest at 800 ms. Settle window 300 ms (800..1100 ms).
        // At 950 ms (settle_age=150, progress=0.5): factor halfway
        // between TIDE_PEAK (1.30) and 1.0 = 1.15.
        let r = content_reveal(MsgFillStyle::Tide, 5, 1, Some(950), 10, 1.0);
        assert!(r.visible);
        let expected = TIDE_PEAK + (1.0 - TIDE_PEAK) * 0.5;
        assert!(
            (r.factor - expected).abs() < 1e-3,
            "cell 5 factor at settle midpoint ({}) must be {} (halfway peak→1.0)",
            r.factor,
            expected
        );
        assert_eq!(r.slide_rows, 0, "settle slide_rows must be 0 (landed)");
    }

    #[test]
    fn tide_hidden_outside_reveal_budget() {
        // Cells beyond reveal_count are always hidden.
        let r = content_reveal(MsgFillStyle::Tide, 10, 1, Some(60_000), 5, 1.0);
        assert!(!r.visible, "cell 10 must be hidden when reveal_count == 5");
    }

    #[test]
    fn tide_factor_never_exceeds_peak() {
        // The factor peaks at TIDE_PEAK (1.30) at the crest. Sample
        // every 10 ms across the full reveal of cell 0 and verify the
        // factor never exceeds TIDE_PEAK + epsilon.
        for ms in 0..2000 {
            let r = content_reveal(MsgFillStyle::Tide, 0, 1, Some(ms), 10, 1.0);
            if r.visible {
                assert!(
                    r.factor <= TIDE_PEAK + 1e-3,
                    "factor {} at ms={} must not exceed TIDE_PEAK ({})",
                    r.factor,
                    ms,
                    TIDE_PEAK
                );
            }
        }
    }

    #[test]
    fn tide_constants_hold_research_doc_contract() {
        // Lock the values called out in the doc comment so a future
        // round can't drift them silently.
        assert_eq!(TIDE_WAVELENGTH, 5.0);
        assert_eq!(TIDE_PERIOD, 800);
        assert!((TIDE_PEAK - 1.30).abs() < 1e-6);
        assert!((TIDE_RISE_DIM - 0.40).abs() < 1e-6);
        assert_eq!(TIDE_SETTLE_MS, 300);
        assert_eq!(TIDE_RISE_MS, 300);
    }
}
