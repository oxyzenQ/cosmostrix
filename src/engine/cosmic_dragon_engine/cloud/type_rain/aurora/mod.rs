// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Aurora rain (NIGHT-special-3): the rain paints the light.
//!
//! The scene reads as a polar night sky. Vertical curtains of glyphs
//! hang from the top edge — each one carried by an invisible "ray
//! bead" drifting near the top of the sky. Down from a bead descends
//! its ray: a column of glyphs whose lowest cell (the fringe) is the
//! emission layer, and whose body fades upward into the dark. The
//! beads spread themselves across the sky, breathe their curtains
//! between two depth bands, and a slow global wind advects the whole
//! veil sideways. The rain is the precipitation: glyphs fall from the
//! top edge, funnel toward the brightest nearby fringe, and are
//! absorbed at its depth — their kinetic charge flares the fringe.
//! The light in this sky is literally where the rain has been
//! landing: the weather paints the curtain.
//!
//! ## Origin (the invention directive, second generation)
//!
//! NIGHT-special-2 (the aeolian weave) established the house rule:
//! flagship styles carry motion DNA derived in this repo, carrying no
//! existing mathematical reference — the LEAP-engine spirit. The
//! aurora continues that lineage. Its relatives all borrow canonical
//! systems (Lorenz 1963, Kepler, FABRIK, Jones 2010, PIC/FLIP,
//! inverse-square gravity, the aeolian hop-clock). The polar veil
//! carries a new one: a drifting flux-tube lattice with a quantized
//! two-band depth breath and a glow-weighted precipitation funnel —
//! an equation set no textbook carries, tuned by derivation for the
//! terminal's discrete cell grid.
//!
//! ## The veil equations (the five laws)
//!
//! The sky is a lattice of N ray beads (N = cols / 6, clamped).
//! Each bead carries a fractional column x, a drift velocity vx, an
//! emission depth D (lines from the top edge), and a fringe charge
//! (glow). The weather is the drop pool (the family lane model, one
//! slot per column).
//!
//! ### Law 1 — the ray lattice (flux-tube repulsion + wind)
//!
//! Beads push each other apart along the top edge: every adjacent
//! pair feels an inverse-gap repulsion (gain / gap, floored at
//! 2 columns), and every bead's velocity additionally relaxes toward
//! the global wind. The wind itself is a target that re-rolls every
//! few seconds (magnitude within the wind span) and is eased toward
//! exponentially. Velocities clamp, positions hard-clamp with a
//! damped wall bounce — the lattice can never bunch into a single
//! cluster and can never leave the screen. The emergent read: rays
//! spread across the sky with organic, uneven spacing, and the whole
//! veil drifts slowly one way, then the other.
//!
//! ### Law 2 — the substorm breath (quantized two-band depth)
//!
//! Each bead's emission depth glides exponentially toward a private
//! anchor, and the anchor flips between two disjoint bands (LOW:
//! 0.24-0.34 of the viewport height, HIGH: 0.46-0.60) on a rolled
//! dwell interval (mean 7 sim-seconds). The QUANTIZED two-band
//! choice is the signature: a continuous random walk would smear
//! curtain heights into a uniform mush, while two disjoint bands
//! make the veil read as layered curtains hanging at distinct
//! altitudes — the classic multiple-band aurora. An anchor flip is a
//! substorm onset: the curtain visibly glides up or down over about
//! a second (the exponential relaxation), never snapping.
//!
//! ### Law 3 — the precipitation funnel (glow-weighted seeking)
//!
//! A falling drop within SEEK_RANGE lines above the nearest ray's
//! depth accelerates laterally toward that ray's column, with a gain
//! scaled by the ray's glow (0.35 + 0.65 * min(1, glow / HOT)):
//! bright fringes attract the weather harder. The loop this closes
//! is the veil's self-organization — the fringe that flared collects
//! the next drops, and their charge keeps it bright: the sky
//! concentrates its light where the rain has been landing.
//!
//! ### Law 4 — the emission charge (bounded glow)
//!
//! A drop absorbed at a ray's depth deposits its accumulated kinetic
//! charge into that ray's glow. Glow decays exponentially (law 4a)
//! and is hard-clamped at GLOW_MAX (law 4b): bounded by
//! construction, no contraction proof needed. The fringe's
//! brightness ladder reads the glow: Mid at rest (the emission layer
//! baseline), Hot while charged, Core for a short flare window after
//! a fresh absorption (the landing punch). While Hot or brighter the
//! fringe spills into its flanking columns one rung dimmer — the
//! emission spreading along the layer.
//!
//! ### Law 5 — the shimmer law (glyph identity)
//!
//! Curtain cells keep the glyph the frame already carries (the
//! fabric identity — curtains are structures, not cascades); fresh
//! cells pick from the pool. Re-rolls ladder with brightness: the
//! body shimmers rarely (a slow air flicker), the fringe often (a
//! bright emission flickers hard). Falling heads re-roll
//! matrix-style as they cross into new cells — the family shimmer
//! contract.
//!
//! ## Stability
//!
//! Every state variable is bounded by direct construction, not by
//! tuning: bead count is viewport-derived and clamped; velocities
//! clamp at VX_MAX; positions hard-clamp inside [0, cols-1] with
//! damped wall reflection; depths hard-clamp inside
//! [DEPTH_MIN, lines-2] with anchors sampled inside the bands; glow
//! decays strictly and clamps at GLOW_MAX; the drop pool is
//! lane-bounded with gravity capped at a terminal velocity and a
//! lifetime backstop. Absorption is a STATE test (drop depth >=
//! emission depth), not a crossing test — as the drop's y only grows
//! and the depth is bounded, the test can only become more true: no
//! tunneling is possible at any dt, at any frame rate.
//!
//! ## FPS invariance
//!
//! Every rate (wind relaxation, repulsion, dwell, depth glide, glow
//! decay, gravity, seeking, charging) is expressed per SIM-second
//! and integrated with the clamped dt on the single family clock
//! (dt_sim = dt_wall * chars_per_sec * SIM_TIME_PER_CPS, eased by
//! resume_blend). The bead glide and the drop fall are smooth
//! integrators quantized to the cell grid only at the draw boundary
//! — the average speeds are exact wherever dt is clamped (the
//! family contract). The bench's uniform stepping exercises one
//! advance + one draw per frame at the standard cadence, the same
//! deterministic-contract behavior the visual metrics (density gini,
//! frame entropy) are measured against.
//!
//! ## Module map
//!
//! - `rays.rs` — the sky: the ray-bead lattice state, the wind, the
//!   repulsion pass, the substorm breath, the glow charge, and the
//!   nearest-ray query (laws 1, 2, 4 executable form).
//! - `drops.rs` — the weather: the drop state struct and its kinetic
//!   ladder (law 3 state carrier).
//! - `aurora.rs` — the orchestration: pool, spawn accumulator,
//!   advance (law 3 physics + law 4 absorption), and the draw pass
//!   (veil + rain + drawn-cell diff cleanup, law 5).

pub(crate) mod aurora;
pub(crate) mod drops;
pub(crate) mod rays;

pub(crate) use aurora::{AuroraRain, AuroraSpawnParams, AuroraStep};
pub(crate) use rays::AuroraRandom;
