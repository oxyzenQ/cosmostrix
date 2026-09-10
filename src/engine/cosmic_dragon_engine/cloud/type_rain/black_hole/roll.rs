// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole see-saw roll scheduler (NIGHT-special-1 stage 2.4,
//! window re-cut NIGHT-research-9): the disk stack's attitude state
//! machine — the lever motion the owner specced, split from `ring.rs`
//! so the physics module stays under the 800-line cap (the family's
//! module-split contract). The scheduler owns the angle, the eased
//! turns, the long holds, the alternating sign, the chain decisions
//! and the dynamic tilt cap; `ring.rs` owns the projection, the
//! integrator and the brightness ladders. The schedule is
//! hash-driven (deterministic, no RNG) so every run plays the same
//! choreography and the tests can pin it.

/// The see-saw roll scheduler (stage 2.4, the owner's lever motion,
/// retuned stage 2.5, window re-cut NIGHT-research-9): a
/// deterministic state machine that owns the disk stack's attitude
/// angle in the screen plane. 0 is the flat horizontal rest line —
/// the Gargantua read, held the longest (the flat hold, 36 s, still
/// the single longest pose per the owner's spec). The attitude
/// window excludes the whole 70-110 degree near-vertical band (the
/// NIGHT-research-9 owner mandate: those attitudes clip the disk
/// against the terminal screen limits and read ugly) — the excursion
/// menu tops out at 60 degrees, and every tilted attitude holds a
/// LONG 30 s dwell across the rest of the window. The sign
/// alternates every excursion so the left-up and right-up tilts take
/// turns. A DYNAMIC tilt cap (`set_tilt_cap`) can lower the ceiling
/// below the menu's on viewports whose vertical budget cannot host
/// the full rung (the cap derives from the disk's semi-major and
/// the viewport's half-height — a stretched wide-screen disk tilts
/// shallower, exactly as a long thin disk must); the scheduler
/// clamps the menu pick to the cap and arms an immediate eased
/// return to the rest line whenever a shrink mid-flight leaves the
/// current attitude outside the window (a resize while tilted).
/// Turns are eased smoothstep sweeps at a fixed angular rate (a
/// 85-degree pivot resolves in ~3.5 s, "within a few seconds"),
/// duration clamped so the widest chained swing still reads as one
/// deliberate swing. An excursion either returns to the rest line
/// (the default) or chains straight into the next excursion — the
/// disk sweeps through horizontal and keeps going, the continuous
/// lever wave. The schedule is hash-driven (no RNG — the advance
/// pass owns no generator), so every run plays the same
/// choreography and the tests can pin it.
pub(crate) struct RingRoll {
    /// Current attitude angle (radians; 0 = horizontal, positive
    /// lifts the left end and drops the right end).
    angle: f32,
    /// Turn start angle and target (the eased lerp's endpoints).
    from: f32,
    target: f32,
    /// Seconds remaining in the current segment: the turn's
    /// remaining sweep time while turning, the hold countdown while
    /// resting at `target`.
    remaining: f32,
    /// Total duration of the current segment (the easing denominator).
    segment_dur: f32,
    /// True while sweeping between attitudes, false while holding.
    turning: bool,
    /// Sign of the NEXT excursion (+1.0 left-up / -1.0 right-up) —
    /// flipped every excursion so the tilts alternate.
    sign: f32,
    /// Schedule step counter (the hash seed — increments every
    /// decision, keeping the sequence deterministic yet varied).
    seq: u32,
    /// Dynamic attitude ceiling (radians, positive): the menu pick
    /// clamps to this, so the stack never parks (or sweeps) outside
    /// the viewport's vertical budget. Defaults to the menu's max
    /// rung (60 degrees); `set_tilt_cap` retunes it on resize.
    tilt_cap: f32,
}

impl RingRoll {
    /// Fresh schedule: flat at the rest line, holding for the flat
    /// hold — the stack introduces itself as the horizontal
    /// Gargantua disk before the first tilt.
    pub(crate) const fn new() -> Self {
        Self {
            angle: 0.0,
            from: 0.0,
            target: 0.0,
            remaining: crate::constants::BLACK_HOLE_ROLL_FLAT_HOLD,
            segment_dur: crate::constants::BLACK_HOLE_ROLL_FLAT_HOLD,
            turning: false,
            sign: 1.0,
            seq: 0,
            tilt_cap: Self::MENU_MAX_RADIANS,
        }
    }

    /// The menu's maximum rung in radians (60 degrees — the highest
    /// attitude the excursion menu can arm, itself below the excluded
    /// 70-110 degree window's edge). Crate-visible so the
    /// orchestrator can default its stored cap before the first
    /// reset lands.
    pub(crate) const MENU_MAX_RADIANS: f32 = 60.0_f32.to_radians();

    /// The live attitude angle (radians) — the projection's roll
    /// input.
    pub(crate) fn angle(&self) -> f32 {
        self.angle
    }

    /// Retune the dynamic attitude ceiling (radians, positive; the
    /// caller clamps to at most the menu max). Called on resize with
    /// the viewport's vertical budget: the disk's semi-major times
    /// the sine of the tilt must stay inside 92% of the half-height.
    /// If the CURRENT attitude (parked target or live sweep) sits
    /// outside the new cap, an immediate eased return to the rest
    /// line is armed — a shrink mid-hold never leaves the stack
    /// clipped for the remaining dwell.
    pub(crate) fn set_tilt_cap(&mut self, cap: f32) {
        self.tilt_cap = cap.clamp(0.0, Self::MENU_MAX_RADIANS);
        if self.target.abs() > self.tilt_cap + 1.0e-4 {
            self.arm_return_to_rest();
        }
    }

