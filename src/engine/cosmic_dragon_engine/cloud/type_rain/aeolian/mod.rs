// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Aeolian rain (NIGHT-special-2): the rain plays the instrument.
//!
//! The scene reads as a dark sky over a set of invisible horizontal
//! strings. Glyph rain falls from the top edge in a calm drizzle.
//! Where a drop lands, the string RINGS — light races away from the
//! impact in both directions, sharpens into a bright hook at its
//! crest, races into the walls, and reflects back. Falling glyphs
//! bend toward the passing wavefronts (they hear the music), flare
//! white as they punch through them, and are captured where the
//! field is brightest — so the rain clusters onto the antinodes and
//! the instrument amplifies the notes the weather prefers. Where
//! two packets cross, a white knot flares. The architecture is
//! invisible until function reveals it: nothing is drawn where the
//! weave is silent.
//!
//! ## Origin (the invention directive)
//!
//! The owner's directive for this style: motion DNA with NO
//! existing mathematical reference — not a published attractor, not
//! a named physics model, not a borrowed algorithm. The eight
//! sibling styles each carry a canonical system (Lorenz's 1963
//! equations, Keplerian orbits, FABRIK, the Jones slime-mold model,
//! PIC/FLIP projection, inverse-square gravity). This one carries a
//! system derived from first principles IN THIS REPO, purpose-built
//! for the terminal's discrete cell grid — the LEAP-engine spirit:
//! an equation set no textbook carries, tuned by derivation rather
//! than by reference. The laws below are that derivation.
//!
//! ## The weave equations (the six laws)
//!
//! The instrument is a lattice of S horizontal strings (1..=4 by
//! viewport height). Each string cell carries TWO one-way channels:
//! `dR` (rightward-traveling amplitude) and `dL` (leftward). The
//! channels are the string's excitation, split by travel direction.
//!
//! ### Law 1 — the hop-clock lattice (the shape of a packet)
//!
//! Every channel cell carries its excitation mass AND a private
//! phase clock. Each tick, the clock advances by the cell's hop
//! rate — a steep bistable switch in the mass:
//!
//! ```text
//! rate(mass) = FAST if mass >= SWITCH else SLOW   (two voices)
//! clock += rate * dt;  when clock >= 1: hop the WHOLE mass
//! one cell in the channel's travel direction (merge on arrival,
//! the receiver's clock resets)
//! ```
//!
//! Signal cells (mass at or above the switch) run their clocks at
//! the sprint, residue cells at the crawl — so a struck packet
//! separates into a sharp bright head racing far ahead of its slow
//! dim wake: the hook shape. Because a hop moves the cell's mass
//! IN QUANTA (never a fractional blend) and every bright cell
//! shares the SAME rate, a pulse hops in lockstep and translates
//! RIGIDLY — shape preserved exactly, zero numerical diffusion,
//! for the packet's whole lifetime. The sweep order (right channel
//! right-to-left, left channel left-to-right) plus the
//! arrival-resets-its-clock rule guarantee each mass unit moves at
//! most one cell per tick, reflections included: no conveyor, no
//! tunneling, at any dt.
//!
//! Design history, preserved for the record: the first
//! formulation was a Michaelis-style fractional blend (a cell
//! forwards a fraction of its amplitude); the second a smooth
//! bistable switch on the same blend. The blend forms obey a max
//! principle — a convex combination can never sustain a peak, so
//! pulses always decayed into dim ramps within ~1.5 s: no racing
//! fronts, no crossing knots (the smooth switch additionally sheared
//! pulses apart, each cell marching at its own speed). The
//! hop-clock moves mass in whole quanta at a shared quantized rate
//! instead of blending it, which transports shape exactly while
//! keeping the identical stability proof (hops are conservative,
//! decay shrinks, walls return at most what they receive).
//!
//! ### Law 2 — wall reflection (the closed instrument)
//!
//! The fraction forwarded off a screen edge re-enters the OPPOSITE
//! channel at `WALL_REFLECT` of its amplitude. The strings are
//! closed at both ends: light bounces between the walls and slowly
//! mutes (15% absorbed per bounce). The instrument never leaks.
//!
//! ### Law 3 — self-similar decay (the shape-preserving fade)
//!
//! Every cell multiplies by `exp(-DECAY * dt)` — a UNIFORM shrink.
//! Because the factor is amplitude-independent, it erodes the
//! envelope without smearing the shape: a packet fades as it
//! travels, it does not blur. (The urgency law's nonlinearity, not
//! the decay, owns the shaping — the two responsibilities are
//! deliberately separated.)
//!
//! ### Law 4 — the pluck (impact injection)
//!
//! A drop captured by a string injects its accumulated kinetic
//! charge symmetrically into BOTH channels at its landing column:
//! light races away from the impact in both directions — the
//! classic pluck read. With probability ECHO_CHANCE the impact also
//! seeds a fainter packet on the string BELOW (the aftershock
//! cascade — the frame resonates through its depth).
//!
//! ### Law 5 — the capture field (the feedback loop)
//!
//! A drop crossing a string row is captured with probability
//! `base + gain * min(1, u / HOT)` where u is the local combined
//! amplitude. Bright antinodes eat rain; silent strings let it
//! through (a graze still murmurs). The loop this closes is the
//! weave's self-organization: drops are deflected toward bright
//! zones (law 6), captured there preferentially, and their captures
//! AMPLIFY exactly those zones — the weather concentrates onto the
//! notes the instrument is already ringing. Saturation is
//! structural, not tuned: the L1 bound below caps the field, so the
//! loop settles into a shifting standing pattern instead of
//! runaway.
//!
//! ### Law 6 — resonance seeking (the rain that hears)
//!
//! Within SEEK_RANGE above the next string, a drop's lateral
//! velocity accelerates along the field's local gradient: glyphs
//! slide toward the passing wavefront's crest, the bend reading as
//! the rain hearing the music. A through-drop crossing a bright
//! packet gains a vertical kick (the surf streak — it flares and
//! accelerates through the wavefront).
//!
//! ## Stability (the L1 contraction proof)
//!
//! Define the field's L1 norm as the sum of |amplitude| over every
//! cell and channel. The conduction sweep (law 1) is an exact
//! transfer: every amount a donor forwards lands in a receiver
//! cell, so the interior sum is IDENTICALLY conserved (the two
//! telescoping sums cancel). Wall reflection (law 2) returns at
//! most the forwarded amount (factor <= 1). Decay (law 3)
//! strictly shrinks. Therefore the norm is non-increasing between
//! plucks and bounded above by the total injected amplitude —
//! unconditionally, at any dt, at any frame rate (the per-tick
//! fraction clamp keeps every operation a convex blend). The
//! steady-state field energy is injection_rate / decay — the
//! instrument rings exactly as loud as the rain plays it, never
//! louder.
//!
//! ## FPS invariance
//!
//! Every rate in the system (hop clocks, decay, gravity, seeking,
//! charging) is expressed per SIM-second and integrated with the
//! clamped dt; the hop clocks accumulate fractional phase, so
//! timing quantizes to the frame but the AVERAGE speed is exact
//! wherever the phase-debt clamp does not engage (rate x dt < 1,
//! i.e. above ~24 fps at the sprint rate). The bench's uniform
//! stepping exercises exactly one sweep per frame at the family's
//! standard cadence — the deterministic-contract behavior the
//! visual metrics (density gini, frame entropy) are measured
//! against.
//!
//! ## Module map
//!
//! - `strings.rs` — the instrument: the field state, the conduction
//!   sweep, reflection, decay, pluck injection, and the amplitude
//!   queries (laws 1-4 executable form).
//! - `drops.rs` — the weather: the drop state struct and its
//!   kinetic ladder (law 5-6 state carriers).
//! - `aeolian.rs` — the orchestration: pool, spawn accumulator,
//!   advance (laws 5-6 physics), and the draw pass (strings +
//!   rain + drawn-cell diff cleanup).

pub(crate) mod aeolian;
pub(crate) mod drops;
pub(crate) mod strings;

pub(crate) use aeolian::{AeolianRain, AeolianRandom, AeolianSpawnParams, AeolianStep};
