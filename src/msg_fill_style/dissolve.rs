// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `dissolve` — ordered dithering noise-to-text condensation.
//!
//! The third (and final) candidate from the post-cascade expansion
//! (see `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §3.G —
//! the noise/dither family). Radar (HUNT-11) was the first SPATIAL
//! style; tide (HUNT-12) was the first WAVE-COHERENT style; dissolve
//! is the first DITHERED style: each content cell starts as a noise
//! glyph and "condenses" into its true character via an ordered
//! Bayer-like dithering pattern. The visual reads as "static
//! resolving into text" — distinct from glitch (scrambled order)
//! and every other shipped style.
//!
//! ## Reveal math (stateless)
//!
//! Each content cell has a base reveal time
//! `reveal_at = content_idx * DISSOLVE_CHAR_MS` (80 ms/char, same as
//! typewriter/engrave/hologram/glitch/scorch). At `reveal_at`, the
//! cell enters a **dissolve window** of `DISSOLVE_DISSOLVE_MS` (200 ms):
//!
//! - **Phase 1 (noise)**: the cell shows a noise glyph (from
//!   `DISSOLVE_NOISE_GLYPHS`) at dim brightness (`DISSOLVE_DIM` =
//!   0.50). The glyph is picked deterministically by
//!   `hash(content_idx, bucket)` so the same cell shows the same
//!   noise at the same elapsed (no `rand` dependency — bit-identical
//!   frames at the same elapsed, per the LTS contract).
//! - **Phase 2 (dithered transition)**: at a per-cell hashed
//!   threshold `dither_t = hash(content_idx) % 100 / 100.0` (in
//!   `[0.0, 1.0)`), the glyph swaps from noise to the true char.
//!   The swap happens at `progress = dither_t` within the dissolve
//!   window — so cells condense at different points in their
//!   dissolve windows, producing the "ordered dithering" pattern
//!   (some cells condense early, some late, in a deterministic
//!   spatial pattern).
//! - **Phase 3 (settled)**: after the dissolve window, the cell
//!   shows the true glyph at factor 1.0.
//!
//! The brightness ramps from `DISSOLVE_DIM` (0.50) to 1.0 over the
//! dissolve window, regardless of when the glyph swaps — so the
//! cell brightens smoothly even as the glyph swaps mid-window.
//!
//! ## Why glyph_override (not slide_rows or tint)
//!
//! Dissolve reuses the `glyph_override: Option<char>` field (the
//! glitch style's extension point) for the noise-glyph substitution.
//! During the dissolve window, `glyph_override = Some(noise_glyph)`;
//! after the swap point, `glyph_override = None` (the renderer
//! unwraps to `mc.val`). This is the SAME mechanism as glitch — zero
//! new API extension. The `slide_rows` field is always 0 (dissolve
//! cells do not move), and `tint` is always `None` (dissolve is
//! brightness + glyph modulated, not color modulated).
//!
//! ## --no-effects contract
//!
//! Dissolve has NO particle sidecar — the noise-to-text condensation
//! IS the reveal math, not a cosmetic overlay. So `--no-effects`
//! does NOT gate anything in this style (same contract as glitch/
//! cascade/radar/tide).
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use super::{index_fraction, index_pacing, lagged_border, CellReveal};

// ── Reveal math constants (stateless) ───────────────────────────────────────

/// Per-character base reveal pacing (same 80 ms as typewriter/engrave/
/// hologram/glitch/scorch). Cells are eligible for reveal every 80 ms,
/// left-to-right (dissolve is paced, not scrambled like glitch).
pub(crate) const DISSOLVE_CHAR_MS: usize = 80;

/// Dissolve window duration. 200 ms = ~12 frames at 60 FPS. During
/// this window, the cell shows a noise glyph that condenses into
/// the true char at a per-cell hashed threshold. Long enough to read
/// as "static resolving", short enough to not feel sluggish.
pub(crate) const DISSOLVE_DISSOLVE_MS: usize = 200;

/// Starting brightness while showing the noise glyph. 0.50 = the cell
/// starts at 50% brightness and ramps to 100% over the dissolve
/// window. Lower than slide's 0.70 because the noise glyph is
/// already visually loud — the dim start keeps the dissolve subtle.
pub(crate) const DISSOLVE_DIM: f32 = 0.50;

/// Noise-glyph table: 8 ASCII graphic glyphs that read as "static
/// noise". Same table as glitch (the wrong-glyph table) — both styles
/// share the "ASCII graphic noise" aesthetic. All single-width, all
/// in the safe ASCII printable range, so the substitution never
/// breaks cell alignment (Bug #11) and never introduces wide CJK
/// chars.
pub(crate) const DISSOLVE_NOISE_GLYPHS: [char; 8] = ['0', '1', '#', '%', '&', '$', '@', '?'];

