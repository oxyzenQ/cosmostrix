// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The data entry pass (NIGHT-research-9, law 3's accumulator
//! half) — the staggered streamer spawn.
//!
//! Split from `neural.rs` for the 800-line source cap (the
//! family's per-concern file split; the cloud's own spawn split
//! precedent): the state machine owns the pools and the clocks,
//! this file owns how the data enters the sky. The streamers
//! ride the family's fractional spawn accumulator exactly as the
//! quasar's infall does — the budget converts elapsed wall time
//! into activations, the remainder carries the fractions forward
//! (capped), and the columns bias toward the input band's built
//! neurons: the data aims at the machine, and the share that
//! falls anywhere is the noise the machine has not learned yet.

use std::time::Duration;

use rand::distr::Distribution;

use crate::constants::{NEUR_SPAWN_RATE_FLOOR, NEUR_SPAWN_RATE_MULT, SPAWN_REMAINDER_CAP};

use super::network::{fall_speed, NeuralRandom};
use super::neural::NeuralRain;

impl NeuralRain {
    /// Amortized free-slot scan for the streamer pool (rotating
    /// cursor — mirrors the family).
    fn find_inactive_streamer(&mut self) -> Option<usize> {
        let len = self.streamers.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.streamer_scan_idx + step) % len;
            if !self.streamers[idx].active {
                self.streamer_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Spawn pass — the staggered streamer entry through the
    /// family's accumulator contract: the budget activates
    /// streamers from the pool's vacant slots at the sky's top
    /// (the columns biased toward the input band's built neurons —
    /// the data aims at the machine; a share falls anywhere, the
    /// noise the machine has not learned yet). The target
    /// breathes with the genesis (the thick data cloud) and the
    /// burst surge; once at target the accumulator idles.
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &super::neural::NeurSpawnParams,
        random: &mut NeuralRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 {
            *spawn_remainder = 0.0;
            return;
        }

        let target = self.streamer_target(params.cols, params.density);
        if self.streamers.len() != target {
            if target < self.streamers.len() {
                let excess = self.streamers.len() - target;
                let dropped_active = self
                    .streamers
                    .iter()
                    .rev()
                    .take(excess)
                    .filter(|d| d.active)
                    .count();
                self.streamer_active = self.streamer_active.saturating_sub(dropped_active);
            }
            self.streamers
                .resize(target, super::network::Streamer::vacant());
            *spawn_remainder = 0.0;
        }

        if target == 0 || self.streamer_active >= target {
            *spawn_remainder = (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.streamer_active;
        let spawn_rate =
            (target as f32 * NEUR_SPAWN_RATE_MULT + NEUR_SPAWN_RATE_FLOOR) * params.spawn_scale;
        let budget =
            elapsed.as_secs_f32() * spawn_rate + (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
        if !budget.is_finite() || budget <= 0.0 {
            *spawn_remainder = 0.0;
            return;
        }

        let to_spawn = (budget.floor() as usize).min(deficit);
        *spawn_remainder = (budget - to_spawn as f32).min(SPAWN_REMAINDER_CAP);
        if to_spawn == 0 {
            return;
        }

        // The built input neurons (the bias targets) — scanned
        // once per spawn pass, not per roll.
        let input_n = self.geom.nodes_per_layer.first().copied().unwrap_or(0);
        let built_inputs: Vec<usize> = (0..input_n.min(self.nodes.len()))
            .filter(|&i| self.nodes[i].active)
            .collect();
        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_streamer() else {
                break;
            };
            let roll_a = random.rand_chance.sample(random.rng);
            let roll_b = random.rand_chance.sample(random.rng);
            let roll_c = random.rand_chance.sample(random.rng);
            // The bias: two of three streamers aim at a built
            // input neuron (within a couple of columns — the
            // machine's meal); the rest fall anywhere.
            let x = if roll_a < 0.66 && !built_inputs.is_empty() {
                let pick = built_inputs
                    [(roll_b * built_inputs.len() as f32) as usize % built_inputs.len()];
                let node_x = self.nodes[pick].x as f32;
                (node_x + (roll_c * 2.0 - 1.0) * 2.5).clamp(0.0, (params.cols - 1) as f32)
            } else {
                roll_b * params.cols.max(1) as f32
            };
            let speed = fall_speed(roll_c);
            let drift = roll_a * std::f32::consts::TAU;
            self.streamers[idx].activate(x, speed, drift, params.active_palette_slot);
            // The glyph is picked at the first draw pass (the
            // lorenz contract).
            self.streamer_active += 1;
        }
    }
}
