// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#![allow(clippy::module_inception)]

//! Quasar rain (NIGHT-research-8, the thirteenth style): the rain
//! feeds the engine.
//!
//! The scene reads as the loudest object in the universe pinned to
//! the terminal's center: a supermassive black hole eating
//! ferociously. Cold glyph gas falls out of the dark, spirals into
//! a thin accretion disk of Keplerian orbits — the inner ring
//! lapping the outer six times over, one limb doppler-brightened
//! like the M87 photograph — while the core burns warm and
//! breathes (flashing white only on the flare event), and the
//! poles fire two collimated jets that precess,
//! pulse and knot. The matrix rain is reborn as accretion: every
//! falling glyph is fuel, and where it lands, the disk carries the
//! light of having been recently fed. The black hole rain style
//! (NIGHT-special-1) is the SAME engine silent; the quasar is it
//! running at full power — the sibling pair completes the story.
//!
//! ## Origin
//!
//! NIGHT-research-8 (the owner's pick over the neural-network
//! proposal): the quasar is a canonical object — the
//! supermassive-black-hole engine observed since 1963 (Schmidt's
//! redshift), its anatomy carried by four decades of accretion
//! physics (Shakura-Sunyaev disk thermodynamics, relativistic
//! beaming, Blandford-Znajek jets) and photographed at last by the
//! Event Horizon Telescope (M87 2019, Sgr A* 2022). Like the DNA
//! helix and the murmuration, this style borrows a canonical
//! reference; what IS derived here is the terminal mapping: the
//! disk's orbits as tilted ellipses on the cell grid with a
//! Keplerian omega ladder that makes the shear legible at 60 fps,
//! the doppler beaming collapsed to a brightness-rung asymmetry
//! (one limb hotter — mono-safe), the jets as recycling particle
//! streams riding a helix with a precessing axis, and the fuel
//! economy that turns the rain itself into the disk's growth and
//! light.
//!
//! ## The engine equations (the five laws)
//!
//! The engine is one center and four populations around it — every
//! particle a glyph carrier bound to the center by construction,
//! none of them free.
//!
//! ### Law 0 — the ignition (the birth)
//!
//! The style does not enter with the engine already burning: the
//! entry replays the ignition in four continuous phases (the DNA
//! genesis contract — the black hole formation precedent's heir).
//! The dark cloud falls first (the thick cold infall, nothing
//! lit), then the disk condenses as captured streamers
//! circularize onto their orbits, then the core ignites — first
//! light — and the luminosity ramps while the doppler asymmetry
//! fades in, then the jets push out to full length. A pure resize
//! keeps the burning engine; only a scene entry re-ignites. The
//! clock rides sim-time (the family speed contract scales the
//! birth with the engine).
//!
//! ### Law 1 — the engine (the core)
//!
//! The center: one cell of glyph at the soft warm ceiling
//! (NIGHT-research-14 — the black hole's soft-light ruling
//! ported to this family: the standing heart reads the palette's
//! bright stop without the Core white blend, breathing on a slow
//! pulse, the luminosity's bounded sine), wrapped in a small
//! glow ring that follows the pulse. On the feed-flare clock a
//! clump arrives — the core locks above its peak for the window
//! (the engine's one event-gated Core flash, the same moment the
//! glyph re-rolls), the infall surges, and a knot climbs each
//! beam: the drama event, the murmuration startle's heir. The
//! core's glyph re-rolls only on flare fire (event-gated mutation
//! — the family contract).
//!
//! ### Law 2 — the disk (Kepler + doppler)
//!
//! The accretion disk: particles on near-circular orbits of
//! fraction f in [DISK_INNER, 1] of the disk axis, each advancing
//! on omega = K / f^1.5 — Kepler's third law, so the inner edge
//! laps the outer ~6x and the shear IS the rotation read. The
//! orbit projects to a thin ellipse (the disk tilt — the classic
//! quasar photograph's geometry). The radial temperature ladder
//! maps the orbit to brightness (warm-bright inner edge, dim rim,
//! composed under the soft-light ceiling), and the doppler beaming
//! splits it: the limb where the plasma
//! approaches swings the brightness factor up and lifts the rung
//! one step, the receding limb dims a step — mono-safe, the M87
//! signature. Captured gas circularizes by exponential damping
//! onto a target orbit (the disk assembles without a pop).
//!
//! ### Law 3 — the fuel (the rain is accretion)
//!
//! The infall: glyph streamers spawn outside the halo and spiral
//! inward, winding tighter and plunging faster as the gravity
//! tightens (the rate carries the 1/f^2 acceleration of free
//! fall). A streamer that reaches the disk's outer edge is
//! absorbed: while the disk is below its target population the
//! capture births a new disk particle (charged to full — the
//! fresh-write economy, the DNA rung charge's heir); once the disk
//! is at target the capture re-charges the nearest-orbit particle
//! instead. The charge decays exponentially — the light on the
//! disk shows where the engine has been recently fed. This is the
//! matrix rain's second life: the falling glyph is no longer the
//! scene, it is the scene's food.
//!
//! ### Law 4 — the jets (the exhaust)
//!
//! Two beams fire from the poles: per beam a recycling population
//! of particles riding s in [0, 1] (0 at the core, 1 at the tip)
//! with a linearly accelerating speed v(s) = V0 (1 + ACC s) — the
//! relativistic read, the tip outracing the collar. The beam's
//! particles ride a helix around the precessing axis (the beams
//! wobble on a slow cone — a full sweep every ~28 sim-seconds),
//! and the brightness ladder runs the energy density: hottest at
//! the launch collar, dim at the tip. A flare launches a knot — a
//! brightness pulse that travels the beam at its own speed, and
//! every particle inside the knot's window reads a rung hotter
//! (the engine's heartbeat visible up the exhaust).
//!
//! ### Law 5 — the glow (the halo)
//!
//! The host halo: a sparse ring of dim particles orbiting far
//! outside the disk (rounder projection — the halo is not flat),
//! gliding in from off-screen at entry (no pop: the halo births
//! itself from beyond the frame). The halo breathes with the
//! core's pulse — when the engine flares, the whole host glows a
//! rung brighter. The quasar is the one object in the catalog
//! whose light is visible in every population at once.
//!
//! ## Stability
//!
//! Every state variable is bounded by direct construction: the
//! partitioned pool is viewport-derived and clamped; the orbit
//! fractions clamp inside [DISK_INNER, 1] by construction
//! (circularization only moves f toward its rolled target); the
//! jet s wraps at the beam's end (the beam recycles, it never
//! accumulates); the infall rate is positive by construction and
//! the streamer absorbs at the disk edge (never crosses the
//! center); the angles wrap at 64 turns (f32 precision kept); the
//! pulse, precession and flare clocks are bounded sines and
//! re-arming timers; the charge decays exponentially toward zero
//! from its hard cap of 1. Positions project inside the viewport
//! by the geometry caps computed at reset (the disk axis clamps
//! against the tilt-projected height, the jet length against the
//! polar clearance).
//!
//! ## FPS invariance
//!
//! Every rate (the Kepler angular velocities, the infall
//! acceleration, the jet speeds, the precession, the pulse, the
//! flare and knot clocks, the ignition timeline) is expressed per
//! SIM-second and integrated with the clamped dt on the single
//! family clock (dt_sim = dt_wall x chars_per_sec x
//! QUAS_SIM_TIME_PER_CPS, eased by resume_blend). The physics is
//! a first-order drift per population (angle += omega dt, f +=
//! df dt, s += v dt) — stable at any dt the cap admits, and the
//! bench's uniform stepping exercises one advance + one draw per
//! frame exactly as the interactive loop does.
//!
//! ## Module map
//!
//! - `particles.rs` — the particle (the four populations' shared
//!   state), the geometry queries (the center, the disk axis, the
//!   jet length), and the per-kind physics steps (laws 1-5
//!   executable form).
//! - `capture.rs` — the capture economy (law 3's accounting): the
//!   free-slot scans and the absorption handler that turns the
//!   falling rain into the disk.
//! - `quasar.rs` — the engine: the four pools, the staggered
//!   infall spawn, the advance pass (the clocks, the physics, the
//!   absorptions), the test diagnostics.
//! - `ignition.rs` — the birth sequence's pure phase math (law 0).
//! - `draw.rs` — the draw pass (halo, disk, jets, infall, core;
//!   the doppler and charge ladders; drawn-cell diff cleanup).
//! - `mod.rs` — this derivation.
//!
//! The draw pass stays with the orchestration family split
//! convention: `particles.rs` carries the physics, `quasar.rs`
//! the state, `draw.rs` the render contract.

pub(crate) mod capture;
pub(crate) mod draw;
pub(crate) mod ignition;
pub(crate) mod particles;
pub(crate) mod quasar;

pub(crate) use particles::QuasarRandom;
pub(crate) use quasar::{QuasSpawnParams, QuasStep, QuasarRain};
