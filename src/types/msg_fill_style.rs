// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Message overlay fill (reveal) style selection.
//!
//! v51 msg-fill-style: the message overlay reveal animation is no longer
//! hardwired to the classic typewriter. Seven styles are selectable via
//! CLI (`-mfs <style>` / `--msg-fill-style <style>`) or config.toml
//! (`msg-fill-style = "<style>"`):
//!
//! | Style       | Text reveal                          | Border                  |
//! |-------------|--------------------------------------|-------------------------|
//! | `typewriter`| 80 ms/char + 100 ms fade-in (30→100%)| lags text (t^1.5)       |
//! | `fade`      | instant, block alpha 0→100% (800 ms) | fades with the block    |
//! | `words`     | 200 ms/word + 150 ms fade-in         | lags word progress      |
//! | `slide`     | 60 ms/char, rises from 1 row below   | lags text (t^1.5)       |
//! | `pulse`     | typewriter + 1.5x scanner cursor     | lags text (t^1.5)       |
//! | `instant`   | full brightness at t=0               | clockwise draw over 1 s |
//! | `engrave`   | 80 ms/char burn-in, 2x hot head      | lags text (t^1.5) + sparks |
//!
//! Default is `typewriter` — bit-identical to the pre-v51 renderer, so
//! upgrading changes nothing unless the user opts in (LTS guarantee).
//!
//! Six styles are purely time-derived (stateless — zero per-frame
//! bookkeeping). `engrave` keeps the REVEAL math stateless like the
//! rest, but adds one bounded stateful sidecar: a 48-slot spark
//! particle pool rendered inside `draw_message` (see
//! `cloud/message_engrave.rs` for why the shared quantum pool cannot
//! be reused — it renders before the overlay and would be overdrawn).
//!
//! Placement mirrors `rain_style.rs`: a neutral types module importable
//! from both the CLI layer (Args) and the rendering engine (Cloud)
//! without either depending on the other.

use clap::ValueEnum;

/// Message overlay reveal style. Exposed as a clap `ValueEnum` for CLI
/// parsing (`-mfs`/`--msg-fill-style`) and consumed by the message
/// overlay renderer in `cloud/message_draw.rs`.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgFillStyle {
    /// Classic per-character typewriter with per-char fade-in.
    #[value(name = "typewriter")]
    Typewriter,
    /// Whole block fades in over 800 ms (text appears instantly).
    #[value(name = "fade")]
    Fade,
    /// Word-by-word reveal (200 ms per word, 150 ms word fade-in).
    #[value(name = "words")]
    Words,
    /// Characters slide up from one row below while fading in.
    #[value(name = "slide")]
    Slide,
    /// Typewriter plus a traveling brightness "scanner" cursor.
    #[value(name = "pulse")]
    Pulse,
    /// Text appears instantly at full brightness; only the border draws.
    #[value(name = "instant")]
    Instant,
    /// Laser engraving: chars burn in at full brightness, glow 2x hot
    /// at the head (cooling over 300 ms), and each newly engraved char
    /// throws a small spark burst (see `cloud/message_engrave.rs`).
    #[value(name = "engrave")]
    Engrave,
}

impl MsgFillStyle {
    /// Canonical lowercase name (matches the clap value and the
    /// config.toml key spelling).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Typewriter => "typewriter",
            Self::Fade => "fade",
            Self::Words => "words",
            Self::Slide => "slide",
            Self::Pulse => "pulse",
            Self::Instant => "instant",
            Self::Engrave => "engrave",
        }
    }

    /// Short human-readable description for verbose output.
    #[must_use]
    pub fn verbose_label(self) -> &'static str {
        match self {
            Self::Typewriter => "typewriter (80 ms/char + 100 ms fade-in, border lags text)",
            Self::Fade => "fade (instant text, block alpha 0-100% over 800 ms)",
            Self::Words => "words (200 ms/word + 150 ms fade-in, border lags word progress)",
            Self::Slide => "slide (60 ms/char, chars rise from one row below)",
            Self::Pulse => "pulse (typewriter + 1.5x scanner cursor, 200 ms decay)",
            Self::Instant => "instant (text immediate, border draws clockwise over 1 s)",
            Self::Engrave => {
                "engrave (80 ms/char burn-in, 2x hot head cooling over 300 ms, spark burst per char)"
            }
        }
    }
}

