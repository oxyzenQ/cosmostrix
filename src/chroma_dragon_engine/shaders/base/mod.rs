// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Base Cell Shader — `resolve_cell_color()`
//!
//! This is the convergence point where palette, position, glyph, transition,
//! and head-state signals resolve to a single `(foreground, bold)` pair.
//!
//! Extracted verbatim from `cloud::render::DrawCtx::get_attr()` in Chroma
//! Dragon Phase 2. The body is identical; only the receiver changed from
//! `&DrawCtx` (which carries many non-color fields) to `&ShaderCtx` (which
//! carries only the inputs the shader actually reads).
//!
//! ## Performance
//!
//! Called 100–300× per frame on the hot path. `ShaderCtx` is a thin borrow
//! view — no allocation, no virtual dispatch. The function and its helper
//! `color_uses_previous_palette()` are marked `#[inline]` so LLVM can fold
//! the `DrawCtx → ShaderCtx → resolve_cell_color` chain at the call site,
//! yielding identical codegen to the pre-extraction monolith.

use bitvec::prelude::BitSlice;
use crossterm::style::Color;

use crate::constants::*;
use crate::runtime::{BoldMode, ColorMode};

/// Position of a cell within its droplet — determines which palette stop to
/// resolve. Moved here from `cloud::render` in Phase 2 because it is a pure
/// shader input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CharLoc {
    Middle,
    Tail,
    /// Multi-cell tail segment for front-layer droplets. `seg` is the
    /// position within the tail region (0 = furthest from head = darkest,
    /// increasing toward body). `total` is the total number of tail cells
    /// for this droplet (1..=FRONT_LAYER_TAIL_MAX_CELLS).
    ///
    /// The seg index is scaled linearly across palette color stops
    /// 0..FRONT_LAYER_MAX_TAIL_STOPS so the tail fades smoothly from the
    /// darkest stop to the body transition regardless of how many tail
    /// cells the droplet has. This keeps the 3-stop tail gradient while
    /// allowing long front-layer droplets to have proportional tails.
    ///
    /// Only used for layer 2 (front). Mid/back layers use `CharLoc::Tail`
    /// (single cell, color_idx=0) to preserve the existing 3-2-2 distribution.
    TailN {
        seg: u8,
        total: u8,
    },
    Head,
}

