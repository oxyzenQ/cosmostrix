// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! DrawCtx — read-only drawing context passed to Droplet::draw.
//!
//! Phase 2 (Chroma Dragon): the cell-color decision logic that lived here as
//! `DrawCtx::get_attr()` has been extracted to
//! `chroma::shaders::base::resolve_cell_color()`. `DrawCtx::get_attr()` is
//! now a thin wrapper that builds a `ShaderCtx` borrow view from its own
//! fields and delegates. `CharLoc` and `TRAIL_EXP_LUT` moved with it and are
//! re-exported from this module so existing `crate::cloud::render::CharLoc`
//! references continue to resolve.

use bitvec::prelude::BitSlice;
use crossterm::style::Color;

use crate::chroma::shaders::base::{color_uses_previous_palette, resolve_cell_color, ShaderCtx};
use crate::constants::*;
use crate::runtime::BoldMode;

// Re-export the moved types so every existing `use crate::cloud::render::CharLoc`
// or `crate::cloud::CharLoc` reference continues to resolve unchanged. The
// `pub use` also brings `CharLoc` into local scope for use in `get_attr`'s
// signature below.
pub(crate) use crate::chroma::shaders::base::CharLoc;

/// Pre-computed view of an active mouse-click flash wave (v30 fix).
///
/// Built once per frame in `cloud::rain::rain_at` and borrowed by `DrawCtx`
/// for the duration of the draw call. The renderer iterates this slice and
/// sums per-wave factor contributions onto each cell's color.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FlashWaveCtx {
    /// Click column (cell-space).
    pub col: u16,
    /// Click line (cell-space).
    pub line: u16,
    /// Seconds since this wave's birth. Always `< MOUSE_FLASH_DURATION_SECS`
    /// (expired waves are filtered out before this struct is built).
    /// Kept for Debug diagnostics; the hot path uses the precomputed fields below.
    #[allow(dead_code)]
    pub elapsed: f32,
    // ── v30 optimize (MOUSE_EFFECTS_AUDIT.md Quick Win #2): precomputed
    // wave-invariant quantities. These are pure functions of `elapsed` and
    // the wave constants, so computing them once per wave (in rain.rs
    // construction) instead of once per cell × per wave eliminates ~48K
    // sqrts/sec + ~290K ops/sec at 60 FPS.
    /// Primary ring radius = elapsed * MOUSE_FLASH_SPEED.
    pub primary_radius: f32,
    /// Secondary ring radius = elapsed * speed * secondary_speed_frac.
    pub secondary_radius: f32,
    /// Quadratic fade = raw_fade * raw_fade.sqrt() (includes one sqrt).
    pub fade: f32,
    /// Squared max reach for early-out: (max(primary, secondary) + ring_width)².
    /// Cells with dist_sq > max_reach_sq skip the wave entirely (no sqrt needed).
    pub max_reach_sq: f32,
}

/// Runtime state of a mouse-click flash wave slot (v30 fix: bounded pool).
///
/// Each click activates a slot; the wave expands as a dual-ring water-drop
/// ripple for `MOUSE_FLASH_DURATION_SECS`, then `active` flips to false.
/// Pool size is `MOUSE_FLASH_POOL_SIZE`. Stored as a fixed array in `Cloud`.
#[derive(Clone, Copy, Debug)]
pub(super) struct FlashWave {
    pub active: bool,
    pub col: u16,
    pub line: u16,
    pub birth: std::time::Instant,
}

