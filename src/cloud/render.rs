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
pub use crate::chroma::shaders::base::CharLoc;

/// Read-only drawing context passed to `Droplet::draw` to avoid borrowing
/// the entire `Cloud` (which would conflict with the mutable droplet loop).
pub struct DrawCtx<'a> {
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
    /// Flash effect click column.
    pub flash_col: u16,
    /// Flash effect click line.
    pub flash_line: u16,
    /// Cached flash elapsed seconds (None if no active flash or expired).
    /// Precomputed once per frame to avoid per-cell `Instant::elapsed()` syscalls.
    pub flash_elapsed: Option<f32>,
    /// Cached result of pool_is_binary check, computed once per DrawCtx
    /// construction to avoid per-cell iteration of the char pool.
    pub pool_is_binary: bool,

    /// Phase 3-G (Chroma Dragon Innovation G): precomputed atmospheric
    /// factors for this frame. `None` disables shader-level atmospheric
    /// (matches pre-Phase-3-G behavior — `apply_atmospheric_frame_effects`
    /// runs as a post-hoc pass instead). When `Some`, the shader applies
    /// atmospheric to each cell's resolved color BEFORE returning, and
    /// `apply_atmospheric_frame_effects` early-returns to avoid
    /// double-application.
    pub atmospheric: Option<crate::chroma::post::atmosphere::AtmosphericCtx>,
}

impl DrawCtx<'_> {
    #[inline]
    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
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
    pub fn edge_fade(&self, line: u16) -> f32 {
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
    pub fn get_char(&self, line: u16, col: u16, char_pool_idx: u16) -> char {
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
    pub fn charset_transitioning(&self) -> bool {
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
    pub fn color_uses_previous_palette(&self, palette_slot: u8, line: u16, col: u16) -> bool {
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
    pub fn get_attr(
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
            // Phase 3-C: column-coherence hue drift is implemented in the
            // shader but not yet wired through DrawCtx. Hard-coded None
            // keeps production rendering identical to pre-Phase-3-C behavior.
            // Plumbing the time phase through DrawCtx + rain.rs is a future
            // commit (the shader logic and tests land now so the innovation
            // is reviewable in isolation).
            column_coherence_phase: None,
            // Phase 3-E: subpixel hue jitter is implemented in the shader
            // but not yet wired through DrawCtx. Hard-coded None keeps
            // production rendering identical to pre-Phase-3-E behavior.
            subpixel_jitter_amplitude: None,
            // Phase 3-G: atmospheric post-processing. When DrawCtx.atmospheric
            // is Some, the shader applies the frame's atmospheric factors to
            // each cell's resolved color before returning. When None, the
            // shader skips atmospheric and the post-hoc pass runs instead.
            atmospheric: self.atmospheric.as_ref(),
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
