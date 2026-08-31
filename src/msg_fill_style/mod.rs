// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Message overlay fill (reveal) style selection — one file per style.
//!
//! v51 msg-fill-style: the message overlay reveal animation is no longer
//! hardwired to the classic typewriter. Nine styles are selectable via
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
//! | `hologram`  | 80 ms/char burn-in, flicker + hum, scanline sweep | lags text (t^1.5) |
//! | `glitch`    | 80 ms/char scrambled reveal, wrong-glyph settle, ±20% flicker | lags text (t^1.5) |
//! | `scorch`    | 80 ms/char ember burn, 400 ms cool tint, slow gray smoke puffs | lags text (t^1.5) + smoke |
//!
//! Default is `typewriter` — bit-identical to the pre-v51 renderer, so
//! upgrading changes nothing unless the user opts in (LTS guarantee).
//!
//! ## One file per style (owner refactor mandate)
//!
//! Each style owns ONE file in this directory — everything about the
//! style lives there: timing constants, reveal math, border curve,
//! optional stateful sidecar, and unit tests. This file (`mod.rs`)
//! holds only the shared skeleton: the enum, the per-cell value type,
//! the shared ramp/lag helpers, and the dispatch that routes the
//! runtime enum to the active style module.
//!
//! ## How to add style #11 (plug-and-play recipe)
//!
//! 1. Copy the closest existing `<style>.rs` to a new file (e.g.
//!    `<new-style>.rs`) and rewrite its reveal math + doc comment.
//!    Keep the four hooks: `reveal`, `reveal_budget`,
//!    `border_progress`, `text_progress` (+ its own `#[cfg(test)]`).
//! 2. In this file: add the `mod` declaration, the enum variant
//!    (with `#[value(name = "...")]`), and one arm in each of the
//!    four dispatch matches + `as_str`/`verbose_label`.
//! 3. Sweep the 9 value surfaces outside this directory (see
//!    `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md`):
//!    clap enum, `argv_expand` (x2), config parse/apply error,
//!    `--dump-config` comment, `--testconf` validation, verbose
//!    label, help reference block, README, CHANGELOG.
//!
//! No other style's file needs to change — that isolation is the
//! point of the directory layout.
//!
//! ## Statelessness contract
//!
//! Eight styles are purely time-derived (stateless — zero per-frame
//! bookkeeping). `engrave` keeps the REVEAL math stateless like the
//! rest, but adds one bounded stateful sidecar: a 48-slot spark
//! particle pool rendered inside `draw_message` (see `engrave.rs`
//! for why the shared quantum pool cannot be reused — it renders
//! before the overlay and would be overdrawn). `hologram` adds a
//! stateless scanline pass rendered at the end of `draw_message`
//! (see `hologram.rs`) — no pool, no per-frame state, pure function
//! of elapsed time. `glitch` extends `CellReveal` with a
//! `glyph_override: Option<char>` field (the ONE structural
//! extension point — see `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md`
//! §2) so the wrong-glyph substitution can flow through the existing
//! dispatch with zero renderer churn. `scorch` extends `CellReveal`
//! with a `tint: Option<(u8, u8, u8, f32)>` field (the ONE structural
//! extension point for color-shifting styles — same §2 ground rule)
//! and adds a 16-slot smoke sidecar cloned from the engrave pattern
//! (see `scorch.rs` for why a dedicated pool is required).
//!
//! Placement is a crate-root module (peer of `types/`): the enum is
//! consumed by both the CLI layer (Args) and the rendering engine
//! (Cloud), so it must not live inside either. The stateful engrave
//! sidecar reaches the engine via a `crate::cloud::Cloud` impl block
//! in `engrave.rs` (same split-impl pattern the engine itself uses).

use clap::ValueEnum;

// `engrave` is the one style module visible outside this directory:
// `cloud/mod.rs` stores its `EngraveState` as a Cloud field and the
// renderer tests read its spark-pool constants. `hologram` is also
// visible: its `Cloud::hologram_scanline_pass` is invoked at the end
// of `draw_message`. `glitch` is encapsulated like the rest — it
// only extends `CellReveal` with a `glyph_override` field that the
// renderer unwraps at draw time. `scorch` is visible like engrave:
// `cloud/mod.rs` stores its `ScorchState` as a Cloud field. Every
// stateless style is fully encapsulated behind the dispatch below.
pub(crate) mod engrave;
mod fade;
pub(crate) mod glitch;
pub(crate) mod hologram;
mod instant;
mod pulse;
pub(crate) mod scorch;
mod slide;
mod typewriter;
mod words;

