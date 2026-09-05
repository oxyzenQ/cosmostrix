// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Water-surface rain for the ripple scene (task-18, fourth rain style).
//!
//! Style DNA: the glyph droplet system still produces the falling rain
//! (this style is droplet-family — same pool, phosphor Pass 2, spawn
//! plumbing), but every droplet's `end_line` is capped just above a
//! virtual water surface near the bottom of the viewport. When a
//! droplet's head reaches the surface, the death hook in `rain_at`
//! calls [`RippleSurface::spawn_impact`], which opens:
//!
//! - an expanding ripple ring — an edge-on wavefront that spreads
//!   horizontally along the surface line with a small downward-only
//!   ellipse bulge (screen-circular with the 1:2 cell aspect), and
//! - a short splash of 2-4 particles hopping up from the impact
//!   point under gravity.
//!
//! A sparse deterministic "surface shimmer" (hash-positioned Ghost
//! glyphs on the water line with a slow phase wobble) keeps the surface
//! perceptible between impacts without adding noise.
//!
//! LOC note: slightly above the 500-line soft target as a single
//! self-contained surface system (rings + splashes + shimmer + diff
//! cleanup); well under the 800 hard cap.
//!
//! Region contract (zero overlap with droplet cells): droplets occupy
//! rows `0..=water_line - 3`, splashes rise at most
//! `RIPPLE_SPLASH_MAX_RISE` rows above the water line, rings live at
//! `water_line..` (downward only). The three zones never collide, so
//! the monolith-style drawn-cell diff cleanup cannot blink a live
//! droplet cell.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::monolith::BrightnessLevel;
use super::monolith_helpers::{bold_for_level, clear_cell, color_for_level};
use super::render::DrawCtx;

/// Ring glyph set — narrow (width-1) water glyphs. Ring position within
/// the set is a stable function of (col, line) so successive frames
/// re-draw the same glyph (no per-frame glyph flicker).
pub(crate) const RIPPLE_RING_CHARS: [char; 5] = ['·', '˚', '°', '∙', '∼'];

/// Surface shimmer glyphs (deterministic per-column pick).
const RIPPLE_SHIMMER_CHARS: [char; 4] = ['·', '.', '`', '\''];

#[derive(Clone, Copy, Debug)]
pub(crate) struct RippleRing {
    pub(crate) active: bool,
    /// Impact column (ring center x).
    pub(crate) col: u16,
    /// Accumulated surface-clock age (HUNT-21 pattern: dt-based, so the
    /// ring completes its trajectory on slow terminals too).
    pub(crate) sim_age: f32,
    /// Total lifetime (variance per impact).
    pub(crate) lifetime: f32,
    /// Max horizontal spread in cells (variance per impact).
    pub(crate) max_radius: f32,
    /// Palette slot snapshot at impact.
    pub(crate) palette_slot: u8,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SplashParticle {
    pub(crate) active: bool,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) vx: f32,
    pub(crate) vy: f32,
    pub(crate) sim_age: f32,
    pub(crate) lifetime: f32,
    pub(crate) palette_slot: u8,
    /// Salt glyph id (index into RIPPLE_RING_CHARS) — stable per particle.
    pub(crate) glyph: u8,
}

/// Per-frame step inputs for the ripple surface.
pub(crate) struct RippleStep {
    pub(crate) now: Instant,
    pub(crate) lines: u16,
    /// chars_per_sec already multiplied by the terminal speed_mult —
    /// drives ring expansion speed so ↑/↓ speed keys feel native.
    pub(crate) chars_per_sec: f32,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

pub(crate) struct RippleSurface {
    rings: [RippleRing; crate::constants::RIPPLE_RING_POOL],
    splashes: [SplashParticle; crate::constants::RIPPLE_SPLASH_POOL],
    ring_scan_idx: usize,
    splash_scan_idx: usize,
    active_rings: usize,
    active_splashes: usize,
    /// Global surface clock (dt-integrated, clamped like the vortex clock).
    last_step: Option<Instant>,
    current_cells: Vec<(u16, u16)>,
    previous_cells: Vec<(u16, u16)>,
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
}

impl RippleSurface {
    pub(crate) fn new() -> Self {
        Self {
            rings: [RippleRing {
                active: false,
                col: 0,
                sim_age: 0.0,
                lifetime: 0.0,
                max_radius: 0.0,
                palette_slot: 0,
            }; crate::constants::RIPPLE_RING_POOL],
            splashes: [SplashParticle {
                active: false,
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                sim_age: 0.0,
                lifetime: 0.0,
                palette_slot: 0,
                glyph: 0,
            }; crate::constants::RIPPLE_SPLASH_POOL],
            ring_scan_idx: 0,
            splash_scan_idx: 0,
            active_rings: 0,
            active_splashes: 0,
            last_step: None,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
        }
    }

