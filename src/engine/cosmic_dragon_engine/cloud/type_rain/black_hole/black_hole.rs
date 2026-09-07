// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole rain for the sorgonemous_intrascals scene (NIGHT-special-1,
//! the eighth rain style — stage 2: the ball plus the orbital ring).
//!
//! Motion DNA — 100% distinct from every existing style: the screen hosts
//! a single gravitating body, not a particle field. Stage 1 rendered the
//! body itself: a medium round ball centered on the viewport with a black
//! empty core (the event horizon) and a bright photon-ring rim that fades
//! outward into the dark (the visual reference is the intro logo emblem —
//! a density-shaded round mark — re-expressed through the engine's own
//! brightness ladder and the active charset pool). Stage 2 orbits a ring
//! of glyphs around it: RK4-Lorenz-turbulent motes on a tilted Keplerian
//! ellipse, far side passing behind the hole, near side crossing in
//! front of the core (the per-mote physics lives in `ring.rs`).
//!
//! Geometry: terminal cells are roughly 1:2 (width:height), so a circle
//! on the physical screen is an ellipse in cell space. All radius math
//! runs in line-height units: a cell offset (dx cols, dy lines) sits at
//! screen distance sqrt((dx / 2)^2 + dy^2). The ball outer radius is a
//! fraction of the viewport's limiting half-extent, so it scales with any
//! screen size from 80x24 to 400x100 without touching the constants.
//!
//! Radial brightness: the annulus between the core radius and the outer
//! radius is banded like the vortex drain, but inverted — the brightest
//! zone hugs the event horizon (the accretion photon ring) and dims
//! toward the outer rim, the way a real black hole silhouette reads:
//! a dark core wrapped in a thin blazing edge.
//!
//! Stage roadmap (owner-approved staged rollout, one commit per stage):
//! - stage 1: the ball — owner visual verification (rated 10/10).
//! - stage 2 (this file + `ring.rs`): the orbital ring — motes on a
//!   tilted Keplerian ellipse, turbulence integrated with the same RK4
//!   machinery the lorenz style ships (the integrator is
//!   attractor-agnostic; see `type_rain/lorenz/lorenz.rs`).
//! - stage 3: glyph rain infall — falling glyphs that bend elegantly
//!   into the core when they approach the ring's capture radius.
//!
//! The glyph pool for each cell re-rolls through a low-probability
//! shimmer gate (matrix DNA: mutation is the engine's life sign), and a
//! full re-roll is armed whenever the charset changes so the ball never
//! carries stale glyphs from a previous pool.
//!
//! Cleanup follows the monolith/vortex three-pass diff pattern: draw
//! into `current_cells`, tag with the `drawn_gen` generation counter,
//! then clear only previous cells NOT redrawn this frame (phosphor
//! metadata and frame blank). The ball is static so its cells are all
//! redrawn each frame, but the stage-2 ring motes vacate cells as they
//! orbit — exactly the case the diff cleanup exists for (stage 3's
//! infalling glyphs will lean on the same contract).

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{clear_cell, pick_pool_char};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};
use super::ball_helpers::{bump_level, conveyor_char, draw_ball_cell, level_for_ring_band};
use super::ring::{
    activate_ring_mote, advance_ring_mote, level_for_ring_z, occludes_ring_cell, project_ring_mote,
    step_down_level, BlackHoleRandom, BlackHoleSpawnParams, BlackHoleStep, RingMote,
};

/// One drawn ball cell: grid position plus its radial brightness band.
/// Own struct instead of reusing monolith's `DrawnCell` because the ball
/// has no Segment/Spine kind distinction (same shape as `VortexCell`,
/// plus the precomputed band level — the band is static geometry, so it
/// is computed once at reset instead of per frame).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlackHoleCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
    pub(crate) level: BrightnessLevel,
}

/// Cell-space aspect divisor: terminal cells are ~1:2 (width:height), so
/// one column of travel covers half a line-height of screen distance.
/// Shared with `ring.rs` (the orbital projection runs in the same
/// line-height units as the ball raster).
pub(crate) const CELL_ASPECT_DIVISOR: f32 = 2.0;

/// Shorthand rank for the brightness ladder (Ghost = 0 ... Core = 4).
/// BrightnessLevel carries no PartialEq, so band tests compare ranks.
#[cfg(test)]
pub(crate) fn level_rank(level: BrightnessLevel) -> u8 {
    match level {
        BrightnessLevel::Ghost => 0,
        BrightnessLevel::Dim => 1,
        BrightnessLevel::Mid => 2,
        BrightnessLevel::Hot => 3,
        BrightnessLevel::Core => 4,
    }
}

