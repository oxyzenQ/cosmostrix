// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Post-processing effects applied after the droplet draw pass.
//!
//! CRT vignette (top/bottom edge dim). The method is a Cloud method
//! split from rain.rs to keep that file under the 800-LOC source cap,
//! in the same impl block via a sibling file.
//! night-moonlight-1: the quantum ripple particle renderer that shared
//! this file is removed (archived at
//! docs/archive/specs/QUANTUM_RIPPLE_MOUSE_CLICK_BURST.md).

use crossterm::style::Color;

use crate::constants::{CRT_VIGNETTE_HEIGHT, CRT_VIGNETTE_PERF_THRESHOLD};
use crate::frame::Frame;

use super::Cloud;

impl Cloud {
    /// Apply the cinematic CRT vignette: dim the top and bottom
    /// `CRT_VIGNETTE_HEIGHT` rows.
    ///
    /// Bug 2 fix: a subtle dimming at the screen edges creates a retro
    /// CRT-glow feel — the screen edges look slightly darker, drawing
    /// the eye toward the center where the rain is densest. The dim
    /// eases out via smoothstep so the inner boundary is imperceptible
    /// (no hard cutoff).
    ///
    /// The factor goes from `CRT_VIGNETTE_EDGE_FACTOR` (0.90,
    /// v50 alpha.2 quick-tune) at the extreme edge row to 1.0 (no dim) at
    /// row `CRT_VIGNETTE_HEIGHT` inward from the edge:
    ///
    ///   t = row_index / CRT_VIGNETTE_HEIGHT          (0 → 1)
    ///   smoothstep(t) = t * t * (3 - 2t)             (0 → 1, C1 continuous)
    ///   factor = EDGE + (1 - EDGE) * smoothstep(t)
    ///
    /// the smoothstep math is extracted into
    /// `crate::droplet::crt_vignette_factor` (single source of truth) so
    /// the SSOT `compounded_brightness` audit function and this render
    /// path agree on the exact curve. The precompute loop below calls
    /// the function 2*H times per frame (once per band row, both bands).
    ///
    /// Runs AFTER the droplet draw pass + rain shadow, but BEFORE
    /// phosphor decay. This ensures the CRT dim propagates into the
    /// phosphor afterglow — edge cells retain less energy when the
    /// cursor passes through them, preventing edge-pile-up artifacts.
    ///
    /// Cost: O(dirty_count) per frame — iterates only cells drawn this
    /// frame that fall inside the vignette bands. At 200×60 with 30%
    /// rain density, that's ~600 candidate cells/frame (filter to ~60 in
    /// the 10-row vignette band), vs the previous O(cols ×
    /// CRT_VIGNETTE_HEIGHT × 2) = 2000 cell reads/frame. Skipped entirely
    /// when `perf_pressure > CRT_VIGNETTE_PERF_THRESHOLD` to preserve
    /// rain throughput under sustained load.
    pub(crate) fn apply_crt_vignette(&mut self, frame: &mut Frame) {
        // Bail early if the screen is too short for the vignette to make
        // sense (would dim the entire screen).
        if self.lines < 2 * CRT_VIGNETTE_HEIGHT {
            return;
        }

        // PERF: skip the vignette entirely under sustained performance
        // pressure. The vignette is a cosmetic-only post-process — when
        // the renderer is struggling (slow frame rate, high dirty count),
        // dropping it preserves rain throughput. The threshold sits above
        // GLITCH_THRESHOLD (0.35) so the vignette survives a bit longer
        // than the glitch effect before being dropped.
        //
        // H2 (internal independent QA): also skip when aggressive_throttle is
        // active — the self-healer's AB-11 flag means sustained high CPU
        // pressure was detected. CRT vignette is non-essential visual work;
        // skipping it under aggressive throttle is consistent with the AB-11
        // design intent of shedding all non-essential visual work. Without
        // this, the vignette could still run when pressure fluctuates between
        // 0.5 and 0.6 while aggressive_throttle is active.
        if self.perf_pressure > CRT_VIGNETTE_PERF_THRESHOLD || self.aggressive_throttle {
            return;
        }

        let cols = self.cols;
        let lines = self.lines;
        let bg = self.palette.bg;

        // Precompute the factor for each vignette row once.
        // Index 0..CRT_VIGNETTE_HEIGHT → top band (row 0 = extreme edge).
        // Index CRT_VIGNETTE_HEIGHT..2*CRT_VIGNETTE_HEIGHT → bottom band
        // (row 0 = lines-1 = extreme edge).
        //
        // the smoothstep math now lives in the single-source-of-truth
        // `crate::droplet::crt_vignette_factor` function, extracted so the
        // SSOT `compounded_brightness` audit function and this render path
        // agree on the exact curve. Both bands share the same symmetric
        // factor sequence (top row v == bottom row v produce the same
        // factor), so we call the function twice per iteration and store
        // the results in their respective slots. Cost: 2*H calls per frame
        // (6 calls for H=3) — negligible vs the dirty-cell scan that
        // follows.
        let mut row_factors = [0.0f32; 2 * CRT_VIGNETTE_HEIGHT as usize];
        for v in 0..CRT_VIGNETTE_HEIGHT {
            let top_factor = crate::droplet::crt_vignette_factor(v, lines);
            let bottom_factor = crate::droplet::crt_vignette_factor(lines - 1 - v, lines);
            row_factors[v as usize] = top_factor;
            row_factors[(CRT_VIGNETTE_HEIGHT + v) as usize] = bottom_factor;
        }

        // Build the row → factor map for O(1) lookup during the dirty scan.
        // Top band: rows 0..CRT_VIGNETTE_HEIGHT.
        // Bottom band: rows (lines - CRT_VIGNETTE_HEIGHT)..lines.
        // Any row outside these two bands has factor 1.0 (no dim) and is
        // skipped by the `factor >= 1.0` check inside the dim helper.
        let top_end = CRT_VIGNETTE_HEIGHT;
        let bottom_start = lines.saturating_sub(CRT_VIGNETTE_HEIGHT);
        let frame_width = frame.width;

        // T1.1-real: use hoisted `crt_vignette_candidates` buffer (Cloud field)
        // instead of a per-frame SmallVec. The old SmallVec<[(u16,u16,f32); 32]>
        // spilled to heap at 33 elements and realloc'd at 65 — ~1 alloc + ~1
        // realloc per frame. The hoisted Vec::clear() preserves capacity, so
        // steady-state is zero alloc + zero realloc. Initial capacity 128
        // covers terminals up to ~200 cols at 8% dirty ratio.
        self.crt_vignette_candidates.clear();
        for &dirty_idx in frame.dirty_indices() {
            let col = (dirty_idx % frame_width as usize) as u16;
            let line = (dirty_idx / frame_width as usize) as u16;
            if col >= cols || line >= lines {
                continue;
            }
            let factor = if line < top_end {
                row_factors[line as usize]
            } else if line >= bottom_start {
                // Distance from the bottom edge: 0 (extreme) → H-1 (inner).
                let v = lines - 1 - line;
                row_factors[(CRT_VIGNETTE_HEIGHT + v) as usize]
            } else {
                continue;
            };
            if factor >= 1.0 {
                continue;
            }
            self.crt_vignette_candidates.push((col, line, factor));
        }

        for &(col, line, factor) in &self.crt_vignette_candidates {
            apply_crt_dim_cell(frame, col, line, factor, bg, self.color_pipeline);
        }
    }
}