// ── Reveal math (stateless) ────────────────────────────────────────────────

/// Per-cell reveal: noise glyph → dithered swap → true glyph.
///
/// Pure function of `(content_idx, elapsed_ms, reveal_count)` — no
/// per-frame state, no per-cell bookkeeping in `Cloud`. The
/// `CellReveal.slide_rows` field is always 0 (dissolve cells do not
/// move — the slide style owns that channel).
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    // Budget gate: the reveal_count budget caps how many cells COULD
    // be revealed. Within that budget, each cell's base pacing (80 ms
    // left-to-right) decides its reveal time.
    if content_idx >= reveal_count {
        return CellReveal::hidden();
    }
    let reveal_at = content_idx * DISSOLVE_CHAR_MS;
    let Some(ms) = elapsed_ms else {
        // No timeline (bench/edge): settled immediately.
        return CellReveal::settled();
    };
    if ms < reveal_at {
        // Cell is within the reveal_count budget but its base pacing
        // puts the reveal in the future — still hidden.
        return CellReveal::hidden();
    }
    let age = ms - reveal_at;
    if age >= DISSOLVE_DISSOLVE_MS {
        // Settled: true glyph at factor 1.0.
        CellReveal::settled()
    } else {
        // Dissolve window: noise glyph + brightness ramp, with a
        // per-cell hashed dither threshold for the noise→true swap.
        let progress = age as f32 / DISSOLVE_DISSOLVE_MS as f32;
        // Dither threshold in [0.0, 1.0): the cell swaps from noise
        // to true glyph when progress >= dither_t. The threshold is
        // a deterministic per-cell hash, so the swap pattern is
        // spatial (ordered dithering) — some cells condense early,
        // some late, in a fixed pattern.
        let dither_t = dither_threshold(content_idx);
        let glyph_override = if progress >= dither_t {
            // Past the dither threshold: show the true glyph.
            None
        } else {
            // Before the dither threshold: show the noise glyph.
            // The noise glyph is picked per-bucket (every 50 ms) so
            // it shimmers during the noise phase — adds to the
            // "static" feel.
            let bucket = age / 50;
            Some(noise_glyph(content_idx, bucket))
        };
        // Brightness ramps from DISSOLVE_DIM (0.50) to 1.0 over the
        // dissolve window, regardless of when the glyph swaps.
        let factor = DISSOLVE_DIM + (1.0 - DISSOLVE_DIM) * progress;
        CellReveal {
            visible: true,
            factor,
            slide_rows: 0,
            glyph_override,
            tint: None,
        }
    }
}

/// Deterministic dither threshold for a content cell, in `[0.0, 1.0)`.
///
/// The threshold decides when (within the dissolve window) the cell
/// swaps from the noise glyph to the true char. A cell with
/// `dither_t = 0.0` condenses immediately at the start of the
/// window; a cell with `dither_t = 0.9` condenses near the end. The
/// pattern is deterministic per cell (no `rand` dependency) and
/// spatial — cells near each other tend to have different thresholds
/// (the hash diffuses), producing the "ordered dithering" look.
#[inline]
fn dither_threshold(content_idx: usize) -> f32 {
    let h = dissolve_hash(content_idx, 0xDEAD_BEEF);
    // Map u32 → [0.0, 1.0) using the high 24 bits (mantissa-sized).
    let bits = h >> 8;
    (bits as f32) / ((1u32 << 24) as f32)
}

/// Deterministic noise-glyph pick during the dissolve window.
///
/// The glyph is picked per-bucket (every 50 ms) so the noise
/// shimmers during the noise phase — adds to the "static" feel.
/// Same hash shape as glitch's wrong_glyph but a different seed
/// so the two styles never correlate.
#[inline]
fn noise_glyph(content_idx: usize, bucket: usize) -> char {
    let idx = dissolve_hash(content_idx, bucket as u32 + 1) % DISSOLVE_NOISE_GLYPHS.len() as u32;
    DISSOLVE_NOISE_GLYPHS[idx as usize]
}

/// 32-bit multiply-xorshift hash with a per-call seed. Same shape
/// as glitch's hash but a different seed space so the dither
/// threshold and noise glyph never correlate with glitch's
/// scramble offset or wrong-glyph pick.
#[inline]
fn dissolve_hash(content_idx: usize, seed: u32) -> u32 {
    let mut h: u32 = (content_idx as u32).wrapping_mul(0x9E37_79B1);
    h = h.wrapping_add(seed);
    h = h.wrapping_mul(0x27D4_EB2F).rotate_left(11);
    h ^= h.rotate_right(7);
    h
}

