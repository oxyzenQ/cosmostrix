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
//! frame rate: frame time accumulates on a QUANTUM CLOCK and every
//! sub-step is EXACTLY `MOUSE_FLASH_DT_MAX` wide — capped at 8
//! sub-steps per frame, so a pathological stall advances the wave by a
//! bounded amount (8 quanta) instead of heating the CPU, and the
//! fixed width keeps the amplification matrix CONSTANT frame to frame
//! (alternating widths resonate — see `step`).
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
//! ## The energy ledger (night-frontier-Z6, S4)
//!
//! The bound above is NAMED, not implicit: the field carries an
//! explicit ledger over the leapfrog scheme's discrete Lyapunov
//! functional
//!
//! ```text
//! E = (1/2)*sum_i (u_i^n - u_i^(n-1))^2
//!   + (rx^2/2)*sum_x (u_(i+1)^n - u_i^n)*(u_(i+1)^(n-1) - u_i^(n-1))
//!   + (ry^2/2)*sum_y (u_(i+r)^n - u_i^n)*(u_(i+r)^(n-1) - u_i^(n-1))
//! ```
//!
//! the standard discrete energy of the three-level leapfrog scheme
//! (kinetic at level n-1/2, potential as the cross-time-level gradient
//! product). Derivation: dot the scheme with the centered velocity
//! `(u^(n+1) - u^(n-1))/2`; the kinetic part telescopes exactly into
//! `(1/2)(||w^n||^2 - ||w^(n-1)||^2)`, the Laplacian part telescopes
//! into the staggered potential difference via the self-adjointness of
//! the discrete Laplacian, and the damping term `2a*||v||^2` is
//! positive-definite removal. Every energy-removing mechanism in the
//! solver rides the same sign: volume damping, the telegraph restoring
//! term, the Mur edges (reflection magnitude `(1-rx)/(1+rx) < 1`), and
//! the explicit edge envelope factor `(1-a) < 1`.
//!
//! What the ledger pins (and the honest residual sources):
//! - STEPS NEVER CREATE ENERGY: per-frame `E` is non-increasing to f64
//!   rounding at the fixed quantum schedule (measured: exactly zero
//!   positive per-frame delta over a full single-click lifecycle).
//! - Under click-storm saturation the Mur boundary extrapolation can
//!   reflect a sliver of oblique energy back into the interior (a
//!   named, bounded reservoir, measured <= 0.031 per frame at 80x24
//!   saturated, decaying with the field). It is exposed as a slightly
//!   negative `dissipated` run — by name, never laundered.
//! - THE CAP: every term of `E` is a difference of values each bounded
//!   by `INJECT_CAP` (the amplitude ceiling the bounded-run test pins),
//!   so per-cell `E <= 2*cap^2 + 2*(rx^2+ry^2)*cap^2 <= 2.5*cap^2`
//!   under the CFL-margin law — under ANY click storm.
//! - SETTLE: the settle sweep lands `E` at exactly zero; across a full
//!   lifecycle `dissipated == injected` (all energy accounted).
//! - AND THE LEDGER EARNS ITS KEEP: measuring the functional is what
//!   caught the alternating-h instability (see `step` — the quantum
//!   clock it forced is a stability fix, not bookkeeping).
//!
//! The functional is measured in f64 over the f32 state (rounding at
//! ~1e-13, far below any law's slack); the per-frame pass and the
//! per-inject touched-set delta are closed-form and allocate nothing.
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

/// The energy ledger's reference Courant number (columns): the
/// sub-step-ceiling scale `c * MOUSE_FLASH_DT_MAX`. At the pinned
/// 60 fps schedule every sub-step runs at exactly this `h`, so the
/// ledger's fixed-scale functional IS the scheme's natural functional
/// there (zero rescale).
const LEDGER_RX: f32 = MOUSE_FLASH_SPEED * MOUSE_FLASH_DT_MAX;

/// The ledger's reference Courant number (rows): `LEDGER_RX` divided
/// by the cell aspect (`dy = CELL_ASPECT * dx`).
const LEDGER_RY: f32 = LEDGER_RX / CELL_ASPECT;

/// The named per-cell energy cap: every term of the ledger functional
/// is a squared difference of values each bounded by `INJECT_CAP` (the
/// amplitude ceiling the bounded-run test pins), so per cell
/// `E <= 2*cap^2` (kinetic) `+ 2*(rx^2+ry^2)*cap^2` (both cross
/// directions) `<= 2.5*cap^2` under the CFL-margin law
/// (`rx^2 + ry^2 <= 0.25`, pinned by the CFL lock test). Holds at ANY
/// pacing under ANY click storm — the cap is value-bounded, not
/// schedule-bounded.
#[cfg_attr(not(test), allow(dead_code))] // named cap constant: pinned by the Z6 lock tests
pub(crate) const WAVE_ENERGY_CAP_PER_CELL: f64 = 2.5 * (INJECT_CAP * INJECT_CAP) as f64;