    /// Arm an immediate eased sweep back to the flat rest line,
    /// aborting the current hold or sweep (the resize-while-tilted
    /// recovery; the sweep runs at the standard turn rate so the
    /// recovery reads as the same deliberate lever motion, not a
    /// snap).
    fn arm_return_to_rest(&mut self) {
        let delta = self.angle.abs();
        self.from = self.angle;
        self.target = 0.0;
        self.segment_dur = (delta / crate::constants::BLACK_HOLE_ROLL_RATE)
            .clamp(
                crate::constants::BLACK_HOLE_ROLL_TURN_MIN_SECS,
                crate::constants::BLACK_HOLE_ROLL_TURN_MAX_SECS,
            )
            .max(0.05);
        self.remaining = self.segment_dur;
        self.turning = true;
    }

    /// Advance the schedule by `dt` wall seconds (the same clock the
    /// motes, the spin and the formation ride — pause freezes the
    /// lever mid-swing, resume continues it). A step that outlives
    /// its segment rolls the leftover time into the next one, so a
    /// large test step (or a slow frame) lands on the same schedule
    /// point as many small ones.
    pub(crate) fn tick(&mut self, dt: f32) {
        let mut leftover = dt;
        while leftover > 0.0 {
            if self.turning {
                let take = leftover.min(self.remaining.max(0.0));
                self.remaining -= take;
                leftover -= take;
                if self.remaining <= 0.0 {
                    // The sweep landed: hold at the target attitude.
                    self.angle = self.target;
                    self.turning = false;
                    self.remaining = self.hold_for(self.target);
                    self.segment_dur = self.remaining;
                } else {
                    // Eased sweep progress (smoothstep: slow departure,
                    // fast middle, soft arrival — a gravitational pivot,
                    // not a linear slide).
                    let p = 1.0 - (self.remaining / self.segment_dur).clamp(0.0, 1.0);
                    let e = p * p * (3.0 - 2.0 * p);
                    self.angle = self.from + (self.target - self.from) * e;
                }
            } else {
                let take = leftover.min(self.remaining.max(0.0));
                self.remaining -= take;
                leftover -= take;
                if self.remaining <= 0.0 {
                    self.begin_turn();
                }
            }
        }
    }

    /// Hold duration for an attitude: every attitude now parks for
    /// its long dwell — 36 s at the flat rest line (the single
    /// longest pose), 30 s at a tilted excursion (the stage-2.5
    /// improved long duration across the whole attitude window).
    fn hold_for(&self, target: f32) -> f32 {
        if target.abs() < 1.0e-4 {
            crate::constants::BLACK_HOLE_ROLL_FLAT_HOLD
        } else {
            crate::constants::BLACK_HOLE_ROLL_TILT_HOLD
        }
    }

    /// Choose and arm the next turn. From the rest line: always an
    /// excursion. From an excursion: usually back to rest, sometimes
    /// (the chain chance) straight into the next excursion with the
    /// sign flipped — the lever wave that sweeps through horizontal
    /// without parking. The menu pick clamps to the live tilt cap
    /// (NIGHT-research-9: viewports whose vertical budget cannot host
    /// the full rung get the rung lowered, never the window breached).
    fn begin_turn(&mut self) {
        let at_rest = self.target.abs() < 1.0e-4;
        let chain = !at_rest
            && (schedule_hash(self.seq, 1) % 100)
                < crate::constants::BLACK_HOLE_ROLL_CHAIN_PCT as u64;
        let target = if at_rest || chain {
            // Outward (or chained) excursion: flip the sign, pick the
            // tilt magnitude from the menu, clamped to the dynamic
            // ceiling.
            self.sign = -self.sign;
            let degs = crate::constants::BLACK_HOLE_ROLL_TILT_DEGS[(schedule_hash(self.seq, 2)
                % crate::constants::BLACK_HOLE_ROLL_TILT_DEGS.len() as u64)
                as usize];
            self.sign * degs.to_radians().min(self.tilt_cap)
        } else {
            0.0
        };
        let delta = (target - self.angle).abs();
        self.from = self.angle;
        self.target = target;
        self.segment_dur = (delta / crate::constants::BLACK_HOLE_ROLL_RATE)
            .clamp(
                crate::constants::BLACK_HOLE_ROLL_TURN_MIN_SECS,
                crate::constants::BLACK_HOLE_ROLL_TURN_MAX_SECS,
            )
            .max(0.05);
        self.remaining = self.segment_dur;
        self.turning = true;
        self.seq = self.seq.wrapping_add(1);
    }
}

// Compile-time tie between the scheduler's hard ceiling and the
// excursion menu (the menu is sorted descending, so the first rung
// is the max): if the menu ever grows a rung above 60 degrees, this
// assert fires and MENU_MAX_RADIANS must be raised with it (the
// 70-110 degree window stays excluded either way — see the
// style_rain.rs const asserts).
const _: () = assert!(crate::constants::BLACK_HOLE_ROLL_TILT_DEGS[0] <= 60.0);

/// Deterministic schedule hash (a Knuth multiplicative mix of the
/// step counter and a salt — the same trick the rim conveyor's glyph
/// hash uses). Two salts spread the magnitude pick and the chain
/// decision so consecutive steps cannot correlate.
fn schedule_hash(seq: u32, salt: u32) -> u64 {
    (seq as u64)
        .wrapping_mul(2_654_435_761)
        .wrapping_add((salt as u64).wrapping_mul(40_503))
}