/// Message overlay reveal style. Exposed as a clap `ValueEnum` for CLI
/// parsing (`-mfs`/`--msg-fill-style`) and consumed by the message
/// overlay renderer in `cloud/message_draw.rs` through the dispatch
/// functions below.
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
    /// throws a small spark burst (see `engrave.rs`).
    #[value(name = "engrave")]
    Engrave,
    /// Hologram projection: chars burn in at full brightness, flicker
    /// for 150 ms (deterministic per-cell interference), breathe with
    /// a 2% ripple for 2 s, and a scanline sweeps the box once over
    /// 600 ms (see `hologram.rs`). Fully stateless.
    #[value(name = "hologram")]
    Hologram,
    /// Cyberpunk glitch: chars reveal in scrambled order (not
    /// left-to-right), each newly revealed char flickers between
    /// wrong glyphs for 90 ms (deterministic per-cell hash) before
    /// settling on the true one — Matrix-decode feel. Extends
    /// `CellReveal` with `glyph_override` (see `glitch.rs`).
    #[value(name = "glitch")]
    Glitch,
    /// Scorch/burn: chars appear in an ember tint (orange/red) at
    /// the head, cooling to the palette color over 400 ms (factor
    /// dips 1.5 → 0.8 → 1.0 — the "charred" dim sub-effect), and
    /// every newly scorch'd char throws a slow upward gray smoke
    /// puff (700 ms lifetime, 16-slot pool). Extends `CellReveal`
    /// with `tint` (see `scorch.rs`). Respects `--no-effects`.
    #[value(name = "scorch")]
    Scorch,
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
            Self::Hologram => "hologram",
            Self::Glitch => "glitch",
            Self::Scorch => "scorch",
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
            Self::Hologram => {
                "hologram (80 ms/char burn-in, 150 ms flicker + 2 s hum, 600 ms scanline sweep)"
            }
            Self::Glitch => {
                "glitch (80 ms/char scrambled reveal, 90 ms wrong-glyph settle, ±20% flicker)"
            }
            Self::Scorch => {
                "scorch (80 ms/char ember burn, 400 ms cool tint, slow gray smoke puffs)"
            }
        }
    }
}

// ── Shared value type ──────────────────────────────────────────────────────

/// Per-content-cell reveal state, resolved per frame from elapsed time.
///
/// Pure value type — no allocation, computed inline in the draw loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CellReveal {
    /// Cell is drawn (true) or blanked to a space (false).
    pub visible: bool,
    /// Brightness factor applied to the content fg color (1.0 = settled).
    /// May exceed 1.0 for the pulse scanner / engrave heat heads
    /// (clamped downstream).
    pub factor: f32,
    /// Rows below the final position while the cell is mid-slide
    /// (slide style only; 0 = at the final position).
    pub slide_rows: u16,
    /// Substitute glyph drawn INSTEAD of the cell's true `mc.val`
    /// while the cell is mid-settle (glitch style only; `None` for
    /// every other style). The renderer unwraps to `mc.val` when
    /// `None`, so existing styles are bit-identical. Added in the
    /// post-hologram round as the ONE structural extension point
    /// shared by every future glyph-substituting style — see
    /// `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §2.
    pub glyph_override: Option<char>,
    /// Tint toward a fixed RGB by a blend factor (scorch style only;
    /// `None` for every other style). The tuple is `(r, g, b, blend)`
    /// where `blend` is 0.0 = palette fg color, 1.0 = full tint.
    /// The renderer applies this AFTER the brightness factor: it
    /// takes the scaled palette color and linearly blends it toward
    /// `(r, g, b)` by `blend` (via
    /// `chroma_dragon_engine::palette::blend_toward_bg_rgb`). Added
    /// in the post-glitch round as the ONE structural extension point
    /// shared by every future color-shifting style — see
    /// `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §2.
    pub tint: Option<(u8, u8, u8, f32)>,
}

impl CellReveal {
    pub(super) fn hidden() -> Self {
        Self {
            visible: false,
            factor: 0.0,
            slide_rows: 0,
            glyph_override: None,
            tint: None,
        }
    }

    pub(super) fn settled() -> Self {
        Self {
            visible: true,
            factor: 1.0,
            slide_rows: 0,
            glyph_override: None,
            tint: None,
        }
    }
}

// ── Shared math helpers ────────────────────────────────────────────────────
//
// Owned here so sibling style files always agree on ramp shape and
// border lag. All styles derive their animation entirely from
// `message_start_time` elapsed time — no per-frame state, no per-char
// bookkeeping in Cloud.

