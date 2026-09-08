// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The nucleotide drop — the weather half of the DNA helix style
//! (NIGHT-research-7).
//!
//! One drop is a free nucleotide in the soup: a glyph carrier
//! falling from the sky inside the molecule's capture band, with
//! a terminal-velocity fall and a small clamped brownian lateral
//! drift. The drop struct is pure state; the physics pass that
//! moves it and the absorption test that ends it live in
//! `DnaHelixRain::advance` (the absorption needs the genome's
//! rung geometry queries — the closed-form spans — so the test
//! belongs to the orchestration, the solar drop-pool precedent).

use crate::constants::{DNA_FALL_MULT, DNA_TRAIL_LEN};

use super::super::monolith::BrightnessLevel;

/// Age-fraction ladder thresholds (law 5's draw read): a fresh
/// nucleotide reads Core, a young one Hot, a mid-age one Mid, an
/// old drifter Ghost — the fade toward the floor. Plain fractions
/// of the lifetime (the solar ejecta precedent: the ladder reads
/// the remaining life, not the speed — the fall is terminal).
const AGE_FRACTION_HOT: f32 = 0.20;
const AGE_FRACTION_MID: f32 = 0.45;
const AGE_FRACTION_GHOST: f32 = 0.75;

/// One falling nucleotide (the soup the genome is fed by).
#[derive(Clone, Copy, Debug)]
pub(crate) struct NucleotideDrop {
    pub(crate) active: bool,
    /// Column position, fractional (drifts — the brownian arm).
    pub(crate) x: f32,
    /// Line position, fractional (y grows downward, the family
    /// convention; the floor expires the drop).
    pub(crate) y: f32,
    /// Lateral drift velocity in columns per sim-second (the
    /// clamped brownian walk, integrated in the advance pass).
    pub(crate) vx: f32,
    /// Fall speed in lines per sim-second (terminal velocity —
    /// constant per drop, rolled at spawn inside its band).
    pub(crate) vy: f32,
    /// Simulation age in sim-seconds (the lifetime backstop clock
    /// + the age-fade ladder input).
    pub(crate) sim_age: f32,
    /// Per-drop lifetime cap (+-25% variance at spawn, the family
    /// contract — no rhythmic mass absorption).
    pub(crate) lifetime: f32,
    /// Glyph carried by the drop; re-rolled matrix-style when the
    /// head crosses into a new cell (motion-gated shimmer).
    pub(crate) ch: char,
    /// Palette slot adopted at spawn / palette transition.
    pub(crate) palette_slot: u8,
    /// Ring buffer of the last DNA_TRAIL_LEN head cells,
    /// shift-left layout (index 0 = oldest).
    trail: [(u16, u16); DNA_TRAIL_LEN],
    trail_len: u8,
}

impl NucleotideDrop {
    pub(crate) const fn vacant() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: DNA_FALL_MULT,
            sim_age: 0.0,
            lifetime: 0.0,
            ch: 'A',
            palette_slot: 0,
            trail: [(0, 0); DNA_TRAIL_LEN],
            trail_len: 0,
        }
    }

    /// Seed a fresh nucleotide at spawn: the fall speed rolled
    /// inside its band (terminal velocity — nucleotides do not
    /// accelerate in free solution), the age and lifetime
    /// backstops. Position and drift are chosen by the spawn
    /// pass's capture-band rolls.
    pub(crate) fn seed(&mut self, x: f32, y: f32, vy: f32, lifetime: f32) {
        self.x = x;
        self.y = y;
        self.vx = 0.0;
        self.vy = vy;
        self.sim_age = 0.0;
        self.lifetime = lifetime;
        self.trail_len = 0;
    }

    /// Push a head cell into the trail ring buffer (shift-left
    /// when full — identical mechanics to the solar/aeolian
    /// comets).
    pub(crate) fn push_trail(&mut self, col: u16, line: u16) {
        if self.trail_len as usize >= DNA_TRAIL_LEN {
            for i in 0..DNA_TRAIL_LEN - 1 {
                self.trail[i] = self.trail[i + 1];
            }
            self.trail[DNA_TRAIL_LEN - 1] = (col, line);
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

    /// The age ladder (law 5's draw read): a fresh nucleotide
    /// reads Core (the soup is bright where it enters), a young
    /// one Hot, a mid-age one Mid, an old drifter Ghost — the
    /// fade toward the floor. The ladder reads the consumed
    /// fraction of the lifetime, not the speed — the fall is
    /// terminal (the solar ejecta precedent).
    pub(crate) fn age_level(&self) -> BrightnessLevel {
        let life = self.lifetime.max(0.001);
        match self.sim_age / life {
            a if a < AGE_FRACTION_HOT => BrightnessLevel::Core,
            a if a < AGE_FRACTION_MID => BrightnessLevel::Hot,
            a if a < AGE_FRACTION_GHOST => BrightnessLevel::Mid,
            _ => BrightnessLevel::Ghost,
        }
    }
}