/// Read-only borrow view of the per-frame inputs that `resolve_cell_color`
/// actually reads. Constructed on-the-fly from `DrawCtx` (or any future
/// shader data source) at the call site — no allocation, just copies of
/// references and cheap scalars.
///
/// Carrying only the shader-visible subset (not the full `DrawCtx`) keeps
/// the borrow footprint small and makes future shader innovations (OKLab
/// gradient, dither LUT, atmospheric state) easy to add as new fields
/// without touching the renderer.
pub(crate) struct ShaderCtx<'a> {
    /// Per-slot palette color arrays for generation-based rendering.
    /// Index by droplet's `palette_slot` to resolve its birth palette.
    pub palette_slices: &'a [&'a [Color]; MAX_PALETTE_SLOTS],

    /// Which palette slot is the currently active (latest) one.
    /// Used for transition glow effects on new-generation streams.
    pub active_palette_slot: u8,

    /// Color transition wave line: during a palette transition, rows above
    /// this value use the new (active) palette; rows below use their birth
    /// palette. Sweeps from 0 to lines+1 over COLOR_TRANSITION_DURATION_MS,
    /// creating a top-to-bottom wave that matches the charset transition.
    pub color_wave_line: Option<f32>,

    pub bold_mode: BoldMode,
    pub lines: u16,
    pub color_map: &'a [u8],

    pub shading_distance: bool,

    pub glitchy: bool,
    pub glitch_map: &'a BitSlice,

    /// Cached `is_bright(now)` snapshot — avoids per-cell Instant arithmetic
    /// in the glitch branch (called 100–300×/frame when glitchy).
    pub glitch_bright: bool,
    /// Cached `is_dim(now)` snapshot.
    pub glitch_dim: bool,

    pub color_mode: ColorMode,

    /// Phase 3-C (Chroma Dragon Innovation C): temporal column hue coherence.
    ///
    /// `Some(lut)` enables a slow per-column hue drift: neighboring columns
    /// get similar color_idx perturbations (low spatial frequency), and the
    /// perturbation oscillates slowly over time (low temporal frequency).
    /// The result is that watching a single column, the colors shimmer
    /// smoothly through the palette instead of jumping per-cell.
    ///
    /// The LUT is pre-computed once per frame in `cloud::rain::rain_at` from
    /// the current time phase: `lut[col] = column_coherence_perturbation(phase,
    /// col)`. Length == `DrawCtx.cols`. The shader hot path is then a single
    /// indexed read instead of a per-cell `sinf + round + cast` (~65-130M
    /// cycles/sec saved at 60 FPS on a 200-col viewport).
    ///
    /// `None` disables the effect (used by shader no-op tests).
    pub column_coherence_lut: Option<&'a [i32]>,

    /// Phase 3-E (Chroma Dragon Innovation E): subpixel hue jitter.
    ///
    /// `Some(amplitude)` applies a per-cell RGB perturbation of ±amplitude
    /// units per channel, driven by a deterministic hash of `(line, col)`.
    /// The effect is fine film-grain texture — breaks up the uniformity of
    /// large same-color regions without changing the palette decision.
    ///
    /// "Subpixel" means the jitter is smaller than one palette step: it
    /// modifies the returned RGB directly, not the `color_idx`. This keeps
    /// the head→body→tail hierarchy intact while adding organic texture.
    ///
    /// `None` disables (current production default — wiring through `DrawCtx`
    /// is a future commit).
    pub subpixel_jitter_amplitude: Option<u8>,

    /// Phase 3-G (Chroma Dragon Innovation G): integrated atmospheric
    /// post-processing.
    ///
    /// `Some(ctx)` applies the frame's atmospheric factors (luminance
    /// dim/boost, saturation drift, persistence glow, instability
    /// flicker) to the resolved cell color BEFORE it is encoded as
    /// `Color::Rgb` and returned. This eliminates the old post-hoc
    /// decode-encode cycle (the `apply_climate_frame_effects`
    /// pass was deleted; climate is shader-only now) — the cell is
    /// written to the frame once with atmospheric already applied.
    ///
    /// `None` disables (cells render with raw palette colors). Production
    /// wires this through `DrawCtx` so the shader always applies atmospheric
    /// when factors are non-neutral.
    pub atmospheric: Option<&'a crate::chroma_dragon_engine::post::climate::ClimateCtx>,

    /// Phase 3-H (Chroma Dragon Innovation H): global hue drift.
    ///
    /// `Some(drift)` applies a global palette-stop offset to all Middle
    /// cells, derived from `ColorEcosystem.hue_drift` (which accumulates
    /// at `COLOR_HUE_DRIFT_RATE` per ecosystem tick and is clamped to
    /// `[-π, π]`). The offset is `(drift / π * 2.0).round() as i32`,
    /// giving a max shift of ±2 palette stops — subtle but visible over
    /// the ~10-minute drift cycle.
    ///
    /// Pre-Phase-3-H, `ColorEcosystem.hue_drift` was dead code: updated
    /// every tick but never read by the render path. Phase 3-H activates
    /// it as a slow global palette drift that makes the rain feel
    /// atmospherically alive — colors slowly cycle through adjacent
    /// stops over minutes, so the same scene looks slightly different
    /// each time you glance at it.
    ///
    /// Only applies to `CharLoc::Middle` (Head and Tail are pinned).
    /// Skipped under `shading_distance` (that path has its own
    /// length-aware gradient; stacking a hue shift would muddy the
    /// brightness-decay signal). Matches the column_coherence pattern.
    ///
    /// `None` disables (matches pre-Phase-3-H behavior — hue_drift
    /// accumulates in ColorEcosystem but never affects rendering).
    ///
    /// Phase C (per-frame hoist): the field carries the PRE-COMPUTED
    /// integer offset (not the raw f32 radians). The conversion
    /// `hue_drift_offset(drift)` runs once per frame at `DrawCtx`
    /// construction in `cloud/rain.rs`, so the per-cell hot path below
    /// is a single integer add — no f32 div/mul/round/cast per cell.
    /// At ~12.9M Middle cells/sec this saves ~65M cycles/sec.
    pub hue_drift_offset: Option<i32>,

    /// Phase 4-D (Chroma Dragon Innovation D — Dragon Awakening): head halo
    /// via background blend.
    ///
    /// `Some(factor)` blends the resolved Head cell color toward the scene
    /// background (`bg`) by `factor` (0.0 = no blend, 1.0 = pure bg). The
    /// effect softens the hard bright head pixel against the dark background
    /// — on a dark-cosmos bg, the head becomes slightly dimmer and bg-tinted,
    /// producing a "dissolve into the scene" rather than a stark white smear.
    ///
    /// Phase 3-D landed `blend_toward_bg` in `chroma::palette` but it had
    /// zero production callers. Phase 4-D wires it into the Head branch of
    /// `resolve_cell_color` so the halo is always-on.
    ///
    /// Applied ONLY to `CharLoc::Head` cells. Middle and Tail stops are
    /// pinned by the palette hierarchy and must not be haloed. Applied
    /// AFTER palette resolution and BEFORE subpixel jitter + atmospheric,
    /// so downstream effects compose on the haloed color.
    ///
    /// `None` disables (matches pre-Phase-4-D dormant behavior — kept for
    /// tests that assert the shader's no-op path). When `Some`, the halo
    /// is still a no-op if `bg` is `None` or `Color::Reset` (no RGB to
    /// blend toward).
    pub head_halo_factor: Option<f32>,

    /// Phase 4-D: the scene background color used by the head halo blend.
    ///
    /// This is the same `bg` carried by `DrawCtx` (used for blank cells,
    /// phosphor decay, etc.) — passed through to the shader so the halo
    /// knows what color to dissolve toward. `None` or `Color::Reset`
    /// disables the halo (no RGB to blend toward).
    pub bg: Option<Color>,

    /// Phase 5 (Chroma Dragon — perceptual L smoothing): precomputed
    /// OKLab L values for each stop index in both the old and new
    /// palettes, plus the current wave line position and smoothing
    /// window.
    ///
    /// `Some(table)` enables per-cell L smoothing within ±`window` lines
    /// of the wave line during a palette transition. The smoothing blends
    /// each cell's OKLab L channel toward the opposite palette's L for
    /// that stop index, eliminating the hard brightness step at the wave
    /// line. See `chroma::shaders::transition` for the full rationale.
    ///
    /// `None` disables (matches pre-Phase-5 behavior — palette
    /// transitions show a hard brightness step at the wave line).
    /// Production wires this through `DrawCtx` only when
    /// `transition_start.is_some()` AND `color_wave_line.is_some()`.
    pub transition_l_table:
        Option<&'a crate::chroma_dragon_engine::shaders::transition::TransitionLTable>,
}

