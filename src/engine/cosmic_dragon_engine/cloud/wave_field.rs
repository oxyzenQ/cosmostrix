// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! # The wave-equation click shockwave (night-frontier-Z4b, Lapis 4b)
//!
//! The mouse-click ripple was a SYNTHETIC pair of kinematic rings: two
//! radii grew linearly with `elapsed * speed` and the renderer painted
//! an annulus around each — a drawing, not a wave. Z4b replaces it
//! with the real physics: each click injects an impulse into a scalar
//! field on the CELL GRID and the field EVOLVES by solving the 2D
//! wave equation's telegraph form, so the expanding ring a viewer sees
//! is the genuine solution of
//! `u_tt + k*u_t + (k/2)^2*u = c^2 * laplacian(u)` — "the click
//! actually solves the wave equation".
//!
//! ## The scheme
//!
//! Standard second-order finite differences (leapfrog) on the cell
//! grid, time-centered damping, one step per sub-step `h` — the
//! TELEGRAPH form of the damped wave equation (the restoring term is
//! what makes the field's life provably finite; see `step_once`):
//!
//! ```text
//! u_next = [ 2u - (1 - k*h/2)*u_prev + rx^2*(u_x- - 2u + u_x+)
//!                                  + ry^2*(u_y- - 2u + u_y+)
//!                                  - (k*h/2)^2 * u ]
//!          / (1 + k*h/2)
//! ```
//!
//! with `rx = c*h/dx` (columns) and `ry = c*h/dy` (rows). Terminal
//! cells are roughly twice as tall as wide, so the grid is PHYSICAL:
//! `dy = 2*dx`, which makes `ry = rx/2` — the front advances half as
//! many rows as columns per step, and the ripple is isotropic in pixel
//! space, i.e. it renders as a CIRCLE on screen (the synthetic ring's
//! equal-cell radii rendered as an ellipse). The von Neumann stability
//! condition for the scheme is `rx^2 + ry^2 <= 1` (CFL); damping only
//! relaxes it. The solver SUB-STEPS so the condition holds at any
//! frame rate: a slow frame splits into more sub-steps of
//! `MOUSE_FLASH_DT_MAX` each, capped so a pathological stall advances
//! the wave by a bounded amount instead of heating the CPU.
//!
//! ## Boundaries and lifetime
//!
//! First-order Mur absorbing conditions on all four edges swallow the
//! front at the screen border (no reflected ring), and the volume
//! damping `k = MOUSE_FLASH_WAVE_DAMPING` gives the field its finite
//! life: the amplitude envelope decays as `exp(-k*t/2)`, calibrated so
//! the visible life (~1.8 s, amplitude to the settle floor) matches
//! the synthetic wave's `MOUSE_FLASH_DURATION_SECS` era. When the
//! whole field falls below `MOUSE_FLASH_SETTLE_EPS` the solver marks
//! itself settled and subsequent frames cost ZERO work until the next
//! click re-arms it.
//!
//! ## Superposition
//!
//! One shared field carries ALL clicks: the wave equation is linear,
//! so simultaneous ripples genuinely interfere (the old bounded pool
//! of 4 slots just drew four independent rings). Clicks past any
//! number never evict — they superpose, with the inject amplitude
//! capped so a click storm saturates instead of blowing up.
//!
//! ## Costs and contracts
//!
//! Two preallocated `f32` grids sized to the current terminal (80x24
//! = 1920 cells x 8 bytes = ~15 KiB), single-threaded, ~10 flops per
//! cell per sub-step — microseconds per frame at 80x24, ONE step per
//! frame. Stepping allocates NOTHING (the zero-alloc ledger's law
//! extends here: resize happens on the one-shot reset surface only),
//! and the solver is deterministic given the click sequence. Honest
//! scope: coarse-grid dispersion makes the front's NUMERICAL speed
//! slightly direction-dependent (a real property of the
//! discretization, asserted within tolerance by the tests); the field
//! is a scalar brightness wave, not a full elastic simulation.

use crate::constants::{
    MOUSE_FLASH_DT_MAX, MOUSE_FLASH_SETTLE_EPS, MOUSE_FLASH_SPEED, MOUSE_FLASH_WAVE_DAMPING,
};

/// The vertical-to-horizontal cell aspect (a terminal cell is ~2x
/// taller than wide) — `dy = CELL_ASPECT * dx` in physical units.
pub(crate) const CELL_ASPECT: f32 = 2.0;

/// Inject amplitude cap: stacked clicks superpose up to this ceiling
/// so a click storm saturates the brightness instead of blowing the
/// field up.
const INJECT_CAP: f32 = 2.0;

/// The seed packet half-width: the impulse spreads over the clicked
/// cell and its 4-neighbors (a compact symmetric packet — a
/// single-cell delta would ring harshly on the coarse grid).
const SEED_NEIGHBOR_FRACTION: f32 = 0.5;