    /// Re-seed for a new viewport. The pool is fixed-size (rings/splashes);
    /// only the diff history resets.
    pub(crate) fn reset(&mut self) {
        for r in &mut self.rings {
            r.active = false;
        }
        for p in &mut self.splashes {
            p.active = false;
        }
        self.ring_scan_idx = 0;
        self.splash_scan_idx = 0;
        self.active_rings = 0;
        self.active_splashes = 0;
        self.last_step = None;
        self.clear_draw_history();
    }

    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// The water line: RIPPLE_SURFACE_ROWS above the bottom edge.
    pub(crate) fn water_line(lines: u16) -> u16 {
        lines
            .saturating_sub(crate::constants::RIPPLE_SURFACE_ROWS)
            .max(1)
    }

    /// Droplet end cap: keep the falling rain 3 rows clear of the surface
    /// so the splash zone never overlaps droplet cells.
    pub(crate) fn droplet_end_line(lines: u16) -> u16 {
        Self::water_line(lines).saturating_sub(crate::constants::RIPPLE_DROPLET_CLEAR_ROWS)
    }

    /// Palette transition completion: rings/splashes adopt the new slot.
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        for r in &mut self.rings {
            if r.active {
                r.palette_slot = palette_slot;
            }
        }
        for p in &mut self.splashes {
            if p.active {
                p.palette_slot = palette_slot;
            }
        }
    }

    /// Impact hook — called from the droplet death branch in `rain_at`
    /// when a droplet's head reaches the water line. Early deaths
    /// (glitch rip mid-air) never reach the surface and are ignored by
    /// the caller's `line >= water_line` gate, so this always fires at
    /// the surface plane.
    pub(crate) fn spawn_impact(
        &mut self,
        col: u16,
        water_line: u16,
        palette_slot: u8,
        rand_chance: &Uniform<f32>,
        rng: &mut StdRng,
    ) {
        // Ripple ring.
        if self.active_rings < self.rings.len() {
            if let Some(idx) = self.find_inactive_ring() {
                let r = &mut self.rings[idx];
                r.active = true;
                r.col = col;
                r.sim_age = 0.0;
                r.lifetime = crate::constants::RIPPLE_RING_LIFETIME
                    * (0.85 + rand_chance.sample(rng) * 0.30);
                r.max_radius = crate::constants::RIPPLE_RING_MAX_RADIUS
                    * (0.80 + rand_chance.sample(rng) * 0.45);
                r.palette_slot = palette_slot;
                self.active_rings += 1;
            }
        }
        // Splash: 2-4 hop particles.
        let hop_count = 2 + (rand_chance.sample(rng) * 2.99) as usize; // 2..=4
        for _ in 0..hop_count {
            if self.active_splashes >= self.splashes.len() {
                break;
            }
            let Some(idx) = self.find_inactive_splash() else {
                break;
            };
            let dir: f32 = if rand_chance.sample(rng) < 0.5 {
                -1.0
            } else {
                1.0
            };
            let spread = 0.6 + rand_chance.sample(rng) * 1.4;
            let up = crate::constants::RIPPLE_SPLASH_SPEED * (0.75 + rand_chance.sample(rng) * 0.5);
            let p = &mut self.splashes[idx];
            p.active = true;
            p.x = col as f32;
            p.y = water_line as f32;
            p.vx = dir * spread;
            p.vy = -up;
            p.sim_age = 0.0;
            p.lifetime =
                crate::constants::RIPPLE_SPLASH_LIFETIME * (0.8 + rand_chance.sample(rng) * 0.4);
            p.palette_slot = palette_slot;
            p.glyph = (rand_chance.sample(rng) * RIPPLE_RING_CHARS.len() as f32) as u8;
            self.active_splashes += 1;
        }
    }

