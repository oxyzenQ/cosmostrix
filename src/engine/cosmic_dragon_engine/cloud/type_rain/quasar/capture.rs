// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The capture economy (NIGHT-research-8, law 3's executable
//! half): the free-slot scans and the absorption handler — the
//! accounting that turns the falling rain into the disk.
//!
//! Split from `quasar.rs` for the 800-line source cap (the
//! family's per-concern file split): the state machine owns the
//! pools and the clocks, this file owns what happens when a
//! streamer lands. The economy's two arms mirror the DNA helix's
//! fresh-write/run-charge split:
//! - Below target: the capture BIRTHS a disk orbit (the streamer
//!   activates a vacant slot at the rim, charged to full — the
//!   fresh write).
//! - At target: the capture re-charges the nearest-angle orbit
//!   (the light shows where the engine has been recently fed).
//!
//! The bookkeeping contract (the DNA starvation lesson, pinned by
//! tests): every deactivation decrements its counter and every
//! activation increments it — the spawn gate reads the true
//! population, the soup never starves.

use rand::distr::Distribution;

use super::particles::QuasarRandom;
use super::quasar::QuasarRain;

impl QuasarRain {
    /// Amortized free-slot scan for the infall pool (rotating
    /// cursor — mirrors the family).
    pub(super) fn find_inactive_infall(&mut self) -> Option<usize> {
        let len = self.infall.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.spawn_scan_idx + step) % len;
            if !self.infall[idx].active {
                self.spawn_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Amortized free-slot scan for the disk pool (the capture
    /// economy's activation path).
    pub(super) fn find_inactive_disk(&mut self) -> Option<usize> {
        let len = self.disk.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (self.disk_scan_idx + step) % len;
            if !self.disk[idx].active {
                self.disk_scan_idx = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// The capture economy (law 3): an absorbed streamer either
    /// births a disk orbit (while the disk is below its target —
    /// the fresh write, charged to full) or re-charges the
    /// nearest-angle orbit (the steady state — the light shows
    /// where the engine has been recently fed).
    pub(super) fn run_capture(&mut self, theta: f32, random: &mut QuasarRandom<'_>) {
        #[cfg(test)]
        {
            self.absorptions_for_test += 1;
        }
        if self.disk_active < self.disk.len() {
            if let Some(idx) = self.find_inactive_disk() {
                let roll = random.rand_chance.sample(random.rng);
                let target = crate::constants::QUAS_DISK_INNER + roll * 0.70;
                self.disk[idx].activate_disk(theta, target, self.field_palette_slot);
                self.disk_active += 1;
                return;
            }
        }
        // The steady feed: the nearest orbit by angular distance
        // takes the charge (wrap-aware).
        let mut best: Option<usize> = None;
        let mut best_d = f32::MAX;
        for (i, p) in self.disk.iter().enumerate() {
            if !p.active {
                continue;
            }
            let d = (p.theta - theta).rem_euclid(std::f32::consts::TAU);
            let d = d.min(std::f32::consts::TAU - d);
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        if let Some(i) = best {
            self.disk[i].feed_disk();
        }
    }
}