/// Shared fade-in ramp start brightness (30%) — the typewriter-family
/// ramp used by typewriter, pulse, words, and slide (phase 2).
pub(super) const FADE_IN_START: f32 = 0.30;

/// Shared fade-in ramp: `FADE_IN_START` → 1.0 over `fade_ms`, indexed
/// by the cell's (or word's) reveal time. `elapsed_ms = None` means
/// "no animation timeline" → fully settled.
#[inline]
pub(super) fn char_fade_in(elapsed_ms: Option<usize>, reveal_at_ms: usize, fade_ms: usize) -> f32 {
    match elapsed_ms {
        None => 1.0,
        Some(ms) => {
            let age = ms.saturating_sub(reveal_at_ms);
            if age >= fade_ms {
                1.0
            } else {
                let progress = age as f32 / fade_ms as f32;
                FADE_IN_START + (1.0 - FADE_IN_START) * progress
            }
        }
    }
}

/// Shared border-lag curve for text-paced styles: `text_progress^1.5`
/// ease-out — the pre-v51 cinematic behavior. Used by typewriter,
/// pulse, slide, engrave, and words.
#[inline]
pub(super) fn lagged_border(text_progress: f32) -> f32 {
    text_progress.clamp(0.0, 1.0).powf(1.5)
}

/// Shared index pacing: cells revealed after `per_char_ms` each, with
/// the pre-v51 `.max(1)` floor (first cell at t=0) and `total_text`
/// ceiling. `elapsed_ms = None` → everything revealed (`usize::MAX`).
#[inline]
pub(super) fn index_pacing(
    per_char_ms: usize,
    elapsed_ms: Option<usize>,
    total_text: usize,
) -> usize {
    match elapsed_ms {
        None => usize::MAX,
        Some(ms) => (ms / per_char_ms).max(1).min(total_text.max(1)),
    }
}

/// Shared index fraction: revealed-cell fraction that feeds the border
/// lag for index-paced styles (typewriter / pulse / slide / engrave).
#[inline]
pub(super) fn index_fraction(reveal_count: usize, total_text: usize) -> f32 {
    (reveal_count.min(total_text.max(1))) as f32 / total_text.max(1) as f32
}

// ── Dispatch (runtime enum → style module) ─────────────────────────────────

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
///   (typewriter/pulse/slide/engrave). Other styles ignore it.
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
        MsgFillStyle::Typewriter => typewriter::reveal(content_idx, elapsed_ms, reveal_count),
        MsgFillStyle::Pulse => pulse::reveal(content_idx, elapsed_ms, reveal_count),
        MsgFillStyle::Fade => fade::reveal(block_alpha),
        MsgFillStyle::Instant => instant::reveal(),
        MsgFillStyle::Engrave => engrave::reveal(content_idx, elapsed_ms, reveal_count),
        MsgFillStyle::Hologram => hologram::reveal(content_idx, elapsed_ms, reveal_count),
        MsgFillStyle::Glitch => glitch::reveal(content_idx, elapsed_ms, reveal_count),
        MsgFillStyle::Scorch => scorch::reveal(content_idx, elapsed_ms, reveal_count),
        MsgFillStyle::Words => words::reveal(word_ord, elapsed_ms),
        MsgFillStyle::Slide => slide::reveal(content_idx, elapsed_ms),
    }
}

/// Resolve the whole-block alpha for the fade style from elapsed time.
#[inline]
pub(crate) fn fade_block_alpha(elapsed_ms: Option<usize>) -> f32 {
    fade::block_alpha(elapsed_ms)
}

/// Resolve the clockwise border progress (0.0 → 1.0) for the active
/// style, given the style's own text-progress input.
///
/// - Typewriter / pulse / slide / engrave / hologram / glitch / scorch / words: border
///   lags behind text (`text_progress^1.5` ease-out) — the pre-v51
///   cinematic behavior.
/// - Fade: border fades together with the text block.
/// - Instant: border draws clockwise on an independent 1 s timeline
///   (text is already fully visible).
pub(crate) fn border_progress(
    style: MsgFillStyle,
    text_progress: f32,
    elapsed_ms: Option<usize>,
) -> f32 {
    match style {
        MsgFillStyle::Typewriter => typewriter::border_progress(text_progress),
        MsgFillStyle::Pulse => pulse::border_progress(text_progress),
        MsgFillStyle::Fade => fade::border_progress(elapsed_ms),
        MsgFillStyle::Instant => instant::border_progress(elapsed_ms),
        MsgFillStyle::Engrave => engrave::border_progress(text_progress),
        MsgFillStyle::Hologram => hologram::border_progress(text_progress),
        MsgFillStyle::Glitch => glitch::border_progress(text_progress),
        MsgFillStyle::Scorch => scorch::border_progress(text_progress),
        MsgFillStyle::Words => words::border_progress(text_progress),
        MsgFillStyle::Slide => slide::border_progress(text_progress),
    }
}