/// Read-only drawing context passed to `Droplet::draw` to avoid borrowing
/// the entire `Cloud` (which would conflict with the mutable droplet loop).
pub(crate) struct DrawCtx<'a> {
    pub lines: u16,
    /// Total column count of the viewport. Used by per-cell effects that
    /// need horizontal positioning (e.g. cinematic radial vignette).
    pub cols: u16,
    pub shading_distance: bool,
    pub bg: Option<Color>,

    pub color_mode: crate::runtime::ColorMode,
    pub bold_mode: BoldMode,
    pub glitchy: bool,

    /// Cached `is_bright(now)` snapshot computed once per DrawCtx construction.
    /// Avoids per-cell Instant::saturating_duration_since + nanos conversion
    /// in get_attr's glitch branch (called 100-300×/frame when glitchy).
    pub glitch_bright: bool,
    /// Cached `is_dim(now)` snapshot computed once per DrawCtx construction.
    pub glitch_dim: bool,

    /// Per-slot palette color arrays for generation-based rendering.
    /// Index by droplet's `palette_slot` to resolve its birth palette.
    pub palette_slices: [&'a [Color]; MAX_PALETTE_SLOTS],

    /// Which palette slot is the currently active (latest) one.
    /// Used for transition glow effects on new-generation streams.
    pub active_palette_slot: u8,

    /// Whether a palette transition is currently in progress.
    /// When true, new-generation streams get enhanced visual effects.
    pub transitioning: bool,

    pub color_map: &'a [u8],
    pub glitch_map: &'a BitSlice,
    pub char_pool: &'a [char],
    pub previous_char_pool: &'a [char],
    /// Precomputed viewport edge fade per line. Indexed by `line`.
    /// Built once per terminal resize in Cloud::reset(); DrawCtx borrows it.
    /// Replaces per-cell `viewport_edge_fade(line, lines)` float division.
    pub edge_fade_lut: &'a [f32],
    pub charset_wave_line: Option<f32>,

    /// Color transition wave line: during a palette transition, rows above
    /// this value use the new (active) palette; rows below use their birth
    /// palette. Sweeps from 0 to lines+1 over COLOR_TRANSITION_DURATION_MS,
    /// creating a top-to-bottom wave that matches the charset transition.
    pub color_wave_line: Option<f32>,

    /// Mouse cursor column (u16::MAX if no mouse).
    pub mouse_col: u16,
    /// Mouse cursor line (u16::MAX if no mouse).
    pub mouse_line: u16,
    /// Active mouse-click flash waves (v30 fix: was single slot, now bounded pool).
    ///
    /// Precomputed once per frame in `rain_at`. Each entry has `elapsed <
    /// MOUSE_FLASH_DURATION_SECS`. Empty slice = no active waves. The slice
    /// is borrowed from a stack-local `SmallVec` in `rain_at` that outlives
    /// the DrawCtx.
    pub flash_waves: &'a [FlashWaveCtx],
    /// Cached result of pool_is_binary check, computed once per DrawCtx
    /// construction to avoid per-cell iteration of the char pool.
    pub pool_is_binary: bool,

    /// Phase 3-G (Chroma Dragon Innovation G): precomputed atmospheric
    /// factors for this frame. `None` disables shader-level atmospheric
    /// (cells render with raw palette colors). When `Some`, the shader
    /// applies atmospheric to each cell's resolved color BEFORE returning.
    /// (v30.1: the old post-hoc `apply_climate_frame_effects` pass was
    /// deleted; climate is shader-only now.)
    pub atmospheric: Option<crate::chroma::post::climate::ClimateCtx>,

    /// Phase 3-H (Chroma Dragon Innovation H): global hue drift.
    ///
    /// `Some(offset)` applies a slow global palette-stop offset to all
    /// Middle cells, derived from `ColorEcosystem.hue_drift`. Pre-Phase-3-H
    /// this field was dead code — updated every tick but never read by
    /// the render path. Phase 3-H activates it.
    ///
    /// Phase C: carries the PRE-COMPUTED `i32` offset (not raw f32
    /// radians). The conversion runs once per frame in `cloud/rain.rs`
    /// via `hue_drift_offset(drift)`, so the per-cell shader hot path
    /// is a single integer add.
    ///
    /// `None` disables (matches pre-Phase-3-H behavior).
    pub hue_drift_offset: Option<i32>,

    /// Phase 4-A (Chroma Dragon Innovation C — Dragon Awakening): temporal
    /// column hue coherence LUT.
    ///
    /// `Some(lut)` enables a slow per-column hue drift: the shader reads
    /// `lut[col]` (an i32 in `{-1, 0, +1}`) and adds it to the Middle
    /// cell's `color_idx`. Neighboring columns get similar perturbations
    /// (low spatial frequency), and the perturbation oscillates slowly
    /// over time (low temporal frequency) — so watching a single column,
    /// the colors shimmer smoothly through adjacent palette stops
    /// instead of jumping per-cell.
    ///
    /// Phase D (hot-path): the LUT is built once per frame in `rain.rs`
    /// via `column_coherence_perturbation(phase, col)` for each col in
    /// `0..cols`. Was: per-cell `sinf + round + cast` (~65-130M
    /// cycles/sec at 60 FPS on a 200-col viewport). Now: a single
    /// indexed i32 read.
    ///
    /// `None` disables (matches pre-Phase-4-A dormant behavior — kept
    /// for tests that assert the shader's no-op path).
    pub column_coherence_lut: Option<&'a [i32]>,

    /// Phase 4-B (Chroma Dragon Innovation E — Dragon Awakening): subpixel
    /// hue jitter amplitude.
    ///
    /// `Some(amp)` applies a per-cell RGB perturbation of `±amp` units
    /// per channel, driven by a deterministic FNV-1a hash of `(line,
    /// col)`. The effect is fine film-grain texture — breaks up the
    /// uniformity of large same-color regions without changing the
    /// palette decision. "Subpixel" means the jitter is smaller than
    /// one palette step: it modifies the returned RGB directly, not
    /// the `color_idx`, so the head→body→tail hierarchy stays intact.
    ///
    /// Phase 3-E landed the shader logic + tests but left this field
    /// hard-coded to `None` in the `ShaderCtx` builder. Phase 4-B wires
    /// a conservative amplitude (`SUBPIXEL_JITTER_AMPLITUDE = 3`)
    /// through `DrawCtx` → `ShaderCtx` so the effect is always-on.
    /// The hash is deterministic, so the same cell always gets the
    /// same jitter — no strobing across frames.
    ///
    /// `None` disables (matches pre-Phase-4-B dormant behavior — kept
    /// for tests that assert the shader's no-op path).
    pub subpixel_jitter_amplitude: Option<u8>,

    /// Phase 4-D (Chroma Dragon Innovation D — Dragon Awakening): head halo
    /// blend factor.
    ///
    /// `Some(factor)` blends the resolved Head cell color toward the scene
    /// background (`bg`) by `factor`, softening the hard bright head pixel
    /// against the dark background. Phase 3-D landed `blend_toward_bg` in
    /// `chroma::palette` but it had zero production callers. Phase 4-D wires
    /// it into the shader's Head branch so the halo is always-on.
    ///
    /// `None` disables (matches pre-Phase-4-D dormant behavior — kept for
    /// tests that assert the shader's no-op path).
    pub head_halo_factor: Option<f32>,

    /// Phase 5 (Chroma Dragon — perceptual L smoothing at palette
    /// transition wave).
    ///
    /// `Some(table)` is built once per frame in `rain.rs` when
    /// `transition_start.is_some()` AND `color_wave_line.is_some()`. The
    /// table pre-computes the OKLab L for each stop index in both the
    /// old and new palettes, plus the current wave line position and
    /// smoothing window.
    ///
    /// The shader's `apply_l_smoothing` blends each cell's OKLab L
    /// channel toward the opposite palette's L within ±`window` lines
    /// of the wave, eliminating the hard brightness step at the wave
    /// line during palette transitions.
    ///
    /// `None` disables (matches pre-Phase-5 behavior — palette
    /// transitions show a hard brightness step at the wave line).
    pub transition_l_table: Option<&'a crate::chroma::shaders::transition::TransitionLTable>,
}