pub(crate) struct BlackHoleRain {
    /// Precomputed ring annulus cells (geometry is static, so it is
    /// built once per reset instead of rescanned per frame).
    ring_cells: Vec<BlackHoleCell>,
    /// Parallel glyph per ring cell, drawn from the active charset pool.
    glyphs: Vec<char>,
    /// Set when the glyph pool may have changed (charset switch, style
    /// entry); the next draw re-rolls every glyph from the live pool.
    glyphs_stale: bool,
    /// Palette slot adopted at entry / palette transition (the ball is a
    /// single body, so one slot covers every cell).
    palette_slot: u8,
    /// Stage 2 orbital ring: one mote per column (the family lane
    /// model). The per-mote physics (RK4 Lorenz turbulence + Keplerian
    /// advance + projection) lives in `ring.rs`.
    motes: Vec<RingMote>,
    /// Active mote count (the spawn target's deficit baseline).
    active_motes: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx`).
    spawn_scan_idx: usize,
    /// Alternating lobe selector for the Lorenz-state seeding (parity
    /// with the lorenz style's spawn — balanced wobble distribution).
    next_lobe: u8,
    /// Global motion clock. dt = now - last_step clamped by
    /// max_sim_delta, scaled by resume_blend (the anti-teleport
    /// contract shared with the structured family).
    last_step: Option<Instant>,
    /// Cached ball anchor: integer center cell (the discrete midpoint
    /// the ball raster scans around). Rebuilt at reset.
    center_col: i32,
    center_line: i32,
    /// Cached ball outer radius in line-height units — the ring
    /// radii are multiples of it (scales with any screen size).
    ball_outer_r: f32,
    /// Ball rim spin phase (radians, unbounded — read through cos so
    /// no wrapping bookkeeping). Advanced by the same clock and
    /// omega as the ring's mean motion: the hole visibly rotates
    /// with its disk (stage 2.1, owner feedback).
    spin_phase: f32,
    /// Per-cell aspect-corrected angle around the ball center
    /// (parallel to `ring_cells` — the conveyor's input geometry,
    /// computed once at reset).
    cell_angles: Vec<f32>,
    /// Last conveyor bucket index per cell (parallel to `ring_cells`
    /// — motion-gated glyph re-roll bookkeeping).
    cell_buckets: Vec<i32>,
    /// Last frame's drawn cells (diff cleanup input).
    previous_cells: Vec<BlackHoleCell>,
    /// This frame's drawn cells (diff cleanup output).
    current_cells: Vec<BlackHoleCell>,
    /// Generation tags for the diff cleanup pass (flat: col * lines +
    /// line), rebuilt when the viewport dimensions change.
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
    drawn_gen_dims: (u16, u16),
}

impl BlackHoleRain {
    pub(crate) fn new() -> Self {
        Self {
            ring_cells: Vec::new(),
            glyphs: Vec::new(),
            glyphs_stale: true,
            palette_slot: 0,
            motes: Vec::new(),
            active_motes: 0,
            spawn_scan_idx: 0,
            next_lobe: 0,
            last_step: None,
            center_col: 0,
            center_line: 0,
            ball_outer_r: 0.0,
            spin_phase: 0.0,
            cell_angles: Vec::new(),
            cell_buckets: Vec::new(),
            previous_cells: Vec::new(),
            current_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            drawn_gen_dims: (0, 0),
        }
    }

    /// Rebuild the ball geometry for a new viewport (or style entry).
    ///
    /// The outer radius is a fraction of the viewport's limiting
    /// half-extent (line-height units): `unit = min(cols / 4, lines / 2)`
    /// — `cols / 4` is the half-width expressed in line units through the
    /// cell aspect, `lines / 2` the half-height. A medium ball reads at
    /// roughly a third of the screen's short axis on every terminal size.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.ring_cells.clear();
        if cols == 0 || lines == 0 {
            self.glyphs.clear();
            self.glyphs_stale = true;
            self.reset_ring_motes(0);
            self.center_col = 0;
            self.center_line = 0;
            self.ball_outer_r = 0.0;
            self.cell_angles.clear();
            self.cell_buckets.clear();
            self.clear_draw_history();
            return;
        }

        let unit = (cols as f32 / (CELL_ASPECT_DIVISOR * 2.0)).min(lines as f32 / 2.0);
        let outer_r = unit * crate::constants::BLACK_HOLE_BALL_FRACTION;
        let core_r = outer_r * crate::constants::BLACK_HOLE_CORE_FRACTION;

        // Degenerate viewport guard: a ball thinner than one cell of
        // annulus width draws nothing (prevents a zero-width division
        // below and an empty-shell flash on tiny terminals). No ball,
        // no ring anchor — the mote pool stays empty so nothing
        // degenerate projects around a zero-radius hole.
        if outer_r - core_r < 1.0 {
            self.glyphs.clear();
            self.glyphs_stale = true;
            self.reset_ring_motes(0);
            self.center_col = 0;
            self.center_line = 0;
            self.ball_outer_r = 0.0;
            self.cell_angles.clear();
            self.cell_buckets.clear();
            self.clear_draw_history();
            return;
        }

        // Integer center cell (the grid's discrete midpoint). Even
        // dimensions land the true center between cells; (n - 1) / 2
        // picks the lower-mid cell so the scan offsets are exact
        // integers and the distance math stays consistent with the
        // rasterized geometry.
        let cx = ((cols - 1) / 2) as i32;
        let cy = ((lines - 1) / 2) as i32;
        let half_w_cells = (outer_r * CELL_ASPECT_DIVISOR).ceil() as i32;
        let half_h_cells = outer_r.ceil() as i32;
        let annulus_width = outer_r - core_r;

        // Cache the ball anchor: the ring projects its ellipse around
        // this exact center and radius every frame (line-height units,
        // same math as the raster above).
        self.center_col = cx;
        self.center_line = cy;
        self.ball_outer_r = outer_r;

        for dy in -half_h_cells..=half_h_cells {
            for dx in -half_w_cells..=half_w_cells {
                let col = (cx + dx) as u16;
                let line = (cy + dy) as u16;
                if col >= cols || line >= lines {
                    continue;
                }
                let dist = ((dx as f32 / CELL_ASPECT_DIVISOR).powi(2) + (dy as f32).powi(2)).sqrt();
                if dist < core_r || dist > outer_r {
                    continue;
                }
                let t = ((dist - core_r) / annulus_width).clamp(0.0, 1.0);
                self.ring_cells.push(BlackHoleCell {
                    col,
                    line,
                    level: level_for_ring_band(t),
                });
            }
        }

        self.glyphs.clear();
        self.glyphs.resize_with(self.ring_cells.len(), || '0');
        self.glyphs_stale = true;

        // Rim conveyor geometry: each cell's angle around the center
        // (aspect-corrected, the same units the ring projection uses)
        // drives the sliding glyph buckets — the visible surface
        // rotation of the ball. Buckets start at 0 so the first draw
        // re-rolls every glyph to its bucket's character.
        self.cell_angles = self
            .ring_cells
            .iter()
            .map(|c| {
                let dx = (c.col as f32 - cx as f32) / CELL_ASPECT_DIVISOR;
                let dy = c.line as f32 - cy as f32;
                dy.atan2(dx)
            })
            .collect();
        self.cell_buckets = vec![0; self.ring_cells.len()];

        self.reset_ring_motes(cols);
        self.clear_draw_history();
    }

    /// Rebuild the ring mote pool: one mote per column (the family
    /// lane model — a wider viewport hosts a longer ring, so more
    /// motes), all vacant. Pass 0 to leave the pool empty (degenerate
    /// viewports with no ball anchor).
    fn reset_ring_motes(&mut self, cols: u16) {
        self.motes.clear();
        if cols > 0 {
            self.motes.resize_with(cols as usize, RingMote::vacant);
        }
        self.active_motes = 0;
        self.spawn_scan_idx = 0;
        self.next_lobe = 0;
        self.last_step = None;
    }

    /// Steady-state drawn-glyph count (the HUD active metric): every
    /// ball annulus cell plus every active ring mote head — the honest
    /// figure of what the style animates each frame.
    pub(crate) fn active_count(&self) -> usize {
        self.ring_cells.len() + self.active_motes
    }

    /// Palette transition completion: the ball adopts the new slot,
    /// and so does every active ring mote (family contract — parity
    /// with the lorenz/vortex mote adoption).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        self.palette_slot = palette_slot;
        for m in &mut self.motes {
            if m.active {
                m.palette_slot = palette_slot;
            }
        }
    }

    /// Steady-state active-mote target from pool size + density
    /// (mirrors `LorenzRain::target_active_count` with the ring's
    /// ratios — a sparser stream than the lorenz field because the
    /// ring is a band, not the whole viewport).
    fn target_active_motes(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (crate::constants::BLACK_HOLE_RING_ACTIVE_BASE
            + density.clamp(0.01, 5.0) * crate::constants::BLACK_HOLE_RING_ACTIVE_DENSITY_MULT)
            .clamp(0.02, crate::constants::BLACK_HOLE_RING_ACTIVE_MAX);
        ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
    }

    /// Amortized free-slot scan (rotating cursor — mirrors vortex's
    /// `find_inactive_mote`).
    fn find_inactive_mote(&mut self) -> Option<usize> {
        let len = self.motes.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.spawn_scan_idx + step) % len;
            if !self.motes[idx].active {
                self.spawn_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Spawn pass — accumulator pattern identical to
    /// `LorenzRain::spawn` (deficit-bounded budget + fractional
    /// remainder carry). New motes enter at a uniform random orbital
    /// phase, so the stream populates around the full circumference
    /// instead of clumping at one angle.
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &BlackHoleSpawnParams,
        random: &mut BlackHoleRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.motes.is_empty() || self.ball_outer_r < 1.0
        {
            *spawn_remainder = 0.0;
            return;
        }

        let target = Self::target_active_motes(self.motes.len(), params.density);
        if self.active_motes >= target {
            *spawn_remainder = (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_motes;
        let spawn_rate = (target as f32 * crate::constants::BLACK_HOLE_RING_SPAWN_RATE_MULT
            + crate::constants::BLACK_HOLE_RING_SPAWN_RATE_FLOOR)
            * params.spawn_scale;
        let budget = elapsed.as_secs_f32() * spawn_rate
            + (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
        if !budget.is_finite() || budget <= 0.0 {
            *spawn_remainder = 0.0;
            return;
        }

        let to_spawn = (budget.floor() as usize).min(deficit);
        *spawn_remainder = (budget - to_spawn as f32).min(crate::constants::SPAWN_REMAINDER_CAP);
        if to_spawn == 0 {
            return;
        }

        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_mote() else {
                break;
            };
            let lobe_sign = if self.next_lobe == 0 { 1.0 } else { -1.0 };
            self.next_lobe = (self.next_lobe + 1) % 2;
            activate_ring_mote(
                &mut self.motes[idx],
                lobe_sign,
                params.active_palette_slot,
                random.rand_chance,
                random.rng,
            );
            self.active_motes += 1;
        }
    }

    /// Motion pass — the clock owner for the ring (the per-mote RK4
    /// Lorenz step + Keplerian advance live in `ring.rs`). dt = now -
    /// last_step clamped by max_sim_delta and scaled by resume_blend
    /// (the anti-teleport contract shared with the structured
    /// family); a fully-paused run simply stops integrating. The
    /// ball's rim spin advances on the same clock and the same mean
    /// omega as the motes — the co-rotation contract — even while no
    /// mote is active (the hole spins on its own phase from the
    /// first frame).
    pub(crate) fn advance(&mut self, step: &BlackHoleStep) {
        let dt_wall = match self.last_step {
            Some(last) => {
                step.now
                    .saturating_duration_since(last)
                    .as_secs_f32()
                    .min(step.max_sim_delta.as_secs_f32())
                    .max(0.0)
                    * step.resume_blend.clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        self.last_step = Some(step.now);
        if dt_wall <= 0.0 {
            return;
        }

        // Integration dt: chars_per_sec mapped onto attractor time
        // (RK4 stability regime — same mapping as the lorenz style),
        // plus the mean orbital rate in radians per wall second.
        let dt_lorenz_base =
            step.chars_per_sec.max(0.0) * crate::constants::BLACK_HOLE_RING_DT_PER_CPS * dt_wall;
        let omega_base =
            step.chars_per_sec.max(0.0) * crate::constants::BLACK_HOLE_RING_OMEGA_PER_CPS;

        // Ball rim co-rotation: the spin phase rides the same clock
        // and omega as the ring's mean motion, scaled by SPIN_RATE
        // (1.0 = lockstep with the disk's phase — the hole rotates
        // with its ring, per the owner's stage-2 feedback).
        self.spin_phase += omega_base * dt_wall * crate::constants::BLACK_HOLE_RING_SPIN_RATE;

        if self.active_motes == 0 {
            return;
        }

        let mut absorbed = 0usize;
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            if advance_ring_mote(m, dt_wall, dt_lorenz_base, omega_base) {
                absorbed += 1;
            }
        }
        if absorbed > 0 {
            self.active_motes = self.active_motes.saturating_sub(absorbed);
        }
    }

    /// Drop the diff-cleanup history and arm a full glyph re-roll
    /// (semantic invalidation: charset switch, palette change, forced
    /// redraw). The next draw pass rebuilds the baseline from scratch.
    pub(crate) fn clear_draw_history(&mut self) {
        self.previous_cells.clear();
        self.current_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
        self.drawn_gen_dims = (0, 0);
        self.glyphs_stale = true;
    }

    /// Draw pass — full annulus render + ring mote render +
    /// monolith-style diff cleanup.
    ///
    /// Stage 2 motion budget: the ball redraws its full annulus every
    /// frame (static body), the ring motes draw head + comet trail on
    /// their projected cells (far-side cells inside the ball silhouette
    /// are occluded — passing behind the hole), and the diff cleanup
    /// clears the cells the orbiting motes vacated — the contract this
    /// pattern has carried since stage 1, now load-bearing.
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        self.current_cells.clear();
        self.current_cells.reserve(self.ring_cells.len());

        // Charset switches and style entries arm a full re-roll so the
        // ball never carries glyphs from a stale pool.
        if self.glyphs_stale {
            for g in &mut self.glyphs {
                *g = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }
            self.glyphs_stale = false;
        }

        for (idx, cell) in self.ring_cells.iter().enumerate() {
            if cell.col >= ctx.cols || cell.line >= ctx.lines {
                // Viewport shrank without a reset (live resize window):
                // skip out-of-bounds geometry this frame; the resize
                // handler calls reset() and rebuilds the annulus.
                continue;
            }

            // Rim conveyor (stage 2.1 — the visible ball rotation): the
            // glyph pattern is bucketed by angle and the buckets slide
            // around the annulus with the spin phase. When a cell's
            // bucket changes the glyph re-rolls to the new bucket's
            // deterministic character — motion-gated mutation, the same
            // life-sign DNA as the family's shimmer gates.
            if self.cell_angles.len() > idx && self.cell_buckets.len() > idx {
                let bucket = ((self.cell_angles[idx] - self.spin_phase)
                    / crate::constants::BLACK_HOLE_RING_CONVEYOR_ARC)
                    .floor() as i32;
                if bucket != self.cell_buckets[idx] {
                    self.cell_buckets[idx] = bucket;
                    self.glyphs[idx] = conveyor_char(ctx.char_pool, bucket);
                }
            }
            // Matrix shimmer: low-probability glyph mutation is the
            // engine's life sign (every style carries one); the ball
            // stays calm, a slow surface flicker at the event horizon.
            if self.glyphs.len() > idx
                && rand_chance.sample(rng) < crate::constants::BLACK_HOLE_SHIMMER_CHANCE
            {
                self.glyphs[idx] = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }
            let ch = if self.glyphs.len() > idx {
                self.glyphs[idx]
            } else {
                '0'
            };

            // Doppler-style lobe (stage 2.1): a bright band sweeps
            // around the rim with the spin phase — cells near the lobe
            // peak brighten one rung, cells near the opposite point dim
            // one. The radial band structure is untouched (bump applied
            // at draw time only, clamped to the ladder), so the
            // approved photon-ring read stays intact while the surface
            // visibly rotates left-to-right in lockstep with the ring.
            let level = if self.cell_angles.len() > idx {
                let lobe = (self.cell_angles[idx] - self.spin_phase).cos();
                if lobe > crate::constants::BLACK_HOLE_RING_LOBE_GAIN {
                    bump_level(cell.level, 1)
                } else if lobe < -crate::constants::BLACK_HOLE_RING_LOBE_GAIN {
                    bump_level(cell.level, -1)
                } else {
                    cell.level
                }
            } else {
                cell.level
            };
            draw_ball_cell(
                ctx,
                frame,
                cell.col,
                cell.line,
                ch,
                self.palette_slot,
                level,
            );
            self.current_cells.push(*cell);
        }

        // Stage 2: the orbital ring — project every active mote onto
        // the wide tilted ellipse around the cached ball anchor, draw
        // the head + comet trail (occluded far-side cells skipped),
        // and record the drawn cells into the same diff-cleanup stream as
        // the ball (one unified current_cells / drawn_gen pipeline).
        if self.active_motes > 0 && self.ball_outer_r >= 1.0 {
            let cx_f = self.center_col as f32;
            let cy_f = self.center_line as f32;
            let outer_r = self.ball_outer_r;
            // Semi-major clamp: 92% of the viewport's half-width (in
            // line-height units) so the disk extremes never clip on
            // narrow terminals — the wide-disk read survives resize.
            let major_limit = 0.92 * ctx.cols as f32 / (CELL_ASPECT_DIVISOR * 2.0);
            for m in &mut self.motes {
                if !m.active {
                    continue;
                }
                let (col_f, line_f) = project_ring_mote(m, cx_f, cy_f, outer_r, major_limit);
                let col = col_f.round() as i32;
                let line = line_f.round() as i32;
                if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                    // Off-screen: skip the draw AND the trail push —
                    // the trail keeps its last in-bounds positions, so
                    // the streak resumes from the last visible cell
                    // when the mote re-enters the viewport (lorenz
                    // parity: no phantom trail cells).
                    continue;
                }
                let (col, line) = (col as u16, line as u16);

                // Matrix shimmer: mutate the glyph when the head lands
                // on a new cell (previous trail head differs), gated
                // by the family chance constant.
                if m.trail_len > 0 {
                    let (prev_col, prev_line) = m.trail[(m.trail_len - 1) as usize];
                    if (prev_col != col || prev_line != line)
                        && rand_chance.sample(rng)
                            < crate::constants::BLACK_HOLE_RING_SHIMMER_CHANCE
                    {
                        m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                } else {
                    m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }

                let head_level = level_for_ring_z(m.z);

                // Head: near-side motes crossing the silhouette draw in
                // front of the annulus / empty core; far-side motes are
                // hidden behind the ball (the 3D layering read).
                if !occludes_ring_cell(col, line, self.center_col, self.center_line, outer_r) {
                    draw_ball_cell(ctx, frame, col, line, m.ch, m.palette_slot, head_level);
                    self.current_cells.push(BlackHoleCell {
                        col,
                        line,
                        level: head_level,
                    });
                }

                // Comet trail: previously occupied cells, one brightness
                // rung dimmer each, drawn only while in bounds and not
                // behind the ball (a trail visibly vanishes into the
                // silhouette and resumes on emergence).
                for t in 0..m.trail_len as usize {
                    let (tc, tl) = m.trail[t];
                    if tc >= ctx.cols || tl >= ctx.lines {
                        continue;
                    }
                    if occludes_ring_cell(tc, tl, self.center_col, self.center_line, outer_r) {
                        continue;
                    }
                    let depth = (m.trail_len as usize - t).min(4) as u8;
                    let trail_level = step_down_level(head_level, depth);
                    draw_ball_cell(ctx, frame, tc, tl, m.ch, m.palette_slot, trail_level);
                    self.current_cells.push(BlackHoleCell {
                        col: tc,
                        line: tl,
                        level: trail_level,
                    });
                }

                m.push_trail(col, line);
            }
        }

        // Pass 2: generation-tag every drawn cell (monolith pattern —
        // u32 counter bump instead of clearing the array). Rebuilt when
        // the viewport dimensions change; sized to the full grid because
        // the flat index col * lines + line spans exactly cols * lines.
        let need_dims = (ctx.cols, ctx.lines);
        let need_len = ctx.cols as usize * lines_us.max(1);
        if self.drawn_gen_dims != need_dims || self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
            self.drawn_gen_dims = need_dims;
        }
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        for cell in &self.current_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
        }

        // Pass 3: clear previous cells NOT redrawn this frame.
        let drawn_gen = &self.drawn_gen[..];
        for cell in &self.previous_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < drawn_gen.len() && drawn_gen[idx] == gen {
                continue;
            }
            clear_cell(frame, cleanup, cell.col, cell.line);
        }

        std::mem::swap(&mut self.previous_cells, &mut self.current_cells);
    }

    // -- Test-only diagnostics (mirrors the monolith/vortex *_for_test API) --

    #[cfg(test)]
    pub(crate) fn ring_cells_for_test(&self) -> &[BlackHoleCell] {
        &self.ring_cells
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[BlackHoleCell] {
        &self.previous_cells
    }

    #[cfg(test)]
    pub(crate) fn motes_for_test(&self) -> &[RingMote] {
        &self.motes
    }

    #[cfg(test)]
    pub(crate) fn active_motes_for_test(&self) -> usize {
        self.active_motes
    }

    #[cfg(test)]
    /// Ball rim spin phase (the co-rotation contract's observable).
    pub(crate) fn spin_phase_for_test(&self) -> f32 {
        self.spin_phase
    }
}
