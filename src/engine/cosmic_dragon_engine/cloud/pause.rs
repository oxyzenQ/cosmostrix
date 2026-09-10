// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Pause/resume easing — extracted from `cloud/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `Cloud::toggle_pause()` — the pause/resume state machine with
//! exponential decay easing (BRANCH 1: abort decel → FAST smooth resume
//! ramp from the current blend — NIGHT-hunter-8, BRANCH 2: pause →
//! start decel, BRANCH 3: resume → start accel).
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
        // p-taps.
        //
        // NIGHT-hunter-8 (owner report: "little jump" on rapid p-taps):
        // the old fix captured the current pause_blend as
        // resume_blend_start (near 0 after significant deceleration)
        // and the slow wake-up ramp made the rain look "stuck" for
        // seconds; the later fix snapped resume_blend to 1.0, which
        // traded the stuck bug for a hard velocity jump — a tap at
        // t≈1s into the decel snapped the rain from ~30% to 100% speed
        // in a single frame.
        //
        // The smooth fix: start a resume ramp FROM the current decel
        // blend (§8.4 interpolation already supports a non-zero start)
        // with the FAST abort rate (RESUME_ABORT_EASE_DECAY_RATE — see
        // its doc for the derivation). The blend is continuous with the
        // decel value at the abort instant, the recovery completes in
        // ~0.5s, and rain_at's rate branch keys off
        // resume_blend_start > 0 to pick the fast constant.
        if self.pause_start.is_some() {
            self.pause_start = None;
            self.pause = false;
            self.pause_time = None;
            self.resume_blend_start = self.resume_blend.max(0.05);
            self.resume_start = Some(Instant::now());
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
                        // NIGHT-hunter-28 (owner report: "for resume like
                        // still have small jump so feel not smooth
                        // elegantly when see seriously high detail"):
                        // the phase is PRESERVED, not re-randomized.
                        //
                        // Droplet::head_brightness() is driven by
                        // advance_remainder (the "energy building"
                        // ramp: brightness = 1.0 + remainder × 0.15),
                        // and the remainder also decides WHEN the next
                        // row advance lands. The old re-randomization
                        // (`d.advance_remainder = rand_chance.sample()`)
                        // therefore reshuffled EVERY droplet's head
                        // brightness by up to ±15% and its advance
                        // timing by up to a full row IN ONE FRAME — a
                        // global shimmer/pop exactly at the resume
                        // instant, most visible at high detail (many
                        // heads + long tails + phosphor afterglow).
                        //
                        // The freeze already preserves the spread:
                        // SPAWN_PHASE_JITTER=true staggers every
                        // droplet's remainder at spawn, and the pause
                        // simply freezes it mid-phase — resuming from
                        // the frozen values is continuous in BOTH
                        // brightness and advance timing (C0 in the
                        // position, C0 in the visual ramp). The
                        // lockstep this line once guarded against came
                        // from an older resume path that ZEROED the
                        // remainders ("was 0, caused lockstep 'loncat'
                        // pops") — dropping to a fixed constant is
                        // what synchronized them, and the jitter spread
                        // was the workaround. Preserving the frozen
                        // phase is strictly better: no lockstep (the
                        // spawn-time spread survives) AND no pop.
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
                // NIGHT-hunter-8 (pause/resume smoothness audit): the
                // wind-gust state machine and the cinematic event clocks
                // were NOT in the §8.5 shift family. On resume, gust's
                // first tick compared now - phase_start against a
                // phase_duration that the pause had already exceeded —
                // one transition fired per tick, so a gust mid-Attack
                // (multiplier 0.6) jumped straight to its Hold PEAK (up
                // to 1.8x) in a single frame, a visible spawn surge; a
                // gust mid-Decay snapped to idle 1.0. Ghost events aged
                // by the full pause duration and expired instantly — a
                // pop-out instead of their fade-out. Both now shift by
                // the pause duration, continuing exactly where they
                // froze.
                self.gust.shift_in_time(elapsed);
                self.event_manager.shift_in_time(elapsed);
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
