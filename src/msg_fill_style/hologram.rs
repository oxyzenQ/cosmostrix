// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `hologram` — projected hologram with scanline.
//!
//! The cheapest candidate in the post-engrave style expansion family
//! (see `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md`): fully
//! stateless, no sidecar, no per-frame bookkeeping, no new `CellReveal`
//! field. Brightness-only — every effect rides the existing `factor`
//! path (factors > 1.0 go through the unclamped boost path like pulse
//! and engrave; < 1.0 through the dim path).
//!
//! ## Reveal math (stateless)
//!
//! Each content cell burns in at full brightness the moment the head
//! reaches it (80 ms/char, same pacing as typewriter/engrave — no 30%
//! fade-in: a hologram snaps on, it does not fade in), then runs
//! through three deterministic phases driven entirely by the elapsed
//! ms since the cell's reveal:
//!
//! 1. **Flicker** (0..150 ms post-reveal): per-cell brightness noise.
//!    A deterministic hash of (content_idx, elapsed/40 ms bucket)
//!    maps to a multiplier in `1.0 ± HOLOGRAM_FLICKER_AMPLITUDE`
//!    (±30%). The 40 ms bucket gives ~3-4 distinct flicker values per
//!    cell during the 150 ms window — fast enough to read as
//!    "hologram interference", slow enough not to strobe.
//! 2. **Breathing** (150..2150 ms post-reveal): subtle 1-2% sin ripple
//!    on the settled brightness, amplitude decaying linearly to zero
//!    by the end of the window. Reads as the "hologram is alive" hum.
//! 3. **Settled** (>= 2150 ms post-reveal): factor = 1.0 exactly.
//!    Bit-identical to engrave's cooled state — hologram text is just
//!    text once the projection stabilizes.
//!
//! Without a timeline (`elapsed_ms = None`), every cell settles
//! instantly (factor = 1.0) — same `usize::MAX` reveal_count
//! semantics every stateless style uses for bench and edge paths.
//!
//! ## Scanline pass (also stateless)
//!
//! A single horizontal scanline sweeps the box top-to-bottom over
//! `HOLOGRAM_SCANLINE_MS` (600 ms) once, then is gone. Implemented as
//! a `Cloud::hologram_scanline_pass` invoked at the END of
//! `draw_message` (after the halo row, alongside the engrave spark
//! pass — only one is wired per style). The pass paints a row of
//! `▔` (U+2594 UPPER ONE EIGHTH BLOCK — a thin line at the top of
//! each cell, so it reads as a scanline crossing the cell without
//! obscuring the glyph body) across every cell of the scanline row
//! inside the message box. The next frame redraws the normal
//! content, so the scanline visually moves down one row per
//! `HOLOGRAM_SCANLINE_MS / height` elapsed — the classic CRT sweep.
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::{index_fraction, index_pacing, lagged_border, CellReveal};
use crate::cell::Cell;
use crate::frame::Frame;
use crate::runtime::ColorMode;

// ── Reveal math constants ───────────────────────────────────────────────────

/// Per-character reveal stagger (same 80 ms pacing as
/// typewriter/engrave, kept as its own constant so the three can
/// diverge if a future tuning round wants hologram to scan faster).
pub(crate) const HOLOGRAM_CHAR_MS: usize = 80;

/// Flicker window after reveal: deterministic per-cell brightness
/// noise. 150 ms = ~4 distinct 40 ms buckets per cell.
pub(crate) const HOLOGRAM_FLICKER_MS: usize = 150;

/// Flicker bucket size: one distinct flicker value per 40 ms. Lower
/// = smoother flicker, higher = choppier. 40 ms sits just under the
/// ~50 ms flicker fusion threshold so the noise reads as a steady
/// "interference" rather than a strobe.
pub(crate) const HOLOGRAM_FLICKER_BUCKET_MS: usize = 40;

/// Flicker amplitude: factor modulates in `1.0 ± 0.30` (70%..130%).
pub(crate) const HOLOGRAM_FLICKER_AMPLITUDE: f32 = 0.30;

/// Breathing ripple window after flicker: subtle sin hum that
/// decays to zero amplitude. 2000 ms = the "hologram warming up"
/// tail before the projection fully stabilizes.
pub(crate) const HOLOGRAM_RIPPLE_MS: usize = 2000;

/// Breathing ripple peak amplitude (decays linearly to 0 across
/// `HOLOGRAM_RIPPLE_MS`). 0.02 = ±2% brightness — subtle but
/// visible in 8-bit color (~5 levels per channel), per the
/// research doc's "1-2% brightness ripple" spec.
pub(crate) const HOLOGRAM_RIPPLE_AMPLITUDE: f32 = 0.02;

