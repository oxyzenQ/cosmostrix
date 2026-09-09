// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! DNA genesis intro (NIGHT-research-7 part 3): the pure phase
//! math for the molecule's birth sequence, split from `helix.rs`
//! the way the black hole's `formation.rs` splits its concerns
//! (the state machine's mutable half — the genesis clock and the
//! formed flag — lives on `DnaGenome`; this file answers the
//! stateless questions).
//!
//! Owner question (the 9/10 rating round): the molecule appears
//! fully formed the moment the style enters — the birth of the
//! genome should be the entry's first act. The answer is the
//! molecular-origin story in four phases, each continuous in
//! time with the next (no pop at any seam):
//!
//! - Soup: the primordial nucleotide rain falls alone (the sky is
//!   the broth; no molecule cell draws). The spawn dial runs its
//!   genesis multiplier — the broth is thicker than the steady
//!   drizzle because while the molecule is absent the soup IS the
//!   scene (the inverse of the black hole's gate: the weather
//!   comes first, the body assembles from under it).
//! - Ladder: the axis spine appears at zero radius and splits
//!   into the two strands as the radius grows (cubic ease-out);
//!   the assembly wave travels top-down writing rungs — each
//!   crossed rung stamped to max charge and a rolled pair (the
//!   fork's fresh-write economy, borrowed for the birth). The
//!   rotation is HELD through the window so the flat ladder
//!   stays face-on (a rotating flat ladder would periodically
//!   collapse edge-on to a single line).
//! - Windup: the twist zips in from the top — above the front
//!   the strands carry the full steady law, below it the flat
//!   ladder extends at the front's angle (the wound top drags
//!   the flat tail around the axis as it descends, the physical
//!   read of winding a ribbon from one end). The rotation
//!   resumes with the windup.
//! - Steady: the full molecule — law 1 through law 5 take over,
//!   the replication fork's clock starts, the soup thins back to
//!   the sparse minority (the excess expires through the floor
//!   expiry and the lifetime backstop, never a mass kill).
//!
//! Timeline (sim-seconds since genesis start, constants in
//! style_rain.rs; the clock rides the molecule's sim time — the
//! family speed contract scales the birth with the molecule,
//! unlike the black hole's wall-clock formation):
//! - [0, SOUP): the soup.
//! - [SOUP, SOUP+LADDER): the ladder (assembly front 0 -> 1 of
//!   the height, linear; radius growth 0 -> 1, cubic ease-out).
//! - [SOUP+LADDER, +WINDUP): the windup (twist front 0 -> 1 of
//!   the height, linear).
//! - after: steady (the formed flag flips; the queries return
//!   the standard laws exactly — the windup's final front is the
//!   full height, so the last frame of the windup and the first
//!   frame of the steady state evaluate to identical geometry).

/// The genesis phase at sim-time `t` since the sequence began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GenesisPhase {
    /// The primordial soup: nucleotide rain alone, no molecule.
    Soup,
    /// The ladder: rungs written top-down, radius growing from
    /// the axis, rotation held face-on.
    Ladder,
    /// The windup: the twist front zips down from the top.
    Windup,
    /// Steady state: the full molecule under laws 1 through 5.
    Steady,
}

/// Classify the genesis timeline at sim-time `t`.
pub(crate) fn genesis_phase(t: f32) -> GenesisPhase {
    let soup = crate::constants::DNA_GENESIS_SOUP_SECS;
    let ladder = soup + crate::constants::DNA_GENESIS_LADDER_SECS;
    let windup = ladder + crate::constants::DNA_GENESIS_WINDUP_SECS;
    if t < soup {
        GenesisPhase::Soup
    } else if t < ladder {
        GenesisPhase::Ladder
    } else if t < windup {
        GenesisPhase::Windup
    } else {
        GenesisPhase::Steady
    }
}

/// Total genesis duration (sim-seconds from the first soup drop
/// to the steady molecule).
pub(crate) fn genesis_total_secs() -> f32 {
    crate::constants::DNA_GENESIS_SOUP_SECS
        + crate::constants::DNA_GENESIS_LADDER_SECS
        + crate::constants::DNA_GENESIS_WINDUP_SECS
}

/// The ladder-assembly wave's front at sim-time `t`, in lines
/// (0 at the soup, the full height from the windup on). Linear
/// travel — the fork's rate law, expressed as a fraction of the
/// height so the choreography paces identically on every
/// terminal (the fork itself keeps its absolute rate: its sweep
/// is a steady-state law, the genesis a one-shot presentation).
pub(crate) fn ladder_front(t: f32, lines: u16) -> f32 {
    let start = crate::constants::DNA_GENESIS_SOUP_SECS;
    let dur = crate::constants::DNA_GENESIS_LADDER_SECS;
    let frac = if t <= start {
        0.0
    } else if t >= start + dur {
        1.0
    } else {
        (t - start) / dur
    };
    frac * lines.max(1) as f32
}

/// The windup's twist front at sim-time `t`, in lines (0 before
/// the windup, the full height after it). Linear, like the
/// assembly front.
pub(crate) fn windup_front(t: f32, lines: u16) -> f32 {
    let start = crate::constants::DNA_GENESIS_SOUP_SECS + crate::constants::DNA_GENESIS_LADDER_SECS;
    let dur = crate::constants::DNA_GENESIS_WINDUP_SECS;
    let frac = if t <= start {
        0.0
    } else if t >= start + dur {
        1.0
    } else {
        (t - start) / dur
    };
    frac * lines.max(1) as f32
}

/// The strand-radius growth fraction in [0, 1] at sim-time `t`
/// (the spine splitting into the two strands): 0 through the
/// soup, a cubic ease-out over the ladder window (fast early
/// split, gentle settle — the horizon-bloom ease), 1 from the
/// windup on. The legibility floor is lifted while the growth
/// runs (a spine clamped to the minimum radius would never read
/// as the single seed line).
pub(crate) fn genesis_radius_growth(t: f32) -> f32 {
    let start = crate::constants::DNA_GENESIS_SOUP_SECS;
    let dur = crate::constants::DNA_GENESIS_LADDER_SECS;
    if t <= start {
        return 0.0;
    }
    let u = ((t - start) / dur).clamp(0.0, 1.0);
    1.0 - (1.0 - u).powi(3)
}

// The shipped calibration must stay internally consistent — the
// compile-time checks mirror the molecule's other constant
// contracts (the phase classifier's ordering depends on every
// window being positive: a zero window would make a phase
// unreachable, a negative one would mis-order the timeline).
const _: () = assert!(crate::constants::DNA_GENESIS_SOUP_SECS > 0.0);
const _: () = assert!(crate::constants::DNA_GENESIS_LADDER_SECS > 0.0);
const _: () = assert!(crate::constants::DNA_GENESIS_WINDUP_SECS > 0.0);
