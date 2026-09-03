// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Pause/resume easing — extracted from `cloud/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `Cloud::toggle_pause()` — the pause/resume state machine with
//! exponential decay easing (BRANCH 1: abort decel → resume, BRANCH 2:
//! pause → start decel, BRANCH 3: resume → start accel).
//!
//! Implemented as a separate `impl Cloud` block.

use std::time::Instant;

#[allow(unused_imports)]
use rand::distr::{Distribution, Uniform};

impl super::Cloud {
    pub fn toggle_pause(&mut self) -> bool {
        // BRANCH 1: mid-deceleration → abort & resume.
        //
        // When the user presses 'p' during deceleration, they're
        // cancelling the pause. This typically happens during rapid
        // p-taps. The old code captured the current pause_blend as
        // resume_blend_start (which could be near 0 after significant
        // deceleration), causing a slow ramp from ~0→1.0 that made
        // the rain look "stuck" for seconds (owner-reported bug).
        //
        // Fix: snap resume_blend to 1.0 (full speed) immediately.
        // The deceleration was aborted — there's no visual discontinuity
        // because pause_blend was still close to 1.0 for rapid taps.
        if self.pause_start.is_some() {
            self.pause_start = None;
            self.pause = false;
            self.pause_time = None;
            self.resume_blend = 1.0;
            self.resume_start = None;
            return true;
        }
        // BRANCH 2: fully paused → unpause. Shift every last_*_time
        // forward by pause duration + visual-subsystem timestamps (§8.5).
        if self.pause {
            self.pause = false;
            if let Some(pt) = self.pause_time.take() {
                let now = Instant::now();
                let elapsed = now.saturating_duration_since(pt);
                self.last_spawn_time = now;
                self.spawn_remainder = 0.0;
                for d in &mut self.droplets {
                    if d.is_alive {
                        d.increment_time(elapsed);
                        d.last_time = Some(now);
                        // randomize advance_remainder on resume (was 0,
                        // caused lockstep "loncat" pops). Jitter spreads them,
                        // matching apply_phase_jitter's per-droplet phase.
                        d.advance_remainder = self.rand_chance.sample(&mut self.mt);
                    }
                }
                // §H10: shift monolith streams' last_time forward by
                // pause duration (was "safe by accident" via resume_blend=0).
                self.monolith_rain.shift_active_streams_last_time(elapsed);
                self.last_phosphor_time += elapsed;
                self.last_quantum_update_time += elapsed;
                // S-master-HUNT-22: shift the msg-fill particle clocks
                // (engrave sparks / scorch smoke) too — same §8.5 family
                // as last_quantum_update_time above. Their dt is now real
                // time bounded by PARTICLE_MAX_FRAME_DT_SECS, so a stale
                // last_update would burn up to 250ms of the anti-teleport
                // budget on the first post-unpause frame for any sparks
                // or smoke that were mid-flight when the pause settled.
                self.engrave.last_update += elapsed;
                self.scorch.last_update += elapsed;
                self.last_glitch_time += elapsed;
                self.next_glitch_time += elapsed;
                self.last_reseed_time += elapsed;
                self.color_ecosystem.shift_in_time(elapsed);
                self.crystal_dragon_sensor.shift_in_time(elapsed);
                if let Some(ref mut cd) = self.crystal_dragon_last_poll {
                    *cd += elapsed;
                }
                if let Some(ref mut d) = self.drift_start {
                    *d += elapsed;
                }
                self.entropy_drift.last_tick += elapsed;
                self.memory.last_sample += elapsed;
                self.storytelling.last_tick += elapsed;
                if let Some(ref mut cd) = self.storytelling.cooldown_until {
                    *cd += elapsed;
                }
                if let Some(ref mut ts) = self.transition_start {
                    *ts += elapsed;
                }
                if let Some(ref mut pt) = self.profile_transition_start {
                    *pt += elapsed;
                }
                if let Some(ref mut ct) = self.charset_transition_start {
                    *ct += elapsed;
                }
                // §8.5: shift visual-subsystem timestamps so they don't
                // skip ahead on resume.
                if let Some(ref mut mt) = self.message_start_time {
                    *mt += elapsed;
                }
                if let Some(ref mut ge) = self.glyph_entry_time {
                    *ge += elapsed;
                }
                // v30 fix: shift ALL active flash wave births (was single slot).
                for w in &mut self.flash_waves {
                    if w.active {
                        w.birth += elapsed;
                    }
                }
                // v30 fix: shift active quantum particle births too. Without
                // this, particles spawned before pause instantly expire on
                // unpause (age includes pause duration, exceeding 0.8s life).
                for p in &mut self.quantum_particles {
                    if p.active {
                        p.birth += elapsed;
                    }
                }
                self.resume_blend_start = 0.0;
                self.resume_blend = 0.0;
                self.resume_start = Some(now);
                true
            } else {
                true
            }
        } else {
            // BRANCH 3: running → start deceleration. Clear stale
            // resume_start (audit §8.3 — rapid triple-tap state hygiene).
            self.pause_start = Some(Instant::now());
            self.resume_start = None;
            true
        }
    }
}