/// Breathing ripple visual frequency in Hz. 2 Hz = two brightness
/// oscillations per second — the "hologram hum" cadence without
/// being so fast it reads as a flicker.
pub(crate) const HOLOGRAM_RIPPLE_HZ: f32 = 2.0;

// ── Scanline constants ──────────────────────────────────────────────────────

/// Single scanline sweep duration: top of box to bottom, once,
/// then gone. 600 ms matches the typical overlay reveal window
/// (80 ms/char × ~7 chars), so the scanline completes around when
/// the text finishes burning in — the projection "stabilizes"
/// visibly as the scanline exits the bottom border.
pub(crate) const HOLOGRAM_SCANLINE_MS: usize = 600;

// ── Reveal math (stateless) ────────────────────────────────────────────────

/// Per-cell reveal: burn-in → flicker → breathing → settled.
///
/// Pure function of `(content_idx, elapsed_ms, reveal_count)` —
/// no per-frame state, no per-cell bookkeeping in `Cloud`. The
/// `CellReveal.slide_rows` field is always 0 (hologram cells do
/// not move — the slide style owns that channel).
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    if content_idx >= reveal_count {
        return CellReveal::hidden();
    }
    let reveal_at = content_idx * HOLOGRAM_CHAR_MS;
    let factor = match elapsed_ms {
        None => 1.0,
        Some(ms) => {
            let age = ms.saturating_sub(reveal_at);
            if age < HOLOGRAM_FLICKER_MS {
                // Phase 1: deterministic flicker (one brightness
                // sample per 40 ms bucket).
                let bucket = age / HOLOGRAM_FLICKER_BUCKET_MS;
                let noise = flicker_noise(content_idx, bucket);
                1.0 + noise * HOLOGRAM_FLICKER_AMPLITUDE
            } else if age < HOLOGRAM_FLICKER_MS + HOLOGRAM_RIPPLE_MS {
                // Phase 2: breathing ripple with linear amplitude
                // decay across the ripple window.
                let ripple_age = age - HOLOGRAM_FLICKER_MS;
                let ripple_progress = ripple_age as f32 / HOLOGRAM_RIPPLE_MS as f32;
                let decay = 1.0 - ripple_progress;
                let omega = std::f32::consts::TAU * HOLOGRAM_RIPPLE_HZ / 1000.0;
                let phase = ripple_age as f32 * omega;
                1.0 + HOLOGRAM_RIPPLE_AMPLITUDE * decay * phase.sin()
            } else {
                // Phase 3: settled.
                1.0
            }
        }
    };
    CellReveal {
        visible: true,
        factor,
        slide_rows: 0,
    }
}

/// Deterministic flicker noise in `[-1.0, 1.0)` from
/// `(content_idx, bucket)`.
///
/// A small FxHash-style multiply-xorshift. No `rand` dependency —
/// the flicker must be deterministic so the same cell at the same
/// elapsed bucket always renders the same brightness (otherwise
/// the hologram would shimmer randomly per frame, not per
/// bucket — and the LTS contract is that the same elapsed_ms
/// always produces the same frame).
#[inline]
fn flicker_noise(content_idx: usize, bucket: usize) -> f32 {
    // 32-bit hash with two odd multipliers (FxHash style). The
    // shift asymmetry between content_idx and bucket prevents
    // vertical stripes where adjacent cells flicker in lockstep.
    let mut h: u32 = (content_idx as u32).wrapping_mul(0x9E37_79B1);
    h ^= (bucket as u32).wrapping_mul(0x85EB_CA6B);
    h = h.wrapping_mul(0x27D4_EB2F).rotate_left(11);
    // Map u32 → [0, 1) using the high 24 bits (mantissa-sized —
    // lower bits of a multiply-xorshift hash are lower quality).
    let bits = h >> 8;
    let unit = (bits as f32) / ((1u32 << 24) as f32);
    // Center to [-1.0, 1.0).
    unit * 2.0 - 1.0
}

/// Index budget: 80 ms/char with the pre-v51 `.max(1)` floor.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(HOLOGRAM_CHAR_MS, elapsed_ms, total_text)
}

/// Border lags text (t^1.5) — the pre-v51 cinematic curve.
pub(super) fn border_progress(text_progress: f32) -> f32 {
    lagged_border(text_progress)
}

/// Text progress: revealed-cell fraction.
pub(super) fn text_progress(reveal_count: usize, total_text: usize) -> f32 {
    index_fraction(reveal_count, total_text)
}

// ── Scanline pass (stateless, called from draw_message) ─────────────────────

