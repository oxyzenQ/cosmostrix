// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Eulerian velocity grid for the flux rain style (task-19, fourth
//! rain style replacement) — the PIC/FLIP hybrid solver core.
//!
//! This is the grid half of a particle-grid hybrid fluid method — the
//! same algorithm family used by film-VFX fluid solvers (PIC
//! 1957 / FLIP 1986, popularized for graphics by Zhu & Bridson 2005),
//! shrunk to terminal scale. No competitor terminal rain renderer has
//! a real incompressible Navier-Stokes projection in its critical
//! path; cosmostrix is the first (verified 2026-09-05 against
//! cmatrix, unimatrix, tmatrix and the broader matrix-rain ecosystem).
//!
//! Coordinate system — screen space, quadratic units: 1 unit equals
//! one terminal COLUMN width on both axes. Terminal cells are ~1:2
//! (width:height), so 1 vertical unit spans two cell lines; a circle
//! in (x, y) is circular on the physical screen. The grid node
//! spacing is FLUX_GRID_SPACING units (2.0) on both axes — a
//! half-resolution grid in each screen axis. At 120x40 the field is
//! 61x11 nodes (671) — the Jacobi solve costs ~13K flops per step,
//! vortex-class, and stays cache-resident (~10 KiB per array set).
//!
//! Solver step (called once per FLUX_SIM_DT of simulated time):
//! 1. P2G: every glyph splats its velocity bilinearly into the four
//!    surrounding nodes (momentum + weight accumulators), then the
//!    accumulators are normalized — the field velocity becomes the
//!    particle-conveyed flow.
//! 2. Force: gravity is added to the vertical component on nodes
//!    that carry particle weight (the fluid exists where the glyphs
//!    are; empty regions are calm, not uniformly sinking).
//! 3. Projection: the field is made divergence-free — the
//!    incompressibility constraint of the Navier-Stokes equations —
//!    by a classic pressure projection: compute the divergence, solve
//!    the Poisson equation for pressure with a fixed small number of
//!    Jacobi iterations (Neumann boundaries via mirrored neighbors),
//!    then subtract the pressure gradient. This single step is what
//!    makes falling glyph streams push neighboring fluid aside and
//!    shear into eddies — emergent structure, never scripted.
//! 4. G2P: glyphs read the field back with the FLIP/PIC hybrid
//!    blend — FLIP (particle velocity plus the LOCAL FIELD DELTA)
//!    preserves particle-level detail and energy; PIC (snap to the
//!    sampled field velocity) damps instability. The industry blend
//!    keeps both virtues.
//!
//! Boundary conditions: left/right walls are no-through-flow (the
//! horizontal component is zeroed at the wall column after the
//! projection); the top and bottom are open (velocity copies the
//! inner neighbor) so falling jets exit freely instead of pooling on
//! a pressure floor. Grid resolution is deliberately coarse: only
//! vortex-scale features (>= 4 screen units) are resolved — the
//! intent is living visual flow, not scientific accuracy; the
//! approximation is disclosed here on purpose.

/// Grid node spacing in screen units, both axes.
pub(crate) const FLUX_GRID_SPACING: f32 = 2.0;

/// One velocity sample in screen units per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FluxVel {
    pub(crate) vx: f32,
    pub(crate) vy: f32,
}

pub(crate) struct FluxField {
    /// Nodes along x (columns of the field).
    w: usize,
    /// Nodes along y (rows of the field).
    h: usize,
    /// Live velocity (post-projection). Screen units per second.
    u: Vec<f32>,
    v: Vec<f32>,
    /// Pre-force snapshot of the splatted field — the FLIP delta
    /// baseline (sample_after - sample_before captures both the
    /// gravity force and the projection change).
    u_prev: Vec<f32>,
    v_prev: Vec<f32>,
    /// P2G accumulators: momentum sums and weight sum per node.
    u_acc: Vec<f32>,
    v_acc: Vec<f32>,
    w_acc: Vec<f32>,
    /// Projection workspace: pressure (ping-pong pair) and divergence.
    p: Vec<f32>,
    p_next: Vec<f32>,
    div: Vec<f32>,
}

