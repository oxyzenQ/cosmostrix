// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#![allow(clippy::module_inception)]

//! DNA helix rain (NIGHT-research-7, the eleventh style): the rain
//! writes the genome.
//!
//! The scene reads as a living DNA molecule standing in the terminal:
//! two sugar-phosphate backbone strands spiraling around a vertical
//! axis, one glyph cell per strand per line, connected by base-pair
//! rungs whose projected width breathes with the turn — wide when the
//! strands swing lateral, collapsed to the single crossing cell when
//! they pass front and back. The molecule rotates slowly on its axis;
//! the X crossings drift down the screen like a corkscrew reading
//! itself. The rain is the nucleotide soup: free base glyphs fall
//! from the sky, and the rung that catches one is charged by it —
//! the light in this sky is where the genome has been recently
//! written. And on a replication clock, the fork: a traveling wave
//! that dissolves the rungs in its window, bows the strands apart
//! (the Y), and re-synthesizes fresh base pairs behind itself —
//! every pass a visible MUTATION, the pair re-rolled. The molecule
//! never stops transcribing.
//!
//! And it is BORN (part 3, the owner's 9/10 round): the style does
//! not enter with the molecule already standing — the entry replays
//! the genesis, the molecular-origin story in four continuous
//! phases (see `genesis.rs`): the primordial soup (the nucleotide
//! rain alone, the broth running thicker than the steady drizzle),
//! the ladder (the axis spine splitting into two strands as the
//! radius grows, the assembly wave writing rungs top-down with the
//! fresh-write light), the windup (the twist zipping in from the
//! top, the wound region dragging the flat tail around the axis),
//! and the steady molecule (the laws below, exactly — the last
//! windup frame and the first steady frame evaluate to identical
//! geometry, no seam, no pop).
//!
//! ## Origin
//!
//! NIGHT-research-7 (the owner's DeepSeek-researched shortlist,
//! first pick): the double helix is a canonical structure (Watson,
//! Crick and Franklin 1953 — the icon needs no invention), so this
//! style borrows a canonical reference like lorenz (Lorenz 1963),
//! physarum (Jones 2010) and flux (PIC/FLIP) do — it is NOT an
//! original-math flagship (that line stays with aeolian and the
//! corona). What IS derived here is the terminal mapping: the
//! projection of a rotating ribbon onto a discrete cell grid, the
//! rung recency economy, and the fork's traveling-wave envelope —
//! the equation set tuned by derivation for the grid, in the house
//! derivation style.
//!
//! ## The ladder equations (the six laws)
//!
//! The molecule is one body: a center column cx, a radius R in
//! columns, a twist k radians per line (k = 2pi / TURN_LINES — one
//! full turn every TURN_LINES lines; with a rung every RUNG_STEP
//! lines that is TURN_LINES / RUNG_STEP ~ 11 base pairs per turn,
//! B-DNA's 10.5 honored), and a rotation phase phi. The weather is
//! the drop pool (the family lane model, one slot per column).
//!
//! ### Law 0 — the genesis (the birth sequence, part 3)
//!
//! The entry does not present a finished molecule — it replays the
//! formation, the black hole's begin_formation contract translated
//! to the genome (a pure resize keeps the steady state; a scene
//! entry re-forms). Four phases on the sim clock (the family speed
//! contract scales the birth with the molecule — the speed keys
//! fast-forward the origin story too): the soup (no molecule, the
//! spawn dial multiplied), the ladder (the radius grows from the
//! axis — the spine splits into the two strands, cubic ease-out —
//! while the assembly wave writes rungs top-down, each crossed rung
//! stamped to max charge with a rolled pair: the genome writes
//! itself into existence), the windup (the twist front zips down
//! from the top: above it the steady law, below it the flat
//! extension at the front's angle), and the steady state (the
//! remaining laws, exactly — the windup's final front is the full
//! height, so the seam evaluates identically on both sides). The
//! rotation is HELD through soup and ladder (the flat pre-molecule
//! stays face-on — a rotating flat ladder periodically collapses
//! edge-on to a line), and the fork is gated: no replication before
//! the genome exists.
//!
//! ### Law 1 — the turn (uniform rotation, one clock)
//!
//! The strand angle at line y is theta(y) = phi + y k. Strand A
//! sits at x = cx + R_eff(y) sin(theta), depth cos(theta); strand
//! B at theta + pi (x mirrored, depth negated). phi advances at
//! the helical rate per SIM-second on the single family clock —
//! no per-strand state, the whole molecule turns as one body, so
//! trajectory shapes survive the speed keys (the family speed
//! contract). The projection is the terminal's own: the crossing
//! cells (sin theta = 0 — one strand front, one back, both at cx)
//! emerge for free, the signature X ladder of every DNA
//! illustration, drifting as phi advances.
//!
//! ### Law 2 — the pairing (Watson-Crick on the grid)
//!
//! Every RUNG_STEP lines a rung joins the strands. The rung's
//! projected span is 2 R_eff |sin theta(y)| cells: wide at the
//! lateral swing, one cell at the crossing — the rung geometry is
//! the projection, not a separate calculation. Each rung carries a
//! pair state from {A-T, T-A, G-C, C-G}; the rung END cells show
//! the bases (the molecule's identity glyphs — they stay A/T/G/C
//! whatever charset the user picks, the dragon-head precedent),
//! the bond cells between them pick from the pool. Every rung cell
//! interpolates the two strand depths: the front half of the rung
//! reads one rung brighter, the back half dimmer — the 3D read
//! without a z-buffer.
//!
//! ### Law 3 — the recency (bounded synthesis charge)
//!
//! Each rung carries a synthesis charge in [0, CHARGE_MAX]
//! (hard-clamped — bounded by construction, no contraction proof
//! needed). Fresh synthesis (a fork pass, law 4) sets it to the
//! max; an absorbed nucleotide (law 5) deposits into it; it decays
//! exponentially. The ladder reads it: Ghost is the ancient
//! archive, Mid a transcribed pair, Hot a freshly-written one,
//! Core the replication window. The light shows where the genome
//! has been recently written — the same deposition economy the
//! corona's footpoints carry, on a ladder instead of a star.
//!
//! ### Law 4 — the replication (the traveling fork)
//!
//! A global replication clock (mean several sim-seconds, variance)
//! opens a fork ABOVE the screen that travels DOWN at the fork
//! rate — a wave of re-synthesis sweeping the molecule's height.
//! Rungs within the fork's GAP window are dissolved (not drawn);
//! the strands bow apart through the window (local radius
//! amplification under a Gaussian envelope — the Y of the
//! replication-icon, rendered as a widening of the whole local
//! cross-section). As the fork center crosses each rung: the rung
//! re-synthesizes — charge to max, the pair RE-ROLLED (the
//! mutation: a visible base change where the wave passed). The
//! fork exits at the bottom, the clock re-arms, the next sweep
//! re-enters from the top. The charge the fork stamps decays while
//! the wave travels on, so the freshest rungs sit just above the
//! fork — a brightness gradient that trails the wave down the
//! molecule, the replication's wake.
//!
//! ### Law 5 — the soup (the nucleotide rain)
//!
//! Free nucleotide drops fall from the top inside the capture
//! band (cx +- CAPTURE_BAND), terminal-velocity fall scaled by
//! the family speed contract, a small clamped brownian lateral
//! drift. When a drop's line crosses a rung line and its column
//! sits inside that rung's projected span (+- the capture
//! margin), the rung ABSORBS it: the charge deposits (clamped),
//! and with the mutation chance the pair re-rolls — the rain
//! visibly edits the genome it lands on. Drops that miss every
//! rung fall to the floor and expire (the free-floating soup);
//! the lifetime backstop sweeps stragglers. A sparse ambient
//! drizzle by the calm-sky dial — the molecule is the hero, the
//! weather the minority layer.
//!
//! ## Stability
//!
//! Every state variable is bounded by direct construction: the
//! phase wraps at 128 turns (f32 ulp far below visual resolution,
//! the vortex arm-phase precedent); the radius clamps inside
//! [R_MIN, R_MAX]; the bow is a bounded envelope multiplier with
//! R_eff clamped to the viewport margins; the charge decays
//! strictly and clamps at CHARGE_MAX; the fork is a linear
//! position in [−GAP, lines] with a clocked re-entry; the drop
//! pool is lane-bounded with velocity clamps and a lifetime
//! backstop; every drawn cell is bounds-checked at the frame.
//!
//! ## FPS invariance
//!
//! Every rate (rotation, fork travel, charge decay, drop fall and
//! drift, the replication clock) is expressed per SIM-second and
//! integrated with the clamped dt on the single family clock
//! (dt_sim = dt_wall x chars_per_sec x SIM_TIME_PER_CPS, eased by
//! resume_blend). The strand and rung geometry is closed-form in
//! phi — position, span and depth are recomputed exactly each
//! frame, quantized to the cell grid only at the draw boundary.
//! The bench's uniform stepping exercises one advance + one draw
//! per frame at the standard cadence, the same
//! deterministic-contract behavior the visual metrics are measured
//! against.
//!
//! ## Module map
//!
//! - `genesis.rs` — the birth sequence's pure phase math (law 0:
//!   the timeline classifier, the two fronts, the radius growth
//!   — the black hole's `formation.rs` split pattern).
//! - `helix.rs` — the genome: the rung states (pair, charge,
//!   dissolution), the fork, the rotation phase, the genesis
//!   clock and formed flag, the closed-form geometry queries
//!   (laws 0-4 executable form).
//! - `drops.rs` — the soup: the nucleotide drop state struct (law
//!   5's carrier).
//! - `dna_helix.rs` — the orchestration: pool, spawn accumulator,
//!   advance (drop physics + absorption), the test diagnostics.
//! - `draw.rs` — the draw pass (strands, rungs, rain, drawn-cell
//!   diff cleanup) — split from the orchestration at the 800-LOC
//!   hard cap (the monolith family's split pattern).

pub(crate) mod dna_helix;
pub(crate) mod draw;
pub(crate) mod drops;
pub(crate) mod genesis;
pub(crate) mod helix;

pub(crate) use dna_helix::{DnaHelixRain, DnaSpawnParams, DnaStep};
pub(crate) use helix::DnaRandom;