// ── Timing constants (ms) ─────────────────────────────────────────────────
//
// Centralized here so tests and the renderer always agree. All styles
// derive their animation entirely from `message_start_time` elapsed
// time — no per-frame state, no per-char bookkeeping in Cloud.

/// Typewriter + pulse: per-character reveal stagger.
pub(crate) const TYPEWRITER_CHAR_MS: usize = 80;
/// Typewriter + pulse: per-character fade-in duration.
pub(crate) const TYPEWRITER_FADE_MS: usize = 100;
/// Typewriter + pulse + words: fade-in start brightness (30%).
pub(crate) const FADE_IN_START: f32 = 0.30;
/// Fade: whole-block alpha ramp duration.
pub(crate) const FADE_BLOCK_MS: usize = 800;
/// Words: per-word reveal stagger.
pub(crate) const WORDS_PER_WORD_MS: usize = 200;
/// Words: per-word fade-in duration.
pub(crate) const WORDS_FADE_MS: usize = 150;
/// Slide: per-character reveal stagger.
pub(crate) const SLIDE_CHAR_MS: usize = 60;
/// Slide: travel time (fade-in below, then land at the final row).
pub(crate) const SLIDE_TRAVEL_MS: usize = 240;
/// Slide: peak brightness while the char is still one row below.
pub(crate) const SLIDE_BELOW_MAX: f32 = 0.70;
/// Pulse: scanner-cursor boost applied to recently revealed chars.
pub(crate) const PULSE_BOOST: f32 = 0.50;
/// Pulse: scanner-cursor decay window.
pub(crate) const PULSE_DECAY_MS: usize = 200;
/// Instant: independent clockwise border draw duration.
pub(crate) const INSTANT_BORDER_MS: usize = 1000;
/// Engrave: per-character reveal stagger (same 80 ms pacing as
/// typewriter, kept as its own constant so the two can diverge).
pub(crate) const ENGRAVE_CHAR_MS: usize = 80;
/// Engrave: head boost above settled brightness (1.0 → peak factor
/// 2.0, routed through the unclamped boost path like pulse).
pub(crate) const ENGRAVE_BOOST: f32 = 1.0;
/// Engrave: heat-glow decay window behind the engraving head.
pub(crate) const ENGRAVE_HEAT_MS: usize = 300;

/// Per-content-cell reveal state, resolved per frame from elapsed time.
///
/// Pure value type — no allocation, computed inline in the draw loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CellReveal {
    /// Cell is drawn (true) or blanked to a space (false).
    pub visible: bool,
    /// Brightness factor applied to the content fg color (1.0 = settled).
    /// May exceed 1.0 for the pulse scanner head (clamped downstream).
    pub factor: f32,
    /// Rows below the final position while the cell is mid-slide
    /// (slide style only; 0 = at the final position).
    pub slide_rows: u16,
}

impl CellReveal {
    fn hidden() -> Self {
        Self {
            visible: false,
            factor: 0.0,
            slide_rows: 0,
        }
    }

    fn settled() -> Self {
        Self {
            visible: true,
            factor: 1.0,
            slide_rows: 0,
        }
    }
}

/// Typewriter-style per-char fade-in: `FADE_IN_START` → 1.0 over
/// `TYPEWRITER_FADE_MS`, indexed by the cell's reveal time.
#[inline]
fn char_fade_in(elapsed_ms: Option<usize>, reveal_at_ms: usize) -> f32 {
    match elapsed_ms {
        None => 1.0,
        Some(ms) => {
            let age = ms.saturating_sub(reveal_at_ms);
            if age >= TYPEWRITER_FADE_MS {
                1.0
            } else {
                let progress = age as f32 / TYPEWRITER_FADE_MS as f32;
                FADE_IN_START + (1.0 - FADE_IN_START) * progress
            }
        }
    }
}