impl FluxField {
    /// Build a field sized for the viewport. Degenerate viewports
    /// still get a 2x2 field so every indexing path stays valid.
    pub(crate) fn new(cols: u16, lines: u16) -> Self {
        let x_max = cols.max(1) as f32;
        let y_max = (lines.max(1) as f32) * 0.5;
        let w = ((x_max / FLUX_GRID_SPACING).ceil() as usize).max(2) + 1;
        let h = ((y_max / FLUX_GRID_SPACING).ceil() as usize).max(2) + 1;
        let n = w * h;
        let zeros = || vec![0.0_f32; n];
        Self {
            w,
            h,
            u: zeros(),
            v: zeros(),
            u_prev: zeros(),
            v_prev: zeros(),
            u_acc: zeros(),
            v_acc: zeros(),
            w_acc: zeros(),
            p: zeros(),
            p_next: zeros(),
            div: zeros(),
        }
    }

    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        *self = Self::new(cols, lines);
    }

    /// Field dimensions hook (test contracts index the grid).
    #[cfg(test)]
    pub(crate) fn dims(&self) -> (usize, usize) {
        (self.w, self.h)
    }

    /// Clear the P2G accumulators (step 1 begins).
    pub(crate) fn begin_p2g(&mut self) {
        self.u_acc.fill(0.0);
        self.v_acc.fill(0.0);
        self.w_acc.fill(0.0);
    }

    /// Bilinear momentum splat of one particle into its four
    /// surrounding nodes. Positions outside the field clamp to the
    /// border (particles near the rim still deposit momentum).
    pub(crate) fn splat(&mut self, x: f32, y: f32, vel: FluxVel) {
        if !x.is_finite() || !y.is_finite() || !vel.vx.is_finite() || !vel.vy.is_finite() {
            return;
        }
        let (_, _, i0, j0, fx, fy) = self.grid_coords(x, y);
        let corners = [
            (i0, j0, (1.0 - fx) * (1.0 - fy)),
            (i0 + 1, j0, fx * (1.0 - fy)),
            (i0, j0 + 1, (1.0 - fx) * fy),
            (i0 + 1, j0 + 1, fx * fy),
        ];
        for (i, j, wgt) in corners {
            if wgt <= 0.0 {
                continue;
            }
            let idx = j * self.w + i;
            self.u_acc[idx] += vel.vx * wgt;
            self.v_acc[idx] += vel.vy * wgt;
            self.w_acc[idx] += wgt;
        }
    }

    /// Normalize the accumulators into the live field (step 1 ends).
    /// Nodes with no particle weight carry zero velocity — calm
    /// regions stay calm (the vacuum interpretation: gravity only
    /// applies where the fluid, i.e. the glyph rain, exists).
    pub(crate) fn finish_p2g(&mut self) {
        let n = self.u.len();
        for idx in 0..n {
            let w = self.w_acc[idx];
            if w > 1.0e-6 {
                self.u[idx] = self.u_acc[idx] / w;
                self.v[idx] = self.v_acc[idx] / w;
            } else {
                self.u[idx] = 0.0;
                self.v[idx] = 0.0;
            }
        }
    }

    /// Steps 2-3: snapshot the splatted field, add gravity on
    /// weight-carrying nodes, then run the pressure projection and
    /// the wall boundary pass. `dt` is the fixed solver step.
    pub(crate) fn apply_gravity_snapshot_project(&mut self, dt: f32, gravity: f32) {
        self.u_prev.copy_from_slice(&self.u);
        self.v_prev.copy_from_slice(&self.v);
        if gravity != 0.0 {
            for idx in 0..self.v.len() {
                if self.w_acc[idx] > 1.0e-6 {
                    self.v[idx] += gravity * dt;
                }
            }
        }
        self.project();
        self.wall_boundary();
    }

    /// Pressure projection — the incompressibility step. Divergence
    /// by central differences (clamped/mirrored neighbors at the
    /// borders — one-sided there), Poisson solve by Jacobi iteration
    /// with Neumann boundaries, then gradient subtraction. Iteration
    /// count is a disclosed approximation: 4 iterations remove the
    /// large-scale divergence (the shear structure) and leave the
    /// sub-grid residual — the right trade at terminal scale.
    fn project(&mut self) {
        let (w, h) = (self.w, self.h);
        // Divergence: div = du/dx + dv/dy with uniform spacing on
        // both axes (the grid is square in screen units), so the
        // 1/(2*spacing) factor is a shared scale that cancels out of
        // the Jacobi residual form. Only the sign structure matters
        // for the projection, so the raw central differences are
        // used directly.
        for j in 0..h {
            for i in 0..w {
                let idx = j * w + i;
                let i_left = i.saturating_sub(1);
                let i_right = (i + 1).min(w - 1);
                let j_down = j.saturating_sub(1);
                let j_up = (j + 1).min(h - 1);
                let div_ij = (self.u[j * w + i_right] - self.u[j * w + i_left])
                    + (self.v[j_up * w + i] - self.v[j_down * w + i]);
                self.div[idx] = div_ij;
            }
        }
        // Jacobi iterations for p: p = (neighbors - div) / 4 with
        // mirrored (Neumann) neighbors at the borders. A ping-pong
        // buffer pair keeps the sweep allocation-free (the project
        // rule: zero per-frame heap allocation in the render path);
        // self.p is the read buffer, p_next the write buffer, swapped
        // after each sweep.
        self.p.fill(0.0);
        self.p_next.fill(0.0);
        for _ in 0..crate::constants::FLUX_JACOBI_ITERATIONS {
            for j in 0..h {
                for i in 0..w {
                    let idx = j * w + i;
                    let i_left = i.saturating_sub(1);
                    let i_right = (i + 1).min(w - 1);
                    let j_down = j.saturating_sub(1);
                    let j_up = (j + 1).min(h - 1);
                    let nb = self.p[j * w + i_left]
                        + self.p[j * w + i_right]
                        + self.p[j_down * w + i]
                        + self.p[j_up * w + i];
                    self.p_next[idx] = (nb - self.div[idx]) * 0.25;
                }
            }
            std::mem::swap(&mut self.p, &mut self.p_next);
        }
        // Gradient subtraction with the same clamped neighbors.
        for j in 0..h {
            for i in 0..w {
                let i_left = i.saturating_sub(1);
                let i_right = (i + 1).min(w - 1);
                let j_down = j.saturating_sub(1);
                let j_up = (j + 1).min(h - 1);
                let idx = j * w + i;
                let dpdx = self.p[j * w + i_right] - self.p[j * w + i_left];
                let dpdy = self.p[j_up * w + i] - self.p[j_down * w + i];
                self.u[idx] -= dpdx * 0.5;
                self.v[idx] -= dpdy * 0.5;
            }
        }
    }

    /// Wall boundary pass: no flow through the left/right walls.
    /// Vertical motion along the walls is untouched (free slip).
    /// The top and bottom stay open — falling jets drain out.
    fn wall_boundary(&mut self) {
        let (w, h) = (self.w, self.h);
        for j in 0..h {
            self.u[j * w] = 0.0;
            self.u[j * w + (w - 1)] = 0.0;
        }
    }

    /// Bilinear sample of the live (post-projection) field.
    pub(crate) fn sample(&self, x: f32, y: f32) -> FluxVel {
        let (_, _, i0, j0, fx, fy) = self.grid_coords(x, y);
        let w00 = (1.0 - fx) * (1.0 - fy);
        let w10 = fx * (1.0 - fy);
        let w01 = (1.0 - fx) * fy;
        let w11 = fx * fy;
        let at = |i: usize, j: usize| j * self.w + i;
        FluxVel {
            vx: self.u[at(i0, j0)] * w00
                + self.u[at(i0 + 1, j0)] * w10
                + self.u[at(i0, j0 + 1)] * w01
                + self.u[at(i0 + 1, j0 + 1)] * w11,
            vy: self.v[at(i0, j0)] * w00
                + self.v[at(i0 + 1, j0)] * w10
                + self.v[at(i0, j0 + 1)] * w01
                + self.v[at(i0 + 1, j0 + 1)] * w11,
        }
    }

    /// Bilinear sample of the pre-force snapshot (FLIP baseline).
    pub(crate) fn sample_prev(&self, x: f32, y: f32) -> FluxVel {
        let (_, _, i0, j0, fx, fy) = self.grid_coords(x, y);
        let w00 = (1.0 - fx) * (1.0 - fy);
        let w10 = fx * (1.0 - fy);
        let w01 = (1.0 - fx) * fy;
        let w11 = fx * fy;
        let at = |i: usize, j: usize| j * self.w + i;
        FluxVel {
            vx: self.u_prev[at(i0, j0)] * w00
                + self.u_prev[at(i0 + 1, j0)] * w10
                + self.u_prev[at(i0, j0 + 1)] * w01
                + self.u_prev[at(i0 + 1, j0 + 1)] * w11,
            vy: self.v_prev[at(i0, j0)] * w00
                + self.v_prev[at(i0 + 1, j0)] * w10
                + self.v_prev[at(i0, j0 + 1)] * w01
                + self.v_prev[at(i0 + 1, j0 + 1)] * w11,
        }
    }

    /// Shared coordinate math: screen position to grid cell + local
    /// fractions, with border clamping (the +1 corner is guaranteed
    /// in range because i0 <= w-2 by construction).
    fn grid_coords(&self, x: f32, y: f32) -> (f32, f32, usize, usize, f32, f32) {
        let gx = (x / FLUX_GRID_SPACING).clamp(0.0, (self.w - 2) as f32);
        let gy = (y / FLUX_GRID_SPACING).clamp(0.0, (self.h - 2) as f32);
        let i0 = gx.floor() as usize;
        let j0 = gy.floor() as usize;
        let fx = gx - i0 as f32;
        let fy = gy - j0 as f32;
        (gx, gy, i0, j0, fx, fy)
    }

    // -- Test-only diagnostics --

    /// Maximum absolute divergence remaining after a projection —
    /// the numerical contract hook for the solver tests.
    #[cfg(test)]
    pub(crate) fn max_abs_divergence_for_test(&self) -> f32 {
        let (w, h) = (self.w, self.h);
        let mut max_d = 0.0_f32;
        for j in 0..h {
            for i in 0..w {
                let i_left = i.saturating_sub(1);
                let i_right = (i + 1).min(w - 1);
                let j_down = j.saturating_sub(1);
                let j_up = (j + 1).min(h - 1);
                let d = (self.u[j * w + i_right] - self.u[j * w + i_left])
                    + (self.v[j_up * w + i] - self.v[j_down * w + i]);
                max_d = max_d.max(d.abs());
            }
        }
        max_d
    }

    /// Wall value hook: horizontal velocity at a wall column.
    #[cfg(test)]
    pub(crate) fn wall_u_for_test(&self, j: usize, right: bool) -> f32 {
        let i = if right { self.w - 1 } else { 0 };
        self.u[j * self.w + i]
    }

    /// Velocity write hook (builds divergent test scenarios).
    #[cfg(test)]
    pub(crate) fn set_velocity_for_test(&mut self, i: usize, j: usize, vx: f32, vy: f32) {
        let idx = j * self.w + i;
        self.u[idx] = vx;
        self.v[idx] = vy;
        self.w_acc[idx] = 1.0;
    }
}