/// A solved 2D wave field on the cell grid: the shockwave state for
/// one `Cloud`. All buffers are preallocated; `step` never allocates.
pub(crate) struct WaveField {
    /// Current displacement, row-major (`idx = line * cols + col`).
    cur: Vec<f32>,
    /// Displacement one step ago (the leapfrog's second history row).
    prev: Vec<f32>,
    cols: usize,
    lines: usize,
    settled: bool,
}

impl WaveField {
    /// The inert empty field (no grid — a Cloud built without a
    /// terminal size yet). Allocates nothing.
    pub(crate) const fn empty() -> WaveField {
        WaveField {
            cur: Vec::new(),
            prev: Vec::new(),
            cols: 0,
            lines: 0,
            settled: true,
        }
    }

    /// (Re)size to the terminal grid. One-shot surface (Cloud reset /
    /// terminal resize — the same class as the palette rebuild);
    /// stepping never calls this. Preserves nothing: a resize clears.
    pub(crate) fn resize(&mut self, cols: u16, lines: u16) {
        let n = cols as usize * lines as usize;
        if self.cur.len() != n {
            self.cur = vec![0.0; n];
            self.prev = vec![0.0; n];
            self.cols = cols as usize;
            self.lines = lines as usize;
        }
        self.settled = true;
    }

    /// Zero the field and mark it settled (scene switch, pause safety).
    pub(crate) fn clear(&mut self) {
        for v in self.cur.iter_mut() {
            *v = 0.0;
        }
        for v in self.prev.iter_mut() {
            *v = 0.0;
        }
        self.settled = true;
    }

    /// True when the field holds no visible energy — the step loop
    /// early-outs on this internally; tests use it to pin the settle
    /// lifecycle (a settled field costs nothing until the next click).
    #[cfg_attr(not(test), allow(dead_code))] // settle introspection: pinned by the Z4b lock tests
    pub(crate) fn is_settled(&self) -> bool {
        self.settled
    }

    /// Grid dimensions backing the field (0x0 when empty). Test-facing
    /// introspection for the Z4b lock tests.
    #[cfg_attr(not(test), allow(dead_code))] // dims introspection: pinned by the Z4b lock tests
    pub(crate) fn dims(&self) -> (usize, usize) {
        (self.cols, self.lines)
    }

    /// The current displacement slice (the renderer's per-cell lookup
    /// table for this frame).
    pub(crate) fn amplitudes(&self) -> &[f32] {
        &self.cur
    }

    /// Inject a click impulse at `(col, line)` — the wave equation's
    /// initial condition. The seed is a compact symmetric packet on
    /// the clicked cell and its 4-neighbors, additive up to the cap
    /// (superposition without blowup). Out-of-range clicks clamp into
    /// the grid.
    pub(crate) fn inject(&mut self, col: u16, line: u16) {
        if self.cols < 1 || self.lines < 1 {
            return;
        }
        let col = (col as usize).min(self.cols - 1);
        let line = (line as usize).min(self.lines - 1);
        let idx = line * self.cols + col;
        // Zero initial velocity: prev carries the seed too.
        let add = |v: &mut f32, a: f32| {
            *v = (*v + a).min(INJECT_CAP);
        };
        add(&mut self.cur[idx], 1.0);
        add(&mut self.prev[idx], 1.0);
        if col > 0 {
            let i = idx - 1;
            add(&mut self.cur[i], SEED_NEIGHBOR_FRACTION);
            add(&mut self.prev[i], SEED_NEIGHBOR_FRACTION);
        }
        if col + 1 < self.cols {
            let i = idx + 1;
            add(&mut self.cur[i], SEED_NEIGHBOR_FRACTION);
            add(&mut self.prev[i], SEED_NEIGHBOR_FRACTION);
        }
        if line > 0 {
            let i = idx - self.cols;
            add(&mut self.cur[i], SEED_NEIGHBOR_FRACTION);
            add(&mut self.prev[i], SEED_NEIGHBOR_FRACTION);
        }
        if line + 1 < self.lines {
            let i = idx + self.cols;
            add(&mut self.cur[i], SEED_NEIGHBOR_FRACTION);
            add(&mut self.prev[i], SEED_NEIGHBOR_FRACTION);
        }
        self.settled = false;
    }

    /// Advance the field by `dt` seconds of wave time. Sub-steps at
    /// most `MOUSE_FLASH_DT_MAX` per sub-step (CFL safety at any frame
    /// rate), capped at 8 sub-steps (a pathological stall advances the
    /// wave less, never heats the CPU). No allocation; a settled field
    /// short-circuits to zero work.
    pub(crate) fn step(&mut self, dt: f32) {
        if self.settled || dt <= 0.0 || self.cols < 3 || self.lines < 3 {
            return;
        }
        let raw_subs = (dt / MOUSE_FLASH_DT_MAX).ceil() as usize;
        let subs = raw_subs.clamp(1, 8);
        let h = dt / subs as f32;
        for _ in 0..subs {
            self.step_once(h);
        }
        if self.peak_amplitude() < MOUSE_FLASH_SETTLE_EPS {
            self.clear();
        }
    }