/// Resolve the reveal state of the `content_idx`-th content cell under
/// the active style.
///
/// Parameters:
/// - `style`: active reveal style.
/// - `content_idx`: 0-based reading order of this content cell.
/// - `word_ord`: 1-based word ordinal of this cell (0 = padding space
///   before the first word). Only read by the `words` style.
/// - `elapsed_ms`: `Some(ms)` since reveal start; `None` means "no
///   animation timeline" (everything settles instantly — benchmark
///   and edge paths).
/// - `reveal_count`: cells revealed by index-based styles
///   (typewriter/pulse/slide). Other styles ignore it.
/// - `block_alpha`: fade-style whole-block alpha (0.0 → 1.0).
pub(crate) fn content_reveal(
    style: MsgFillStyle,
    content_idx: usize,
    word_ord: u32,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
    block_alpha: f32,
) -> CellReveal {
    match style {
        MsgFillStyle::Typewriter => {
            if content_idx < reveal_count {
                let reveal_at = content_idx * TYPEWRITER_CHAR_MS;
                CellReveal {
                    visible: true,
                    factor: char_fade_in(elapsed_ms, reveal_at),
                    slide_rows: 0,
                }
            } else {
                CellReveal::hidden()
            }
        }
        MsgFillStyle::Pulse => {
            if content_idx < reveal_count {
                let reveal_at = content_idx * TYPEWRITER_CHAR_MS;
                let base = char_fade_in(elapsed_ms, reveal_at);
                // Scanner head: recently revealed chars glow up to
                // (1 + PULSE_BOOST) and decay to 1.0 over PULSE_DECAY_MS.
                let boost = match elapsed_ms {
                    None => 1.0,
                    Some(ms) => {
                        let age = ms.saturating_sub(reveal_at);
                        if age >= PULSE_DECAY_MS {
                            1.0
                        } else {
                            let decay = 1.0 - age as f32 / PULSE_DECAY_MS as f32;
                            1.0 + PULSE_BOOST * decay
                        }
                    }
                };
                CellReveal {
                    visible: true,
                    factor: base * boost,
                    slide_rows: 0,
                }
            } else {
                CellReveal::hidden()
            }
        }
        MsgFillStyle::Fade => {
            // Whole block appears instantly, then the shared alpha ramps.
            // At alpha 0.0 nothing is drawn (matches "fade from 0%").
            if block_alpha > 0.0 {
                CellReveal {
                    visible: true,
                    factor: block_alpha,
                    slide_rows: 0,
                }
            } else {
                CellReveal::hidden()
            }
        }
        MsgFillStyle::Instant => CellReveal::settled(),
        MsgFillStyle::Engrave => {
            // Burn-in reveal: a char appears at FULL brightness the
            // instant the head reaches it (no 30% fade-in — a laser
            // burns text in, it does not fade it in), then cools from
            // (1 + ENGRAVE_BOOST) back to 1.0 over ENGRAVE_HEAT_MS. The
            // last ~4 chars are always cooling at any moment, forming
            // the heat trail behind the engraving head. The spark
            // burst itself lives in `cloud/message_engrave.rs` (the
            // only stateful part of the style family).
            if content_idx < reveal_count {
                let reveal_at = content_idx * ENGRAVE_CHAR_MS;
                let heat = match elapsed_ms {
                    None => 1.0,
                    Some(ms) => {
                        let age = ms.saturating_sub(reveal_at);
                        if age >= ENGRAVE_HEAT_MS {
                            1.0
                        } else {
                            let decay = 1.0 - age as f32 / ENGRAVE_HEAT_MS as f32;
                            1.0 + ENGRAVE_BOOST * decay
                        }
                    }
                };
                CellReveal {
                    visible: true,
                    factor: heat,
                    slide_rows: 0,
                }
            } else {
                CellReveal::hidden()
            }
        }
        MsgFillStyle::Words => {
            let word_reveal_at = word_ord.saturating_sub(1) as usize * WORDS_PER_WORD_MS;
            match elapsed_ms {
                None => CellReveal::settled(),
                Some(ms) => {
                    if word_ord == 0 || ms >= word_reveal_at {
                        let age = ms.saturating_sub(word_reveal_at);
                        let factor = if age >= WORDS_FADE_MS {
                            1.0
                        } else {
                            let progress = age as f32 / WORDS_FADE_MS as f32;
                            FADE_IN_START + (1.0 - FADE_IN_START) * progress
                        };
                        CellReveal {
                            visible: true,
                            factor,
                            slide_rows: 0,
                        }
                    } else {
                        CellReveal::hidden()
                    }
                }
            }
        }
        MsgFillStyle::Slide => {
            let reveal_at = content_idx * SLIDE_CHAR_MS;
            match elapsed_ms {
                None => CellReveal::settled(),
                Some(ms) => {
                    if ms < reveal_at {
                        CellReveal::hidden()
                    } else {
                        let age = ms - reveal_at;
                        let progress = (age as f32 / SLIDE_TRAVEL_MS as f32).min(1.0);
                        if progress < 0.5 {
                            // Phase 1: fading in one row below the final
                            // position (dim — max SLIDE_BELOW_MAX).
                            CellReveal {
                                visible: true,
                                factor: progress * 2.0 * SLIDE_BELOW_MAX,
                                slide_rows: 1,
                            }
                        } else {
                            // Phase 2: landed. Complete the fade at the
                            // final position (30% → 100%).
                            let p2 = (progress - 0.5) * 2.0;
                            CellReveal {
                                visible: true,
                                factor: FADE_IN_START + (1.0 - FADE_IN_START) * p2,
                                slide_rows: 0,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Resolve the whole-block alpha for the fade style from elapsed time.
#[inline]
pub(crate) fn fade_block_alpha(elapsed_ms: Option<usize>) -> f32 {
    match elapsed_ms {
        None => 1.0,
        Some(ms) => (ms as f32 / FADE_BLOCK_MS as f32).min(1.0),
    }
}

/// Resolve the clockwise border progress (0.0 → 1.0) for the active
/// style, given the style's own text-progress input.
///
/// - Typewriter / pulse / slide / engrave / words: border lags behind
///   text (`text_progress^1.5` ease-out) — the pre-v51 cinematic behavior.
/// - Fade: border fades together with the text block.
/// - Instant: border draws clockwise on an independent 1 s timeline
///   (text is already fully visible).
pub(crate) fn border_progress(
    style: MsgFillStyle,
    text_progress: f32,
    elapsed_ms: Option<usize>,
) -> f32 {
    match style {
        MsgFillStyle::Fade => fade_block_alpha(elapsed_ms),
        MsgFillStyle::Instant => match elapsed_ms {
            None => 1.0,
            Some(ms) => (ms as f32 / INSTANT_BORDER_MS as f32).min(1.0),
        },
        _ => text_progress.clamp(0.0, 1.0).powf(1.5),
    }
}

/// Resolve the text progress (0.0 → 1.0) that feeds the border lag for
/// styles whose text reveal is index- or word-paced.
///
/// - Typewriter / pulse / slide / engrave: `reveal_count / total_text`.
/// - Words: revealed-word fraction (`total_words` from the word
///   ordinals built in `reset_message`).
/// - Fade / instant: 1.0 (text is not the pacing element).
#[inline]
pub(crate) fn text_progress(
    style: MsgFillStyle,
    reveal_count: usize,
    total_text: usize,
    total_words: usize,
    elapsed_ms: Option<usize>,
) -> f32 {
    let total_text = total_text.max(1);
    match style {
        MsgFillStyle::Words => {
            let total_words = total_words.max(1);
            match elapsed_ms {
                None => 1.0,
                Some(ms) => {
                    let revealed_words = (ms / WORDS_PER_WORD_MS + 1).min(total_words);
                    revealed_words as f32 / total_words as f32
                }
            }
        }
        MsgFillStyle::Fade | MsgFillStyle::Instant => 1.0,
        _ => (reveal_count.min(total_text)) as f32 / total_text as f32,
    }
}

/// Number of content cells revealed by index-based styles
/// (typewriter/pulse/slide/engrave) at the given elapsed time.
///
/// The `.max(1)` mirrors the pre-v51 renderer: the first cell appears
/// immediately so the very first frame is never fully empty.
#[inline]
pub(crate) fn index_reveal_count(
    style: MsgFillStyle,
    elapsed_ms: Option<usize>,
    total_text: usize,
) -> usize {
    let per_char_ms = match style {
        MsgFillStyle::Slide => SLIDE_CHAR_MS,
        MsgFillStyle::Engrave => ENGRAVE_CHAR_MS,
        _ => TYPEWRITER_CHAR_MS,
    };
    match elapsed_ms {
        None => usize::MAX,
        Some(ms) => (ms / per_char_ms).max(1).min(total_text.max(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_matches_clap_value_names() {
        // The as_str() spelling must equal the clap ValueEnum name so
        // verbose output, config keys, and CLI values never diverge.
        assert_eq!(MsgFillStyle::Typewriter.as_str(), "typewriter");
        assert_eq!(MsgFillStyle::Fade.as_str(), "fade");
        assert_eq!(MsgFillStyle::Words.as_str(), "words");
        assert_eq!(MsgFillStyle::Slide.as_str(), "slide");
        assert_eq!(MsgFillStyle::Pulse.as_str(), "pulse");
        assert_eq!(MsgFillStyle::Instant.as_str(), "instant");
        assert_eq!(MsgFillStyle::Engrave.as_str(), "engrave");
    }

    #[test]
    fn typewriter_reveals_at_80ms_per_char() {
        // 40-char message: cell 0 visible at t=0 (max(1) rule), cell 4
        // only after 320 ms.
        let total = 40;
        let count = index_reveal_count(MsgFillStyle::Typewriter, Some(319), total);
        assert_eq!(count, 3); // 319/80 = 3
        let count = index_reveal_count(MsgFillStyle::Typewriter, Some(320), total);
        assert_eq!(count, 4); // 320/80 = 4, .max(1) floor
        let first_frame = index_reveal_count(MsgFillStyle::Typewriter, Some(0), total);
        assert_eq!(first_frame, 1);
    }

    #[test]
    fn typewriter_fade_in_ramps_from_30_to_100_percent() {
        let reveal = content_reveal(MsgFillStyle::Typewriter, 0, 1, Some(0), 10, 1.0);
        assert!((reveal.factor - FADE_IN_START).abs() < 1e-6);
        let settled = content_reveal(
            MsgFillStyle::Typewriter,
            0,
            1,
            Some(TYPEWRITER_FADE_MS),
            10,
            1.0,
        );
        assert!((settled.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hidden_cells_stay_hidden_until_reveal_count_reaches_them() {
        let r = content_reveal(MsgFillStyle::Typewriter, 5, 1, Some(100), 3, 1.0);
        assert!(!r.visible);
    }

    #[test]
    fn fade_block_alpha_ramps_over_800ms() {
        assert!((fade_block_alpha(Some(0)) - 0.0).abs() < 1e-6);
        assert!((fade_block_alpha(Some(400)) - 0.5).abs() < 1e-6);
        assert!((fade_block_alpha(Some(800)) - 1.0).abs() < 1e-6);
        assert!((fade_block_alpha(Some(10_000)) - 1.0).abs() < 1e-6);
        // No timeline (None) → fully visible.
        assert!((fade_block_alpha(None) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fade_style_hides_block_at_alpha_zero() {
        let r = content_reveal(MsgFillStyle::Fade, 0, 1, Some(0), 0, 0.0);
        assert!(!r.visible);
        let r = content_reveal(MsgFillStyle::Fade, 0, 1, Some(1), 0, 0.001);
        assert!(r.visible);
        assert!((r.factor - 0.001).abs() < 1e-6);
    }

    #[test]
    fn instant_style_settles_immediately() {
        let r = content_reveal(MsgFillStyle::Instant, 0, 1, Some(0), 0, 1.0);
        assert!(r.visible);
        assert!((r.factor - 1.0).abs() < 1e-6);
        assert_eq!(r.slide_rows, 0);
    }

    #[test]
    fn instant_border_draws_over_one_second() {
        assert!((border_progress(MsgFillStyle::Instant, 1.0, Some(0)) - 0.0).abs() < 1e-6);
        assert!((border_progress(MsgFillStyle::Instant, 1.0, Some(500)) - 0.5).abs() < 1e-6);
        assert!(
            (border_progress(MsgFillStyle::Instant, 1.0, Some(INSTANT_BORDER_MS)) - 1.0).abs()
                < 1e-6
        );
    }

    #[test]
    fn words_reveal_word_by_word_at_200ms() {
        // Word 1 (ord=1) reveals at t=0; word 2 (ord=2) at t=200.
        let word1 = content_reveal(MsgFillStyle::Words, 0, 1, Some(0), 0, 1.0);
        assert!(word1.visible);
        let word2_early = content_reveal(MsgFillStyle::Words, 3, 2, Some(199), 0, 1.0);
        assert!(!word2_early.visible);
        let word2_on_time = content_reveal(MsgFillStyle::Words, 3, 2, Some(200), 0, 1.0);
        assert!(word2_on_time.visible);
        // Word fade-in completes after WORDS_FADE_MS.
        let word1_settled = content_reveal(MsgFillStyle::Words, 0, 1, Some(WORDS_FADE_MS), 0, 1.0);
        assert!((word1_settled.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn words_padding_spaces_reveal_immediately() {
        // ord=0 (padding before the first word) must never delay the
        // (invisible) padding cells.
        let r = content_reveal(MsgFillStyle::Words, 0, 0, Some(0), 0, 1.0);
        assert!(r.visible);
    }

    #[test]
    fn slide_char_rises_from_below_then_lands() {
        // Cell 0 reveal at t=0. Phase 1 (age < 120 ms): one row below,
        // dim. Phase 2 (120..240 ms): landed, ramping 30%→100%.
        let below = content_reveal(MsgFillStyle::Slide, 0, 1, Some(60), 10, 1.0);
        assert!(below.visible);
        assert_eq!(below.slide_rows, 1);
        assert!(below.factor < SLIDE_BELOW_MAX + 1e-6);

        let landed = content_reveal(MsgFillStyle::Slide, 0, 1, Some(180), 10, 1.0);
        assert!(landed.visible);
        assert_eq!(landed.slide_rows, 0);
        assert!(landed.factor > FADE_IN_START);
        assert!(landed.factor < 1.0);

        let settled = content_reveal(MsgFillStyle::Slide, 0, 1, Some(SLIDE_TRAVEL_MS), 10, 1.0);
        assert!(settled.visible);
        assert_eq!(settled.slide_rows, 0);
        assert!((settled.factor - 1.0).abs() < 1e-6);

        // Cell 3 does not start until 3 * 60 = 180 ms.
        let not_yet = content_reveal(MsgFillStyle::Slide, 3, 1, Some(179), 10, 1.0);
        assert!(!not_yet.visible);
    }

    #[test]
    fn pulse_scanner_boosts_recent_chars_and_decays() {
        // Cell 0 at age 0: fade factor 0.30 * (1 + 0.5) = 0.45.
        let head = content_reveal(MsgFillStyle::Pulse, 0, 1, Some(0), 10, 1.0);
        assert!((head.factor - FADE_IN_START * (1.0 + PULSE_BOOST)).abs() < 1e-6);
        // After the decay window: back to the plain typewriter curve.
        let decayed = content_reveal(
            MsgFillStyle::Pulse,
            0,
            1,
            Some(PULSE_DECAY_MS + TYPEWRITER_FADE_MS),
            10,
            1.0,
        );
        assert!((decayed.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn engrave_reveals_at_80ms_per_char() {
        // Same index pacing as typewriter: 319 ms → 3 chars, 320 ms → 4.
        let total = 40;
        let count = index_reveal_count(MsgFillStyle::Engrave, Some(319), total);
        assert_eq!(count, 3);
        let count = index_reveal_count(MsgFillStyle::Engrave, Some(320), total);
        assert_eq!(count, 4);
        let count = index_reveal_count(MsgFillStyle::Engrave, Some(0), total);
        assert_eq!(count, 1, "max(1) floor: first char at t=0");
    }

    #[test]
    fn engrave_chars_burn_in_hot_and_cool_off() {
        // Age 0: burned in at (1 + ENGRAVE_BOOST) = 2.0 — NOT the 30%
        // fade-in start the typewriter family uses.
        let head = content_reveal(MsgFillStyle::Engrave, 0, 1, Some(0), 10, 1.0);
        assert!(head.visible);
        assert!((head.factor - (1.0 + ENGRAVE_BOOST)).abs() < 1e-6);
        // Mid-decay: age 150 of 300 ms → 1 + 1.0 * 0.5 = 1.5.
        let mid = content_reveal(MsgFillStyle::Engrave, 0, 1, Some(150), 10, 1.0);
        assert!((mid.factor - 1.5).abs() < 1e-6);
        // Cooled: age >= ENGRAVE_HEAT_MS → settled at 1.0.
        let cooled = content_reveal(MsgFillStyle::Engrave, 0, 1, Some(ENGRAVE_HEAT_MS), 10, 1.0);
        assert!((cooled.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn engrave_hidden_until_reveal_count_reaches_the_cell() {
        let r = content_reveal(MsgFillStyle::Engrave, 7, 1, Some(400), 7, 1.0);
        assert!(!r.visible, "cell 7 must stay hidden until reveal_count > 7");
        let r = content_reveal(MsgFillStyle::Engrave, 6, 1, Some(400), 7, 1.0);
        assert!(r.visible);
    }

    #[test]
    fn border_lags_text_with_power_15_for_paced_styles() {
        // Pre-v51 cinematic behavior preserved for typewriter-style
        // pacing: border_progress = text_progress^1.5.
        let bp = border_progress(MsgFillStyle::Typewriter, 0.5, Some(10_000));
        assert!((bp - 0.353_553_39).abs() < 1e-5);
        // Fade: border follows the block alpha, not the text progress.
        let bp = border_progress(MsgFillStyle::Fade, 1.0, Some(400));
        assert!((bp - 0.5).abs() < 1e-6);
    }

    #[test]
    fn text_progress_words_uses_word_fraction() {
        // 3 words: at t=0 one word is (about to be) visible → 1/3.
        let tp = text_progress(MsgFillStyle::Words, 0, 9, 3, Some(0));
        assert!((tp - 1.0 / 3.0).abs() < 1e-6);
        // At t=199ms still 1/3; at t=200ms 2/3.
        let tp = text_progress(MsgFillStyle::Words, 0, 9, 3, Some(199));
        assert!((tp - 1.0 / 3.0).abs() < 1e-6);
        let tp = text_progress(MsgFillStyle::Words, 0, 9, 3, Some(200));
        assert!((tp - 2.0 / 3.0).abs() < 1e-6);
        // No timeline → complete.
        let tp = text_progress(MsgFillStyle::Words, 0, 9, 3, None);
        assert!((tp - 1.0).abs() < 1e-6);
    }

    #[test]
    fn none_elapsed_timeline_settles_everything() {
        // elapsed_ms = None (no animation state): every style must show
        // all cells at full brightness — matches pre-v51 usize::MAX
        // reveal_count semantics. The draw loop passes the reveal_count
        // from index_reveal_count (usize::MAX when there is no timeline).
        for style in [
            MsgFillStyle::Typewriter,
            MsgFillStyle::Fade,
            MsgFillStyle::Words,
            MsgFillStyle::Slide,
            MsgFillStyle::Pulse,
            MsgFillStyle::Instant,
            MsgFillStyle::Engrave,
        ] {
            let r = content_reveal(style, 0, 1, None, usize::MAX, 1.0);
            assert!(r.visible, "{style:?} must be visible without a timeline");
            assert!((r.factor - 1.0).abs() < 1e-6, "{style:?} factor");
            let count = index_reveal_count(style, None, 10);
            assert_eq!(count, usize::MAX, "{style:?} reveal_count");
        }
    }
}
