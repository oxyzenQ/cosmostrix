// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Spawn logic — extracted from `cloud/spawn.rs` to keep that file
//! under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `Cloud::spawn_droplets()` + `Cloud::build_droplet_spec()` —
//! the per-frame droplet spawn decision + spec construction.

use std::time::{Duration, Instant};

#[allow(unused_imports)]
use rand::distr::Distribution;

use crate::constants::*;

use super::state::DropletSpawnSpec;

impl super::Cloud {
    pub(crate) fn build_droplet_spec(&mut self, col: u16) -> DropletSpawnSpec {
        let mut end_line = self.lines.saturating_sub(1);
        // NIGHT-research-4: the ripple surface contract (capping
        // droplet end_line above a virtual water plane) is removed —
        // Lorenz is fully structured (no droplet pool) and never
        // reaches this branch. The old ripple path was the only
        // caller that needed the cap; Glyph family is unaffected.
        if self.rand_chance.sample(&mut self.mt) <= self.die_early_pct {
            end_line = self.rand_line.sample(&mut self.mt).min(end_line);
        }
        let cp_idx = self.rand_cpidx.sample(&mut self.mt);

        let mut len = self.lines;
        if self.rand_chance.sample(&mut self.mt) <= self.short_pct {
            len = self.rand_len.sample(&mut self.mt);
        }

        // Assign parallax layer (0=far, 1=mid, 2=near)
        //
        // v25 "cinematic depth" final calibration: [0.35, 0.30, 0.35].
        // Back and front layers share equal 35% spawn probability, mid
        // is 30%. This balanced distribution creates depth via `speed`
        // and `brightness` (PARALLAX_SPEED_MULT, PARALLAX_BRIGHTNESS_MULT)
        // rather than via droplet count — the secret to a realistic,
        // organic, cinematic rain field where all three layers feel
        // equally alive.
        //
        // History: [0.35,0.40,0.25] (back-heavy) → [0.15,0.30,0.55]
        // (windshield) → [0.50,0.15,0.35] (deep atmospheric) →
        // [0.35,0.30,0.35] (cinematic depth, final).
        let layer_roll = self.rand_chance.sample(&mut self.mt);
        let layer: u8 = if layer_roll < 0.35 {
            0
        } else if layer_roll < 0.65 {
            1
        } else {
            2
        };

        // Adjust length by parallax layer
        let len_mult = PARALLAX_LENGTH_MULT[layer as usize];
        len = ((len as f32) * len_mult).max(1.0) as u16;

        // Cinematic final polish: enforce minimum trail length so every
        // droplet has visible head→body→tail structure. Without this floor,
        // short back-layer droplets (length=1 or 2) appeared as bare heads
        // with no fade-out — reading as "stuck pixels" rather than rain
        // streaks. MIN_DROPLET_LENGTH=4 is the smallest length that
        // produces a recognizable gradient.
        //
        // Also cap at MAX_DROPLET_LENGTH_CAP to prevent degenerate values
        // on huge screens (8K UHD bench = 4320 lines) where a full-column
        // droplet would saturate the column for many seconds.
        len = len.clamp(MIN_DROPLET_LENGTH, MAX_DROPLET_LENGTH_CAP);

        // Front-layer proportional tail allocation: assign tail cell count
        // as a percentage of total droplet length (45% per FRONT_LAYER_TAIL_PCT),
        // capped at FRONT_LAYER_TAIL_MAX_CELLS. This restores visible
        // proportional tails on long front-layer droplets (layer 2) —
        // previously they used a fixed [1, 3] cell count, which made very
        // long streams show a long head+body with an almost invisible tail,
        // reading as an unnatural "line" instead of a cinematic rain streak.
        //
        // Mid/back layers keep tail_cells=1 (existing single-cell tail) to
        // preserve the 3-2-2 stop distribution. The CharLoc::TailN variant
        // scales seg across FRONT_LAYER_MAX_TAIL_STOPS so long tails still
        // use only the 3 darkest palette stops, maintaining the existing
        // color-stop hierarchy while expanding the visible tail length.
        let tail_cells: u8 = if layer == 2 {
            let raw = (len as f32 * FRONT_LAYER_TAIL_PCT).round() as u32;
            let clamped = raw.max(1).min(FRONT_LAYER_TAIL_MAX_CELLS as u32);
            clamped as u8
        } else {
            1
        };

        let mut ttl = Duration::from_millis(1);
        if end_line <= len {
            let ms = self.rand_linger_ms.sample(&mut self.mt) as u64;
            ttl = Duration::from_millis(ms);
        }

        // Determine which palette this droplet inherits from its column.
        // During a transition, columns adopt the new palette at staggered times,
        // creating an organic propagation wave instead of a simultaneous switch.
        let palette_slot = self
            .column_palette_slot
            .get(col as usize)
            .copied()
            .unwrap_or(self.active_palette_slot);

        // Adjust speed by parallax layer
        // v50.0.0-beta.6: apply terminal-aware speed_mult at spawn time
        // so newly spawned droplets immediately benefit from the multiplier.
        let layer_speed = PARALLAX_SPEED_MULT[layer as usize];
        let mut speed = self
            .col_stat
            .get(col as usize)
            .map(|cs| cs.max_speed_pct)
            .unwrap_or(1.0)
            * self.chars_per_sec
            * self.speed_mult
            * layer_speed;

        // Transition momentum: new-generation streams get a subtle velocity
        // boost during active transitions, creating a feeling of an incoming wave.
        if palette_slot == self.active_palette_slot && self.transition_start.is_some() {
            speed *= 1.0 + TRANSITION_VELOCITY_BOOST;
        }

        // Initialize turbulence: unique phase offset per droplet
        let turb_phase = (cp_idx as f32 * 0.73).fract() * std::f32::consts::TAU;

        DropletSpawnSpec {
            col,
            end_line,
            char_pool_idx: cp_idx,
            length: len,
            chars_per_sec: speed,
            time_to_linger: ttl,
            layer,
            tail_cells,
            palette_slot,
            turb_phase,
        }
    }

