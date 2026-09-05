// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Strange-attractor rain for the lorenz scene (NIGHT-research-4, the
//! fourth rain style — replaces the rejected `ripple` style).
//!
//! Motion DNA — 100% distinct from `cinematic` (column cascade),
//! `monolith` (segmented pillars), and `vortex` (polar-orbit spiral):
//! each mote is a glyph carried by a trajectory of the canonical Lorenz
//! strange attractor (sigma=10, rho=28, beta=8/3 — the foundational
//! chaotic system of modern nonlinear dynamics, published by Edward
//! Lorenz in 1963 as the simplified atmospheric-convection model that
//! first exhibited deterministic chaos and gave the "butterfly effect"
//! its name).
//!
//! Integration is classical fourth-order Runge-Kutta (RK4) — the
//! standard numerical ODE integrator for smooth nonlinear systems,
//! chosen over Euler because the Lorenz vector field is stiff near the
//! lobe crossings (high curvature) and Euler drifts visibly within
//! seconds. RK4 keeps trajectories on the true attractor for the
//! mote's full lifetime, so the visual signature is the real
//! butterfly, not a numerical artifact.
//!
//! The attractor's two lobes (one for x>0, one for x<0) are projected
//! to the terminal's two halves. Glyphs spawn near the canonical
//! unstable equilibria C+ = (sqrt(beta*(rho-1)), ...) and C- = -C+,
//! with tiny per-mote perturbations (LORENZ_SPAWN_PERTURB = 1e-3) so
//! neighbors visibly diverge over a few seconds — the butterfly
//! effect made visible. Motes are absorbed after a lifetime
//! (LORENZ_MAX_AGE_SECS) and respawn, giving the field continuous
//! living flow without ever repeating a trajectory.
//!
//! Depth cue: z maps to brightness zone (z high = lobe peak hot;
//! z low = saddle transition dim). Trails (last
//! `LORENZ_TRAIL_LEN` cell positions per mote) render as dimming
//! comet streaks, showing the recent trajectory arc — the visual
//! signature of "glyphs riding a chaotic attractor".
//!
//! Future-proof engineering — the masterpiece legacy: this file is
//! the project's first strange-attractor renderer. The architecture
//! (RK4 step + derivative function + project + diff cleanup) is
//! attractor-agnostic: swapping the Lorenz derivative for Rössler,
//! Aizawa, Thomas, or Chen is a single function replacement (each is
//! a 3D ODE the same RK4 integrates unchanged). The pattern sets a
//! reusable standard for future attractor styles.
//!
//! LOC note: at ~560 lines this matches the vortex file's soft-target
//! budget — a single self-contained style system (state, spawn,
//! advance, draw, and diff cleanup are one algorithm; splitting them
//! mirrors the monolith family split only once the file approaches
//! the 800-line hard cap). Well under the hard limit.
//!
//! Cleanup follows the monolith/vortex three-pass diff pattern: draw
//! into `current_cells`, tag with the `drawn_gen` generation counter,
//! then clear only previous cells NOT redrawn this frame (phosphor
//! metadata and frame blank). There is no spine equivalent, so no
//! extra phosphor pass.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::monolith::BrightnessLevel;
use super::monolith_helpers::{bold_for_level, clear_cell, color_for_level, pick_pool_char};
use super::render::DrawCtx;

/// Trail depth per mote (comet streak length in cells). Five cells
/// gives a slightly longer arc than vortex's four — the chaotic
/// trajectory bends more, so the recent path is more visually
/// informative and worth one extra cell of phosphor cost.
pub(crate) const LORENZ_TRAIL_LEN: usize = 5;

/// One drawn cell (col, line). Own struct instead of reusing
/// monolith's `DrawnCell` because lorenz has no Segment/Spine kind
/// distinction (same shape as `VortexCell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LorenzCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LorenzMote {
    pub(crate) active: bool,
    /// Lorenz state vector (x, y, z) — the integration variables.
    /// Initial values are seeded near the unstable equilibria with
    /// tiny perturbations so neighbors diverge over a few seconds
    /// (the visible butterfly effect).
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    /// Simulation age (seconds, dt-integrated). Drives absorption.
    pub(crate) sim_age: f32,
    /// Per-mote lifetime cap (variance per spawn so absorption is
    /// staggered — no rhythmic mass respawn).
    pub(crate) lifetime: f32,
    /// Per-mote integration step multiplier (0.85..1.15). Slight
    /// variation so two motes seeded identically still diverge in
    /// phase over time (intentional instability demonstration).
    pub(crate) pace: f32,
    /// Glyph carried by the mote; re-rolled matrix-style when the
    /// head crosses into a new cell (matrix-shimmer tied to motion).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Ring buffer of the last `LORENZ_TRAIL_LEN` head cell positions.
    /// Shift-left layout: index 0 = oldest, len-1 = most recent.
    trail: [(u16, u16); LORENZ_TRAIL_LEN],
    trail_len: u8,
}

