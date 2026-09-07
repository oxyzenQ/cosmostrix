// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The aeolian rain drop — the weather half of the weave
//! (NIGHT-special-2).
//!
//! One drop is a falling glyph carrier: position (fractional col,
//! line), a lateral velocity for the resonance-seeking bend, a fall
//! speed that gravity feeds and wavefront surf kicks lift, and a
//! pluck charge that grows with the fall (deep fast drops ring the
//! strings hard). The drop struct is pure state; the physics pass
//! that moves it lives in `AeolianRain::advance` (it needs the
//! string field's queries and the capture rolls).

use crate::constants::{
    AEOLIAN_DROP_DRIFT_FRACTION, AEOLIAN_DROP_FALL_BASE, AEOLIAN_SPEED_CORE, AEOLIAN_SPEED_GHOST,
    AEOLIAN_SPEED_MID, AEOLIAN_TRAIL_LEN,
};

use super::super::monolith::BrightnessLevel;

/// One falling glyph (the rain the instrument is played by).
#[derive(Clone, Copy, Debug)]
pub(crate) struct AeolianDrop {
    pub(crate) active: bool,
    /// Column position, fractional (steering bends this smoothly).
    pub(crate) x: f32,
    /// Line position, fractional (y grows downward, the family
    /// convention).
    pub(crate) y: f32,
    /// Lateral velocity in cells per sim-second (resonance seeking).
    pub(crate) vx: f32,
    /// Fall speed in lines per sim-second (gravity + surf kicks).
    pub(crate) vy: f32,
    /// Pluck charge accumulated over the fall (kinetic charge —
    /// spent at the string the drop is captured by).
    pub(crate) charge: f32,
    /// Simulation age in sim-seconds (lifetime backstop clock).
    pub(crate) sim_age: f32,
    /// Per-drop lifetime cap (+-15% variance at spawn, the family
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
    /// Ring buffer of the last `AEOLIAN_TRAIL_LEN` head cells,
    /// shift-left layout (index 0 = oldest).
    trail: [(u16, u16); AEOLIAN_TRAIL_LEN],
    trail_len: u8,
}

impl AeolianDrop {
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
            trail: [(0, 0); AEOLIAN_TRAIL_LEN],
            trail_len: 0,
        }
    }

    /// Seed a fresh drop's motion state at spawn: a calm entry speed
    /// (the Ghost read, the stage-4 dim-entry DNA), a slight lateral
    /// drift so consecutive plucks never ring the same column in
    /// rhythm, and the charge floor every impact carries.
    pub(crate) fn seed_motion(&mut self, drift_roll: f32, pace: f32, lifetime: f32) {
        self.vx = drift_roll * AEOLIAN_DROP_FALL_BASE * AEOLIAN_DROP_DRIFT_FRACTION;
        self.vy = AEOLIAN_DROP_FALL_BASE;
        self.charge = crate::constants::AEOLIAN_CHARGE_SEED;
        self.sim_age = 0.0;
        self.lifetime = lifetime;
        self.pace = pace;
        self.trail_len = 0;
    }

    /// Push a head cell into the trail ring buffer (shift-left when
    /// full — identical mechanics to the lorenz comet).
    pub(crate) fn push_trail(&mut self, col: u16, line: u16) {
        if self.trail_len as usize >= AEOLIAN_TRAIL_LEN {
            for i in 0..AEOLIAN_TRAIL_LEN - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[AEOLIAN_TRAIL_LEN - 1] = (col, line);
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
    /// Ghost rung sits ABOVE the fresh fall speed (the dim calm
    /// entry), Mid is the accelerating fall's band, Hot the surfed
    /// streak, Core the full white punch through a bright packet.
    pub(crate) fn kinetic_level(&self) -> BrightnessLevel {
        let speed = self.vy.abs();
        if speed > AEOLIAN_SPEED_CORE {
            BrightnessLevel::Core
        } else if speed > AEOLIAN_SPEED_MID {
            BrightnessLevel::Hot
        } else if speed > AEOLIAN_SPEED_GHOST {
            BrightnessLevel::Mid
        } else {
            BrightnessLevel::Ghost
        }
    }
}
