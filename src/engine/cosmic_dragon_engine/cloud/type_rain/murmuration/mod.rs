// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#![allow(clippy::module_inception)]

//! Murmuration rain (NIGHT-research-7, the twelfth style): the rain
//! is a flock.
//!
//! The scene reads as a starling murmuration wheeling over a dark
//! sky: hundreds of glyph birds — nabla strokes, one per bird —
//! flying as one shape-shifting body. The flock tightens into a
//! ball, pours across the sky as a ribbon, loosens into a wide
//! cloud, and blooms apart when a predator startles it, then
//! re-gathers: the classic murmuration behaviors, none of them
//! drawn — all of them emergent from local rules. The sky is the
//! terminal; the birds are glyphs; the neighbor graph is a spatial
//! hash on the cell grid (the physarum precedent: the terminal's
//! discrete geometry IS the simulation substrate).
//!
//! ## Origin
//!
//! NIGHT-research-7 (the owner's DeepSeek-researched shortlist,
//! second pick): the murmuration is a canonical system — Reynolds
//! 1987 boids (separation, alignment, cohesion), refined by the
//! starling-field findings (Ballerini's topological interaction:
//! each starling tracks its ~7 nearest neighbors, not a fixed
//! metric ball; Cavagna's scale-free correlation: one bird's turn
//! propagates through the whole flock). Like the DNA helix (the
//! shortlist's first pick), this style borrows a canonical
//! reference; the original-math line stays with aeolian and the
//! corona. What IS derived here is the terminal mapping: the
//! neighbor radius sized to the topological number on a cell grid,
//! the spatial hash that makes O(n) neighbors out of O(n^2) pairs,
//! the roaming anchor that gives the flock its macro intent, the
//! cohesion breathing that produces the signature
//! tighten-and-loosen shape cycles, and the startle clock — the
//! predator event that turns the flock inside out.
//!
//! ## The flock equations (the five laws)
//!
//! The flock is N birds (viewport-derived, density-dialed), each
//! one a glyph carrier with a position, a velocity and nothing
//! else — no leader, no global plan. Every bird sees only its
//! neighbors (the hash window), and every force is local.
//!
//! ### Law 1 — the three forces (Reynolds 1987)
//!
//! Each bird steers by the classic triad, integrated per sim-tick
//! on the family clock:
//!
//! - Separation: steer away from neighbors inside the tight
//!   radius, weighted inverse-distance (birds never overlap — the
//!   minimum spacing IS the visual bird density).
//! - Alignment: steer toward the neighbors' mean heading (the
//!   local velocity match that makes a hundred strokes read as
//!   ONE body pouring through the sky).
//! - Cohesion: steer toward the neighbors' centroid (a weak
//!   spring — the local attraction that keeps the flock one
//!   flock).
//!
//! A small per-bird jitter acceleration (a clamped random walk)
//! keeps the system organic — no dead-locked symmetric
//! configurations — and the speed clamps [V_MIN, V_MAX] keep every
//! bird flying: a starling never hovers, never teleports.
//!
//! ### Law 2 — the neighbor window (the topological homage)
//!
//! The alignment/cohesion radius is sized so the average bird
//! scans ~7-10 neighbors — Ballerini's topological number (~7),
//! measured on the cell grid: at the shipped population dial the
//! hash window reproduces the interaction topology the real
//! flocks carry. The window is the spatial hash's 3x3 bucket
//! scan: birds bucket by (floor(x / R), floor(y / R)) and each
//! bird reads only its 9 buckets — O(n) pair checks per frame
//! instead of O(n^2), the reason a 200-bird flock costs the same
//! as a 30-bird naive scan.
//!
//! ### Law 3 — the thought (the roaming anchor)
//!
//! The flock's collective intent is an anchor point that random-
//! walks across the sky (a clamped drift that re-rolls its target
//! every few seconds and reflects off the walls). Every bird
//! carries a WEAK attraction to it — weak against the local
//! triad, strong against nothing: it is what keeps the flock a
//! traveling shape instead of a static blob, and what makes the
//! flock pour along the screen edges when the anchor roams near
//! them (the wall banking: birds near the margins steer inward
//! along a soft margin force — they curve along the edge, never
//! hit it).
//!
//! ### Law 4 — the breathing (the shape cycle)
//!
//! The cohesion weight itself breathes on a slow oscillator: the
//! flock periodically tightens into a dense ball and loosens into
//! a wide cloud — THE signature murmuration shape-shift, emergent
//! from one scalar changing on a sine. The oscillator period is
//! several sim-seconds with a rolled phase offset, so the
//! tightening never syncs with the startle clock.
//!
//! ### Law 5 — the startle (the predator)
//!
//! On a global clock (mean several sim-seconds, variance banded),
//! a predator flashes at a rolled position: one glyph at Core
//! brightness for the flash window, and every bird inside the
//! panic radius takes an outward velocity kick (an impulse, not a
//! force — the scatter is instant), scaled by proximity. The
//! flock blooms apart and re-gathers under the triad — the drama
//! event, the flare eruption's heir. The panic also floors the
//! affected birds' speed near V_MAX for a moment: the scatter
//! reads fast because it IS fast.
//!
//! ## Stability
//!
//! Every state variable is bounded by direct construction: the
//! pool is viewport-derived and clamped; positions clamp inside
//! the viewport by the wall banking (plus a hard re-project
//! backstop that never fires in practice — the banking turns
//! before the margin); velocities clamp inside [V_MIN, V_MAX]
//! after every force integration; the anchor's drift clamps and
//! reflects; the startle impulse lands on the velocity BEFORE the
//! clamp (a kick can saturate V_MAX, never exceed it); the
//! breathing oscillator is a bounded sine; the spatial hash's
//! bucket counts are viewport-derived (no allocation in the
//! advance loop — the bucket store is rebuilt per frame from a
//! persistent allocation).
//!
//! ## FPS invariance
//!
//! Every rate (the force magnitudes, the speed clamps, the anchor
//! drift, the breathing oscillator, the startle clock) is
//! expressed per SIM-second and integrated with the clamped dt on
//! the single family clock (dt_sim = dt_wall x chars_per_sec x
//! SIM_TIME_PER_CPS, eased by resume_blend). The force pass is a
//! semi-implicit Euler (velocity first, position second, both
//! clamped) — stable at any dt the cap admits, and the bench's
//! uniform stepping exercises one advance + one draw per frame
//! exactly as the interactive loop does.
//!
//! ## Module map
//!
//! - `boids.rs` — the bird: the state struct, the spatial hash
//!   (the neighbor window), and the three forces + walls +
//!   anchor + startle steering (laws 1-3, 5 executable form).
//! - `murmuration.rs` — the flock: the pool, the staggered-entry
//!   spawn, the advance pass (hash build, force integration, the
//!   breathing oscillator, the startle clock), the test
//!   diagnostics.
//! - `draw.rs` — the draw pass (birds + comet trails + the
//!   predator flash, drawn-cell diff cleanup, the speed ladder).
//! - `mod.rs` — this derivation.
//!
//! The draw pass is small enough to stay with the orchestration
//! family split convention: `boids.rs` carries the physics, the
//! orchestration and draw files share the render contract.

pub(crate) mod boids;
pub(crate) mod draw;
pub(crate) mod murmuration;

pub(crate) use boids::BirdRandom;
pub(crate) use murmuration::{MurmSpawnParams, MurmStep, MurmurationRain};