impl LorenzMote {
    const fn vacant() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            sim_age: 0.0,
            lifetime: 0.0,
            pace: 1.0,
            ch: '0',
            palette_slot: 0,
            trail: [(0, 0); LORENZ_TRAIL_LEN],
            trail_len: 0,
        }
    }

    fn push_trail(&mut self, col: u16, line: u16) {
        if self.trail_len as usize >= LORENZ_TRAIL_LEN {
            // Shift-left: drop oldest, append newest at the tail.
            for i in 0..LORENZ_TRAIL_LEN - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[LORENZ_TRAIL_LEN - 1] = (col, line);
        } else {
            let idx = self.trail_len as usize;
            self.trail[idx] = (col, line);
            self.trail_len += 1;
        }
    }
}

/// Spawn inputs (mirrors `VortexSpawnParams` — bundle keeps clippy's
/// `too_many_arguments` threshold respected at the call site).
pub(crate) struct LorenzSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
}

/// RNG bundle (mirrors `VortexRandom`).
pub(crate) struct LorenzRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
}

/// Per-frame step inputs for the advance pass. Viewport geometry is
/// NOT carried here — the draw pass derives it from DrawCtx (the
/// authoritative frame dimensions); advance only needs time + speed.
pub(crate) struct LorenzStep {
    pub(crate) now: Instant,
    /// chars_per_sec already multiplied by the terminal speed_mult.
    /// Drives the integration dt scaling so ↑/↓ speed keys feel
    /// native on the chaotic trajectory.
    pub(crate) chars_per_sec: f32,
    pub(crate) max_sim_delta: Duration,
    pub(crate) resume_blend: f32,
}