    pub(crate) fn spawn_droplets(&mut self, now: Instant, scale: f32) {
        let mut elapsed = now.saturating_duration_since(self.last_spawn_time);
        if self.max_sim_delta > Duration::from_millis(0) {
            elapsed = elapsed.min(self.max_sim_delta);
        }
        self.last_spawn_time = now;

        let elapsed_sec = elapsed.as_secs_f32();
        // Clamp spawn remainder to prevent debt accumulation at high speeds
        // or after timing spikes. Without this cap, a long stall could dump
        // hundreds of droplets in one frame, overwhelming the bottom rows.
        let clamped_remainder = self.spawn_remainder.min(SPAWN_REMAINDER_CAP);
        let budget = (elapsed_sec * self.droplets_per_sec * scale).max(0.0) + clamped_remainder;
        if !budget.is_finite() {
            self.spawn_remainder = 0.0;
            return;
        }
        let to_spawn = (budget.floor() as usize).min(self.droplets.len());
        self.spawn_remainder = (budget - (to_spawn as f32)).min(SPAWN_REMAINDER_CAP);
        if !self.spawn_remainder.is_finite() {
            self.spawn_remainder = 0.0;
        }
        if to_spawn == 0 {
            return;
        }

        // v30 Hinnant: hoist time computation out of the spawn loop. Previously
        // called per-droplet (5-30×/frame), each a `clock_gettime` syscall
        // (~20-50ns). `column_density_modifier` quantizes input into 10s
        // buckets, so the per-droplet calls were pure waste. At 30K FPS
        // benchmark mode this recovered ~9-22ms/sec of CPU on this single line.
        // CC-01: saturating_duration_since so bench-mode synthetic sim_now
        // degrades to 0.0 instead of underflowing.
        // BN-04 (Dragon Hunt v3): use `start_anchor` (captured once at
        // Cloud::new) instead of `Instant::now().saturating_duration_since(now)`.
        // The old formula returned frame-start-delta (~5-15µs in interactive
        // mode) which quantized to the same 10s bucket every frame — the
        // density-noise modifier was effectively static. Now it returns real
        // session-elapsed seconds, so the 10s-bucket drift actually varies
        // over the session (the intended behavior). Zero syscalls per frame.
        let now_secs_for_density = now
            .saturating_duration_since(self.start_anchor)
            .as_secs_f64();

        for _ in 0..to_spawn {
            let col = self.rand_col.sample(&mut self.mt);

            if col as usize >= self.col_stat.len() {
                continue;
            }

            if !self.col_stat[col as usize].can_spawn
                || self.col_stat[col as usize].num_droplets >= self.max_droplets_per_column
            {
                continue;
            }

            // v17 mastery: mouse spawn avoidance REMOVED. Owner reported rain
            // becoming empty under the cursor ("the rain becomes empty like
            // that"). The old MOUSE_AVOID_RADIUS_COLS check skipped
            // spawning within 5 columns of the cursor, creating a visible
            // empty zone that moved with the cursor. Removed for peak visual
            // continuity — rain now flows naturally through the cursor
            // position without gaps.

            // Atmospheric depth: apply per-layer density control.
            // Pre-determine the layer for this spawn to check density.
            //
            // v30 fix: distribution was [0.35, 0.40, 0.25] (mid-heavy),
            // mismatching build_droplet_spec()'s [0.35, 0.30, 0.35]
            // distribution used for the actual droplet layer assignment.
            // This caused the density gate to over-allocate spawns to the
            // mid layer (40% pass-through vs the intended 30%), amplifying
            // the "mid layer too noisy" complaint. Now unified to match
            // build_droplet_spec's distribution so the density gate and
            // actual layer assignment agree.
            let layer_roll = self.rand_chance.sample(&mut self.mt);
            let layer: u8 = if layer_roll < 0.35 {
                0
            } else if layer_roll < 0.65 {
                1
            } else {
                2
            };
            // Far layer (0) spawns less frequently
            let density_mult = PARALLAX_DENSITY_MULT[layer as usize];
            // Dynamic density noise: each column has a spatial modifier
            // in [DENSITY_NOISE_MIN, DENSITY_NOISE_MAX] that re-rolls every
            // DENSITY_NOISE_PERIOD_SECS. Kills the "uniform grid" feel
            // without per-frame allocation — single O(1) hash per spawn.
            let col_modifier =
                super::living_rain::column_density_modifier(col, now_secs_for_density);
            let effective_density = density_mult * col_modifier;
            if self.rand_chance.sample(&mut self.mt) > effective_density {
                continue;
            }

            // PERF: O(1) free-list pop replaces the previous O(N) linear scan
            // that searched droplets[] for the next !is_alive slot. The
            // free-list is seeded in reset() with 0..len and maintained
            // push-on-death / pop-on-spawn, so it always contains exactly
            // the dead droplet indices.
            let Some(di) = self.droplet_free_list.pop() else {
                break;
            };

            let spec = self.build_droplet_spec(col);
            let d = &mut self.droplets[di];
            spec.apply_to(d);
            d.activate(now);
            // Apply spawn phase jitter: randomize the fractional advance offset
            // so droplets don't all advance on the same frame cadence. This
            // breaks the "robotic march" where every stream moves its head on
            // the same tick, making the rain feel organic and alive.
            if SPAWN_PHASE_JITTER {
                let jitter = self.rand_chance.sample(&mut self.mt);
                d.apply_phase_jitter(jitter);
            }

            self.col_stat[col as usize].can_spawn = false;
            self.col_stat[col as usize].num_droplets += 1;
        }
    }
}