    /// One CFL-safe leapfrog step of `h` seconds (interior FDTD + Mur
    /// absorbing edges).
    ///
    /// The PDE is the TELEGRAPH form of the damped wave equation:
    /// `u_tt + k*u_t + (k/2)^2*u = c^2*laplacian(u)`. The restoring
    /// term `(k/2)^2*u` is what gives the field a PROVABLE finite life:
    /// in the pure damped wave equation the spatial-mean (DC) mode
    /// converges to a NONZERO constant — a visible brightness residue
    /// that never decays and never settles. With the telegraph term
    /// every mode (DC critically damped, propagating modes
    /// underdamped) rides the same envelope `exp(-k*t/2)`, so the peak
    /// provably crosses the settle floor at `2*ln(A0/eps)/k` and the
    /// settle check is a law, not a hope. The term's frequency shift
    /// for the visible modes is ~17% at the lowest grid frequency —
    /// inside the documented dispersion tolerance the speed tests pin.
    fn step_once(&mut self, h: f32) {
        let (cols, lines) = (self.cols, self.lines);
        // Courant numbers: `c*h/dx` horizontally, `dy = CELL_ASPECT*dx`.
        let rx = MOUSE_FLASH_SPEED * h;
        let ry = MOUSE_FLASH_SPEED * h / CELL_ASPECT;
        let rx2 = rx * rx;
        let ry2 = ry * ry;
        // CFL: the scheme is stable for rx^2 + ry^2 <= 1; DT_MAX pins
        // the sub-step far inside that bound (asserted by the lock
        // test). Damping terms; the telegraph restoring term
        // (k*h)^2/4 is unconditionally stabilizing (0.0006 at DT_MAX).
        let damp = MOUSE_FLASH_WAVE_DAMPING;
        let a = damp * h * 0.5;
        let denom = 1.0 + a;
        let omega2h2 = a * a;

        // Rotate the three buffers by ownership: take both field vecs
        // out (their capacity travels with them — zero allocation). The
        // OLD prev buffer holds the leapfrog's history AND becomes the
        // output: each cell reads its own history value before the
        // same-index write (safe, single loop). The old cur becomes the
        // new history.
        let cur = std::mem::take(&mut self.cur);
        let mut scratch = std::mem::take(&mut self.prev);
        for line in 1..lines.saturating_sub(1) {
            let row = line * cols;
            for col in 1..cols.saturating_sub(1) {
                let i = row + col;
                let lap = rx2 * (cur[i - 1] - 2.0 * cur[i] + cur[i + 1])
                    + ry2 * (cur[i - cols] - 2.0 * cur[i] + cur[i + cols]);
                let v = (2.0 * cur[i] - (1.0 - a) * scratch[i] + lap - omega2h2 * cur[i]) / denom;
                scratch[i] = v;
            }
        }
        // Mur first-order absorbing edges (normal-incidence absorption;
        // the edges read the UPDATED interior value — the standard
        // single-pass form). Corners stay clamped to their edge
        // neighbors (the damping eats the residue).
        let mur_x = (rx - 1.0) / (rx + 1.0);
        let mur_y = (ry - 1.0) / (ry + 1.0);
        for line in 0..lines {
            let row = line * cols;
            // Left edge: u_next[0] = cur[1] + mur_x * (u_next[1] - cur[0]).
            let i = row;
            scratch[i] = cur[i + 1] + mur_x * (scratch[i + 1] - cur[i]);
            // Right edge.
            let i = row + cols - 1;
            scratch[i] = cur[i - 1] + mur_x * (scratch[i - 1] - cur[i]);
        }
        for col in 0..cols {
            // Top edge.
            let i = col;
            scratch[i] = cur[i + cols] + mur_y * (scratch[i + cols] - cur[i]);
            // Bottom edge.
            let i = (lines - 1) * cols + col;
            scratch[i] = cur[i - cols] + mur_y * (scratch[i - cols] - cur[i]);
        }
        // The Mur edges ride the SAME telegraph envelope as the interior:
        // first-order Mur alone decays boundary cells slower than the
        // damped interior (the outgoing-wave assumption ignores the volume
        // damping), which would leave a visible one-cell boundary residue
        // after the interior has settled. Multiplying the Mur result by
        // the per-sub-step damping factor keeps the whole field on the
        // documented exp(-k*t/2) schedule.
        let edge_damp = 1.0 - a;
        for line in 0..lines {
            let row = line * cols;
            scratch[row] *= edge_damp;
            scratch[row + cols - 1] *= edge_damp;
        }
        for col in 0..cols {
            scratch[col] *= edge_damp;
            scratch[(lines - 1) * cols + col] *= edge_damp;
        }
        self.cur = scratch; // next -> cur
        self.prev = cur; // old cur -> prev
    }

    /// The largest |displacement| currently in the field (the settle
    /// check). Single linear pass over a preallocated slice.
    fn peak_amplitude(&self) -> f32 {
        let mut peak = 0.0f32;
        for &v in &self.cur {
            let a = v.abs();
            if a > peak {
                peak = a;
            }
        }
        peak
    }
}
