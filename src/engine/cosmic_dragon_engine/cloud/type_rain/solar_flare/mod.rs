// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#![allow(clippy::module_inception)]

//! Solar flare rain (NIGHT-special-4): the rain rides the magnetism.
//!
//! The scene reads as a star's limb. A granulated photosphere glows
//! along the bottom edge — cells flickering like convection grit.
//! Up from it arc the coronal loops: parabolic glyph filaments, each
//! rooted at two footpoints on the surface, breathing in width and
//! height while the whole arcade drifts on a slow differential wind.
//! The rain is the corona's own: glyph drops condense near the loop
//! tops, slide down the legs with energy-conserving acceleration, and
//! land at the footpoints, flashing them — the deposition heating.
//! The light in this sky is where the rain has been landing: a
//! rain-fed loop climbs the brightness ladder until it can no longer
//! hold. Then the flare: the loop stretches, sprays ejecta, tears
//! free of the surface and dissolves upward — and a fresh arc emerges
//! from the photosphere in its place. The magnetic carpet never
//! stops re-weaving itself.
//!
//! ## Origin (the invention directive, third generation)
//!
//! NIGHT-special-2 (the aeolian weave) established the house rule:
//! flagship styles carry motion DNA derived in this repo, carrying no
//! existing mathematical reference — the LEAP-engine spirit. The
//! aurora veil (NIGHT-special-3, owner-rated 5/10 and retired at the
//! owner's direction) carried it second. The corona arcade carries it
//! third. Its relatives all borrow canonical systems (Lorenz 1963,
//! Kepler, FABRIK, Jones 2010, PIC/FLIP, inverse-square gravity, the
//! aeolian hop-clock, the aurora ray lattice). The corona carries a
//! new one: a parabolic flux-tube arcade with a closed-form
//! along-arc energy descent and a flux-threshold destabilization
//! cycle — an equation set no textbook carries, tuned by derivation
//! for the terminal's discrete cell grid.
//!
//! ## The corona equations (the five laws)
//!
//! The star is an arcade of N loops (N = cols / 10, clamped). Each
//! loop carries a center column cx, a footpoint span w (the two feet
//! sit at cx - w/2 and cx + w/2), an apex height h in lines, a
//! deposited flux (heat), and a lifecycle phase. The weather is the
//! drop pool (the family lane model, one slot per column).
//!
//! ### Law 1 — the magnetic carpet (footpoint order)
//!
//! The arcade self-organizes: facing footpoints of adjacent loops
//! repel with an inverse-gap force (gain / gap, floored), so loops
//! never merge into one; every loop's drift relaxes toward a global
//! wind that re-rolls every few seconds and is eased toward
//! exponentially; the footpoint span w breathes toward an anchor
//! re-rolled inside [W_MIN, W_MAX] on a rolled dwell, gliding
//! exponentially (an anchor resolve reads as the loop slowly
//! fattening or slimming, never snapping). Positions hard-clamp with
//! damped wall reflection. The lifecycle is the carpet's turnover:
//! EMERGING (the arc grows out of the surface over ~1 s), STABLE
//! (the quiet corona), ERUPTING (law 4), DETACHING (the lifted arc
//! rises and dissolves), then re-EMERGING elsewhere — flux
//! emergence, submergence, and re-emergence, the solar magnetic
//! cycle in miniature.
//!
//! ### Law 2 — the coronal condensation (energy-conserving descent)
//!
//! A drop condenses near a loop's apex (s in a small window around
//! 0.5, weighted toward hot loops by tournament selection) and rides
//! one leg down toward its footpoint. The loop is a parabola: the
//! arc's height above the surface at parameter s is h x 4s(1-s), so
//! the drop's height loss from the apex is h x (2|s - 0.5|)^2.
//! Energy conservation from a thermal kick v0 gives the closed-form
//! along-arc speed:
//!
//! v(s) = sqrt(v0^2 + 2 x LEG_G x h x (2|s - 0.5|)^2)
//!
//! — bounded by construction (v <= sqrt(v0^2 + 2 x LEG_G x H_MAX)),
//! exact at any dt (no integrator drift), and monotonically
//! accelerating down the leg. The arc-length metric
//! sqrt(w^2 + 16 h^2 (1-2s)^2) converts v to parameter motion:
//! ds/dt = v / metric(s) — the drop leaves the apex lazily and
//! arrives at the footpoint at full speed. The landing test is a
//! STATE test (s <= 0 or s >= 1 with v >= v0 > 0: s motion is
//! strictly monotone toward the chosen foot), not a crossing test —
//! no tunneling at any dt, any frame rate.
//!
//! ### Law 3 — the footpoint deposition (bounded flux)
//!
//! A drop landing at its footpoint deposits its kinetic charge
//! (proportional to its arrival speed) into the loop's flux. Flux
//! decays exponentially (law 3a) and is hard-clamped at FLUX_MAX
//! (law 3b): bounded by construction, no contraction proof needed.
//! The loop's ladder reads the flux: Ghost is the quiet corona's
//! baseline dim, Mid a rained-on loop, Hot a heavily-fed arcade
//! member, Core the flaring window. While a footpoint's flash is
//! fresh (flare_age < FLASH_SECS) its cells read one rung hotter —
//! the landing punch, exactly where the drop was absorbed.
//!
//! ### Law 4 — the flare eruption (flux-threshold destabilization)
//!
//! A loop whose flux crosses the ERUPT threshold destabilizes when
//! the global flare clock allows it (the cadence gate: mean several
//! sim-seconds, at most one erupting loop at a time — a flare is a
//! singular event, never a chorus). The eruption cycle runs in
//! phases: ERUPTING (~1.4 s) — the apex target stretches by the
//! STRETCH factor, the whole arc reads Core-then-Hot, riders are
//! flung as ejecta and a burst spawns at the apex; DETACHING
//! (~1.8 s) — the arc translates upward at the lift rate while its
//! cells step down the ladder and dim to nothing; then re-EMERGING
//! with fresh geometry, flux reset to zero. Ejecta fly ballistic
//! arcs under a stellar gravity decelerating the rise, fading by
//! age, splashing a small heat pulse into the granule they fall
//! back on.
//!
//! ### Law 5 — the shimmer law (glyph identity)
//!
//! Field cells (arc filaments, granulation) keep the glyph the
//! frame already carries (the fabric identity — the corona is a
//! structure, not a cascade); fresh cells pick from the pool.
//! Re-rolls ladder with heat: the quiet corona shimmers rarely (a
//! slow coronal flicker), hot loops and the photosphere granules
//! flicker harder. Riding heads re-roll matrix-style as they cross
//! into new cells — the family shimmer contract.
//!
//! ## Stability
//!
//! Every state variable is bounded by direct construction, not by
//! tuning: loop count is viewport-derived and clamped; drift clamps
//! at DRIFT_MAX; centers hard-clamp inside the footpoint margins
//! with damped wall reflection; spans clamp inside [W_MIN, W_MAX];
//! heights clamp inside [H_MIN, H_CAP]; flux decays strictly and
//! clamps at FLUX_MAX; the along-arc speed is a closed form bounded
//! by sqrt(v0^2 + 2 LEG_G H_CAP); s motion is monotone with the
//! landing state test; ejecta are age-capped with wall and surface
//! kills; the drop pool is lane-bounded with a lifetime backstop.
//! The flare clock bounds the eruption cadence globally.
//!
//! ## FPS invariance
//!
//! Every rate (wind relaxation, repulsion, breath glides, flux
//! decay, phase clocks, ejecta gravity, lifetime) is expressed per
//! SIM-second and integrated with the clamped dt on the single
//! family clock (dt_sim = dt_wall x chars_per_sec x
//! SIM_TIME_PER_CPS, eased by resume_blend). The riding speed is a
//! closed form of s (dt-independent); the s integration and the
//! ejecta ballistics are smooth integrators quantized to the cell
//! grid only at the draw boundary — the average speeds are exact
//! wherever dt is clamped (the family contract). The bench's
//! uniform stepping exercises one advance + one draw per frame at
//! the standard cadence, the same deterministic-contract behavior
//! the visual metrics (density gini, frame entropy) are measured
//! against.
//!
//! ## Module map
//!
//! - `loops.rs` — the star: the arcade state machine, the arc
//!   geometry (parabola sampling + metric), the carpet forces, the
//!   flux ladder, and the granulation surface (laws 1, 3, 4
//!   executable form).
//! - `drops.rs` — the weather: the drop state struct, riding and
//!   ejecta modes, the kinetic ladder (law 2 state carrier).
//! - `solar_flare.rs` — the orchestration: pool, spawn
//!   accumulator, advance (law 2 physics + law 3 deposition + law
//!   4 ejecta), and the test diagnostics.
//! - `draw.rs` — the draw pass (surface + arcs + rain + ejecta,
//!   drawn-cell diff cleanup, law 5) — split from the orchestration
//!   at the 800-LOC hard cap (the monolith family's split pattern).

pub(crate) mod draw;
pub(crate) mod drops;
pub(crate) mod loops;
pub(crate) mod solar_flare;

pub(crate) use loops::SolarRandom;
pub(crate) use solar_flare::{SolarFlareRain, SolarFlareSpawnParams, SolarFlareStep};
