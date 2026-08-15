// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-processing effects applied after the droplet draw pass.
//!
//! CRT vignette (top/bottom edge dim) + quantum ripple (click-triggered expanding
//! ring). Both are Cloud methods split from rain.rs to keep that file under the
//! 1500-LOC source cap. Same impl block, just in a sibling file.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use crate::constants::{
    CRT_VIGNETTE_HEIGHT, CRT_VIGNETTE_PERF_THRESHOLD, QUANTUM_BODY_TONE_DOWN,
    QUANTUM_RIPPLE_BOUNCE_DAMPING, QUANTUM_RIPPLE_HEAD_END_FRAC, QUANTUM_RIPPLE_LIFETIME_SECS,
    QUANTUM_RIPPLE_TAIL_START_FRAC,
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

    /// Update + render Quantum Ripple particles (v25 masterclass, v50 retune).
    ///
    /// Active particles move outward radially, fade based on age, and are
    /// rendered as glyphs (*, +, ·) tinted by each particle's snapshot of
    /// the palette body color captured at spawn time. The body stop is the
    /// middle index of `palette.colors` — the saturated hue the eye reads
    /// as "the rain color" (the head/last stop is intentionally near-white
    /// to give droplets their bright leading edge, which is why we don't
    /// snapshot it). When the user switches color theme mid-flight, the
    /// existing cohort keeps fading in its original body color while only
    /// newly-spawned particles pick up the new body color — a natural
    /// crossfade. Expired particles are deactivated (returned to the
    /// free-list).
    ///
    /// v50 stabilization (owner-requested): particles now **bounce**
    /// off the four screen edges instead of dying on border crossing.
    /// Each bounce applies `QUANTUM_RIPPLE_BOUNCE_DAMPING` to the
    /// crossed axis only — perpendicular velocity is untouched so the
    /// trajectory mirrors like a specular reflection. Age-based expiry
    /// is unchanged: bouncing keeps a particle alive within its
    /// `QUANTUM_RIPPLE_LIFETIME_SECS` window, but does not extend it.
    ///
    /// v50 masterclass retune (owner feedback 8/10): the brightness
    /// curve is now a three-segment HEAD/BODY/TAIL fade (see
    /// `QUANTUM_RIPPLE_HEAD_END_FRAC` / `QUANTUM_RIPPLE_TAIL_START_FRAC`)
    /// replacing the old `fade*fade` quadratic. Combined with the
    /// longer 2.5s lifespan and 30 cells/sec speed, this produces
    /// the smooth drift + graceful fade-out the owner requested.
    ///
    /// Runs O(active_particles) per frame. Cost is negligible —
    /// typically 0-20 active particles, peaking at ~60-80 during rapid
    /// multi-click bursts (96-slot pool absorbs this).
    pub(super) fn apply_quantum_ripple(&mut self, frame: &mut Frame, now: Instant) {
        // PERF: O(1) early-out when no particles are active. This is the
        // common case in interactive rendering (no recent clicks) and in
        // benchmark mode (no clicks at all). Avoids the 96-element pool
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
        // preserving intended speed across the 2.5s lifespan.
        //
        // Also capped by max_sim_delta (set by the event loop under perf
        // pressure) so quantum ripple decelerates in lockstep with rain
        // and monolith when the system is overloaded. Without this cap,
        // quantum particles would race ahead of the rain during throttling
        // — visually incongruous (rain crawls, particles zip).
        // When max_sim_delta is zero (bench mode / tests), the cap is
        // disabled — matching the droplet/monolith convention.
        let dt_raw = now
            .saturating_duration_since(self.last_quantum_update_time)
            .as_secs_f32();
        let sim_cap = if self.max_sim_delta > Duration::from_millis(0) {
            self.max_sim_delta.as_secs_f32()
        } else {
            f32::MAX
        };
        let dt = dt_raw.min(1.0 / 30.0).min(sim_cap) * self.resume_blend.clamp(0.0, 1.0);
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
            // visual speed across the full 2.5s lifespan.
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            // Bounce off the four screen edges (owner-requested v50
            // stabilization — previously particles died as soon as they
            // crossed the border, which clipped the burst on small
            // viewports or edge clicks). Specular reflection along the
            // crossed axis with a per-bounce damping factor
            // (`QUANTUM_RIPPLE_BOUNCE_DAMPING`) so the cohort gradually
            // loses energy instead of ricocheting forever. The
            // perpendicular axis is untouched, preserving the angle of
            // incidence == angle of reflection.
            //
            // We mirror the position across the offending edge AND flip
            // the velocity in one step. The mirror formula
            // `2 * edge - p.x` projects the particle back inside bounds
            // by the same distance it overshot — so a 0.4-cell overshoot
            // becomes a 0.4-cell inward position. This is the standard
            // specular-bounce correction and prevents the "stuck on the
            // edge" artifact that a plain clamp + flip would produce.
            //
            // Safety: `cols`/`lines` are u16, so `saturating_sub(1)`
            // guards against the degenerate 0-col / 0-line terminal
            // (where every position is "out of bounds"). In that case
            // `max_x == 0.0` and the particle gets clamped to the
            // single edge forever — visually a no-op since the screen
            // has no interior to draw on anyway.
            let max_x = cols.saturating_sub(1) as f32;
            let max_y = lines.saturating_sub(1) as f32;
            if p.x < 0.0 {
                p.x = -p.x;
                p.vx = -p.vx * QUANTUM_RIPPLE_BOUNCE_DAMPING;
            } else if p.x > max_x {
                p.x = 2.0 * max_x - p.x;
                p.vx = -p.vx * QUANTUM_RIPPLE_BOUNCE_DAMPING;
            }
            if p.y < 0.0 {
                p.y = -p.y;
                p.vy = -p.vy * QUANTUM_RIPPLE_BOUNCE_DAMPING;
            } else if p.y > max_y {
                p.y = 2.0 * max_y - p.y;
                p.vy = -p.vy * QUANTUM_RIPPLE_BOUNCE_DAMPING;
            }
            // Final clamp: defense-in-depth against multi-bounce in a single
            // frame (e.g. 30 cells/sec * 1/30 sec dt = 1 cell; a particle
            // near the edge can overshoot, bounce, and overshoot again).
            // The mirror formula handles each individual bounce correctly;
            // this clamp catches the accumulated error after multiple bounces.
            // Without it, the `as u16` cast below would wrap on out-of-bounds.
            p.x = p.x.clamp(0.0, max_x);
            p.y = p.y.clamp(0.0, max_y);

            let life_frac = age / QUANTUM_RIPPLE_LIFETIME_SECS;

            // v50 masterclass brightness curve (owner feedback 8/10):
            // three-segment fade replacing the old `fade*fade` quadratic.
            // The quadratic spent 50% of lifespan below 25% brightness —
            // at the new 2.5s lifespan that meant 1.25s of nearly invisible
            // drift, the "not smooth" complaint.
            //
            // Segment layout (life_frac ∈ [0, 1]):
            //  - HEAD  [0, HEAD_END_FRAC):           brightness = 1.0
            //  - BODY  [HEAD_END_FRAC, TAIL_START):  smoothstep 1.0 → TAIL_FLOOR
            //  - TAIL  [TAIL_START, 1]:              linear TAIL_FLOOR → 0.0
            //
            // TAIL_FLOOR is the brightness at the BODY→TAIL handoff.
            // Empirically 0.35 keeps the particle clearly visible through
            // the BODY segment while leaving enough headroom for the TAIL
            // to fade perceptibly. Higher (0.5) makes the TAIL segment
            // too short to read as a "fade out"; lower (0.2) makes the
            // BODY segment too dim.
            const TAIL_FLOOR: f32 = 0.35;
            let brightness = if life_frac < QUANTUM_RIPPLE_HEAD_END_FRAC {
                // HEAD: full brightness. The particle is at peak visibility
                // during the initial burst outward.
                1.0
            } else if life_frac < QUANTUM_RIPPLE_TAIL_START_FRAC {
                // BODY: smoothstep from 1.0 down to TAIL_FLOOR.
                // Normalize life_frac into [0, 1] within the BODY segment.
                let body_t = (life_frac - QUANTUM_RIPPLE_HEAD_END_FRAC)
                    / (QUANTUM_RIPPLE_TAIL_START_FRAC - QUANTUM_RIPPLE_HEAD_END_FRAC);
                // smoothstep: t*t*(3 - 2t), C1 continuous at both ends.
                // At body_t=0 → 0 (brightness = 1.0); at body_t=1 → 1
                // (brightness = TAIL_FLOOR).
                let s = body_t * body_t * (3.0 - 2.0 * body_t);
                1.0 - s * (1.0 - TAIL_FLOOR)
            } else {
                // TAIL: linear fade from TAIL_FLOOR down to 0.
                // Clamp tail_t to [0, 1] to guard against float precision
                // drift when life_frac is very close to 1.0 — prevents
                // negative brightness from 1.0 - tail_t going below 0.
                let tail_t = ((life_frac - QUANTUM_RIPPLE_TAIL_START_FRAC)
                    / (1.0 - QUANTUM_RIPPLE_TAIL_START_FRAC))
                    .clamp(0.0, 1.0);
                TAIL_FLOOR * (1.0 - tail_t)
            };

            let col = p.x as u16;
            let line = p.y as u16;
            if col >= cols || line >= lines {
                // Defensive: should be unreachable after the clamp
                // above, but a degenerate 0×0 terminal could still
                // trip it. Deactivate rather than panic.
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
