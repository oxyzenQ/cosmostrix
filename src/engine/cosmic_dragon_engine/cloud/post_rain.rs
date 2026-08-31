// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-rain post-processing — extracted from `cloud/rain.rs` to keep
//! that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `Cloud::post_rain_processing()` — the anomaly events + glitch
//! timing + cinematic event engine + flash wave sweep block that runs
//! AFTER the droplet draw pass + phosphor decay, but BEFORE the message
//! overlay + periodic full redraw.
//!
//! Implemented as a separate `impl Cloud` block.

use std::time::Instant;

use rand::distr::Distribution;

use super::ecosystem::EmergentMoment;

use crate::constants::*;

impl super::Cloud {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn post_rain_processing(
        &mut self,
        frame: &mut crate::frame::Frame,
        now: Instant,
        enable_timing: bool,
        t1: Instant,
        phosphor_elapsed: f32,
        time_for_glitch: bool,
        glitch_due: bool,
        in_transition: bool,
    ) {
        // --- Rare anomaly events ---
        // Check for new anomaly spawn. The product of multipliers creates a
        // (Glitch-P1 fix): gate spawn by `perf_pressure < EVENT_PERF_GATE`,
        // mirroring ghost events (`cloud/ghost_events.rs:155`). Each active
        // anomaly writes ~12 KB/frame of cache-missed SGR bytes; without
        // this gate, anomalies continue to spawn under sustained CPU
        // overload, prolonging Tier 2 (xterm.js) backpressure recovery by
        // ~10-20%. Existing anomalies continue to apply (they have a 1.5s
        // lifetime cap), but no new ones spawn during overload.
        let anomaly_chance = (ANOMALY_CHANCE_PER_SEC
            * self.profile_current.anomaly_freq_mult as f64
            * (1.0 + self.entropy_drift.anomaly_offset as f64)
            * (1.0 + self.memory.instability_pressure as f64))
            .min(ANOMALY_CHANCE_PER_SEC * 3.0);
        // PERF-1: skip anomaly spawn in benchmark mode — anomalies are
        // cosmetics (luminance overlays, not rain simulation). Owner
        // directive: bench = rain + 3 dragons only.
        // PERF-4: skip anomaly spawn under --no-effects. The apply path
        // (apply_anomalies below) is already gated on effects_enabled,
        // but without this spawn gate, new anomaly zones continued to be
        // created + retained (1.5s lifetime) under --no-effects — wasted
        // CPU + Vec churn for zones that are never rendered. This closes
        // the last --no-effects partial-disable leak in the anomaly
        // subsystem (Z-master-1X audit). The comment at line 68 claiming
        // "spawn_anomaly is already gated" was stale and wrong — spawn
        // was bench-only-gated, not effects-gated.
        if !self.bench_mode
            && self.effects_enabled
            && phosphor_elapsed > 0.0
            && self.perf_pressure <= EVENT_PERF_GATE
            && !in_transition
            && (self.rand_chance.sample(&mut self.mt) as f64)
                <= anomaly_chance * phosphor_elapsed as f64
        {
            self.spawn_anomaly(now);
        }
        // Expire old anomaly zones
        self.anomaly_zones.retain(|z| {
            now.saturating_duration_since(z.start_time).as_secs_f32() < ANOMALY_DURATION_SECS
        });
        // PERF-1: skip anomaly apply in benchmark mode — cosmetics.
        // PERF-4: skip anomaly apply under --no-effects — anomaly halos
        // are cosmetic overlays (luminance blend on border cells), not
        // rain simulation. Both spawn and apply are now gated on
        // effects_enabled (Z-master-1X audit closed the spawn-side leak),
        // so under --no-effects no zones exist and this branch is a no-op.
        if !self.bench_mode && self.effects_enabled {
            self.apply_anomalies(frame, now);
        }

        // ── Cinematic Event Engine: render active events ──
        // PERF-4: skip ghost event rendering under --no-effects — ghost
        // events are cosmetic overlays (colored glyph streaks on the rain
        // field), not rain simulation. Ghost event SPAWNING is gated
        // separately (ghost_events.rs checks effects_enabled before
        // scheduling a new event); this gate covers the RENDER half so
        // already-scheduled events don't render their overlays while
        // effects are disabled. The events still expire on their normal
        // lifetime — they just don't paint.
        if !self.event_manager.is_empty() && self.effects_enabled {
            // Phase 3-I: same palette-aware ghost color as the pre-rain pass.
            let ghost_base_color =
                crate::chroma_dragon_engine::post::ghost::ghost_base_color(&self.palette.colors);
            let event_ctx = crate::cloud::ghost_events::EventCtx {
                cols: self.cols,
                lines: self.lines,
                ghost_base_color,
                color_pipeline: self.color_pipeline,
                now,
            };
            self.event_manager.render(&event_ctx, frame);

            // Recycle finished events.
            // v30 dragon-egg hunt: dropped the phosphor-seeding path that
            // fired on Active→Decay transitions (no event ever entered
            // Decay — see ghost_events.rs).
            self.event_manager.update(&event_ctx);
        }

        // --- Autonomous cinematic ecosystem tick ---
        // 1. Color ecosystem climate drift (luminance/saturation/hue only)
        // The ecosystem ticks unconditionally for climate drift — this only
        // modulates rendering params, not the palette scheme.
        //
        // Palette drift (scheme replacement) is handled by the Crystal Dragon
        // Engine (see step 1b below), gated so explicit CLI/config/profile
        // color remains sticky.
        //
        // ambient/crystal-dragon masterclass (v50.0.0-beta.7):
        // When both ambient AND crystal-dragon are enabled, Crystal Dragon
        // WINS — it can override the ambient palette at any time (sensor-
        // driven drift). But ambient can still revert via the snapback
        // mechanism (after ambient-snapback-secs of idle). This creates a
        // unique visual where colors change suddenly — the intended
        // consequence of two systems cooperating. Users who find this too
        // dynamic should turn off either ambient or crystal-dragon (not
        // both are needed). See docs/AMBIENT_SCHEDULER.md "Crystal Dragon
        // wins" section.
        //
        // Note: custom_palette_active is NOT a drift gate. When the user
        // explicitly enables --crystal-dragon with a custom palette (-c
        // tron_legacy), drift is allowed — the first drift event replaces
        // the custom palette with a builtin one via set_color_scheme (which
        // clears custom_palette_active). If the user doesn't want drift,
        // they should not enable --crystal-dragon.
        self.color_ecosystem.tick(now, &mut self.mt);

        // 1b. Crystal Dragon Engine drift
        // v50.0.0-beta.7 masterclass state machine: drift fires only when
        // crystal_dragon is enabled AND no drift is already active AND no
        // user override is pending. When drift fires, it sets drift_active
        // = true + drift_start = now. try_auto_snapback checks drift_start
        // and reverts after ambient-snapback-secs, clearing drift_active.
        // This gives a deterministic rhythm:
        //   60s ambient → drift fires → drift visible for snapback-secs →
        //   snapback reverts → drift_active cleared → next drift at +60s.
        // If snapback-secs >= 60, the next drift poll is skipped (drift still
        // active) — drift fires at +120s instead. This is by design.
        //
        // Z-master-1X bug fix (commit c12580a): the `user_override_since_ambient`
        // gate only applies when ambient is active. That flag is forced to
        // `true` at startup (event_loop_setup.rs — coredump fix, commit 2b0e28b)
        // and is only cleared by an ambient fire. When the ambient schedule is
        // empty, no ambient fire ever happens, so the flag stays `true`
        // forever and would permanently block crystal dragon drift despite
        // `crystal_dragon = true` in config. `ambient_schedule_active` is
        // the authoritative signal — when false, skip the user-override
        // check entirely.
        //
        // Z-master-1X round 2 fix: when ambient is OFF, try_auto_snapback
        // never runs (it early-returns on empty schedule), so drift_active
        // was never cleared after the first drift — permanently blocking
        // all subsequent drifts (owner symptom: "1 color change then nothing
        // for 5+ minutes"). The self-reset below clears drift_active after
        // CRYSTAL_DRAGON_POLLING_SECS of visibility, decoupling the drift
        // cycle from the ambient snapback mechanism. The 60s window matches
        // the polling cadence: drift is visible for one poll cycle, then the
        // cycle resets so the next poll can fire a new drift. When ambient
        // is ON, the snapback path (which reverts the palette AND clears
        // drift_active) takes precedence — the self-reset only fires if
        // snapback hasn't, which is the correct ordering (snapback at 30s
        // < self-reset at 60s).
        if self.crystal_dragon
            && !self.drift_active
            && (!self.user_override_since_ambient || !self.ambient_schedule_active)
        {
            if let Some(new_scheme) = self.crystal_dragon_tick(now) {
                self.set_color_scheme(new_scheme);
                self.user_override_since_ambient = true;
                self.drift_active = true;
                self.drift_start = Some(now);
            }
        }
        // Z-master-1X round 2: self-reset the drift cycle when ambient is
        // off. Without this, the first drift sets drift_active=true and no
        // mechanism ever clears it (try_auto_snapback early-returns on
        // empty schedule), so every subsequent poll hits the !drift_active
        // gate and is blocked. The 60s visibility window matches
        // CRYSTAL_DRAGON_POLLING_SECS so the next drift is eligible on the
        // very next poll after the cycle resets. When ambient is on, the
        // snapback path clears drift_active first (at ~30s) and this branch
        // is a no-op (drift_active already false).
        if self.drift_active && !self.ambient_schedule_active {
            if let Some(start) = self.drift_start {
                let drift_visible_secs = now.saturating_duration_since(start).as_secs_f32();
                if drift_visible_secs
                    >= crate::crystal_dragon_engine::crystal_dragon_control::CRYSTAL_DRAGON_POLLING_SECS
                {
                    self.drift_active = false;
                    self.drift_start = None;
                    // Reset the poll timer so the next drift fires 60s from
                    // now, not instantly (mirrors try_auto_snapback's
                    // crystal_dragon_last_poll reset at input.rs:534).
                    // Without this, the poll is already "due" and drift
                    // would re-fire on the very next frame, preventing the
                    // just-drifted palette from being visible.
                    self.crystal_dragon_last_poll = Some(now);
                }
            }
        }

        // 2. Entropy drift
        self.entropy_drift
            .tick(now, self.profile_current.entropy_rate);

        // 3. Renderer memory sampling
        let anomaly_density = self.anomaly_zones.len() as f32 / ANOMALY_MAX_ZONES.max(1) as f32;
        let rain_density = self.droplet_density;
        self.memory.record_sample(
            now,
            anomaly_density,
            rain_density,
            self.color_ecosystem.luminance_climate,
        );
        self.memory.recompute_derived();

        // 4. Emergent storytelling
        // PERF-1-Supreme: skip the storytelling engine entirely in
        // benchmark mode. Emergent moments (LuminanceSwell,
        // DensityPulse, TemporalDilation) are cinematic "emotionally
        // resonant" events that perturb spawn density, luminance and
        // speed — owner directive: bench measures the critical path
        // only (rain + 3 dragon engines), so no moments may spawn and
        // no per-frame moment bookkeeping may run during measurement.
        // PERF-4: skip under --no-effects — emergent moments perturb
        // render params (density, luminance, speed) purely for
        // cinematic effect, not for rain accuracy.
        if !self.bench_mode && self.effects_enabled {
            if let Some(kind) = self.storytelling.tick(
                now,
                &mut self.mt,
                &self.entropy_drift,
                &self.memory,
                &self.color_ecosystem,
            ) {
                self.storytelling.moments.push(EmergentMoment {
                    kind,
                    start_time: now,
                    duration: EMERGENT_MOMENT_DURATION_SECS,
                });
                self.storytelling.cooldown_until = Some(
                    now + std::time::Duration::from_secs_f32(EMERGENT_MOMENT_DURATION_SECS + 60.0),
                );
            }
            self.storytelling.expire_moments(now);
        }

        // 5. Profile interpolation (smooth transition)
        if let Some(transition_start) = self.profile_transition_start {
            let elapsed = now
                .saturating_duration_since(transition_start)
                .as_secs_f32();
            let t = (elapsed / PROFILE_TRANSITION_SECS).min(1.0);
            // Min-rate-floor smoothstep interpolation: PROFILE_INTERPOLATION_RATE
            // (0.02) floors the per-frame lerp factor so the transition does
            // not stall at very small t values. The smoothstep curve still
            // governs the upper range.
            let t = t * t * (3.0 - 2.0 * t);
            self.profile_current = super::ecosystem::lerp_profile_params(
                self.profile_current,
                self.profile_target,
                PROFILE_INTERPOLATION_RATE.max(t),
            );
            if t >= 1.0 {
                self.profile_current = self.profile_target;
                self.profile_transition_start = None;
            }
        }

        // 7. (removed) Apply global atmospheric frame effects.
        //    Was a no-op post-hoc pass — climate effects are now applied
        //    in the shader pipeline at resolve_cell_color via
        //    chroma::post::climate::apply_climate. The post-hoc function
        //    `apply_climate_frame_effects` was deleted; see the Phase 3-G
        //    note at the atmospheric ctx construction above.

        // 8. Draw message box LAST — survives phosphor, anomaly, atmospheric.
        // Glow (60% white blend) + style-driven reveal (msg-fill-style,
        // default typewriter 80 ms/char; engrave adds its spark pass
        // at the end of draw_message; hologram adds its scanline pass;
        // glitch's wrong-glyph substitution is part of the reveal math
        // itself, no extra pass; scorch adds its smoke pass at the end
        // of draw_message and tints the cooling chars via CellReveal;
        // cascade reuses the signed slide_rows field for drop-from-above,
        // handled by the slide deferred second pass).
        // Z-6: skip in benchmark mode — owner directive: bench measures
        // critical path only (rain + 3 dragons), not message cosmetics.
        // This eliminates 8 per-frame heap allocs in draw_message.
        if !self.bench_mode && !self.message.is_empty() {
            self.draw_message(frame, now);
        }

        // --- Periodic full redraw for ANSI drift correction ---
        // Every N frames, force a complete screen refresh. This corrects any
        // accumulated terminal state desync (e.g., from resize, scroll, or
        // rare edge cases in differential rendering) without measurable perf
        // impact since full redraws are already optimized with row batching.
        self.frames_since_full_redraw += 1;
        if self.frames_since_full_redraw >= FULL_REDRAW_INTERVAL_FRAMES {
            self.frames_since_full_redraw = 0;
            self.force_draw_everything = true;
        }

        if time_for_glitch || glitch_due {
            self.last_glitch_time = now;
            let ms = self.rand_glitch_ms.sample(&mut self.mt) as u64;
            self.next_glitch_time = self.last_glitch_time + std::time::Duration::from_millis(ms);
        }

        // ── Cinematic Event Engine ──
        // v30 dragon-egg hunt: removed clean_stale_phosphor() call.
        // The function was unreachable in practice — it only cleared
        // phosphor cells with energy ≤ EVENT_PHOSPHOR_SEED_ENERGY, but
        // such cells are only ever set by event.seed_phosphor() (which
        // was a no-op for GhostEvent). The function itself was removed
        // from GhostEventScheduler.

        // v30 fix: sweep ALL active flash waves, not single slot.
        // Each wave expires independently after MOUSE_FLASH_DURATION_SECS.
        // Multiple concurrent waves (from rapid clicks) each get their own
        // lifetime — no more reset-on-second-click.
        for w in &mut self.flash_waves {
            if w.active
                && now.saturating_duration_since(w.birth).as_secs_f32() >= MOUSE_FLASH_DURATION_SECS
            {
                w.active = false;
            }
        }

        // ── Component timing: render end ────────────────────────────────
        // Capture render_ms AFTER all frame mutations complete. Anything
        // after this point (flash expiry bookkeeping) is trivial scalar
        // work and not worth attributing to either half.
        //
        // P1: only capture t2 when component timing is enabled.
        if enable_timing {
            let t2 = Instant::now();
            self.last_render_ms = t2.saturating_duration_since(t1).as_secs_f64() * 1000.0;
        }
    }
}