/// Precomputed exponential decay lookup table for trail brightness.
/// Maps 256 normalized distances → exp(-TRAIL_EXPONENTIAL_K * t).
/// Eliminates ~3,000 exp() calls per frame in shading_distance mode.
///
/// Moved here from `cloud::render` in Phase 2 — it is a shader resource
/// owned by the chroma engine, not by the renderer.
pub(crate) static TRAIL_EXP_LUT: std::sync::LazyLock<[f32; 256]> = std::sync::LazyLock::new(|| {
    let mut lut = [0.0f32; 256];
    for (i, entry) in lut.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        *entry = (-(TRAIL_EXPONENTIAL_K as f32) * t).exp();
    }
    lut
});

/// Phase 3-F (Chroma Dragon Innovation F): luminance-remap threshold for
/// short droplets.
///
/// Droplets with `length <= SHORT_DROPLET_LUMINANCE_REMAP_THRESHOLD` get
/// their `CharLoc::Middle` cells remapped from the (random-uniform)
/// `color_map` value to a position-based ramp that spans the full palette
/// range — head-adjacent cells land on the brightest stop, tail-adjacent
/// cells on the darkest. Without this, short droplets (4–8 cells) sample
/// only 2–6 random `color_map` entries and look perceptually flat compared
/// to long droplets where the same random distribution produces visible
/// shimmering across many cells.
///
/// Threshold of 8 = 2× `MIN_DROPLET_LENGTH` (4). Below this, the visible
/// Middle range is too small for the random color_map to read as a
/// gradient. Above this, the existing color_map path produces enough
/// inter-cell variation to look natural.
///
/// Only applies when `!shading_distance` — that branch already has its
/// own length-aware exponential decay ramp. Also only applies to
/// `CharLoc::Middle` — Head and Tail stops are pinned by the shader
/// (`last` and `0` respectively) and should not be perturbed.
const SHORT_DROPLET_LUMINANCE_REMAP_THRESHOLD: u16 = 8;