    fn find_inactive_ring(&mut self) -> Option<usize> {
        let len = self.rings.len();
        for step in 0..len {
            let idx = (self.ring_scan_idx + step) % len;
            if !self.rings[idx].active {
                self.ring_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    fn find_inactive_splash(&mut self) -> Option<usize> {
        let len = self.splashes.len();
        for step in 0..len {
            let idx = (self.splash_scan_idx + step) % len;
            if !self.splashes[idx].active {
                self.splash_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Surface physics pass: integrate ring ages + splash ballistics.
    ///
    /// Ring radius grows with an ease-out profile (fast open, slow
    /// settle — `sqrt` of normalized age), scaled by chars_per_sec so
    /// speed keys feel native. Splashes are simple ballistic hops:
    /// constant gravity, linear integration, real-clock dt bounded by
    /// the anti-teleport cap (HUNT-22 pattern).
    pub(crate) fn advance(&mut self, step: &RippleStep) {
        let has_activity = self.active_rings > 0 || self.active_splashes > 0;
        let dt = match self.last_step {
            Some(last) => {
                step.now
                    .saturating_duration_since(last)
                    .as_secs_f32()
                    .min(
                        step.max_sim_delta
                            .as_secs_f32()
                            .min(crate::constants::PARTICLE_MAX_FRAME_DT_SECS),
                    )
                    .max(0.0)
                    * step.resume_blend.clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        self.last_step = Some(step.now);
        if !has_activity || dt <= 0.0 {
            return;
        }

        let water_line = Self::water_line(step.lines);
        let speed_scale =
            (step.chars_per_sec / crate::constants::RIPPLE_SPEED_REF_CPS).clamp(0.25, 3.0);

        let mut rings_done = 0usize;
        for r in &mut self.rings {
            if !r.active {
                continue;
            }
            // cps-scaled aging: rings open (and expire) faster at higher
            // rain speed — the ↑/↓ speed keys feel native.
            r.sim_age += dt * speed_scale;
            if r.sim_age >= r.lifetime {
                r.active = false;
                rings_done += 1;
            }
        }
        if rings_done > 0 {
            self.active_rings = self.active_rings.saturating_sub(rings_done);
        }

        let g = crate::constants::RIPPLE_SPLASH_GRAVITY;
        let mut splash_done = 0usize;
        for p in &mut self.splashes {
            if !p.active {
                continue;
            }
            p.sim_age += dt;
            if p.sim_age >= p.lifetime {
                p.active = false;
                splash_done += 1;
                continue;
            }
            p.x += p.vx * dt * speed_scale;
            p.y += p.vy * dt * speed_scale;
            p.vy += g * dt;
            // Splash cells never rise above the clear zone (region contract).
            let min_y =
                (water_line.saturating_sub(crate::constants::RIPPLE_SPLASH_MAX_RISE)) as f32;
            if p.y < min_y {
                p.y = min_y;
                p.vy = p.vy.max(0.0);
            }
        }
        if splash_done > 0 {
            self.active_splashes = self.active_splashes.saturating_sub(splash_done);
        }
    }

    /// Draw pass: surface shimmer + rings + splashes, with the
    /// monolith-style drawn-cell diff cleanup.
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut super::monolith::MonolithCleanup<'_>,
        now_secs: u32,
    ) {
        let lines_us = ctx.lines as usize;
        let water_line = Self::water_line(ctx.lines);

        self.current_cells.clear();

        // 1. Surface shimmer: hash-positioned Ghost glyphs with a slow
        // brightness wobble. Deterministic positions keep the diff
        // cleanup stable (no per-frame repositioning flicker).
        for col in 0..ctx.cols {
            if !surface_hash(col).is_multiple_of(crate::constants::RIPPLE_SHIMMER_SPACING) {
                continue;
            }
            let glyph = RIPPLE_SHIMMER_CHARS
                [(surface_hash(col.wrapping_add(7)) % RIPPLE_SHIMMER_CHARS.len() as u16) as usize];
            // Wobble in {0.55, 0.70, 0.85, 1.0} — quantized to keep the
            // frame diff mostly stable across frames.
            let phase = (now_secs.wrapping_add((surface_hash(col) / 8) as u32)) & 3;
            let factor = 0.55 + phase as f32 * 0.15;
            let shimmer_level = if factor > 0.9 {
                BrightnessLevel::Dim
            } else {
                BrightnessLevel::Ghost
            };
            draw_surface_cell(ctx, frame, col, water_line, glyph, 0, shimmer_level, factor);
            self.current_cells.push((col, water_line));
        }

        // 2. Ripple rings — expanding edge-on wavefronts.
        for r in &self.rings {
            if !r.active {
                continue;
            }
            let age_t = (r.sim_age / r.lifetime).clamp(0.0, 1.0);
            // Ease-out opening: sqrt profile (fast open, slow settle).
            let radius = r.max_radius * age_t.sqrt();
            if radius < 0.5 {
                continue;
            }
            let fade = 1.0 - age_t;
            let level = if fade > 0.66 {
                BrightnessLevel::Hot
            } else if fade > 0.33 {
                BrightnessLevel::Mid
            } else {
                BrightnessLevel::Ghost
            };
            // Walk the ellipse: horizontal semi-axis = radius, vertical
            // = radius / 2 (screen-circular at 1:2 cell aspect), downward
            // half only (the surface plane hides the upper half).
            let steps = (radius * std::f32::consts::TAU / 2.0).ceil().max(6.0) as usize;
            for s in 0..steps {
                let phi = std::f32::consts::TAU * s as f32 / steps as f32;
                let dx = radius * phi.cos();
                let dy = radius * 0.5 * phi.abs().sin();
                let col = r.col as i32 + dx.round() as i32;
                let line = water_line as i32 + dy.round() as i32;
                if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                    continue;
                }
                let (col, line) = (col as u16, line as u16);
                let ch = ring_char_at(col, line);
                draw_surface_cell(ctx, frame, col, line, ch, r.palette_slot, level, 1.0);
                self.current_cells.push((col, line));
            }
        }

        // 3. Splash particles — hop glyphs above the impact points.
        for p in &self.splashes {
            if !p.active {
                continue;
            }
            let col = p.x.round() as i32;
            let line = p.y.round() as i32;
            if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                continue;
            }
            let (col, line) = (col as u16, line as u16);
            let age_t = (p.sim_age / p.lifetime).clamp(0.0, 1.0);
            let level = if age_t < 0.5 {
                BrightnessLevel::Hot
            } else {
                BrightnessLevel::Mid
            };
            let ch = RIPPLE_RING_CHARS[(p.glyph as usize) % RIPPLE_RING_CHARS.len()];
            draw_surface_cell(ctx, frame, col, line, ch, p.palette_slot, level, 1.0);
            self.current_cells.push((col, line));
        }

        // Pass 2: generation-tag (monolith pattern).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let need_len = ctx.cols as usize * lines_us.max(1);
        if self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
        }
        for &(col, line) in &self.current_cells {
            let idx = col as usize * lines_us + line as usize;
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
        }

        // Pass 3: clear stale cells.
        let drawn_gen = &self.drawn_gen[..];
        for &(col, line) in &self.previous_cells {
            let idx = col as usize * lines_us + line as usize;
            if idx < drawn_gen.len() && drawn_gen[idx] == gen {
                continue;
            }
            clear_cell(frame, cleanup, col, line);
        }

        std::mem::swap(&mut self.previous_cells, &mut self.current_cells);
    }

    // -- Test-only diagnostics --

    #[cfg(test)]
    pub(crate) fn active_ring_count_for_test(&self) -> usize {
        self.active_rings
    }

    #[cfg(test)]
    pub(crate) fn active_splash_count_for_test(&self) -> usize {
        self.active_splashes
    }
}

/// Stable position hash for shimmer placement / glyph pick.
fn surface_hash(col: u16) -> u16 {
    // Knuth multiplicative hash — cheap, deterministic, well-spread.
    ((col as u32).wrapping_mul(2654435761) >> 13) as u16
}

fn ring_char_at(col: u16, line: u16) -> char {
    let h = surface_hash(col.wrapping_add(line.wrapping_mul(31)));
    RIPPLE_RING_CHARS[(h % RIPPLE_RING_CHARS.len() as u16) as usize]
}

/// Render one surface cell (palette-aware color + bold, mono-safe).
// 8 tracked params (ctx+frame+6 cell fields) mirrors the monolith draw
// helpers; bundling a single-cell struct would add a per-cell constructor
// in 3 loops for no readability gain.
#[allow(clippy::too_many_arguments)]
fn draw_surface_cell(
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    col: u16,
    line: u16,
    ch: char,
    palette_slot: u8,
    level: BrightnessLevel,
    factor: f32,
) {
    if line >= ctx.lines || col >= ctx.cols {
        return;
    }
    let fg = color_for_level(ctx, palette_slot, line, col, level, factor);
    let bold = bold_for_level(ctx.bold_mode, level, line, col);
    let cell = crate::cell::Cell {
        ch,
        fg,
        bg: ctx.bg,
        bold,
    };
    frame.set(col, line, cell);
}