/// The explicit energy ledger: the named, running account of the wave
/// field's discrete Lyapunov functional. Buckets (all f64 over the f32
/// state, exact attribution, zero allocation):
/// - `energy`: the current value of the functional.
/// - `injected`: cumulative SIGNED energy attributed to click
///   injections (the seed packet raises it; the inject clamp can
///   partially cancel an already-saturated cell, so the sign is kept,
///   never laundered).
/// - `dissipated`: cumulative energy removed by solver steps
///   (positive = removed; a slightly negative run marks the Mur
///   boundary reflection reservoir, exposed by name rather than
///   hidden).
/// - `rescale`: measurement-scale reconciliation (identically zero
///   under the quantum clock — one fixed sub-step width — kept as the
///   named identity term so the law's arithmetic stays explicit).
/// - `peak`: the largest `energy` ever observed (session record).
/// - `frames`: frames accounted.
/// - `substeps`: quantum sub-steps executed (the wave's discrete
///   time in `MOUSE_FLASH_DT_MAX` units).
pub(crate) struct EnergyLedger {
    energy: f64,
    injected: f64,
    dissipated: f64,
    rescale: f64,
    peak: f64,
    frames: u64,
    substeps: u64,
}

impl EnergyLedger {
    /// The fresh ledger of a zero field (exactly zero energy).
    const fn fresh() -> EnergyLedger {
        EnergyLedger {
            energy: 0.0,
            injected: 0.0,
            dissipated: 0.0,
            rescale: 0.0,
            peak: 0.0,
            frames: 0,
            substeps: 0,
        }
    }

    /// Attribute an injection's exact local energy delta (touched-set
    /// closed form; signed — see the field docs).
    fn account_inject(&mut self, delta: f64) {
        self.injected += delta;
        self.energy += delta;
        if self.energy > self.peak {
            self.peak = self.energy;
        }
    }

    /// Account one frame. `rescale` is the measurement-scale
    /// reconciliation (pre-frame reading in this frame's sub-step
    /// scale minus the book value); `pre` and `after` are the
    /// functional's readings in the SAME (this frame's) scale, so
    /// `pre - after` is the honest per-frame Lyapunov delta with the
    /// rescale kept OUT of the dissipation bucket. `ran` is the
    /// quantum sub-step count this frame.
    fn account_frame(&mut self, rescale: f64, pre: f64, after: f64, ran: u32) {
        self.rescale += rescale;
        self.dissipated += pre - after;
        self.energy = after;
        if self.energy > self.peak {
            self.peak = self.energy;
        }
        self.frames += 1;
        self.substeps += ran as u64;
    }

    /// Read-only introspection (ledger law tests). Dead in release
    /// builds — the ledger is instrumentation, the render path never
    /// reads it.
    #[cfg_attr(not(test), allow(dead_code))] // ledger introspection: pinned by the Z6 lock tests
    pub(crate) fn energy(&self) -> f64 {
        self.energy
    }

    #[cfg_attr(not(test), allow(dead_code))] // ledger introspection: pinned by the Z6 lock tests
    pub(crate) fn injected(&self) -> f64 {
        self.injected
    }

    #[cfg_attr(not(test), allow(dead_code))] // ledger introspection: pinned by the Z6 lock tests
    pub(crate) fn dissipated(&self) -> f64 {
        self.dissipated
    }

    #[cfg_attr(not(test), allow(dead_code))] // ledger introspection: pinned by the Z6 lock tests
    pub(crate) fn rescale(&self) -> f64 {
        self.rescale
    }

    #[cfg_attr(not(test), allow(dead_code))] // ledger introspection: pinned by the Z6 lock tests
    pub(crate) fn peak(&self) -> f64 {
        self.peak
    }

    #[cfg_attr(not(test), allow(dead_code))] // ledger introspection: pinned by the Z6 lock tests
    pub(crate) fn frames(&self) -> u64 {
        self.frames
    }