/// Resolve the text progress (0.0 → 1.0) that feeds the border lag for
/// styles whose text reveal is index- or word-paced.
///
/// - Typewriter / pulse / slide / engrave / hologram / glitch / scorch:
///   `reveal_count / total_text`.
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
    match style {
        MsgFillStyle::Typewriter => typewriter::text_progress(reveal_count, total_text),
        MsgFillStyle::Pulse => pulse::text_progress(reveal_count, total_text),
        MsgFillStyle::Fade => fade::text_progress(),
        MsgFillStyle::Instant => instant::text_progress(),
        MsgFillStyle::Engrave => engrave::text_progress(reveal_count, total_text),
        MsgFillStyle::Hologram => hologram::text_progress(reveal_count, total_text),
        MsgFillStyle::Glitch => glitch::text_progress(reveal_count, total_text),
        MsgFillStyle::Scorch => scorch::text_progress(reveal_count, total_text),
        MsgFillStyle::Words => words::text_progress(total_words, elapsed_ms),
        MsgFillStyle::Slide => slide::text_progress(reveal_count, total_text),
    }
}

/// Number of content cells revealed by the active style at the given
/// elapsed time. Index-paced styles (typewriter/pulse/slide/engrave/
/// hologram/glitch/scorch) pace cells by their per-char constant; word/block styles
/// (words/fade/instant) reveal everything (their reveal math decides
/// per-cell and never reads the budget; only the `None` timeline →
/// `usize::MAX` is meaningful for them).
///
/// The `.max(1)` mirrors the pre-v51 renderer: the first cell appears
/// immediately so the very first frame is never fully empty.
#[inline]
pub(crate) fn index_reveal_count(
    style: MsgFillStyle,
    elapsed_ms: Option<usize>,
    total_text: usize,
) -> usize {
    match style {
        MsgFillStyle::Typewriter => typewriter::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Pulse => pulse::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Fade => fade::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Instant => instant::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Engrave => engrave::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Hologram => hologram::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Glitch => glitch::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Scorch => scorch::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Words => words::reveal_budget(elapsed_ms, total_text),
        MsgFillStyle::Slide => slide::reveal_budget(elapsed_ms, total_text),
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
        assert_eq!(MsgFillStyle::Hologram.as_str(), "hologram");
        assert_eq!(MsgFillStyle::Glitch.as_str(), "glitch");
        assert_eq!(MsgFillStyle::Scorch.as_str(), "scorch");
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
            MsgFillStyle::Hologram,
            MsgFillStyle::Glitch,
            MsgFillStyle::Scorch,
        ] {
            let r = content_reveal(style, 0, 1, None, usize::MAX, 1.0);
            assert!(r.visible, "{style:?} must be visible without a timeline");
            assert!((r.factor - 1.0).abs() < 1e-6, "{style:?} factor");
            let count = index_reveal_count(style, None, 10);
            assert_eq!(count, usize::MAX, "{style:?} reveal_count");
        }
    }

    #[test]
    fn shared_ramp_starts_at_30_percent_and_settles() {
        // The shared char_fade_in ramp (typewriter family shape).
        assert!((char_fade_in(Some(0), 0, 100) - FADE_IN_START).abs() < 1e-6);
        assert!((char_fade_in(Some(100), 0, 100) - 1.0).abs() < 1e-6);
        assert!((char_fade_in(None, 0, 100) - 1.0).abs() < 1e-6);
        // Mid-ramp at half the duration: exactly halfway 30% → 100%.
        assert!((char_fade_in(Some(50), 0, 100) - 0.65).abs() < 1e-6);
    }

    #[test]
    fn shared_lag_curve_is_power_15() {
        let bp = lagged_border(0.5);
        assert!((bp - 0.353_553_39).abs() < 1e-5);
        // Clamp guard: out-of-range input never produces NaN/growth.
        assert!((lagged_border(-0.5) - 0.0).abs() < 1e-6);
        assert!((lagged_border(1.5) - 1.0).abs() < 1e-6);
    }
}
