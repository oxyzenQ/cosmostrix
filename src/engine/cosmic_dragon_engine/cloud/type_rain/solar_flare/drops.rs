// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The coronal rain drop — the weather half of the solar flare
//! style (NIGHT-special-4).
//!
//! One drop is a glyph carrier in one of two modes: RIDING (law 2 —
//! the drop condenses near a loop's apex and slides down one leg
//! with the closed-form energy-conserving speed) or EJECTA (law 4 —
//! the drop was flung by a flare and flies a ballistic arc under
//! stellar gravity). The drop struct is pure state; the physics pass
//! that moves it lives in `SolarFlareRain::advance` (it needs the
//! arcade's geometry queries and the deposition bookkeeping).

use crate::constants::{
    SOLAR_RAIN_V0, SOLAR_SPEED_CORE, SOLAR_SPEED_GHOST, SOLAR_SPEED_MID, SOLAR_TRAIL_LEN,
};

use super::super::monolith::BrightnessLevel;

/// Which leg of the loop the drop rides (the footpoint it lands on).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Leg {
    /// Toward s = 0 (the left footpoint).
    Left,
    /// Toward s = 1 (the right footpoint).
    Right,
}

impl Leg {
    /// The sign of ds for this leg (Left: s decreases toward 0).
    pub(crate) fn sign(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }
}

/// The drop's mode: riding a loop leg, or ballistic ejecta.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DropMode {
    /// Law 2: riding the loop `loop_idx` at parameter `s` down the
    /// chosen leg (the along-arc speed lives in the struct's `v`).
    Riding { loop_idx: usize, s: f32, leg: Leg },
    /// Law 4: flare ejecta — position free, velocity (vx, vy), the
    /// age drives the fade and the lifetime backstop.
    Ejecta { age: f32 },
}

/// One falling glyph (the coronal rain the corona is fed by).
#[derive(Clone, Copy, Debug)]
pub(crate) struct SolarDrop {
    pub(crate) active: bool,
    pub(crate) mode: DropMode,
    /// Column position, fractional (riding drops: derived from the
    /// loop geometry each tick; ejecta: integrated).
    pub(crate) x: f32,
    /// Line position, fractional (y grows downward, the family
    /// convention; the surface sits at the bottom).
    pub(crate) y: f32,
    /// Ejecta lateral velocity in columns per sim-second.
    pub(crate) vx: f32,
    /// Ejecta vertical velocity in lines per sim-second (negative
    /// while rising — stellar gravity pulls it positive).
    pub(crate) vy: f32,
    /// Along-arc speed in lines per sim-second (riding drops — the
    /// closed-form energy-conserving speed, law 2).
    pub(crate) v: f32,
    /// Simulation age in sim-seconds (lifetime backstop clock).
    pub(crate) sim_age: f32,
    /// Per-drop lifetime cap (+-25% variance at spawn, the family
    /// contract — no rhythmic mass absorption).
    pub(crate) lifetime: f32,
    /// Glyph carried by the drop; re-rolled matrix-style when the
    /// head crosses into a new cell (motion-gated shimmer).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Ring buffer of the last SOLAR_TRAIL_LEN head cells,
    /// shift-left layout (index 0 = oldest).
    trail: [(u16, u16); SOLAR_TRAIL_LEN],
    trail_len: u8,
}

impl SolarDrop {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            mode: DropMode::Riding {
                loop_idx: 0,
                s: 0.5,
                leg: Leg::Right,
            },
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            v: SOLAR_RAIN_V0,
            sim_age: 0.0,
            lifetime: 0.0,
            ch: '0',
            palette_slot: 0,
            trail: [(0, 0); SOLAR_TRAIL_LEN],
            trail_len: 0,
        }
    }

    /// Seed a fresh rider's motion state at spawn (law 2's
    /// condensation): the thermal kick v0 every descent starts from
    /// (the lazy apex departure), the age and lifetime backstops.
    /// The position (s, leg) is chosen by the spawn pass's hot-loop
    /// tournament; the geometry read happens in the advance pass.
    pub(crate) fn seed_rider(&mut self, loop_idx: usize, s: f32, leg: Leg, lifetime: f32) {
        self.mode = DropMode::Riding { loop_idx, s, leg };
        self.v = SOLAR_RAIN_V0;
        self.vx = 0.0;
        self.vy = 0.0;
        self.sim_age = 0.0;
        self.lifetime = lifetime;
        self.trail_len = 0;
    }

    /// Seed a fresh ejecta at conversion (law 4's fling): position
    /// and velocity from the eruption (the tangent fling for riders,
    /// the apex burst for fresh sparks), the ejecta lifetime.
    pub(crate) fn seed_ejecta(&mut self, x: f32, y: f32, vx: f32, vy: f32, lifetime: f32) {
        self.mode = DropMode::Ejecta { age: 0.0 };
        self.x = x;
        self.y = y;
        self.vx = vx;
        self.vy = vy;
        self.v = SOLAR_RAIN_V0;
        self.sim_age = 0.0;
        self.lifetime = lifetime;
        self.trail_len = 0;
    }

    /// Push a head cell into the trail ring buffer (shift-left when
    /// full — identical mechanics to the aeolian/aurora comets).
    pub(crate) fn push_trail(&mut self, col: u16, line: u16) {
        if self.trail_len as usize >= SOLAR_TRAIL_LEN {
            for i in 0..SOLAR_TRAIL_LEN - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[SOLAR_TRAIL_LEN - 1] = (col, line);
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

    /// The kinetic-heat ladder (law 2's draw read): along-arc speed
    /// to brightness rung. The Ghost rung sits at the thermal kick
    /// (the lazy apex departure), Mid the accelerating descent, Hot
    /// the fast lower leg, Core the full-speed footpoint arrival.
    pub(crate) fn kinetic_level(&self) -> BrightnessLevel {
        let speed = match self.mode {
            DropMode::Riding { .. } => self.v.abs(),
            DropMode::Ejecta { age } => {
                // Ejecta brighten at birth and cool as they fly: the
                // speed proxy is the remaining fraction of life.
                let life = self.lifetime.max(0.001);
                return match age / life {
                    a if a < 0.25 => BrightnessLevel::Hot,
                    a if a < 0.60 => BrightnessLevel::Mid,
                    _ => BrightnessLevel::Ghost,
                };
            }
        };
        if speed > SOLAR_SPEED_CORE {
            BrightnessLevel::Core
        } else if speed > SOLAR_SPEED_MID {
            BrightnessLevel::Hot
        } else if speed > SOLAR_SPEED_GHOST {
            BrightnessLevel::Mid
        } else {
            BrightnessLevel::Ghost
        }
    }
}
