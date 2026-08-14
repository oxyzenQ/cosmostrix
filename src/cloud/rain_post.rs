// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-processing effects applied after the droplet draw pass.
//!
//! CRT vignette (top/bottom edge dim) + quantum ripple (click-triggered expanding
//! ring). Both are Cloud methods split from rain.rs to keep that file under the
//! 1200-LOC source cap. Same impl block, just in a sibling file.

use std::time::Instant;

use crossterm::style::Color;

use crate::constants::{
    CRT_VIGNETTE_HEIGHT, CRT_VIGNETTE_PERF_THRESHOLD, QUANTUM_BODY_TONE_DOWN,
    QUANTUM_RIPPLE_LIFETIME_SECS,
};
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
    /// The factor goes from `CRT_VIGNETTE_EDGE_FACTOR` (0.82,
    /// masterclass retune) at the extreme edge row to 1.0 (no dim) at
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
    pub(super) fn apply_crt_vignette(&mut self, frame: &mut Frame) {
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

    /// Update + render Quantum Ripple particles (v25 masterclass).
    ///
    /// Active particles move outward radially, fade based on age
    /// (bright at birth → dim at lifespan end), and are rendered as
    /// glyphs (*, +, ·) tinted by each particle's snapshot of the
    /// palette body color captured at spawn time. The body stop is the
    /// middle index of `palette.colors` — the saturated hue the eye
    /// reads as "the rain color" (the head/last stop is intentionally
    /// near-white to give droplets their bright leading edge, which is
    /// why we don't snapshot it). When the user switches color theme
    /// mid-flight, the existing cohort keeps fading in its original
    /// body color while only newly-spawned particles pick up the new
    /// body color — a natural crossfade. Expired particles are
    /// deactivated (returned to the free-list).
    ///
    /// Runs O(active_particles) per frame. Cost is negligible —
    /// typically 0-20 active particles, peaking at ~40 during rapid
    /// multi-click bursts.
    pub(super) fn apply_quantum_ripple(&mut self, frame: &mut Frame, now: Instant) {
        // PERF: O(1) early-out when no particles are active. This is the
        // common case in interactive rendering (no recent clicks) and in
        // benchmark mode (no clicks at all). Avoids the 64-element pool
        // scan + palette color decode + per-particle Instant math.
        if self.quantum_active_count == 0 {
            // Keep the timestamp fresh so the first frame after a click
            // doesn't compute a huge dt (which would otherwise be clamped
            // to 1/30 sec, causing a tiny position jump on spawn).
            self.last_quantum_update_time = now;
            return;
        }

        // Frame-rate-independent motion: use ACTUAL delta time since last
        // update (not hardcoded 1/60), clamped to 1/30 to prevent teleport
        // after pause/resume or window focus loss. At 60 FPS this matches
        // the old behavior; at 30 FPS particles now travel 2x per frame,
        // preserving intended speed across the 0.8s lifespan.
        //
        // scale by resume_blend so ripple motion eases in lockstep
        // with spawn/droplet/phosphor during pause-deceleration and
        // resume-acceleration (audit §8.1 — previously ripple ran at full
        // speed during the 0.30s decel, visually incongruous with rain).
        let dt = now
            .saturating_duration_since(self.last_quantum_update_time)
            .as_secs_f32()
            .min(1.0 / 30.0)
            * self.resume_blend;
        self.last_quantum_update_time = now;

        let cols = self.cols;
        let lines = self.lines;
        let bg = self.palette.bg;

        let mut deactivated = 0usize;
        for p in &mut self.quantum_particles {
            if !p.active {
                continue;
            }
            let age = now.saturating_duration_since(p.birth).as_secs_f32();
            if age >= QUANTUM_RIPPLE_LIFETIME_SECS {
                p.active = false;
                deactivated += 1;
                continue;
            }
            // Use the real frame dt (clamped above) so motion stays
            // consistent regardless of frame rate. At 60 FPS this matches
            // the old `1/60` behavior exactly; at 30 FPS particles now
            // travel twice as far per frame, preserving the intended
            // visual speed across the full 0.8s lifespan.
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            let life_frac = age / QUANTUM_RIPPLE_LIFETIME_SECS;
            let fade = 1.0 - life_frac;
            let brightness = fade * fade;

            let col = p.x as u16;
            let line = p.y as u16;
            if col >= cols || line >= lines {
                p.active = false;
                deactivated += 1;
                continue;
            }

            let Some(idx) = frame.index(col, line) else {
                continue;
            };
            let cell = frame.cell_at_index(idx);

            // Each particle carries the RGB snapshot of the palette body
            // color it had at spawn time. Reading `p.r/p.g/p.b` here —
            // instead of decoding `palette.colors` mid-index live — means
            // a palette switch mid-flight leaves the existing cohort
            // tinted in its original body color while only newly-spawned
            // particles pick up the new body color. The two cohorts fade
            // out independently, producing a cinematic crossfade.
            //
            // v30 masterclass: apply QUANTUM_BODY_TONE_DOWN at render
            // time so the snapshot stored on the particle stays equal
            // to the palette body stop (preserving the crossfade and
            // "snapshot matches body stop" regression-test contracts),
            // while the rendered pixel is dimmed to match the rain's
            // perceived average brightness rather than the saturated
            // body stop alone. See the constant's doc comment for the
            // empirical rationale.
            //
            // (chroma audit, A1): tone-down scale routes through
            // chroma engine when active, legacy scale_rgb otherwise.
            // Both paths use the same `(c * factor).round().clamp(0,255)`
            // equation -- the original code used f32 multiply+round which
            // is bit-identical to what chroma::palette::apply_brightness_rgb
            // and chroma::legacy::scale_rgb produce for the same factor.
            // (Color-#4): corrected misleading "bit-identical" claim.
            // The chroma path uses apply_brightness_rgb_unclamped (integer
            // `>> 8` math via scale_rgb semantics); the legacy path uses
            // f32 round. The two differ by ±1 per channel for some inputs
            // for QUANTUM_BODY_TONE_DOWN = 0.72 (constants.rs). The test at
            // tests_quantum.rs:614 accepts ±1 tolerance.
            // (Color-#5): chroma path now uses apply_brightness_rgb_unclamped
            // (returns tuple directly) instead of apply_brightness_rgb +
            // decode_color round-trip — saves ~9 cycles/call. The call-site
            // guard (factor is a const in [0,1]) means the unclamped variant
            // is bit-identical to the clamped one here.
            let (pr, pg, pb) = if self.color_pipeline.is_chroma() {
                crate::chroma::palette::apply_brightness_rgb_unclamped(
                    p.r,
                    p.g,
                    p.b,
                    QUANTUM_BODY_TONE_DOWN,
                )
            } else {
                // Legacy fallback: preserve the original f32 round behavior
                // (NOT legacy::scale_rgb, which uses integer >> 8 math).
                // The difference vs chroma is ±1 per channel for QUANTUM_BODY_TONE_DOWN.
                (
                    (p.r as f32 * QUANTUM_BODY_TONE_DOWN)
                        .round()
                        .clamp(0.0, 255.0) as u8,
                    (p.g as f32 * QUANTUM_BODY_TONE_DOWN)
                        .round()
                        .clamp(0.0, 255.0) as u8,
                    (p.b as f32 * QUANTUM_BODY_TONE_DOWN)
                        .round()
                        .clamp(0.0, 255.0) as u8,
                )
            };

            // Base color: use cell's fg if present, else bg, else the
            // particle's snapshot color (so particles are visible even
            // on transparent backgrounds with no rain at that cell).
            let (br, bg_, bb) = if let Some(fg) = cell.fg {
                crate::palette::decode_color(fg).unwrap_or((pr, pg, pb))
            } else if let Some(bg_color) = bg {
                crate::palette::decode_color(bg_color).unwrap_or((pr, pg, pb))
            } else {
                // Blank cell on transparent bg — use the particle's
                // snapshot color as the base so it stays visible.
                (pr, pg, pb)
            };

            // (chroma audit, A1): blend toward particle snapshot
            // routes through chroma engine when active, legacy
            // blend_toward_rgb otherwise. Same equation both paths:
            // (c + (target - c) * (factor * 256) + 128) / 256 clamped.
            let (nr, ng, nb) = if self.color_pipeline.is_chroma() {
                crate::chroma::palette::blend_toward_bg_rgb(br, bg_, bb, pr, pg, pb, brightness)
            } else {
                crate::chroma::legacy::blend_toward_rgb(br, bg_, bb, pr, pg, pb, brightness)
            };
            let new_fg = Color::Rgb {
                r: nr,
                g: ng,
                b: nb,
            };
            // Use force-set so the particle always writes, even if the
            // cell was blank. This ensures visibility on transparent bg.
            // The next frame's clear_with_bg() will clean the cell.
            frame.set_force(
                col,
                line,
                crate::cell::Cell {
                    ch: p.ch,
                    fg: Some(new_fg),
                    bg: cell.bg,
                    bold: true,
                },
            );
        }

        // Decrement the active count by the number deactivated this frame.
        // Saturating sub protects against any drift between the counter
        // and the actual pool state.
        if deactivated > 0 {
            self.quantum_active_count = self.quantum_active_count.saturating_sub(deactivated);
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
        crate::chroma::palette::apply_brightness_rgb(r, g, b, factor)
    } else {
        let (nr, ng, nb) = crate::chroma::legacy::scale_rgb(r, g, b, factor);
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