impl crate::cloud::Cloud {
    /// Hologram scanline pass — one entry point, called at the END
    /// of `draw_message` (after the halo row, alongside the
    /// `engrave_spark_pass` — only one is wired per style).
    /// Stateless: every input is derived from `elapsed_ms`.
    ///
    /// Paints a row of `▔` (U+2594 UPPER ONE EIGHTH BLOCK) across
    /// every message cell at the scanline row, in the palette head
    /// color. `▔` fills only the top 1/8 of each cell, so it reads
    /// as a thin scanline crossing the cell without obscuring the
    /// glyph body. The next frame redraws the normal content, so
    /// the scanline visually sweeps down the box.
    ///
    /// `elapsed_ms = None` (no animation timeline — bench/edge
    /// paths) skips the pass entirely.
    pub(crate) fn hologram_scanline_pass(&mut self, frame: &mut Frame, elapsed_ms: Option<usize>) {
        let Some(ms) = elapsed_ms else {
            return;
        };
        // Single sweep: after HOLOGRAM_SCANLINE_MS the scanline
        // is gone for the rest of the overlay's lifetime (Space
        // restart rewinds elapsed_ms to 0, re-arming the sweep).
        if ms >= HOLOGRAM_SCANLINE_MS {
            return;
        }
        // PERF-4: --no-effects suppresses the scanline pass the
        // same way every particle subsystem is suppressed. The
        // reveal math itself is unaffected (text still burns in
        // at full brightness — the scanline is a cosmetic overlay
        // on top, not part of the text reveal).
        if !self.effects_enabled {
            return;
        }
        // Determine box row range from the populated message
        // cells. Empty message → no-op.
        let mut min_line = u16::MAX;
        let mut max_line: u16 = 0;
        for mc in &self.message {
            if mc.line < min_line {
                min_line = mc.line;
            }
            if mc.line > max_line {
                max_line = mc.line;
            }
        }
        if min_line == u16::MAX {
            return;
        }
        let height = (max_line - min_line + 1) as f32;
        let progress = ms as f32 / HOLOGRAM_SCANLINE_MS as f32;
        // Saturating add guards degenerate tiny terminals; the
        // max_line guard below catches the (progress * height)
        // rounding into the row below the box.
        let scanline_row = min_line.saturating_add((progress * height) as u16);
        if scanline_row > max_line {
            return;
        }
        let bg = self.palette.bg;
        let scanline_fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            // Palette head color (near-white on most schemes):
            // the scanline follows the active theme like the
            // border spark and engrave head do.
            self.palette.colors.last().copied()
        };
        for mc in &self.message {
            if mc.line != scanline_row {
                continue;
            }
            frame.set_force(
                mc.col,
                mc.line,
                Cell {
                    ch: '▔',
                    fg: scanline_fg,
                    bg,
                    bold: false,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{content_reveal, index_reveal_count, MsgFillStyle};
    use super::*;

    #[test]
    fn hologram_reveals_at_80ms_per_char() {
        // Same index pacing as typewriter/engrave: 319 ms → 3
        // chars, 320 ms → 4. The `.max(1)` floor keeps the first
        // char visible at t=0.
        let total = 40;
        let count = index_reveal_count(MsgFillStyle::Hologram, Some(319), total);
        assert_eq!(count, 3);
        let count = index_reveal_count(MsgFillStyle::Hologram, Some(320), total);
        assert_eq!(count, 4);
        let count = index_reveal_count(MsgFillStyle::Hologram, Some(0), total);
        assert_eq!(count, 1, "max(1) floor: first char at t=0");
    }

    #[test]
    fn hologram_chars_settle_at_full_brightness_without_timeline() {
        // No timeline (bench/edge): every cell settles at 1.0,
        // matching the shared `none_elapsed_timeline_settles_everything`
        // contract in mod.rs.
        let r = content_reveal(MsgFillStyle::Hologram, 0, 1, None, 10, 1.0);
        assert!(r.visible);
        assert!((r.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hologram_flicker_stays_within_amplitude_band_during_reveal() {
        // Cell 0 reveals at t=0; age 0..150 ms is the flicker
        // phase. Factor must stay within
        // [1 - AMP, 1 + AMP] at every sampled bucket.
        let lo = 1.0 - HOLOGRAM_FLICKER_AMPLITUDE;
        let hi = 1.0 + HOLOGRAM_FLICKER_AMPLITUDE;
        for ms in [0usize, 1, 39, 40, 80, 119, 120, 149] {
            let r = content_reveal(MsgFillStyle::Hologram, 0, 1, Some(ms), 10, 1.0);
            assert!(r.visible, "must be visible at ms={ms}");
            assert!(
                r.factor >= lo - 1e-6 && r.factor <= hi + 1e-6,
                "factor {} at ms={} must stay within [{}, {}]",
                r.factor,
                ms,
                lo,
                hi
            );
        }
    }

    #[test]
    fn hologram_breathing_phase_stays_within_ripple_amplitude() {
        // 150 ms boundary: first breathing sample. Amplitude
        // decays linearly across the 2000 ms window.
        let early = content_reveal(
            MsgFillStyle::Hologram,
            0,
            1,
            Some(HOLOGRAM_FLICKER_MS + 1),
            10,
            1.0,
        );
        assert!(early.visible);
        assert!(
            (early.factor - 1.0).abs() <= HOLOGRAM_RIPPLE_AMPLITUDE + 1e-6,
            "early breathing factor {} must stay within ±{} of 1.0",
            early.factor,
            HOLOGRAM_RIPPLE_AMPLITUDE
        );
        // Mid-ripple: amplitude is half the peak.
        let mid = content_reveal(
            MsgFillStyle::Hologram,
            0,
            1,
            Some(HOLOGRAM_FLICKER_MS + HOLOGRAM_RIPPLE_MS / 2),
            10,
            1.0,
        );
        assert!(
            (mid.factor - 1.0).abs() <= HOLOGRAM_RIPPLE_AMPLITUDE * 0.5 + 1e-6,
            "mid-ripple factor {} must stay within ±{} of 1.0 (decayed amplitude)",
            mid.factor,
            HOLOGRAM_RIPPLE_AMPLITUDE * 0.5
        );
    }

    #[test]
    fn hologram_settles_to_one_after_ripple_window() {
        // age >= flicker + ripple → settled at exactly 1.0.
        let settled = content_reveal(
            MsgFillStyle::Hologram,
            0,
            1,
            Some(HOLOGRAM_FLICKER_MS + HOLOGRAM_RIPPLE_MS),
            10,
            1.0,
        );
        assert!((settled.factor - 1.0).abs() < 1e-6);
        // And well past it (no regression at large elapsed).
        let far = content_reveal(MsgFillStyle::Hologram, 0, 1, Some(60_000), 10, 1.0);
        assert!((far.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hologram_hidden_until_reveal_count_reaches_the_cell() {
        // Same visibility gate as engrave: cell N stays hidden
        // until reveal_count > N. The flicker math must never
        // resurrect an unrevealed cell.
        let r = content_reveal(MsgFillStyle::Hologram, 7, 1, Some(400), 7, 1.0);
        assert!(!r.visible, "cell 7 must stay hidden until reveal_count > 7");
        let r = content_reveal(MsgFillStyle::Hologram, 6, 1, Some(400), 7, 1.0);
        assert!(r.visible, "cell 6 must be visible when reveal_count == 7");
    }

    #[test]
    fn flicker_noise_is_deterministic_and_bounded() {
        // Same input → same output (no `rand` dependency, the
        // hologram must render bit-identical at the same elapsed).
        let a = flicker_noise(3, 5);
        let b = flicker_noise(3, 5);
        assert_eq!(a, b, "flicker_noise must be deterministic");
        // Output is in [-1.0, 1.0).
        assert!((-1.0..1.0).contains(&a), "flicker_noise out of range: {a}");
        // Different content_idx → different output (hash should
        // scatter across cells, otherwise the hologram flickers in
        // vertical stripes).
        let c = flicker_noise(4, 5);
        assert!(
            (a - c).abs() > 1e-3,
            "flicker_noise should scatter across content_idx"
        );
        // Different bucket → different output (otherwise the
        // flicker is a constant brightness, not noise).
        let d = flicker_noise(3, 6);
        assert!(
            (a - d).abs() > 1e-3,
            "flicker_noise should scatter across bucket"
        );
    }

    #[test]
    fn hologram_constants_hold_research_doc_contract() {
        // Lock the values called out in
        // MSG_FILL_STYLE_EXPANSION_RESEARCH.md so a future tuning
        // round can't drift them silently.
        assert_eq!(HOLOGRAM_CHAR_MS, 80);
        assert_eq!(HOLOGRAM_FLICKER_MS, 150);
        assert_eq!(HOLOGRAM_FLICKER_BUCKET_MS, 40);
        assert!((HOLOGRAM_FLICKER_AMPLITUDE - 0.30).abs() < 1e-6);
        assert_eq!(HOLOGRAM_RIPPLE_MS, 2000);
        assert!((HOLOGRAM_RIPPLE_AMPLITUDE - 0.02).abs() < 1e-6);
        assert!((HOLOGRAM_RIPPLE_HZ - 2.0).abs() < 1e-6);
        assert_eq!(HOLOGRAM_SCANLINE_MS, 600);
    }
}