pub(crate) struct LorenzRain {
    pub(crate) motes: Vec<LorenzMote>,
    active_count: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx`).
    spawn_scan_idx: usize,
    /// Alternating lobe selector (0 = right C+, 1 = left C-) so
    /// successive spawns populate both lobes evenly. The visual
    /// signature is symmetric butterfly coverage from frame 1.
    next_lobe: u8,
    /// Global motion clock. dt = now - last_step clamped by
    /// max_sim_delta; a fully-paused run simply stops integrating
    /// (rain_at early-return), so no pause-time shift is required
    /// (unlike per-droplet last_time).
    last_step: Option<Instant>,
    current_cells: Vec<LorenzCell>,
    previous_cells: Vec<LorenzCell>,
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
}

impl LorenzRain {
    pub(crate) fn new() -> Self {
        Self {
            motes: Vec::new(),
            active_count: 0,
            spawn_scan_idx: 0,
            next_lobe: 0,
            last_step: None,
            current_cells: Vec::new(),
            previous_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
        }
    }

    /// Rebuild the mote pool for a new viewport (or style entry). The
    /// pool is sized one mote per column (mirroring vortex's lane
    /// model); the active target is a density-driven ratio of that
    /// pool.
    pub(crate) fn reset(&mut self, cols: u16) {
        let lanes = cols.max(1) as usize;
        self.motes.clear();
        self.motes.resize_with(lanes, LorenzMote::vacant);
        self.active_count = 0;
        self.spawn_scan_idx = 0;
        self.next_lobe = 0;
        self.last_step = None;
        self.clear_draw_history();
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    /// Palette transition completion: all motes adopt the new slot
    /// (mirrors `VortexRain::adopt_palette_slot`).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        for m in &mut self.motes {
            if m.active {
                m.palette_slot = palette_slot;
            }
        }
    }

    /// Drop the diff-cleanup history (semantic invalidation / forced
    /// redraw). The next draw pass rebuilds it from an empty baseline.
    pub(crate) fn clear_draw_history(&mut self) {
        self.current_cells.clear();
        self.previous_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
    }

    /// Steady-state active-mote target from pool size + density
    /// (mirrors `VortexRain::target_active_count`).
    fn target_active_count(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (crate::constants::LORENZ_ACTIVE_BASE
            + density.clamp(0.01, 5.0) * crate::constants::LORENZ_ACTIVE_DENSITY_MULT)
            .clamp(0.02, crate::constants::LORENZ_ACTIVE_MAX);
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
    /// `MonolithRain::spawn` and `VortexRain::spawn` (deficit-bounded
    /// budget + fractional remainder carry).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &LorenzSpawnParams,
        random: &mut LorenzRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.motes.is_empty() {
            *spawn_remainder = 0.0;
            return;
        }

        let target = Self::target_active_count(self.motes.len(), params.density);
        if self.active_count >= target {
            *spawn_remainder = (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_count;
        let spawn_rate = (target as f32 * crate::constants::LORENZ_SPAWN_RATE_MULT
            + crate::constants::LORENZ_SPAWN_RATE_FLOOR)
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

        let pool = random.rand_chance.sample(random.rng);
        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_mote() else {
                break;
            };
            self.activate_mote(
                idx,
                params.active_palette_slot,
                pool,
                random.rand_chance,
                random.rng,
            );
            self.active_count += 1;
        }
    }

    /// Activate a vacant mote near one of the two lobe equilibria,
    /// with a tiny per-mote perturbation so neighbors diverge over
    /// time (the visible butterfly effect). The lobe alternates per
    /// spawn for symmetric coverage.
    ///
    /// Equilibria (C+ right, C- left) are derived from the standard
    /// Lorenz analysis: the C-plus and C-minus coordinates equal
    /// (sqrt(beta times (rho minus 1)), sqrt(beta times (rho minus 1)),
    /// rho minus 1). With beta 8/3 and rho 28, that evaluates to
    /// approximately (plus-or-minus 8.485, plus-or-minus 8.485, 27.0).
    /// Spawning AT the equilibrium would never leave (it's an
    /// unstable fixed point of the deterministic flow with slow local
    /// velocity), so motes spawn at the classic textbook initial
    /// condition (plus-or-minus 1, 1, 1) — well inside the saddle
    /// region's unstable manifold, immediately entering the chaotic
    /// butterfly flow. A small random perturbation (±2.0) is added
    /// so two motes seeded identically diverge visibly — the
    /// butterfly effect.
    fn activate_mote(
        &mut self,
        idx: usize,
        palette_slot: u8,
        _pool_roll: f32,
        rand_chance: &Uniform<f32>,
        rng: &mut StdRng,
    ) {
        // Lobe alternating selection (right C+ or left C-).
        let lobe_sign = if self.next_lobe == 0 { 1.0 } else { -1.0 };
        self.next_lobe = (self.next_lobe + 1) % 2;

        // Equilibrium coordinates for the selected lobe.
        let eq_x = lobe_sign * crate::constants::LORENZ_EQ_X;
        let eq_y = lobe_sign * crate::constants::LORENZ_EQ_Y;
        let eq_z = crate::constants::LORENZ_EQ_Z;

        // Tiny perturbation — the butterfly effect seed.
        let perturb = crate::constants::LORENZ_SPAWN_PERTURB;
        let px = (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;
        let py = (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;
        let pz = (rand_chance.sample(rng) - 0.5) * 2.0 * perturb;

        // Per-mote pace variation (0.85..1.15) so even identically
        // seeded motes drift in phase over time.
        let pace = 0.85 + rand_chance.sample(rng) * 0.30;

        // Lifetime cap with ±15% variance (staggered absorption).
        let lifetime =
            crate::constants::LORENZ_MAX_AGE_SECS * (0.85 + rand_chance.sample(rng) * 0.30);

        let m = &mut self.motes[idx];
        m.active = true;
        m.x = eq_x + px;
        m.y = eq_y + py;
        m.z = eq_z + pz;
        m.sim_age = 0.0;
        m.lifetime = lifetime;
        m.pace = pace;
        m.palette_slot = palette_slot;
        m.trail_len = 0;
    }

    /// Motion pass — the Lorenz ODE integration core.
    ///
    /// Classical RK4 integration of the Lorenz system:
    ///   dx/dt = sigma * (y - x)
    ///   dy/dt = x * (rho - z) - y
    ///   dz/dt = x * y - beta * z
    ///
    /// dt is derived from chars_per_sec (so ↑/↓ speed keys feel
    /// native) via LORENZ_DT_PER_CPS, scaled by per-mote pace and
    /// clamped by max_sim_delta (anti-teleport contract shared with
    /// vortex/monolith). Motes whose sim_age exceeds their lifetime
    /// are absorbed (deactivated → free slot).
    pub(crate) fn advance(&mut self, step: &LorenzStep) {
        if self.active_count == 0 {
            self.last_step = Some(step.now);
            return;
        }
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

        // Integration dt: chars_per_sec mapped onto Lorenz time via
        // LORENZ_DT_PER_CPS (the "speed" of one cell of motion in
        // attractor time units). Speed-24 scene → ~0.024 attractor
        // time/sec, well within the RK4 stability region for the
        // Lorenz system at canonical parameters (literature uses
        // 0.005-0.01 for visualization; we're conservative).
        let dt_lorenz_base =
            step.chars_per_sec.max(0.0) * crate::constants::LORENZ_DT_PER_CPS * dt_wall;

        let sigma = crate::constants::LORENZ_SIGMA;
        let rho = crate::constants::LORENZ_RHO;
        let beta = crate::constants::LORENZ_BETA;

        let mut absorbed = 0usize;
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            let dt = dt_lorenz_base * m.pace;
            // RK4 step (classical 4th-order Runge-Kutta).
            let (k1x, k1y, k1z) = lorenz_deriv(m.x, m.y, m.z, sigma, rho, beta);
            let (k2x, k2y, k2z) = lorenz_deriv(
                m.x + 0.5 * dt * k1x,
                m.y + 0.5 * dt * k1y,
                m.z + 0.5 * dt * k1z,
                sigma,
                rho,
                beta,
            );
            let (k3x, k3y, k3z) = lorenz_deriv(
                m.x + 0.5 * dt * k2x,
                m.y + 0.5 * dt * k2y,
                m.z + 0.5 * dt * k2z,
                sigma,
                rho,
                beta,
            );
            let (k4x, k4y, k4z) = lorenz_deriv(
                m.x + dt * k3x,
                m.y + dt * k3y,
                m.z + dt * k3z,
                sigma,
                rho,
                beta,
            );
            m.x += (dt / 6.0) * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
            m.y += (dt / 6.0) * (k1y + 2.0 * k2y + 2.0 * k3y + k4y);
            m.z += (dt / 6.0) * (k1z + 2.0 * k2z + 2.0 * k3z + k4z);

            m.sim_age += dt_wall;
            if m.sim_age >= m.lifetime {
                m.active = false;
                absorbed += 1;
                m.trail_len = 0;
            }
        }
        if absorbed > 0 {
            self.active_count = self.active_count.saturating_sub(absorbed);
        }
    }

    /// Draw pass — head cell + comet trail + monolith-style diff
    /// cleanup (mirrors `VortexRain::draw`).
    ///
    /// Matrix-style glyph mutation: when the head crosses into a new
    /// cell, the glyph re-rolls with probability
    /// LORENZ_SHIMMER_CHANCE. This is the authentic matrix-rain
    /// shimmer (mutation tied to motion), not a timer — it stays
    /// deterministic under the bench's uniform stepping.
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut super::monolith::MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        let cols_f = ctx.cols as f32;
        let lines_f = ctx.lines as f32;
        // Affine projection from Lorenz space to terminal cells.
        // The attractor is centered on (0, 0, LORENZ_EQ_Z/2); we map
        // x to col and y to line, both scaled by their respective
        // half-ranges so the full attractor fits inside the viewport
        // (with a small inset so trajectories never clip edges).
        let col_half = (cols_f * 0.5) * crate::constants::LORENZ_VIEW_INSET;
        let line_half = (lines_f * 0.5) * crate::constants::LORENZ_VIEW_INSET;
        let cx = cols_f * 0.5 - 0.5;
        let cy = lines_f * 0.5 - 0.5;
        let x_scale = col_half / crate::constants::LORENZ_X_HALF_RANGE;
        let y_scale = line_half / crate::constants::LORENZ_Y_HALF_RANGE;

        self.current_cells.clear();
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            // Project Lorenz (x, y) → cell (col, line). Note: terminal
            // y grows downward, so we flip the sign of y to make
            // "positive y" point up on screen (matches mathematical
            // convention and makes the butterfly visually upright).
            let col_f = cx + m.x * x_scale;
            let line_f = cy - m.y * y_scale;
            let col = col_f.round() as i32;
            let line = line_f.round() as i32;
            if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                // Off-screen: skip the draw AND the trail push — the
                // trail keeps its last in-bounds positions, so when the
                // mote re-enters the viewport the streak resumes from
                // the last visible cell (pushing off-screen positions
                // would leave phantom trail cells pointing outside).
                continue;
            }
            let (col, line) = (col as u16, line as u16);

            // Matrix shimmer: mutate the glyph when the head lands on
            // a new cell (previous trail head differs) with a chance
            // gate.
            if m.trail_len > 0 {
                let (prev_col, prev_line) = m.trail[(m.trail_len - 1) as usize];
                if (prev_col != col || prev_line != line)
                    && rand_chance.sample(rng) < crate::constants::LORENZ_SHIMMER_CHANCE
                {
                    m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }
            } else {
                m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }

            let head_level = level_for_z(m.z);
            draw_lorenz_cell(ctx, frame, col, line, m.ch, m.palette_slot, head_level);
            self.current_cells.push(LorenzCell { col, line });

            // Comet trail: previously occupied cells, one brightness
            // step dimmer each, drawn only while in bounds.
            for t in 0..m.trail_len as usize {
                let (tc, tl) = m.trail[t];
                if tc >= ctx.cols || tl >= ctx.lines {
                    continue;
                }
                let depth = (m.trail_len as usize - t).min(4) as u8;
                let trail_level = step_down_level(head_level, depth);
                draw_lorenz_cell(ctx, frame, tc, tl, m.ch, m.palette_slot, trail_level);
                self.current_cells.push(LorenzCell { col: tc, line: tl });
            }

            m.push_trail(col, line);
        }

        // Pass 2: generation-tag every drawn cell (monolith pattern —
        // u32 counter bump instead of clearing the array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let need_len = self.motes.len().saturating_mul(lines_us.max(1));
        if self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
        }
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

    // -- Test-only diagnostics (mirrors vortex's *_for_test API) --

    #[cfg(test)]
    pub(crate) fn active_states_for_test(&self) -> Vec<(f32, f32, f32)> {
        self.motes
            .iter()
            .filter(|m| m.active)
            .map(|m| (m.x, m.y, m.z))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[LorenzCell] {
        &self.current_cells
    }
}

/// The Lorenz derivative function — the canonical 3D ODE right-hand
/// side. Pure function (no state); called four times per RK4 step.
///
/// Returns the instantaneous rate of change (dx/dt, dy/dt, dz/dt)
/// at the given state. The attractor's two-lobe structure emerges
/// naturally from this single equation — no special-casing.
#[inline]
fn lorenz_deriv(x: f32, y: f32, z: f32, sigma: f32, rho: f32, beta: f32) -> (f32, f32, f32) {
    let dx = sigma * (y - x);
    let dy = x * (rho - z) - y;
    let dz = x * y - beta * z;
    (dx, dy, dz)
}

/// Brightness zone by Lorenz z-coordinate: z high = lobe peak hot;
/// z mid = lobe body; z low = saddle transition dim. The z range of
/// the canonical attractor is approximately [0, 50], with lobe peaks
/// near z≈40 and saddle crossings near z≈13. Four brightness zones
/// give the depth cue even in Color16 mode (palette index selection,
/// not blend math).
pub(crate) fn level_for_z(z: f32) -> BrightnessLevel {
    if z > crate::constants::LORENZ_Z_HOT {
        BrightnessLevel::Core
    } else if z > crate::constants::LORENZ_Z_MID {
        BrightnessLevel::Hot
    } else if z > crate::constants::LORENZ_Z_DIM {
        BrightnessLevel::Mid
    } else {
        BrightnessLevel::Ghost
    }
}

/// Step a brightness level down (toward Ghost) by `depth` ladder
/// rungs (mirrors vortex's `step_down_level`).
fn step_down_level(level: BrightnessLevel, depth: u8) -> BrightnessLevel {
    match level {
        BrightnessLevel::Core if depth >= 2 => BrightnessLevel::Mid,
        BrightnessLevel::Core => BrightnessLevel::Hot,
        BrightnessLevel::Hot if depth >= 2 => BrightnessLevel::Ghost,
        BrightnessLevel::Hot => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Ghost,
        BrightnessLevel::Ghost | BrightnessLevel::Dim => BrightnessLevel::Ghost,
    }
}

/// Render one lorenz cell (palette-aware color + bold, mono-safe).
fn draw_lorenz_cell(
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    col: u16,
    line: u16,
    ch: char,
    palette_slot: u8,
    level: BrightnessLevel,
) {
    if line >= ctx.lines || col >= ctx.cols {
        return;
    }
    let fg = color_for_level(ctx, palette_slot, line, col, level, 1.0);
    let bold = bold_for_level(ctx.bold_mode, level, line, col);
    let cell = crate::cell::Cell {
        ch,
        fg,
        bg: ctx.bg,
        bold,
    };
    frame.set(col, line, cell);
}