    #[cfg_attr(not(test), allow(dead_code))] // ledger introspection: pinned by the Z6 lock tests
    pub(crate) fn substeps(&self) -> u64 {
        self.substeps
    }
}

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
    /// Unconsumed simulation time (seconds) — the quantum clock's
    /// accumulator. The field advances in EXACT `MOUSE_FLASH_DT_MAX`
    /// quanta (see `step`): fractional frame deltas accumulate until
    /// they complete another sub-step. Reset by the settle sweep and
    /// the lifecycle-reset surfaces.
    acc: f32,
    /// The explicit Lyapunov ledger (see the module docs): the bound
    /// is named, measured, and pinned by the Z6 lock tests.
    ledger: EnergyLedger,
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
            acc: 0.0,
            ledger: EnergyLedger::fresh(),
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
        self.acc = 0.0;
        self.ledger = EnergyLedger::fresh();
    }

    /// Zero the field and mark it settled (scene switch, pause safety).
    /// A full lifecycle reset: the ledger restarts too (a zero field
    /// holds exactly zero energy).
    pub(crate) fn clear(&mut self) {
        self.zero_buffers();
        self.ledger = EnergyLedger::fresh();
    }

    /// Zero the field buffers without touching the ledger — the settle
    /// sweep's path (the ledger keeps the session record: energy lands
    /// at exactly zero, the buckets hold the full lifecycle).
    fn zero_buffers(&mut self) {
        for v in self.cur.iter_mut() {
            *v = 0.0;
        }
        for v in self.prev.iter_mut() {
            *v = 0.0;
        }
        self.settled = true;
        self.acc = 0.0;
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

    /// The ledger's read-only view (Z6 lock tests). Dead in release
    /// builds — instrumentation, never read by the render path.
    #[cfg_attr(not(test), allow(dead_code))] // ledger introspection: pinned by the Z6 lock tests
    pub(crate) fn ledger(&self) -> &EnergyLedger {
        &self.ledger
    }

    /// Inject a click impulse at `(col, line)` — the wave equation's
    /// initial condition. The seed is a compact symmetric packet on
    /// the clicked cell and its 4-neighbors, additive up to the cap
    /// (superposition without blowup). Out-of-range clicks clamp into
    /// the grid. The energy ledger takes the exact touched-set delta
    /// (closed form over the seed cells and their incident edges — no
    /// grid pass, zero allocation).
    pub(crate) fn inject(&mut self, col: u16, line: u16) {
        if self.cols < 1 || self.lines < 1 {
            return;
        }
        let col = (col as usize).min(self.cols - 1);
        let line = (line as usize).min(self.lines - 1);
        let idx = line * self.cols + col;
        // The touched set: the seed cell + in-range 4-neighbors (at
        // most 5), sorted ascending so the edge-delta pass can dedup
        // shared edges by endpoint order.
        let mut touched = [idx; 5];
        let mut n = 1usize;
        if col > 0 {
            touched[n] = idx - 1;
            n += 1;
        }
        if col + 1 < self.cols {
            touched[n] = idx + 1;
            n += 1;
        }
        if line > 0 {
            touched[n] = idx - self.cols;
            n += 1;
        }
        if line + 1 < self.lines {
            touched[n] = idx + self.cols;
            n += 1;
        }
        touched[..n].sort_unstable();
        let e_before = self.inject_local_energy(&touched[..n]);
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
        let e_after = self.inject_local_energy(&touched[..n]);
        self.ledger.account_inject(e_after - e_before);
        self.settled = false;
    }

    /// The ledger functional restricted to the touched set and its
    /// incident edges (the exact local piece the inject can move —
    /// everything else is untouched, so its contribution to the DELTA
    /// cancels). Fixed `LEDGER_RX/RY` scale; f64 accumulation. Each
    /// edge between two touched cells is counted once (from its lower
    /// endpoint — the caller passes the set sorted ascending).
    fn inject_local_energy(&self, touched: &[usize]) -> f64 {
        let (cols, lines) = (self.cols, self.lines);
        let rx2 = (LEDGER_RX * LEDGER_RX) as f64;
        let ry2 = (LEDGER_RY * LEDGER_RY) as f64;
        let mut e = 0.0f64;
        for &i in touched {
            let d = (self.cur[i] - self.prev[i]) as f64;
            e += 0.5 * d * d;
        }
        for (k, &i) in touched.iter().enumerate() {
            let (c, l) = (i % cols, i / cols);
            // The four incident edges, guarded per direction (no eager
            // arithmetic — corner/edge cells must not compute
            // out-of-range partners). Coefficients: rx^2 horizontal,
            // ry^2 vertical.
            let mut dirs: [Option<(usize, f64)>; 4] = [None; 4];
            if c > 0 {
                dirs[0] = Some((i - 1, rx2));
            }
            if c + 1 < cols {
                dirs[1] = Some((i + 1, rx2));
            }
            if l > 0 {
                dirs[2] = Some((i - cols, ry2));
            }
            if l + 1 < lines {
                dirs[3] = Some((i + cols, ry2));
            }
            for dir in dirs.iter().flatten() {
                let (p, k2) = *dir;
                // Dedup shared edges: count only from the lower
                // endpoint (a touched partner below `i` already counted
                // this edge in an earlier iteration).
                if touched[..k].contains(&p) {
                    continue;
                }
                let dc = (self.cur[p] - self.cur[i]) as f64;
                let dp = (self.prev[p] - self.prev[i]) as f64;
                e += 0.5 * k2 * dc * dp;
            }
        }
        e
    }

    /// Advance the field by `dt` seconds of real frame time. The wave
    /// runs on a QUANTUM CLOCK: every sub-step is EXACTLY
    /// `MOUSE_FLASH_DT_MAX` wide and `dt` accumulates until it
    /// completes another quantum (capped at 8 per frame — a
    /// pathological stall advances the wave by a bounded amount, never
    /// heats the CPU; the unconsumed remainder carries over). No
    /// allocation; a settled field short-circuits to zero work.
    ///
    /// The fixed sub-step width is a STABILITY LAW, not a convenience:
    /// the three-level leapfrog's amplification factor is bounded only
    /// for a CONSTANT `h` — alternating sub-step widths (frame jitter,
    /// throttle cycles) compose amplification matrices whose products
    /// resonate (a parametric pump the Z6 energy ledger caught
    /// empirically: exponential growth under a synthetic 4-step pacing
    /// cycle, absent at any constant pacing). Quantizing the schedule
    /// to one width eliminates the alternating-h class BY CONSTRUCTION
    /// — every frame runs the exact scheme the von Neumann and damping
    /// analysis pins, and the visible timing error is bounded by one
    /// quantum (8.3 ms on a 1.8 s life, invisible).
    ///
    /// The energy ledger accounts every frame that ran at least one
    /// quantum, measuring in the ONE fixed scale (the ledger's
    /// reference Courant numbers — the rescale bucket stays exactly
    /// zero by construction).
    pub(crate) fn step(&mut self, dt: f32) {
        if self.settled || dt <= 0.0 || self.cols < 3 || self.lines < 3 {
            return;
        }
        self.acc += dt;
        // The measurement scale is the ledger's reference scale (every
        // sub-step is exactly MOUSE_FLASH_DT_MAX wide).
        let rx2 = (LEDGER_RX * LEDGER_RX) as f64;
        let ry2 = (LEDGER_RY * LEDGER_RY) as f64;
        let pre = self.frame_energy(rx2, ry2);
        let rescale = pre - self.ledger.energy;
        let mut ran = 0u32;
        while self.acc >= MOUSE_FLASH_DT_MAX && ran < 8 {
            self.step_once(MOUSE_FLASH_DT_MAX);
            self.acc -= MOUSE_FLASH_DT_MAX;
            ran += 1;
        }
        if ran == 0 {
            // Not enough accumulated time for a full quantum — the
            // state is untouched, the ledger carries over (no
            // measurement, no bookkeeping churn).
            return;
        }
        if self.peak_amplitude() < MOUSE_FLASH_SETTLE_EPS {
            self.zero_buffers();
        }
        let after = self.frame_energy(rx2, ry2);
        self.ledger.account_frame(rescale, pre, after, ran);
    }

    /// The ledger's discrete Lyapunov functional over the whole grid
    /// (kinetic at level n-1/2 plus the cross-time-level gradient
    /// products; see the module docs for the derivation). f64
    /// accumulation over the f32 state, single pass, zero allocation.
    /// The squared Courant numbers arrive as parameters so the frame
    /// accounting measures in the frame's own sub-step scale.
    fn frame_energy(&self, rx2: f64, ry2: f64) -> f64 {
        let (cols, lines) = (self.cols, self.lines);
        let mut e = 0.0f64;
        // Kinetic: (1/2) sum (cur - prev)^2 over all cells.
        for i in 0..self.cur.len() {
            let d = (self.cur[i] - self.prev[i]) as f64;
            e += 0.5 * d * d;
        }
        // Horizontal cross-terms (cols-1 edges per row).
        for line in 0..lines {
            let row = line * cols;
            for col in 0..cols - 1 {
                let i = row + col;
                let dc = (self.cur[i + 1] - self.cur[i]) as f64;
                let dp = (self.prev[i + 1] - self.prev[i]) as f64;
                e += 0.5 * rx2 * dc * dp;
            }
        }
        // Vertical cross-terms (lines-1 edges per column).
        for line in 0..lines - 1 {
            let row = line * cols;
            for col in 0..cols {
                let i = row + col;
                let dc = (self.cur[i + cols] - self.cur[i]) as f64;
                let dp = (self.prev[i + cols] - self.prev[i]) as f64;
                e += 0.5 * ry2 * dc * dp;
            }
        }
        e
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