/// Bayer 4×4 ordered dithering threshold matrix.
///
/// Each entry is in {0..=15}. The cell at `(line, col)` reads
/// `BAYER_4X4[line & 3][col & 3]`, divides by 16, and compares against the
/// fractional part of the continuous color value to decide whether to round
/// up or down. The matrix is laid out so the spatial average of the
/// up/down decisions equals undithered rounding — no brightness shift,
/// just banding broken into fine-grain texture.
///
/// Phase 3-B (Chroma Dragon Innovation B): eliminates visible banding on
/// long shading-distance droplets where many cells would otherwise share
/// the same `color_idx`.
pub(super) const BAYER_4X4: [[u8; 4]; 4] =
    [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Resolve a single cell's `(foreground, bold)` attribute pair.
///
/// This is the renderer's convergence point for palette, position, glyph,
/// transition, and head-state signals. Pure function — no hidden state,
/// no allocation, no side effects.
#[inline]
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_cell_color(
    shader: &ShaderCtx<'_>,
    palette_slot: u8,
    line: u16,
    col: u16,
    val: char,
    loc: CharLoc,
    head_put_line: u16,
    length: u16,
) -> (Option<Color>, bool) {
    // Resolve this stream's palette from the generation table.
    // During a color transition, cells above the wave line adopt the new
    // (active) palette even if the droplet was born with the old one,
    // creating a visible top-to-bottom cascade.
    let effective_slot = if color_uses_previous_palette(
        shader.color_wave_line,
        shader.active_palette_slot,
        palette_slot,
        line,
        col,
    ) {
        palette_slot // Below wave: keep birth palette
    } else {
        shader.active_palette_slot // Above wave or no transition: use new palette
    };
    let palette_colors = if (effective_slot as usize) < MAX_PALETTE_SLOTS {
        shader.palette_slices[effective_slot as usize]
    } else {
        // Fallback: use active palette for invalid slots
        shader.palette_slices[shader.active_palette_slot as usize]
    };

    let mut bold = false;
    // Phase 4-D: track whether this is a Head cell so the post-resolution
    // halo blend can apply exclusively to Head. Middle/Tail stops are pinned
    // by the palette hierarchy and must not be haloed.
    let mut is_head = false;
    if shader.bold_mode == BoldMode::Random {
        bold = (((line as u32) ^ (val as u32)) % 2) == 1;
    }

    let idx = col as usize * shader.lines as usize + line as usize;
    // Cosmic Dragon egg #15: bounds-check + direct indexing for color_map.
    // color_map is sized cols*lines. Callers pass col < cols and
    // line < lines (from droplet iteration), but defensive check is cheap
    // and avoids Option alloc on the hot path.
    let mut color_idx = if idx < shader.color_map.len() {
        shader.color_map[idx] as i32
    } else {
        0
    };
    // v50 (2026-08-17) LTS chroma dragon sync: track a float `t_param`
    // alongside the integer `color_idx` so the final palette lookup can
    // use `interpolate_palette_color` (linear lerp between adjacent stops)
    // instead of discrete `palette_colors[color_idx]`. This eliminates
    // visible bands when the palette has fewer stops than the droplet has
    // cells — matching the smooth gradient behavior the border message
    // (C4) and HUD overlay (C5) already use.
    //
    // Integer case is a no-op: when t_param lands exactly on an integer
    // palette position, interpolate_palette_color returns that stop
    // exactly (boundary branch). Real interpolation kicks in for the
    // three float-derived paths: shading_distance (v_continuous),
    // luminance-remap (v_continuous), and TailN (scaled_f).
    //
    // LTS stability: interpolate_palette_color is NaN/Inf-safe (returns
    // the first stop defensively), so a future upstream bug cannot crash
    // the renderer or produce garbage colors. Performance: ~3 ns per
    // call (one decode_color + one blend_toward_rgb) — well under 0.1%
    // CPU on a 200-col viewport at 60 FPS.
    let palette_last_f32 = (palette_colors.len().saturating_sub(1)).max(1) as f32;
    let mut t_param: f32 = color_idx as f32 / palette_last_f32;

    if shader.shading_distance {
        let last = palette_colors.len().saturating_sub(1) as u64;
        let dist = head_put_line.saturating_sub(line) as f64;
        let len = length.max(1) as f64;

        // Exponential decay: brightness = exp(-k * distance/length)
        let normalized_dist = (dist / len).clamp(0.0, 1.0);
        // OPTIMIZED: use precomputed LUT instead of exp() per cell
        let lut_idx = (normalized_dist * 255.0) as usize;
        let brightness = TRAIL_EXP_LUT[lut_idx.min(255)];
        let v_continuous = brightness * last as f32;

        // Phase 3-B (Chroma Dragon Innovation B): Bayer 4×4 ordered dithering.
        //
        // Rounding `v_continuous` to the nearest integer produces visible
        // banding — long droplets where many cells share the same distance
        // bucket all land on the same `color_idx`, so the trail shows hard
        // stair-steps instead of a smooth gradient. Bayer dithering splits
        // the rounding decision per-cell using a 4×4 threshold matrix:
        // neighboring cells alternate between adjacent palette stops, and
        // the eye perceives an intermediate color.
        //
        // The matrix is laid out so the spatial average of up/down decisions
        // equals undithered rounding — no brightness shift, just banding
        // broken into fine-grain texture. Threshold ∈ {0..15}/16, indexed
        // by (line mod 4, col mod 4) so the pattern tiles seamlessly.
        let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
        let frac = v_continuous - v_continuous.floor();
        let mut v = if frac > bayer_t {
            (v_continuous.floor() as u64 + 1).min(last)
        } else {
            v_continuous.floor() as u64
        };

        // Bloom: cells right behind head get extra brightness
        if dist < HEAD_BLOOM_CELLS as f64 {
            v = (v + 1).min(last);
        }

        color_idx = v as i32;
        // v50 (2026-08-17) LTS chroma dragon sync: capture the float
        // position BEFORE Bayer dithering rounds it away. This is the
        // primary smooth-gradient path — long droplets with many cells
        // now get smoothly-varying colors instead of stair-stepped bands.
        t_param = (v_continuous / last as f32).clamp(0.0, 1.0);
    }

    // Cosmic Dragon egg #16: bounds-check + direct indexing for glitch_map.
    // idx = col*lines + line, same as color_map above. Already computed.
    if shader.glitchy && idx < shader.glitch_map.len() && shader.glitch_map[idx] {
        // PERF: glitch_bright/glitch_dim are cached once per DrawCtx
        // construction (rain_at) — they depend only on `now`, not on
        // cell position, so recomputing per-cell was pure waste.
        if shader.glitch_bright {
            color_idx += 1;
            // v50 (2026-08-17) LTS chroma dragon sync: mirror the integer
            // ±1 glitch adjustment on t_param so the smooth interpolation
            // path stays consistent with the discrete color_idx path.
            t_param += 1.0 / palette_last_f32;
            bold = true;
        } else if shader.glitch_dim {
            color_idx -= 1;
            t_param -= 1.0 / palette_last_f32;
            bold = false;
        }
    }

    let last = palette_colors.len().saturating_sub(1) as i32;
    match loc {
        CharLoc::Tail => {
            color_idx = 0;
            // v50 (2026-08-17) LTS chroma dragon sync: Tail is the
            // darkest stop (palette[0]); t_param = 0.0.
            t_param = 0.0;
            bold = false;
        }
        CharLoc::TailN { seg, total } => {
            // Front-layer multi-cell tail: scale seg across palette color
            // stops 0..FRONT_LAYER_MAX_TAIL_STOPS. seg=0 → darkest
            // (furthest from head), seg=total-1 → brightest tail stop
            // (closest to body). When total > MAX_STOPS, multiple cells
            // share a stop, producing a smooth gradient even for long
            // proportional tails. Clamped to valid palette range.
            let max_stop = (crate::constants::FRONT_LAYER_MAX_TAIL_STOPS as i32).min(last);
            let total_cells = (total as i32).max(1);
            // Map seg [0, total-1] to color_idx [0, max_stop] linearly.
            // Using (max_stop + 1) as numerator ensures seg=total-1 maps
            // exactly to max_stop (no off-by-one at the bright end).
            let scaled = (seg as i32 * (max_stop + 1)) / total_cells;
            color_idx = scaled.min(max_stop).max(0);
            // v50 (2026-08-17) LTS chroma dragon sync: capture the FLOAT
            // position BEFORE integer truncation rounds it away. This is
            // the third primary smooth-gradient path — multi-cell tails
            // with more cells than palette stops now interpolate smoothly
            // between adjacent stops instead of showing discrete bands.
            let scaled_f = seg as f32 * (max_stop as f32 + 1.0) / total_cells as f32;
            t_param = (scaled_f / palette_last_f32).clamp(0.0, 1.0);
            bold = false;
        }
        CharLoc::Head => {
            color_idx = last;
            // v50 (2026-08-17) LTS chroma dragon sync: Head is the
            // brightest stop (palette[last]); t_param = 1.0.
            t_param = 1.0;
            bold = true;
            is_head = true;
        }
        CharLoc::Middle => {
            color_idx = color_idx.clamp(0, last.max(0));
            t_param = t_param.clamp(0.0, 1.0);
            // Phase 3-F (Chroma Dragon Innovation F): luminance-remap for
            // short droplets.
            //
            // For droplets with `length <= SHORT_DROPLET_LUMINANCE_REMAP_THRESHOLD`,
            // replace the random-uniform color_map value with a position-based
            // ramp that maps the Middle range (dist_from_head ∈ [1, length-2])
            // onto the full palette range [last, 0]. Head-adjacent cells land
            // on the brightest stop, tail-adjacent cells on the darkest —
            // giving short droplets a visible head→tail gradient even when
            // they only have 2–6 Middle cells.
            //
            // Without this, short droplets look perceptually flat: the
            // color_map gives each cell an independent random color_idx in
            // [1, n-2], so a 4-cell droplet samples 2 random stops and reads
            // as a uniform-ish blob. Long droplets have many Middle cells so
            // the same distribution produces visible shimmer — but the
            // short-droplet case needs an explicit ramp.
            //
            // Skipped under shading_distance: that branch already computes
            // a length-aware exponential decay ramp and would be overwritten
            // here. length >= MIN_DROPLET_LENGTH (4), so `length - 3 >= 1`
            // — no div-by-zero.
            if !shader.shading_distance && length <= SHORT_DROPLET_LUMINANCE_REMAP_THRESHOLD {
                let dist_from_head = head_put_line.saturating_sub(line);
                let denom = ((length as i32) - 3).max(1) as f32;
                let t = (((dist_from_head as i32) - 1) as f32 / denom).clamp(0.0, 1.0);
                // masterclass: Bayer 4×4 ordered dithering on the
                // short-droplet luminance-remap path. Same pattern as the
                // shading_distance branch (line 500-506) — breaks up palette-
                // boundary banding on short droplets (2-6 Middle cells) where
                // discrete rounding would assign the same color_idx to
                // adjacent cells. Both built-in and colors-custom palettes
                // benefit equally (same shader path = same dithering).
                let v_continuous = (1.0 - t) * last as f32;
                let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
                let frac = v_continuous - v_continuous.floor();
                color_idx = if frac > bayer_t {
                    (v_continuous.floor() as i32 + 1).min(last)
                } else {
                    v_continuous.floor() as i32
                };
                // v50 (2026-08-17) LTS chroma dragon sync: capture the
                // float position for smooth interpolation on short
                // droplets (2-6 Middle cells). The Bayer dithering still
                // applies to color_idx for backward compatibility with any
                // downstream integer-math consumers, but the final palette
                // lookup uses t_param (smooth) instead of color_idx
                // (discrete+dithered).
                t_param = (v_continuous / palette_last_f32).clamp(0.0, 1.0);
            }
            // Phase 3-H + Phase C: global hue drift, now pre-computed
            // per-frame. The `hue_drift_offset` fn ran once at DrawCtx
            // construction (cloud/rain.rs) — here we just add the integer.
            // Saves a per-cell f32 div + mul + round + cast (~5 cycles)
            // × ~12.9M Middle cells/sec ≈ 65M cycles/sec.
            //
            // Skipped under shading_distance (matches column_coherence
            // pattern — that path has its own length-aware gradient and
            // stacking a hue shift would muddy the brightness-decay
            // signal). Head and Tail are pinned (last and 0 respectively)
            // and must not be perturbed.
            //
            // Offset is in {-2, -1, 0, +1, +2} — subtle enough to feel
            // atmospheric, visible enough to notice over the ~10-minute
            // drift cycle.
            if let Some(offset) = shader.hue_drift_offset {
                if !shader.shading_distance {
                    color_idx = (color_idx + offset).clamp(0, last.max(0));
                    // v50 (2026-08-17) LTS chroma dragon sync: mirror the
                    // integer hue-drift offset on t_param as a fractional
                    // delta. Offset ∈ {-2, -1, 0, +1, +2} — subtle enough
                    // to feel atmospheric, visible enough to notice over the
                    // ~10-minute drift cycle.
                    t_param = (t_param + offset as f32 / palette_last_f32).clamp(0.0, 1.0);
                }
            }
            // Phase 3-C (Chroma Dragon Innovation C): temporal column hue
            // coherence. For body cells (not Head/Tail — those need to stay
            // anchored to their stop), apply a slow per-column hue drift
            // driven by the time phase. Disabled when column_coherence_lut
            // is None (test no-op path).
            //
            // Effect: neighboring columns get similar perturbations, and the
            // perturbation oscillates slowly over time, so a single column
            // shimmers smoothly through adjacent palette stops instead of
            // being locked to one color_idx.
            //
            // Not applied under shading_distance: that path already has its
            // own continuous gradient + Bayer dithering, and stacking a hue
            // perturbation on top would muddy the brightness-decay signal.
            //
            // Phase D (hot-path): perturbation is now a single indexed read
            // from `column_coherence_lut[col]` instead of a per-cell
            // `sinf + round + cast`. The LUT is built once per frame in
            // `rain.rs`. Saves ~65-130M cycles/sec at 60 FPS on a 200-col
            // viewport.
            if let Some(lut) = shader.column_coherence_lut {
                if !shader.shading_distance {
                    let perturbation = lut[col as usize];
                    color_idx = (color_idx + perturbation).clamp(0, last.max(0));
                    // v50 (2026-08-17) LTS chroma dragon sync: mirror the
                    // integer column-coherence perturbation on t_param. A
                    // single column now shimmers smoothly through adjacent
                    // palette stops instead of jumping between discrete
                    // color_idx values.
                    t_param = (t_param + perturbation as f32 / palette_last_f32).clamp(0.0, 1.0);
                }
            }
        }
    }

    match shader.bold_mode {
        BoldMode::Off => bold = false,
        BoldMode::All => bold = true,
        BoldMode::Random => {}
    }

    let fg = if shader.color_mode == ColorMode::Mono {
        None
    } else {
        // v50 (2026-08-17) LTS chroma dragon sync: use the smooth
        // interpolation helper introduced in C4 (border message fix)
        // and reused in C5 (HUD chroma gradient fix). For integer
        // t_param (Tail/Head/Middle default paths), the helper returns
        // palette[pos] exactly (boundary branch, no interpolation) —
        // no behavior change for the discrete case. For float-derived
        // t_param (shading_distance / luminance-remap / TailN paths),
        // the helper linearly blends between adjacent palette stops,
        // eliminating visible bands.
        //
        // Fallback to discrete `palette_colors.get(color_idx)` if the
        // helper returns None (empty palette defensive case). This
        // preserves the original behavior under degenerate palettes.
        crate::cloud::interpolate_palette_color(palette_colors, t_param)
            .or_else(|| palette_colors.get(color_idx as usize).copied())
    };

    // Phase 4-D (Chroma Dragon Innovation D — Dragon Awakening): head halo
    // via background blend.
    //
    // Blend the resolved Head cell color toward the scene background by
    // `head_halo_factor`. This softens the hard bright head pixel against
    // the dark background — on a dark-cosmos bg, the head becomes slightly
    // dimmer and bg-tinted, producing a "dissolve into the scene" rather
    // than a stark white smear.
    //
    // Applied ONLY to Head cells (is_head gate). Applied AFTER palette
    // resolution and BEFORE subpixel jitter + atmospheric, so downstream
    // effects compose on the haloed color.
    //
    // `blend_toward_bg` auto-no-ops when factor ≤ 0, color is Reset, or bg
    // is Reset — so `None` bg or `Color::Reset` bg safely disables the
    // halo without an extra branch here.
    let fg = if is_head {
        fg.map(|c| match (shader.head_halo_factor, shader.bg) {
            (Some(factor), Some(bg)) => {
                crate::chroma_dragon_engine::palette::blend_toward_bg(c, bg, factor)
            }
            _ => c,
        })
    } else {
        fg
    };

    // Phase 5 (Chroma Dragon — perceptual L smoothing at palette
    // transition wave).
    //
    // During a palette transition, `color_wave_line` sweeps top-to-bottom.
    // Cells above use the new palette; cells below use the old. If the
    // two palettes have different perceptual luminance (OKLab L) at
    // corresponding stop indices, the wave line becomes a visible
    // brightness step — a hard horizontal stripe.
    //
    // Within ±`window` lines of the wave, blend each cell's OKLab L
    // channel toward the opposite palette's L for that stop index. The
    // blend peaks at 0.5 at the wave line (50% midpoint — no palette
    // swap) and falls off linearly to 0 at ±window.
    //
    // Applied AFTER palette resolution + head halo, BEFORE subpixel
    // jitter + atmospheric. This ensures:
    // - The L blend composes on the post-halo color (haloed head also
    //   gets smoothed, which is correct — the halo is a per-cell effect
    //   that should not override the cross-cell transition smoothing).
    // - The jitter composes on the smoothed color (so film-grain
    //   perturbs the smoothed result, not the other way around).
    //
    // `apply_l_smoothing` early-returns cheaply when:
    // - `table` is None (transition inactive — most frames)
    // - color is Reset (no RGB to modify)
    // - cell is outside the smoothing window (most cells during transition)
    // - stop_idx is out of the table's range
    // - L_old == L_new (no luminance difference)
    //
    // See `chroma::shaders::transition` for the full rationale and
    // the per-cell cost analysis.
    let fg = fg.map(|c| {
        crate::chroma_dragon_engine::shaders::transition::apply_l_smoothing(
            c,
            shader.transition_l_table,
            color_idx,
            line,
        )
    });

    // Phase 3-E (Chroma Dragon Innovation E): subpixel hue jitter.
    //
    // Apply a per-cell RGB perturbation driven by a deterministic hash of
    // (line, col). The effect is fine film-grain texture — breaks up the
    // uniformity of large same-color regions without changing the palette
    // decision. "Subpixel" means the jitter is smaller than one palette
    // step: it modifies the returned RGB directly, not the color_idx, so
    // the head→body→tail hierarchy stays intact.
    //
    // Disabled when subpixel_jitter_amplitude is None (production default).
    // The hash is deterministic, so the same cell always gets the same
    // jitter — no strobing across frames.
    let fg = fg.map(|c| match shader.subpixel_jitter_amplitude {
        Some(amp) => apply_subpixel_jitter(c, cell_hash(line, col), amp),
        None => c,
    });

    // Phase 3-G (Chroma Dragon Innovation G): integrated atmospheric
    // post-processing.
    //
    // Apply the frame's atmospheric factors (luminance dim/boost, saturation
    // drift, persistence glow, instability flicker) to the resolved cell
    // color BEFORE returning. This eliminates the old post-hoc decode-encode
    // cycle (`apply_climate_frame_effects` was deleted; climate is
    // shader-only now) — the cell is written to the frame once with
    // atmospheric already applied.
    //
    // Disabled when `atmospheric` is None (cells render with raw palette
    // colors). When Some, the shader is the sole source of atmospheric
    // modification.
    //
    // `Color::Reset` (None fg) is skipped — there's no RGB to modify.
    let fg = fg.map(|c| {
        let Some(ctx) = shader.atmospheric else {
            return c;
        };
        // Fast path: neutral ctx is a no-op (matches the old post-hoc
        // "skip if all neutral" early-return, now deleted).
        if ctx.is_neutral() {
            return c;
        }
        let (r, g, b) = crate::chroma_dragon_engine::palette::color_to_rgb(c);
        let (r, g, b) =
            crate::chroma_dragon_engine::post::climate::apply_climate(r, g, b, line, col, ctx);
        Color::Rgb { r, g, b }
    });

    (fg, bold)
}

/// Test helper: build a minimal ShaderCtx for testing resolve_cell_color.
/// Caller supplies the `palette_slices` array (so it outlives the
/// ShaderCtx borrow) and the color_map slice. color_map is initialized
/// to a constant value in the tests so we can detect when the remap
/// overrides it.
#[cfg(test)]
pub(super) fn make_test_shader<'a>(
    palette_slices: &'a [&'a [Color]; MAX_PALETTE_SLOTS],
    color_map: &'a [u8],
    shading_distance: bool,
) -> ShaderCtx<'a> {
    ShaderCtx {
        palette_slices,
        active_palette_slot: 0,
        color_wave_line: None,
        bold_mode: BoldMode::Random,
        lines: 50,
        color_map,
        shading_distance,
        glitchy: false,
        glitch_map: <&BitSlice>::default(),
        glitch_bright: false,
        glitch_dim: false,
        color_mode: ColorMode::TrueColor,
        column_coherence_lut: None,
        subpixel_jitter_amplitude: None,
        atmospheric: None,
        hue_drift_offset: None,
        head_halo_factor: None,
        transition_l_table: None,
        bg: None,
    }
}

/// Test helper: build a `MAX_PALETTE_SLOTS`-sized palette_slices array with
/// slot 0 pointing to the given palette and all other slots empty.
#[cfg(test)]
pub(super) fn slot_array(palette: &[Color]) -> [&[Color]; MAX_PALETTE_SLOTS] {
    let mut arr: [&[Color]; MAX_PALETTE_SLOTS] = [&[]; MAX_PALETTE_SLOTS];
    arr[0] = palette;
    arr
}

// v50.0.0-beta.7 LOC refactor: 6 shader helper functions extracted to
// helpers.rs. The pub(crate) ones are re-exported here; the pub(super)
// ones (bayer_threshold, cell_hash, apply_subpixel_jitter) are imported
// for direct use in resolve_cell_color.
mod helpers;
use helpers::{apply_subpixel_jitter, bayer_threshold, cell_hash};
pub(crate) use helpers::{
    color_uses_previous_palette, column_coherence_perturbation, hue_drift_offset,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_activation;

#[cfg(test)]
mod tests_bold_audit;