/// Helper: dim a single cell by `factor` (0.0 = full black,
/// 1.0 = no dim). Cells without a foreground color (blank cells) are
/// skipped — the dim only applies to painted cells (droplet trail + head).
/// The background is preserved so the dim reads as a darkening of the
/// glyph, not a tint of the empty space.
///
/// This is the per-cell variant of the previous `apply_crt_dim_row` row
/// scan, called from the dirty-cell intersect loop in `apply_crt_vignette`.
/// Identical math, identical output — just narrowed to cells that were
/// actually drawn this frame.
///
/// Dim a single cell toward black by the given factor (0.0..=1.0).
/// Free function (not a method) — called from apply_crt_vignette's
/// dirty-cell intersect loop. Identical math to the inline path in
/// rain_at, just narrowed to cells that were actually drawn this frame.
fn apply_crt_dim_cell(
    frame: &mut Frame,
    col: u16,
    row: u16,
    factor: f32,
    bg: Option<Color>,
    pipeline: crate::runtime::ColorPipeline,
) {
    // Integer-friendly brightness scale: factor * 256, rounded.
    // factor=0.8 → 205; factor=1.0 → 256 (no dim, but we skip writes
    // entirely when factor >= 1.0 via the early-return in the caller).
    if factor >= 1.0 {
        return;
    }
    let Some(idx) = frame.index(col, row) else {
        return;
    };
    let cell = frame.cell_at_index(idx);
    // Skip blank cells (no foreground) — dimming empty space would
    // tint the background, which is NOT the CRT-vignette aesthetic.
    let Some(fg) = cell.fg else {
        return;
    };
    let Some((r, g, b)) = crate::palette::decode_color(fg) else {
        return;
    };
    // (chroma audit, A8): route brightness scale through the chroma
    // engine when active, fall back to chroma::legacy::scale_rgb otherwise.
    // Both paths use the same `((c * fi + 128) >> 8).clamp(0,255)` equation
    // -- the difference is auditability. See
    // docs/research/CHROMA_DRAGON_ENGINE_AUDIT.md §6.4.
    let new_fg = if pipeline.is_chroma() {
        crate::chroma_dragon_engine::palette::apply_brightness_rgb(r, g, b, factor)
    } else {
        let (nr, ng, nb) = crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, factor);
        Color::Rgb {
            r: nr,
            g: ng,
            b: nb,
        }
    };
    let new_cell = crate::cell::Cell {
        ch: cell.ch,
        fg: Some(new_fg),
        bg: cell.bg,
        bold: cell.bold,
    };
    // Suppress unused-warning: `bg` is referenced for future
    // extension (e.g., dimming blank cells toward bg instead of
    // skipping them). Currently we skip blank cells.
    let _ = bg;
    frame.set(col, row, new_cell);
}