/// Index budget: 80 ms/char with the pre-v80.0.0-beta.1 `.max(1)` floor.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(DISSOLVE_CHAR_MS, elapsed_ms, total_text)
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
    fn dissolve_reveals_progressively_at_80ms_per_char() {
        // 80 ms/char: cell 0 reveals at t=0, cell 1 at t=80, cell 2 at t=160.
        // At 0 ms: cell 0 visible (reveal_at=0, age=0, noise phase).
        // At 80 ms: cell 1 visible (reveal_at=80, age=0, noise phase).
        // At 159 ms: cell 1 still mid-dissolve (age=79 < 200), cell 2
        // hidden (reveal_at=160 not reached).
        let r0_at_0 = content_reveal(MsgFillStyle::Dissolve, 0, 1, Some(0), 10, 1.0);
        assert!(
            r0_at_0.visible,
            "cell 0 must be visible at t=0 (reveal_at=0)"
        );

        let r1_at_80 = content_reveal(MsgFillStyle::Dissolve, 1, 1, Some(80), 10, 1.0);
        assert!(
            r1_at_80.visible,
            "cell 1 must be visible at 80ms (reveal_at=80)"
        );

        let r2_at_159 = content_reveal(MsgFillStyle::Dissolve, 2, 1, Some(159), 10, 1.0);
        assert!(
            !r2_at_159.visible,
            "cell 2 must be hidden at 159ms (reveal_at=160 not reached)"
        );

        let r2_at_160 = content_reveal(MsgFillStyle::Dissolve, 2, 1, Some(160), 10, 1.0);
        assert!(
            r2_at_160.visible,
            "cell 2 must be visible at 160ms (reveal_at=160)"
        );
    }

    #[test]
    fn dissolve_settles_after_dissolve_window() {
        // At age >= DISSOLVE_DISSOLVE_MS (200 ms): glyph_override = None,
        // factor = 1.0 (settled).
        let r = content_reveal(
            MsgFillStyle::Dissolve,
            0,
            1,
            Some(DISSOLVE_DISSOLVE_MS),
            10,
            1.0,
        );
        assert!(r.visible);
        assert!(
            r.glyph_override.is_none(),
            "glyph_override must be None after dissolve window (true glyph)"
        );
        assert!(
            (r.factor - 1.0).abs() < 1e-6,
            "factor must be 1.0 after dissolve window"
        );
        assert_eq!(r.slide_rows, 0);
        assert!(r.tint.is_none());
    }

    #[test]
    fn dissolve_settles_without_timeline() {
        // No timeline (bench/edge): settled immediately.
        let r = content_reveal(MsgFillStyle::Dissolve, 0, 1, None, 10, 1.0);
        assert!(r.visible);
        assert_eq!(r.slide_rows, 0);
        assert!(r.glyph_override.is_none());
        assert!((r.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn dissolve_noise_phase_shows_noise_glyph_at_dim_brightness() {
        // At age 0 (just revealed): progress = 0. If dither_t > 0
        // (the common case — dither_t is in [0, 1)), the cell shows
        // a noise glyph at DISSOLVE_DIM (0.50).
        // Cell 0's dither_t is deterministic; we don't know its exact
        // value, but we can verify the noise-phase behavior holds
        // for cells where dither_t > 0.
        // Find a cell with dither_t > 0 (almost all cells).
        for content_idx in 0..20 {
            let dt = dither_threshold(content_idx);
            if dt > 0.0 {
                let r = content_reveal(
                    MsgFillStyle::Dissolve,
                    content_idx,
                    1,
                    Some(content_idx * DISSOLVE_CHAR_MS),
                    10,
                    1.0,
                );
                assert!(
                    r.visible,
                    "cell {} must be visible at reveal_at",
                    content_idx
                );
                if dt > 0.0 {
                    assert!(
                        r.glyph_override.is_some(),
                        "cell {} (dither_t={:.3}) must show noise glyph at progress=0 (before dither threshold)",
                        content_idx,
                        dt
                    );
                    let noise = r.glyph_override.unwrap();
                    assert!(
                        DISSOLVE_NOISE_GLYPHS.contains(&noise),
                        "noise glyph '{}' must be in DISSOLVE_NOISE_GLYPHS table",
                        noise
                    );
                }
                assert!(
                    (r.factor - DISSOLVE_DIM).abs() < 1e-3,
                    "factor at progress=0 ({}) must be DISSOLVE_DIM ({})",
                    r.factor,
                    DISSOLVE_DIM
                );
                break; // one cell is enough to verify the noise phase
            }
        }
    }

    #[test]
    fn dissolve_dither_swap_switches_to_true_glyph_at_threshold() {
        // The cell swaps from noise to true glyph when progress >=
        // dither_t. Verify: at progress = dither_t - epsilon, noise
        // glyph; at progress = dither_t + epsilon, true glyph.
        let content_idx = 0;
        let dt = dither_threshold(content_idx);
        let reveal_at = content_idx * DISSOLVE_CHAR_MS;
        // Just before the threshold: noise glyph.
        let before_progress = (dt - 0.01).max(0.0);
        let before_age = (before_progress * DISSOLVE_DISSOLVE_MS as f32) as usize;
        let before = content_reveal(
            MsgFillStyle::Dissolve,
            content_idx,
            1,
            Some(reveal_at + before_age),
            10,
            1.0,
        );
        if dt > 0.01 {
            assert!(
                before.glyph_override.is_some(),
                "before dither threshold (progress={:.3}, dt={:.3}): noise glyph expected",
                before_progress,
                dt
            );
        }

        // At/after the threshold: true glyph (glyph_override = None).
        let after_progress = (dt + 0.01).min(0.99);
        let after_age = (after_progress * DISSOLVE_DISSOLVE_MS as f32) as usize;
        let after = content_reveal(
            MsgFillStyle::Dissolve,
            content_idx,
            1,
            Some(reveal_at + after_age),
            10,
            1.0,
        );
        assert!(
            after.glyph_override.is_none(),
            "at/after dither threshold (progress={:.3}, dt={:.3}): true glyph expected (glyph_override = None)",
            after_progress,
            dt
        );
    }

    #[test]
    fn dissolve_brightness_ramps_from_dim_to_one_over_window() {
        // The brightness ramps from DISSOLVE_DIM (0.50) to 1.0 over
        // the dissolve window, regardless of when the glyph swaps.
        // At progress = 0.5 (age = 100 ms): factor = 0.50 + 0.50 * 0.5 = 0.75.
        let r = content_reveal(MsgFillStyle::Dissolve, 0, 1, Some(100), 10, 1.0);
        assert!(r.visible);
        let expected = DISSOLVE_DIM + (1.0 - DISSOLVE_DIM) * 0.5;
        assert!(
            (r.factor - expected).abs() < 1e-3,
            "factor at progress=0.5 ({}) must be {} (halfway dim→1.0)",
            r.factor,
            expected
        );
    }

    #[test]
    fn dissolve_hidden_until_reveal_time() {
        // Cell 5 reveals at 5 * 80 = 400 ms. At 399 ms, hidden.
        // At 400 ms, visible (noise phase start).
        let r = content_reveal(MsgFillStyle::Dissolve, 5, 1, Some(399), 10, 1.0);
        assert!(!r.visible, "cell 5 must be hidden at 399ms");
        let r = content_reveal(MsgFillStyle::Dissolve, 5, 1, Some(400), 10, 1.0);
        assert!(r.visible, "cell 5 must be visible at 400ms (reveal_at)");
    }

    #[test]
    fn dissolve_hidden_outside_reveal_budget() {
        // Cells beyond reveal_count are always hidden.
        let r = content_reveal(MsgFillStyle::Dissolve, 10, 1, Some(60_000), 5, 1.0);
        assert!(!r.visible, "cell 10 must be hidden when reveal_count == 5");
    }

    #[test]
    fn dissolve_factor_never_exceeds_one() {
        // The brightness ramps from DISSOLVE_DIM (0.50) to 1.0 — it
        // never exceeds 1.0 (no head boost, unlike radar/tide/engrave/
        // scorch). Sample every 10 ms across the dissolve window.
        for ms in 0..DISSOLVE_DISSOLVE_MS {
            let r = content_reveal(MsgFillStyle::Dissolve, 0, 1, Some(ms), 10, 1.0);
            if r.visible {
                assert!(
                    r.factor <= 1.0 + 1e-3,
                    "factor {} at ms={} must not exceed 1.0 (dissolve has no head boost)",
                    r.factor,
                    ms
                );
            }
        }
    }

    #[test]
    fn dissolve_dither_threshold_is_deterministic() {
        // The dither threshold is a pure function of content_idx —
        // same input → same output (no rand). Verify by calling twice.
        for content_idx in 0..10 {
            let dt1 = dither_threshold(content_idx);
            let dt2 = dither_threshold(content_idx);
            assert_eq!(dt1, dt2, "dither_threshold must be deterministic");
            assert!(
                (0.0..1.0).contains(&dt1),
                "dither_threshold {} must be in [0.0, 1.0)",
                dt1
            );
        }
    }

    #[test]
    fn dissolve_constants_hold_research_doc_contract() {
        // Lock the values called out in the doc comment so a future
        // round can't drift them silently.
        assert_eq!(DISSOLVE_CHAR_MS, 80);
        assert_eq!(DISSOLVE_DISSOLVE_MS, 200);
        assert!((DISSOLVE_DIM - 0.50).abs() < 1e-6);
        assert_eq!(
            DISSOLVE_NOISE_GLYPHS,
            ['0', '1', '#', '%', '&', '$', '@', '?']
        );
    }
}