impl DrawCtx<'_> {
    #[inline]
    pub(crate) fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        let idx = col as usize * self.lines as usize + line as usize;
        // Cosmic Dragon egg #17: bounds-check + direct indexing.
        // BitSlice implements Index<usize> returning bool.
        idx < self.glitch_map.len() && self.glitch_map[idx]
    }

    /// Lookup precomputed viewport edge fade for a given line.
    /// Falls back to 1.0 (no fade) if the LUT doesn't cover the line index,
    /// which is safe — the LUT is rebuilt on every terminal resize.
    #[inline]
    pub(crate) fn edge_fade(&self, line: u16) -> f32 {
        // Cosmic Dragon egg #13: direct indexing — edge_fade_lut is always sized to
        // `lines` in spawn.rs, and callers pass line < lines (from droplet
        // iteration). The .get().copied().unwrap_or(1.0) was defensive but
        // adds Option alloc + unwrap_or branching.
        let idx = line as usize;
        if idx < self.edge_fade_lut.len() {
            self.edge_fade_lut[idx]
        } else {
            1.0
        }
    }

    #[inline]
    pub(crate) fn get_char(&self, line: u16, col: u16, char_pool_idx: u16) -> char {
        let pool = if self.charset_uses_previous_pool(line, col) {
            self.previous_char_pool
        } else {
            self.char_pool
        };
        // OPTIMIZED: use bitmask instead of modulo (CHAR_POOL_SIZE is power of 2)
        let idx = ((char_pool_idx as usize) + (line as usize)) & (CHAR_POOL_SIZE - 1);
        // Cosmic Dragon egg #11 (revised): char_pool is always CHAR_POOL_SIZE (2048),
        // but previous_char_pool may be smaller during transition. Use .get()
        // for safety when pool is smaller than CHAR_POOL_SIZE.
        if pool.len() >= CHAR_POOL_SIZE {
            pool[idx]
        } else {
            pool.get(idx).copied().unwrap_or('0')
        }
    }

    #[inline]
    pub(crate) fn charset_transitioning(&self) -> bool {
        self.charset_wave_line.is_some()
    }

    #[inline]
    fn charset_uses_previous_pool(&self, line: u16, col: u16) -> bool {
        let Some(wave_line) = self.charset_wave_line else {
            return false;
        };
        if self.previous_char_pool.is_empty() {
            return false;
        }

        let jitter =
            (((line as u32).wrapping_mul(17) ^ (col as u32).wrapping_mul(31)) % 3) as f32 * 0.18;
        (line as f32) > wave_line + jitter
    }

    /// During a color transition, returns whether a cell at (line, col) should
    /// use its birth (previous) palette rather than the new (active) palette.
    /// Rows below the wave line use the old palette; rows above use the new.
    /// This creates a top-to-bottom cascade matching the charset transition.
    ///
    /// Delegates to the chroma shader's free function so the renderer and
    /// `resolve_cell_color()` share one source of truth.
    #[inline]
    pub(crate) fn color_uses_previous_palette(
        &self,
        palette_slot: u8,
        line: u16,
        col: u16,
    ) -> bool {
        color_uses_previous_palette(
            self.color_wave_line,
            self.active_palette_slot,
            palette_slot,
            line,
            col,
        )
    }

    /// Thin wrapper around `chroma::shaders::base::resolve_cell_color()`.
    ///
    /// Builds a `ShaderCtx` borrow view from the relevant DrawCtx fields and
    /// delegates. The shader body is identical to the pre-Phase-2 inlined
    /// body — `#[inline]` on both sides lets LLVM fold the chain at the call
    /// site, yielding equivalent codegen.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_attr(
        &self,
        palette_slot: u8,
        line: u16,
        col: u16,
        val: char,
        loc: CharLoc,
        head_put_line: u16,
        length: u16,
    ) -> (Option<Color>, bool) {
        let shader = ShaderCtx {
            palette_slices: &self.palette_slices,
            active_palette_slot: self.active_palette_slot,
            color_wave_line: self.color_wave_line,
            bold_mode: self.bold_mode,
            lines: self.lines,
            color_map: self.color_map,
            shading_distance: self.shading_distance,
            glitchy: self.glitchy,
            glitch_map: self.glitch_map,
            glitch_bright: self.glitch_bright,
            glitch_dim: self.glitch_dim,
            color_mode: self.color_mode,
            // Phase 4-A (Dragon Awakening) + Phase D (hot-path): column-
            // coherence hue drift is now wired through DrawCtx as a
            // precomputed LUT. rain.rs builds `column_coherence_lut[col]`
            // once per frame from the time phase (COLUMN_COHERENCE_FREQ
            // rad/s, ~60 s period). The shader hot path is a single indexed
            // i32 read (was: per-cell sinf + round + cast). None disables
            // (used by shader no-op tests).
            column_coherence_lut: self.column_coherence_lut,
            // Phase 4-B (Dragon Awakening): subpixel hue jitter is now wired
            // through DrawCtx. rain.rs sets a conservative amplitude
            // (SUBPIXEL_JITTER_AMPLITUDE = 3) for subtle film-grain texture.
            // None disables (used by shader no-op tests).
            subpixel_jitter_amplitude: self.subpixel_jitter_amplitude,
            // Phase 3-G: atmospheric post-processing. When DrawCtx.atmospheric
            // is Some, the shader applies the frame's atmospheric factors to
            // each cell's resolved color before returning. When None, the
            // shader skips atmospheric and the post-hoc pass runs instead.
            atmospheric: self.atmospheric.as_ref(),
            // Phase 3-H + Phase C: global hue drift. Pre-computed once
            // per frame in rain.rs (via hue_drift_offset fn) — the shader
            // hot path is now a single integer add. None disables
            // (pre-Phase-3-H behavior — drift accumulates but never
            // affects rendering).
            hue_drift_offset: self.hue_drift_offset,
            // Phase 4-D (Dragon Awakening): head halo via background blend.
            // rain.rs sets Some(HEAD_HALO_FACTOR) and passes self.bg so the
            // shader can blend the Head color toward the scene background.
            // None disables (used by shader no-op tests). When Some, the
            // halo still auto-no-ops if bg is None or Color::Reset.
            head_halo_factor: self.head_halo_factor,
            bg: self.bg,
            // Phase 5: perceptual L smoothing at palette transition wave.
            // rain.rs builds the table when transition_start.is_some() AND
            // color_wave_line.is_some(). None disables (most frames — no
            // transition active). The shader's apply_l_smoothing early-
            // returns cheaply when this is None.
            transition_l_table: self.transition_l_table,
        };
        resolve_cell_color(
            &shader,
            palette_slot,
            line,
            col,
            val,
            loc,
            head_put_line,
            length,
        )
    }
}
