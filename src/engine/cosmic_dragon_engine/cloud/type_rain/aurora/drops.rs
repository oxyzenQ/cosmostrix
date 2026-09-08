// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The aurora precipitation drop — the weather half of the polar
//! veil (NIGHT-special-3).
//!
//! One drop is a falling glyph carrier: position (fractional col,
//! line), a lateral velocity for the glow-weighted funnel (law 3), a
//! fall speed gravity feeds, and a kinetic charge that grows over
//! the fall and is spent into the fringe at absorption (law 4). The
//! drop struct is pure state; the physics pass that moves it lives
//! in `AuroraRain::advance` (it needs the ray lattice's queries and
//! the absorption bookkeeping).

use crate::constants::{
    AURORA_CHARGE_SEED, AURORA_DROP_DRIFT_FRACTION, AURORA_DROP_FALL_BASE, AURORA_SPEED_CORE,
    AURORA_SPEED_GHOST, AURORA_SPEED_MID, AURORA_TRAIL_LEN,
};

use super::super::monolith::BrightnessLevel;

/// One falling glyph (the precipitation the veil is painted by).
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuroraDrop {
    pub(crate) active: bool,
    /// Column position, fractional (the funnel bends this smoothly).
    pub(crate) x: f32,
    /// Line position, fractional (y grows downward, the family
    /// convention).
    pub(crate) y: f32,
    /// Lateral velocity in columns per sim-second (law 3 seeking).
    pub(crate) vx: f32,
    /// Fall speed in lines per sim-second (gravity-capped).
    pub(crate) vy: f32,
    /// Kinetic charge accumulated over the fall (deposited into the
    /// fringe glow at absorption — law 4).
    pub(crate) charge: f32,
    /// Simulation age in sim-seconds (lifetime backstop clock).
    pub(crate) sim_age: f32,
    /// Per-drop lifetime cap (+-25% variance at spawn, the family
    /// contract — no rhythmic mass absorption).
    pub(crate) lifetime: f32,
    /// Per-drop fall-speed multiplier (0.85..1.15): siblings seeded
    /// at the same column visibly separate over a few seconds.
    pub(crate) pace: f32,
    /// Glyph carried by the drop; re-rolled matrix-style when the
    /// head crosses into a new cell (motion-gated shimmer).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Ring buffer of the last `AURORA_TRAIL_LEN` head cells,
    /// shift-left layout (index 0 = oldest).
    trail: [(u16, u16); AURORA_TRAIL_LEN],
    trail_len: u8,
}

impl AuroraDrop {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            charge: 0.0,
            sim_age: 0.0,
            lifetime: 0.0,
            pace: 1.0,
            ch: '0',
            palette_slot: 0,
            trail: [(0, 0); AURORA_TRAIL_LEN],
            trail_len: 0,
        }
    }

    /// Seed a fresh drop's motion state at spawn: a calm entry speed
    /// (the Ghost read, the stage-4 dim-entry DNA shared with the
    /// aeolian drizzle), a slight lateral drift so consecutive
    /// absorptions never land on the same fringe in rhythm, and the
    /// charge floor every absorption carries.
    pub(crate) fn seed_motion(&mut self, drift_roll: f32, pace: f32, lifetime: f32) {
        self.vx = drift_roll * AURORA_DROP_FALL_BASE * AURORA_DROP_DRIFT_FRACTION;
        self.vy = AURORA_DROP_FALL_BASE;
        self.charge = AURORA_CHARGE_SEED;
        self.sim_age = 0.0;
        self.lifetime = lifetime;
        self.pace = pace;
        self.trail_len = 0;
    }

    /// Push a head cell into the trail ring buffer (shift-left when
    /// full — identical mechanics to the aeolian comet).
    pub(crate) fn push_trail(&mut self, col: u16, line: u16) {
        if self.trail_len as usize >= AURORA_TRAIL_LEN {
            for i in 0..AURORA_TRAIL_LEN - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[AURORA_TRAIL_LEN - 1] = (col, line);
        } else {
            let idx = self.trail_len as usize;
            self.trail[idx] = (col, line);
            self.trail_len += 1;
        }
    }

    /// Trail cell read by age (0 = oldest kept, len-1 = newest).
    pub(crate) fn trail_cell(&self, idx: usize) -> Option<(u16, u16)> {
        if idx < self.trail_len as usize {
            Some(self.trail[idx])
        } else {
            None
        }
    }

    pub(crate) fn trail_len(&self) -> u8 {
        self.trail_len
    }

    /// Wipe the trail (deactivation / respawn — the family
    /// contract: a dead drop leaves no phantom trail cells).
    pub(crate) fn clear_trail(&mut self) {
        self.trail_len = 0;
    }

    /// The kinetic-heat ladder: fall speed to brightness rung. The
    /// Ghost rung sits at the fresh fall speed (the dim calm
    /// entry), Mid is the accelerating fall's band, Hot the deep
    /// streak, Core the full-terminal dive.
    pub(crate) fn kinetic_level(&self) -> BrightnessLevel {
        let speed = self.vy.abs();
        if speed > AURORA_SPEED_CORE {
            BrightnessLevel::Core
        } else if speed > AURORA_SPEED_MID {
            BrightnessLevel::Hot
        } else if speed > AURORA_SPEED_GHOST {
            BrightnessLevel::Mid
        } else {
            BrightnessLevel::Ghost
        }
    }
}
