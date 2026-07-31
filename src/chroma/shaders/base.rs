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
pub enum CharLoc {
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
pub struct ShaderCtx<'a> {
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
    /// `Some(phase)` enables a slow per-column hue drift: neighboring columns
    /// get similar color_idx perturbations (low spatial frequency), and the
    /// perturbation oscillates slowly over time (low temporal frequency).
    /// The result is that watching a single column, the colors shimmer
    /// smoothly through the palette instead of jumping per-cell.
    ///
    /// `None` disables the effect (current production default — wiring this
    /// through `DrawCtx` and `rain.rs` is a future commit).
    pub column_coherence_phase: Option<f32>,

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
    /// `Color::Rgb` and returned. This eliminates the post-hoc
    /// decode-encode cycle that `cloud::phosphor::apply_atmospheric_frame_effects`
    /// performed on dirty cells — the cell is now written to the frame
    /// once with atmospheric already applied.
    ///
    /// `None` disables (matches pre-Phase-3-G behavior — the post-hoc
    /// pass runs instead). Production wires this through `DrawCtx` so
    /// the shader always applies atmospheric when factors are non-neutral.
    pub atmospheric: Option<&'a crate::chroma::post::atmosphere::AtmosphericCtx>,

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
    pub hue_drift: Option<f32>,
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
const BAYER_4X4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Read the Bayer 4×4 threshold for a cell at `(line, col)`.
/// Returns a value in `{0..=15}`. Indexed by `line mod 4` and `col mod 4`
/// so the pattern tiles every 4×4 block.
#[inline]
fn bayer_threshold(line: u16, col: u16) -> u8 {
    // Bitwise AND with 3 is equivalent to % 4 but avoids the division.
    BAYER_4X4[(line as usize) & 3][(col as usize) & 3]
}

/// Phase 3-C: compute the per-cell column-coherence hue perturbation.
///
/// Returns an integer offset in `{-1, 0, +1}` that nudges the cell's
/// `color_idx` based on its column position and the current time phase.
/// Neighboring columns get similar offsets (low spatial frequency:
/// 0.05 rad/col → period ~125 columns), and the offset drifts slowly
/// over time (caller advances `phase` by a small amount per frame).
///
/// Amplitude is ±0.5 before rounding, so the offset is 0 most of the
/// time and ±1 near the peaks of the sine. This produces a gentle
/// "shimmer" rather than a strong hue shift.
#[inline]
fn column_coherence_perturbation(phase: f32, col: u16) -> i32 {
    // Spatial frequency: 0.05 rad/col → period ~125 cols
    let spatial = (col as f32) * 0.05;
    // Amplitude: ±0.5 → rounds to {-1, 0, +1}
    ((phase + spatial).sin() * 0.5_f32).round() as i32
}

/// Phase 3-H: compute the global hue-drift palette-stop offset.
///
/// Maps `ColorEcosystem.hue_drift` (in radians, clamped to `[-π, π]`) to
/// an integer palette-stop offset in `{-2, -1, 0, +1, +2}`. The scaling
/// `drift / π * 2.0` means a full π rotation shifts by 2 stops — subtle
/// enough to feel atmospheric, visible enough to notice over the ~10-minute
/// drift cycle (COLOR_HUE_DRIFT_RATE = 0.015 rad/tick, 1 tick/3sec).
///
/// Unlike column_coherence (per-cell perturbation), hue_drift is a GLOBAL
/// offset: every Middle cell in every column shifts by the same amount.
/// The effect is that the entire scene's palette slowly cycles through
/// adjacent stops, so the same column looks slightly different each minute.
///
/// Returns 0 for `drift = 0.0` (no shift) and for very small drifts
/// (|drift| < π/4 ≈ 0.785 rad, which rounds to 0).
#[inline]
fn hue_drift_offset(drift: f32) -> i32 {
    (drift / std::f32::consts::PI * 2.0_f32).round() as i32
}

/// Phase 3-E: deterministic per-cell hash for subpixel jitter.
///
/// Returns a u32 that varies pseudo-randomly with `(line, col)`. The same
/// input always produces the same output (deterministic), but different
/// inputs produce uncorrelated outputs (low collision rate). Used to drive
/// per-cell RGB perturbation so the film-grain texture is stable across
/// frames — the same cell always gets the same jitter, so it doesn't strobe.
///
/// Implementation: FNV-1a variant with line and col mixed in via XOR
/// after each multiply step. Cheap (3 multiplies + 2 XORs), no allocation.
#[inline]
fn cell_hash(line: u16, col: u16) -> u32 {
    let mut h = 0x811C9DC5u32; // FNV offset basis
    h ^= line as u32;
    h = h.wrapping_mul(16777619); // FNV prime
    h ^= col as u32;
    h = h.wrapping_mul(16777619);
    h
}

/// Phase 3-E: apply per-cell RGB jitter to a color.
///
/// Perturbs each channel by an independent signed offset in `[-amp, +amp]`,
/// derived from three independent 4-bit slices of `hash`. The result is
/// clamped to `[0, 255]` per channel.
///
/// `amplitude = 0` or `Color::Reset` input returns the original unchanged.
/// Output is always `Color::Rgb` (normalized via `color_to_rgb`).
#[inline]
fn apply_subpixel_jitter(color: Color, hash: u32, amplitude: u8) -> Color {
    if amplitude == 0 || matches!(color, Color::Reset) {
        return color;
    }
    let (r, g, b) = crate::chroma::palette::color_to_rgb(color);
    let amp = i32::from(amplitude);
    // Three independent 4-bit signed offsets in [-8, +7].
    let dr_raw = (hash & 0xF) as i32 - 8;
    let dg_raw = ((hash >> 4) & 0xF) as i32 - 8;
    let db_raw = ((hash >> 8) & 0xF) as i32 - 8;
    // Scale [-8, +7] → [-amp, +amp*7/8]. Slight asymmetry is acceptable
    // for film-grain — the perceptual effect is symmetric.
    let dr = dr_raw * amp / 8;
    let dg = dg_raw * amp / 8;
    let db = db_raw * amp / 8;
    Color::Rgb {
        r: (i32::from(r) + dr).clamp(0, 255) as u8,
        g: (i32::from(g) + dg).clamp(0, 255) as u8,
        b: (i32::from(b) + db).clamp(0, 255) as u8,
    }
}

/// During a color transition, returns whether a cell at `(line, col)` should
/// use its birth (previous) palette rather than the new (active) palette.
/// Rows below the wave line use the old palette; rows above use the new.
/// This creates a top-to-bottom cascade matching the charset transition.
///
/// Extracted as a free function so both `DrawCtx::color_uses_previous_palette`
/// (called from `monolith.rs`) and `resolve_cell_color` share one source of
/// truth — previously the shader inlined its own copy of the wave test.
#[inline]
pub fn color_uses_previous_palette(
    color_wave_line: Option<f32>,
    active_palette_slot: u8,
    palette_slot: u8,
    line: u16,
    col: u16,
) -> bool {
    let Some(wave_line) = color_wave_line else {
        return false;
    };
    // Only applies to droplets that still carry the old palette slot
    if palette_slot == active_palette_slot {
        return false;
    }
    // Jitter for organic edge (same pattern as charset wave)
    let jitter =
        (((line as u32).wrapping_mul(13) ^ (col as u32).wrapping_mul(29)) % 3) as f32 * 0.15;
    (line as f32) > wave_line + jitter
}

/// Resolve a single cell's `(foreground, bold)` attribute pair.
///
/// This is the renderer's convergence point for palette, position, glyph,
/// transition, and head-state signals. Pure function — no hidden state,
/// no allocation, no side effects.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn resolve_cell_color(
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
    }

    // Cosmic Dragon egg #16: bounds-check + direct indexing for glitch_map.
    // idx = col*lines + line, same as color_map above. Already computed.
    if shader.glitchy && idx < shader.glitch_map.len() && shader.glitch_map[idx] {
        // PERF: glitch_bright/glitch_dim are cached once per DrawCtx
        // construction (rain_at) — they depend only on `now`, not on
        // cell position, so recomputing per-cell was pure waste.
        if shader.glitch_bright {
            color_idx += 1;
            bold = true;
        } else if shader.glitch_dim {
            color_idx -= 1;
            bold = false;
        }
    }

    let last = palette_colors.len().saturating_sub(1) as i32;
    match loc {
        CharLoc::Tail => {
            color_idx = 0;
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
            bold = false;
        }
        CharLoc::Head => {
            color_idx = last;
            bold = true;
        }
        CharLoc::Middle => {
            color_idx = color_idx.clamp(0, last.max(0));
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
                color_idx = ((1.0 - t) * last as f32).round() as i32;
            }
            // Phase 3-H (Chroma Dragon Innovation H): global hue drift.
            //
            // Apply the frame's global hue_drift as a palette-stop offset
            // to all Middle cells. This activates the previously-dead
            // ColorEcosystem.hue_drift field — it was updated every tick
            // but never read by the render path. Phase 3-H makes the
            // accumulated drift visible as a slow palette cycle.
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
            if let Some(drift) = shader.hue_drift {
                if !shader.shading_distance {
                    let offset = hue_drift_offset(drift);
                    color_idx = (color_idx + offset).clamp(0, last.max(0));
                }
            }
            // Phase 3-C (Chroma Dragon Innovation C): temporal column hue
            // coherence. For body cells (not Head/Tail — those need to stay
            // anchored to their stop), apply a slow per-column hue drift
            // driven by the time phase. Disabled when column_coherence_phase
            // is None (current production default).
            //
            // Effect: neighboring columns get similar perturbations, and the
            // perturbation oscillates slowly over time, so a single column
            // shimmers smoothly through adjacent palette stops instead of
            // being locked to one color_idx.
            //
            // Not applied under shading_distance: that path already has its
            // own continuous gradient + Bayer dithering, and stacking a hue
            // perturbation on top would muddy the brightness-decay signal.
            if let Some(phase) = shader.column_coherence_phase {
                if !shader.shading_distance {
                    let perturbation = column_coherence_perturbation(phase, col);
                    color_idx = (color_idx + perturbation).clamp(0, last.max(0));
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
        palette_colors.get(color_idx as usize).copied()
    };

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
    // color BEFORE returning. This eliminates the post-hoc decode-encode
    // cycle that `cloud::phosphor::apply_atmospheric_frame_effects` performed
    // on dirty cells — the cell is now written to the frame once with
    // atmospheric already applied.
    //
    // Disabled when `atmospheric` is None (matches pre-Phase-3-G behavior —
    // the post-hoc pass runs instead). When Some, the post-hoc pass is a
    // no-op (early return), so the shader is the sole source of atmospheric
    // modification — no double-application.
    //
    // `Color::Reset` (None fg) is skipped — there's no RGB to modify.
    let fg = fg.map(|c| {
        let Some(ctx) = shader.atmospheric else {
            return c;
        };
        // Fast path: neutral ctx is a no-op (matches the pre-Phase-3-G
        // "skip if all neutral" early-return in apply_atmospheric_frame_effects).
        if ctx.is_neutral() {
            return c;
        }
        let (r, g, b) = crate::chroma::palette::color_to_rgb(c);
        let (r, g, b) = crate::chroma::post::atmosphere::apply_atmospheric(r, g, b, line, col, ctx);
        Color::Rgb { r, g, b }
    });

    (fg, bold)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bayer matrix tiles every 4×4 block — same (line, col) mod 4 returns
    /// the same threshold.
    #[test]
    fn bayer_threshold_tiles_4x4() {
        for line in 0..16u16 {
            for col in 0..16u16 {
                let a = bayer_threshold(line, col);
                let b = bayer_threshold(line + 4, col + 4);
                assert_eq!(a, b, "Bayer threshold must tile every 4×4 block");
            }
        }
    }

    /// Bayer matrix is a permutation of {0..=15} — every threshold appears
    /// exactly once per 4×4 tile. This is what makes the spatial average
    /// equal undithered rounding.
    #[test]
    fn bayer_matrix_is_permutation_of_0_to_15() {
        let mut seen = [false; 16];
        for row in &BAYER_4X4 {
            for &v in row {
                let v = v as usize;
                assert!(v < 16, "Bayer entry out of range: {v}");
                assert!(!seen[v], "Bayer entry {v} appears more than once");
                seen[v] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "Not all thresholds 0..=15 present");
    }

    /// Bayer dithering preserves the spatial average: averaging the
    /// up/down decision across a full 4×4 tile equals undithered rounding
    /// of `v_continuous + 0.5/16` (a tiny bias from the threshold layout).
    /// In practice this means no brightness shift — just banding broken
    /// into fine texture.
    #[test]
    fn bayer_dither_preserves_spatial_average() {
        // Pick a continuous value whose fractional part is exactly 0.5 —
        // the worst case for rounding bias.
        let v_continuous = 4.5_f32;
        let frac = v_continuous - v_continuous.floor();
        assert!((frac - 0.5).abs() < 1e-6);

        // For each of the 16 cells in a 4×4 tile, compute the dithered v.
        let mut sum: u64 = 0;
        for line in 0..4u16 {
            for col in 0..4u16 {
                let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
                let v = if frac > bayer_t {
                    v_continuous.floor() as u64 + 1
                } else {
                    v_continuous.floor() as u64
                };
                sum += v;
            }
        }
        // Average should be ~4.5 (between 4 and 5). With 16 cells and
        // thresholds {0..15}/16, exactly 8 cells round up (frac > t when
        // t ∈ {0..7}/16) and 8 round down. So sum = 8*4 + 8*5 = 72,
        // average = 4.5 — exactly the continuous value.
        assert_eq!(sum, 72, "16-cell sum should be 8*4 + 8*5 = 72");
        let avg = sum as f32 / 16.0;
        assert!(
            (avg - v_continuous).abs() < 1e-6,
            "Spatial average {avg} should equal continuous value {v_continuous}"
        );
    }

    /// Bayer dithering rounds down to floor when frac=0 (no fractional part).
    #[test]
    fn bayer_dither_rounds_down_at_zero_frac() {
        let v_continuous = 4.0_f32; // no fractional part
        let frac = v_continuous - v_continuous.floor();
        assert!(frac < 1e-6);
        for line in 0..4u16 {
            for col in 0..4u16 {
                let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
                let v = if frac > bayer_t {
                    v_continuous.floor() as u64 + 1
                } else {
                    v_continuous.floor() as u64
                };
                assert_eq!(v, 4, "Zero frac should always round down");
            }
        }
    }

    /// Bayer dithering rounds up to ceil when frac ≥ 15/16 (nearly 1.0).
    #[test]
    fn bayer_dither_rounds_up_at_near_one_frac() {
        let v_continuous = 4.9375_f32; // frac = 15/16
        let frac = v_continuous - v_continuous.floor();
        // 15/16 = 0.9375; only the threshold 15/16 itself fails (frac > t
        // is false when frac == t). So 15 of 16 cells round up.
        let mut ups = 0;
        let mut downs = 0;
        for line in 0..4u16 {
            for col in 0..4u16 {
                let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
                let v = if frac > bayer_t {
                    v_continuous.floor() as u64 + 1
                } else {
                    v_continuous.floor() as u64
                };
                if v == 5 {
                    ups += 1;
                } else {
                    downs += 1;
                }
            }
        }
        assert_eq!(ups, 15, "frac=15/16 should round up in 15 of 16 cells");
        assert_eq!(downs, 1, "frac=15/16 should round down in 1 of 16 cells");
    }

    // ── Phase 3-C: column-coherence perturbation ──────────────────────────

    /// Perturbation is always in {-1, 0, +1} — never larger.
    #[test]
    fn column_coherence_perturbation_bounded() {
        for col in 0..256u16 {
            for phase_deg in 0..360 {
                let phase = phase_deg as f32 * std::f32::consts::PI / 180.0;
                let p = column_coherence_perturbation(phase, col);
                assert!(
                    (-1..=1).contains(&p),
                    "perturbation {p} out of [-1, +1] for phase={phase}, col={col}"
                );
            }
        }
    }

    /// Perturbation at phase=0, col=0 is exactly 0 (sin(0) * 0.5 = 0).
    #[test]
    fn column_coherence_perturbation_zero_at_origin() {
        assert_eq!(column_coherence_perturbation(0.0, 0), 0);
    }

    /// Perturbation at phase=π/2, col=0 is +1 (sin(π/2) * 0.5 = 0.5, rounds to 1).
    #[test]
    fn column_coherence_perturbation_peaks_at_plus_one() {
        let phase = std::f32::consts::FRAC_PI_2;
        // f32::round rounds half away from zero, so 0.5 → 1.
        assert_eq!(column_coherence_perturbation(phase, 0), 1);
    }

    /// Perturbation at phase=3π/2, col=0 is -1 (sin(3π/2) * 0.5 = -0.5, rounds to -1).
    #[test]
    fn column_coherence_perturbation_troughs_at_minus_one() {
        let phase = 3.0 * std::f32::consts::FRAC_PI_2;
        assert_eq!(column_coherence_perturbation(phase, 0), -1);
    }

    /// Spatial coherence: adjacent columns get similar perturbations.
    /// The spatial frequency is 0.05 rad/col, so the perturbation difference
    /// between col=N and col=N+1 is bounded by `sin(N+1) - sin(N)` ≤ 0.05.
    /// After rounding, this means neighboring columns usually share the
    /// same perturbation, and never differ by more than 1.
    #[test]
    fn column_coherence_perturbation_spatially_smooth() {
        let phase = 0.7_f32; // arbitrary nonzero phase
        for col in 0..512u16 {
            let a = column_coherence_perturbation(phase, col);
            let b = column_coherence_perturbation(phase, col + 1);
            let diff = (a - b).abs();
            assert!(
                diff <= 1,
                "Adjacent cols {col} and {} differ by {diff} (phase={phase})",
                col + 1
            );
        }
    }

    /// Temporal coherence: at a fixed column, small phase changes produce
    /// small or zero perturbation changes. This is what makes the effect
    /// "shimmer" rather than strobe.
    #[test]
    fn column_coherence_perturbation_temporally_smooth() {
        let col = 42u16;
        let mut prev = column_coherence_perturbation(0.0, col);
        // Advance phase by 0.1 rad per step (slow temporal freq).
        for step in 1..100 {
            let phase = step as f32 * 0.1;
            let curr = column_coherence_perturbation(phase, col);
            let diff = (curr - prev).abs();
            // 0.1 rad phase change → at most sin(0.1) ≈ 0.1 amplitude change
            // → rounding can flip by at most 1.
            assert!(
                diff <= 1,
                "Temporal step {step} (phase={phase}) changed perturbation by {diff}"
            );
            prev = curr;
        }
    }

    // ── Phase 3-E: subpixel hue jitter ─────────────────────────────────────

    /// cell_hash is deterministic: same input → same output.
    #[test]
    fn cell_hash_is_deterministic() {
        for line in 0..32u16 {
            for col in 0..32u16 {
                let a = cell_hash(line, col);
                let b = cell_hash(line, col);
                assert_eq!(a, b, "hash must be deterministic for ({line}, {col})");
            }
        }
    }

    /// cell_hash has low collision rate: distinct inputs rarely collide.
    /// Test across a 64×64 grid and verify no two distinct (line, col)
    /// pairs produce the same hash.
    #[test]
    fn cell_hash_low_collision_rate() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        let mut collisions = 0;
        for line in 0..64u16 {
            for col in 0..64u16 {
                let h = cell_hash(line, col);
                if !seen.insert(h) {
                    collisions += 1;
                }
            }
        }
        // 4096 distinct inputs into a u32 space should produce ~0 collisions.
        // Allow up to 2 for bad luck.
        assert!(
            collisions <= 2,
            "cell_hash produced {collisions} collisions across 4096 inputs (expected ≤ 2)"
        );
    }

    /// Jitter with amplitude 0 returns the input unchanged.
    #[test]
    fn subpixel_jitter_zero_amplitude_unchanged() {
        let c = Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        };
        assert_eq!(apply_subpixel_jitter(c, 0xDEADBEEF, 0), c);
    }

    /// Jitter with Color::Reset returns Reset unchanged.
    #[test]
    fn subpixel_jitter_reset_unchanged() {
        assert_eq!(
            apply_subpixel_jitter(Color::Reset, 0xDEADBEEF, 16),
            Color::Reset
        );
    }

    /// Jitter perturbs each channel by at most `amplitude` units.
    #[test]
    fn subpixel_jitter_bounded_by_amplitude() {
        let c = Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        };
        let amp: u8 = 8;
        // Sample many hashes to cover the offset space.
        for line in 0..32u16 {
            for col in 0..32u16 {
                let h = cell_hash(line, col);
                let result = apply_subpixel_jitter(c, h, amp);
                let Color::Rgb { r, g, b } = result else {
                    panic!("expected Rgb");
                };
                let dr = (i32::from(r) - 128).abs();
                let dg = (i32::from(g) - 128).abs();
                let db = (i32::from(b) - 128).abs();
                assert!(
                    dr <= i32::from(amp),
                    "r delta {dr} exceeds amp {amp} (line={line}, col={col}, h={h:#x})"
                );
                assert!(
                    dg <= i32::from(amp),
                    "g delta {dg} exceeds amp {amp} (line={line}, col={col}, h={h:#x})"
                );
                assert!(
                    db <= i32::from(amp),
                    "b delta {db} exceeds amp {amp} (line={line}, col={col}, h={h:#x})"
                );
            }
        }
    }

    /// Jitter clamps to [0, 255] — near-zero and near-255 channels don't
    /// wrap around.
    #[test]
    fn subpixel_jitter_clamps_to_valid_range() {
        let dark = Color::Rgb { r: 0, g: 0, b: 0 };
        let bright = Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        // Try many hashes to exercise both negative and positive offsets.
        for line in 0..16u16 {
            for col in 0..16u16 {
                let h = cell_hash(line, col);
                let r_dark = apply_subpixel_jitter(dark, h, 16);
                let r_bright = apply_subpixel_jitter(bright, h, 16);
                let Color::Rgb { r, g, b } = r_dark else {
                    panic!("expected Rgb");
                };
                assert!(r <= 16, "dark r {r} should be ≤ 16 after +amp jitter");
                assert!(g <= 16, "dark g {g} should be ≤ 16 after +amp jitter");
                assert!(b <= 16, "dark b {b} should be ≤ 16 after +amp jitter");
                let Color::Rgb { r, g, b } = r_bright else {
                    panic!("expected Rgb");
                };
                assert!(
                    r >= 255 - 16,
                    "bright r {r} should be ≥ {} after -amp jitter",
                    255 - 16
                );
                assert!(
                    g >= 255 - 16,
                    "bright g {g} should be ≥ {} after -amp jitter",
                    255 - 16
                );
                assert!(
                    b >= 255 - 16,
                    "bright b {b} should be ≥ {} after -amp jitter",
                    255 - 16
                );
            }
        }
    }

    /// Jitter is deterministic: same (color, hash, amp) → same result.
    #[test]
    fn subpixel_jitter_deterministic() {
        let c = Color::Rgb {
            r: 100,
            g: 50,
            b: 200,
        };
        let h = 0x12345678u32;
        let a = apply_subpixel_jitter(c, h, 8);
        let b = apply_subpixel_jitter(c, h, 8);
        assert_eq!(a, b);
    }

    /// Different hashes produce different jitter (high probability).
    /// Verify by sampling many hashes and counting distinct results.
    #[test]
    fn subpixel_jitter_varies_with_hash() {
        let c = Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        };
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for h in 0..256u32 {
            seen.insert(apply_subpixel_jitter(c, h, 8));
        }
        // 256 distinct hashes into a 17^3 ≈ 4913 space should produce
        // many distinct results. Allow at least 50 (very conservative).
        assert!(
            seen.len() >= 50,
            "jitter produced only {} distinct results across 256 hashes (expected ≥ 50)",
            seen.len()
        );
    }

    // ── Phase 3-F: luminance-remap for short droplets ─────────────────────

    /// Helper: build a minimal ShaderCtx for testing resolve_cell_color.
    /// Caller supplies the `palette_slices` array (so it outlives the
    /// ShaderCtx borrow) and the color_map slice. color_map is initialized
    /// to a constant value in the tests so we can detect when the remap
    /// overrides it.
    fn make_test_shader<'a>(
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
            column_coherence_phase: None,
            subpixel_jitter_amplitude: None,
            atmospheric: None,
            hue_drift: None,
        }
    }

    /// Build a `MAX_PALETTE_SLOTS`-sized palette_slices array with slot 0
    /// pointing to the given palette and all other slots empty. Returned
    /// by value so callers can bind it to a local with the right lifetime.
    fn slot_array(palette: &[Color]) -> [&[Color]; MAX_PALETTE_SLOTS] {
        let mut arr: [&[Color]; MAX_PALETTE_SLOTS] = [&[]; MAX_PALETTE_SLOTS];
        arr[0] = palette;
        arr
    }

    /// Short droplet (length=4) Middle cells get a position-based ramp
    /// spanning the full palette range, not the random color_map value.
    ///
    /// Setup: 5-stop palette (last=4), length=4, color_map all set to 1
    /// (would normally give every Middle cell color_idx=1).
    /// Expectation: the two Middle cells (dist_from_head=1 and 2) get
    /// remapped to color_idx=4 and 0 respectively (t=0 → last, t=1 → 0).
    #[test]
    fn short_droplet_middle_cells_get_remapped() {
        let palette: Vec<Color> = (0..5)
            .map(|i| Color::Rgb {
                r: i as u8 * 50,
                g: i as u8 * 50,
                b: i as u8 * 50,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![1u8; 50 * 100]; // all cells → color_idx 1
        let color_map: &[u8] = &color_map;
        let slots = slot_array(palette);
        let shader = make_test_shader(&slots, color_map, false);

        // length=4, head_put_line=20. Middle cells are at line 19 and 18
        // (dist_from_head = 1 and 2). denom = length-3 = 1.
        // dist=1 → t = 0 → color_idx = last = 4
        // dist=2 → t = 1 → color_idx = 0
        let (fg1, _) = resolve_cell_color(
            &shader,
            0,
            19, // line (head_put_line - 1)
            5,  // col
            'x',
            CharLoc::Middle,
            20, // head_put_line
            4,  // length
        );
        let (fg2, _) = resolve_cell_color(
            &shader,
            0,
            18, // line (head_put_line - 2)
            5,
            'x',
            CharLoc::Middle,
            20,
            4,
        );
        // fg1 should be palette[4] (brightest), fg2 should be palette[0] (darkest)
        assert_eq!(
            fg1,
            Some(palette[4]),
            "head-adjacent Middle cell should be brightest"
        );
        assert_eq!(
            fg2,
            Some(palette[0]),
            "tail-adjacent Middle cell should be darkest"
        );
    }

    /// Long droplet (length > threshold=8) Middle cells keep the color_map
    /// value — remap is not applied.
    #[test]
    fn long_droplet_middle_cells_unchanged() {
        let palette: Vec<Color> = (0..5)
            .map(|i| Color::Rgb {
                r: i as u8 * 50,
                g: i as u8 * 50,
                b: i as u8 * 50,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![2u8; 50 * 100]; // all cells → color_idx 2
        let color_map: &[u8] = &color_map;
        let slots = slot_array(palette);
        let shader = make_test_shader(&slots, color_map, false);

        // length=9 (> threshold of 8). Middle cell at line 19 (dist=1).
        // Remap NOT applied → color_idx stays at color_map value = 2.
        let (fg, _) = resolve_cell_color(
            &shader,
            0,
            19,
            5,
            'x',
            CharLoc::Middle,
            20,
            9, // length > threshold
        );
        assert_eq!(
            fg,
            Some(palette[2]),
            "long droplet Middle cell should use color_map value"
        );
    }

    /// Threshold boundary: length=8 (exactly the threshold) → remap applies.
    #[test]
    fn threshold_boundary_length_8_remapped() {
        let palette: Vec<Color> = (0..5)
            .map(|i| Color::Rgb {
                r: i as u8 * 50,
                g: i as u8 * 50,
                b: i as u8 * 50,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![1u8; 50 * 100];
        let color_map: &[u8] = &color_map;
        let slots = slot_array(palette);
        let shader = make_test_shader(&slots, color_map, false);

        // length=8 (= threshold). denom = 5. dist=1 → t=0 → color_idx = 4 (last).
        let (fg, _) = resolve_cell_color(&shader, 0, 19, 5, 'x', CharLoc::Middle, 20, 8);
        assert_eq!(
            fg,
            Some(palette[4]),
            "length=8 should still be remapped (≤ threshold)"
        );
    }

    /// shading_distance=true disables the remap even for short droplets.
    /// The shading_distance path has its own length-aware exponential decay.
    #[test]
    fn shading_distance_disables_remap() {
        let palette: Vec<Color> = (0..5)
            .map(|i| Color::Rgb {
                r: i as u8 * 50,
                g: i as u8 * 50,
                b: i as u8 * 50,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![1u8; 50 * 100];
        let color_map: &[u8] = &color_map;
        let slots = slot_array(palette);
        let shader = make_test_shader(&slots, color_map, true);

        // length=4 with shading_distance=true. Remap NOT applied —
        // shading_distance path overrides color_idx with exponential decay.
        // Just verify it doesn't panic and returns some color.
        let (fg, _) = resolve_cell_color(&shader, 0, 19, 5, 'x', CharLoc::Middle, 20, 4);
        assert!(fg.is_some(), "shading_distance path must return a color");
    }

    /// Head and Tail are unaffected by the remap — only Middle cells change.
    #[test]
    fn head_and_tail_unaffected_by_remap() {
        let palette: Vec<Color> = (0..5)
            .map(|i| Color::Rgb {
                r: i as u8 * 50,
                g: i as u8 * 50,
                b: i as u8 * 50,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![1u8; 50 * 100];
        let color_map: &[u8] = &color_map;
        let slots = slot_array(palette);
        let shader = make_test_shader(&slots, color_map, false);

        // length=4 (short). Head should be palette[4] (last). Tail should be palette[0].
        let (fg_head, bold_head) = resolve_cell_color(
            &shader,
            0,
            20, // head line
            5,
            'x',
            CharLoc::Head,
            20,
            4,
        );
        let (fg_tail, bold_tail) = resolve_cell_color(
            &shader,
            0,
            17, // tail line (head - 3)
            5,
            'x',
            CharLoc::Tail,
            20,
            4,
        );
        assert_eq!(fg_head, Some(palette[4]));
        assert!(bold_head, "Head should be bold");
        assert_eq!(fg_tail, Some(palette[0]));
        assert!(!bold_tail, "Tail should not be bold");
    }

    /// Short droplet with length=4 produces a strict head→tail gradient:
    /// Head=last, Middle1=last, Middle2=0, Tail=0. The two Middle cells
    /// are visually distinct, breaking the "flat short droplet" look.
    #[test]
    fn short_droplet_produces_visible_gradient() {
        let palette: Vec<Color> = (0..8)
            .map(|i| Color::Rgb {
                r: i as u8 * 30,
                g: i as u8 * 30,
                b: i as u8 * 30,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![3u8; 50 * 100]; // uniform "flat" baseline
        let color_map: &[u8] = &color_map;
        let slots = slot_array(palette);
        let shader = make_test_shader(&slots, color_map, false);

        // length=4, 8-stop palette (last=7). denom = 1.
        // Middle1 (dist=1): t=0 → color_idx = 7 (last)
        // Middle2 (dist=2): t=1 → color_idx = 0
        let (fg_m1, _) = resolve_cell_color(&shader, 0, 19, 5, 'x', CharLoc::Middle, 20, 4);
        let (fg_m2, _) = resolve_cell_color(&shader, 0, 18, 5, 'x', CharLoc::Middle, 20, 4);
        // The two Middle cells must differ — that's the whole point of 3-F.
        assert_ne!(
            fg_m1, fg_m2,
            "short droplet Middle cells must differ after remap (was uniform before)"
        );
        // And specifically: m1 brighter than m2 (head-side brighter than tail-side).
        let Color::Rgb { r: r1, .. } = fg_m1.unwrap() else {
            panic!("expected Rgb");
        };
        let Color::Rgb { r: r2, .. } = fg_m2.unwrap() else {
            panic!("expected Rgb");
        };
        assert!(
            r1 > r2,
            "head-side Middle ({r1}) should be brighter than tail-side ({r2})"
        );
    }

    // ── Phase 3-H: global hue drift ───────────────────────────────────────

    /// hue_drift_offset maps drift values to integer offsets:
    ///   0 → 0, π/2 → +1, -π/2 → -1, π → +2, -π → -2.
    #[test]
    fn hue_drift_offset_known_values() {
        assert_eq!(hue_drift_offset(0.0), 0);
        assert_eq!(hue_drift_offset(std::f32::consts::PI), 2);
        assert_eq!(hue_drift_offset(-std::f32::consts::PI), -2);
        assert_eq!(hue_drift_offset(std::f32::consts::FRAC_PI_2), 1);
        assert_eq!(hue_drift_offset(-std::f32::consts::FRAC_PI_2), -1);
    }

    /// Small drifts (|drift| < π/4) round to 0 — the common production
    /// case because COLOR_HUE_DRIFT_RATE is small (0.015 rad/tick).
    #[test]
    fn hue_drift_offset_small_drifts_round_to_zero() {
        assert_eq!(hue_drift_offset(std::f32::consts::FRAC_PI_8), 0);
        assert_eq!(hue_drift_offset(-std::f32::consts::FRAC_PI_8), 0);
        assert_eq!(hue_drift_offset(0.78), 0);
        assert_eq!(hue_drift_offset(-0.78), 0);
    }

    /// Offset is bounded to {-2, -1, 0, +1, +2} across [-π, π] and is
    /// monotonic non-decreasing + odd (offset(-x) = -offset(x)).
    #[test]
    fn hue_drift_offset_bounded_monotonic_odd() {
        let steps = 1000;
        let mut prev = hue_drift_offset(-std::f32::consts::PI);
        for i in 0..=steps {
            let drift =
                -std::f32::consts::PI + 2.0 * std::f32::consts::PI * (i as f32) / (steps as f32);
            let offset = hue_drift_offset(drift);
            let neg_offset = hue_drift_offset(-drift);
            assert!(
                (-2..=2).contains(&offset),
                "drift {drift} → {offset} out of [-2,2]"
            );
            assert!(
                offset >= prev,
                "non-monotonic at drift {drift}: {offset} < {prev}"
            );
            assert_eq!(
                offset, -neg_offset,
                "not odd: offset({drift})={offset} != -offset(-drift)"
            );
            prev = offset;
        }
    }

    /// Integration: resolve_cell_color with hue_drift applies the offset
    /// to Middle cells. Verify a Middle cell's color shifts when hue_drift
    /// is non-zero (vs. None which leaves it unchanged).
    #[test]
    fn hue_drift_shifts_middle_color() {
        let palette: Vec<Color> = (0..8)
            .map(|i| Color::Rgb {
                r: i as u8 * 30,
                g: i as u8 * 30,
                b: i as u8 * 30,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![3u8; 50 * 100];
        let color_map: &[u8] = &color_map;

        let slots = slot_array(palette);
        let shader_none = make_test_shader(&slots, color_map, false);
        let (fg_none, _) = resolve_cell_color(&shader_none, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
        assert_eq!(fg_none, Some(palette[3]));

        let mut shader_drift = make_test_shader(&slots, color_map, false);
        shader_drift.hue_drift = Some(std::f32::consts::PI);
        let (fg_drift, _) =
            resolve_cell_color(&shader_drift, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
        assert_eq!(fg_drift, Some(palette[5]), "hue_drift=π should shift 3 → 5");
    }

    /// hue_drift does NOT affect Head or Tail — those are pinned.
    #[test]
    fn hue_drift_does_not_affect_head_or_tail() {
        let palette: Vec<Color> = (0..8)
            .map(|i| Color::Rgb {
                r: i as u8 * 30,
                g: i as u8 * 30,
                b: i as u8 * 30,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![3u8; 50 * 100];
        let color_map: &[u8] = &color_map;

        let slots = slot_array(palette);
        let mut shader = make_test_shader(&slots, color_map, false);
        shader.hue_drift = Some(std::f32::consts::PI);

        let (fg_head, _) = resolve_cell_color(&shader, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
        assert_eq!(fg_head, Some(palette[7]));

        let (fg_tail, _) = resolve_cell_color(&shader, 0, 9, 5, 'x', CharLoc::Tail, 20, 12);
        assert_eq!(fg_tail, Some(palette[0]));
    }

    /// hue_drift is skipped under shading_distance — that path has its own
    /// length-aware gradient and stacking a hue shift would muddy the signal.
    #[test]
    fn hue_drift_skipped_under_shading_distance() {
        let palette: Vec<Color> = (0..8)
            .map(|i| Color::Rgb {
                r: i as u8 * 30,
                g: i as u8 * 30,
                b: i as u8 * 30,
            })
            .collect();
        let palette: &[Color] = &palette;
        let color_map: Vec<u8> = vec![3u8; 50 * 100];
        let color_map: &[u8] = &color_map;

        let slots = slot_array(palette);
        let mut shader_off = make_test_shader(&slots, color_map, true);
        shader_off.hue_drift = None;
        let mut shader_on = make_test_shader(&slots, color_map, true);
        shader_on.hue_drift = Some(std::f32::consts::PI);

        let (fg_off, _) = resolve_cell_color(&shader_off, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
        let (fg_on, _) = resolve_cell_color(&shader_on, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
        assert_eq!(
            fg_off, fg_on,
            "hue_drift must not affect shading_distance path"
        );
    }

    /// hue_drift clamps to valid palette range — offset that would push
    /// color_idx below 0 or above last is clamped.
    #[test]
    fn hue_drift_clamps_to_palette_range() {
        let palette: Vec<Color> = (0..3)
            .map(|i| Color::Rgb {
                r: i as u8 * 100,
                g: i as u8 * 100,
                b: i as u8 * 100,
            })
            .collect();
        let palette: &[Color] = &palette;

        // Lower bound: color_map=0, hue_drift=-π → offset -2, clamped to 0.
        let color_map_lo: Vec<u8> = vec![0u8; 50 * 100];
        let color_map_lo: &[u8] = &color_map_lo;
        let slots_lo = slot_array(palette);
        let mut shader_lo = make_test_shader(&slots_lo, color_map_lo, false);
        shader_lo.hue_drift = Some(-std::f32::consts::PI);
        let (fg_lo, _) = resolve_cell_color(&shader_lo, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
        assert_eq!(fg_lo, Some(palette[0]));

        // Upper bound: color_map=2, hue_drift=+π → offset +2, clamped to 2.
        let color_map_hi: Vec<u8> = vec![2u8; 50 * 100];
        let color_map_hi: &[u8] = &color_map_hi;
        let slots_hi = slot_array(palette);
        let mut shader_hi = make_test_shader(&slots_hi, color_map_hi, false);
        shader_hi.hue_drift = Some(std::f32::consts::PI);
        let (fg_hi, _) = resolve_cell_color(&shader_hi, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
        assert_eq!(fg_hi, Some(palette[2]));
    }
}
