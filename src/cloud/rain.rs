// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Main render loop: rain() and rain_at().

use std::time::Instant;

use crossterm::style::Color;
use rand::distr::Distribution;

use crate::constants::*;
use crate::frame::Frame;

use super::ecosystem::EmergentMoment;
use super::render::DrawCtx;
use super::Cloud;

impl Cloud {
    pub fn rain(&mut self, frame: &mut Frame) {
        self.rain_at(frame, Instant::now());
    }

    pub fn rain_at(&mut self, frame: &mut Frame, now: Instant) {
        if self.pause {
            return;
        }

        // Update color transition: during a palette transition, check if the
        // wave has completed (all rows have adopted the new palette).
        // The visual wave is driven by color_wave_line_at() in DrawCtx;
        // here we just detect completion and update droplet palette slots
        // for streams that are now fully above the wave.
        if let Some(transition_start) = self.transition_start {
            let elapsed_ms = now.saturating_duration_since(transition_start).as_millis() as u64;
            if elapsed_ms >= COLOR_TRANSITION_DURATION_MS as u64 {
                // Transition complete: all droplets adopt the new palette
                for d in &mut self.droplets {
                    if d.is_alive {
                        d.palette_slot = self.active_palette_slot;
                    }
                }
                self.transition_start = None;
            }
        }

        let charset_wave_line = self.charset_wave_line_at(now);
        if self.charset_transition_start.is_some_and(|start| {
            now.saturating_duration_since(start).as_millis()
                >= CHARSET_TRANSITION_DURATION_MS as u128
        }) {
            self.charset_transition_start = None;
            self.previous_char_pool.clear();
        }

        // Periodically re-seed RNG for very long sessions
        self.maybe_reseed_rng(now);

        // Advance cinematic resume easing: smoothstep S-curve from 0→1 over
        // RESUME_EASE_DURATION_SECS (300ms) after unpause. Unlike exponential
        // easing or position-delta scaling, this interpolates the simulation
        // time scale itself — the physics clock runs in slow motion during
        // the transition, producing genuine inertia recovery rather than a
        // frozen-then-unfrozen snap.
        if let Some(rs) = self.resume_start {
            let t = now.saturating_duration_since(rs).as_secs_f32();
            let normalized = (t / RESUME_EASE_DURATION_SECS).min(1.0);
            // Smoothstep: 3t² - 2t³ — slow start, fast middle, slow end.
            self.resume_blend = normalized * normalized * (3.0 - 2.0 * normalized);
            if normalized >= 1.0 {
                self.resume_blend = 1.0;
                self.resume_start = None; // Transition complete — stop tracking
            }
        }

        let mut spawn_scale = (1.0 - (PERF_PRESSURE_SPAWN_FACTOR * self.perf_pressure))
            .clamp(PERF_SPAWN_SCALE_MIN, 1.0);
        // Apply atmospheric density modulation
        spawn_scale *= 1.0 + self.atmosphere.density_offset;
        // Apply profile density modulation
        spawn_scale *= self.profile_current.density_mult;
        // Apply emergent density boost
        spawn_scale += self.storytelling.active_effects(now).density_boost;
        // Apply resume time-scale easing: spawn rate ramps with the smoothstep
        // curve so new streams appear gradually during the inertia recovery.
        spawn_scale *= self.resume_blend;
        spawn_scale = spawn_scale.clamp(0.0, 3.0);
        self.spawn_droplets(now, spawn_scale);

        // Process pending semantic invalidation BEFORE force_draw_everything.
        // Semantic mutations (charset switch, shading mode toggle) require
        // invalidate_semantic() which bumps semantic_gen, ensuring the
        // Terminal's LastFrame cache is fully synchronized.
        // Also clear stale ghost glyph characters to prevent the full redraw
        // from exposing phosphor_base_ch entries as visible background charset
        // glyphs — the same "ghost background" bug that affects
        // force_draw_everything. Active trail cells will have their
        // phosphor_base_ch repopulated by Pass 1 (current-gen cells) and
        // Pass 2 (active droplet trail protection) of phosphor_decay_pass.
        if self.semantic_invalidate {
            self.semantic_invalidate = false;
            frame.invalidate_semantic(self.palette.bg);
            for ch in self.phosphor_base_ch.iter_mut() {
                *ch = '\0';
            }
        }

        let force_draw_everything = self.force_draw_everything;
        if force_draw_everything {
            frame.clear_with_bg(self.palette.bg);
            // Clear stale ghost glyph characters on force_draw_everything.
            // Without this, a full redraw (triggered by paste, focus regain,
            // idle resync, etc.) would expose all phosphor_base_ch entries
            // as visible background charset glyphs — the "ghost background"
            // bug. Active trail cells will have their phosphor_base_ch
            // repopulated by Pass 1 (current-gen cells) and Pass 2 (active
            // droplet trail protection) of phosphor_decay_pass, so clearing
            // here only affects stale afterglow cells that should not render
            // character glyphs during a full redraw.
            for ch in self.phosphor_base_ch.iter_mut() {
                *ch = '\0';
            }
            self.force_draw_everything = false;
        }

        let glitch_due = self.time_for_glitch(now);
        let allow_glitch = glitch_due && self.perf_pressure < GLITCH_THRESHOLD;
        let time_for_glitch = allow_glitch;

        let max_sim_delta = self.max_sim_delta;
        let use_sim_cap = max_sim_delta > std::time::Duration::from_millis(0);

        // Update pass (mut self)
        for i in 0..self.droplets.len() {
            if !self.droplets[i].is_alive {
                continue;
            }

            let (col, start_line, hp, cp_idx, free_col, died) = {
                let d = &mut self.droplets[i];
                let adv_now = if use_sim_cap {
                    if let Some(last) = d.last_time {
                        let max_now = last + max_sim_delta;
                        if now > max_now {
                            max_now
                        } else {
                            now
                        }
                    } else {
                        now
                    }
                } else {
                    now
                };
                let free_col = d.advance(adv_now, self.lines, self.resume_blend);
                let col = d.bound_col;
                let start_line = d.tail_put_line.map(|v| v + 1).unwrap_or(0);
                let hp = d.head_put_line;
                let cp_idx = d.char_pool_idx;
                let died = !d.is_alive;
                (col, start_line, hp, cp_idx, free_col, died)
            };

            if died {
                if let Some(cs) = self.col_stat.get_mut(col as usize) {
                    cs.num_droplets = cs.num_droplets.saturating_sub(1);
                    cs.can_spawn = true;
                }
                continue;
            }

            if free_col {
                self.set_column_spawn(col, true);
            }

            if time_for_glitch {
                self.do_glitch_span(start_line, hp, col, cp_idx);
            }
        }

        // Build palette_slices for DrawCtx from the palette table.
        // Each slot either has a Palette (Some) or is empty (None) — use an
        // empty slice for empty slots so hot-path rendering stays branch-free.
        let empty: &[Color] = &[];
        let mut palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [&[]; MAX_PALETTE_SLOTS];
        for (i, slot) in palette_slices.iter_mut().enumerate() {
            if let Some(ref p) = self.palette_table[i] {
                *slot = &p.colors;
            } else {
                *slot = empty;
            }
        }

        let transitioning = self.transition_start.is_some();
        let charset_wave_line = if self.charset_transition_start.is_some() {
            charset_wave_line
        } else {
            None
        };
        let color_wave_line = self.color_wave_line_at(now);

        // Draw pass (split-borrows via DrawCtx)
        let draw_everything = force_draw_everything || time_for_glitch;
        let ctx = DrawCtx {
            lines: self.lines,
            full_width: self.full_width,
            shading_distance: self.shading_distance,
            bg: self.palette.bg,
            color_mode: self.color_mode,
            bold_mode: self.bold_mode,
            glitchy: self.glitchy,
            last_glitch_time: self.last_glitch_time,
            next_glitch_time: self.next_glitch_time,
            palette_slices,
            active_palette_slot: self.active_palette_slot,
            transitioning,
            color_map: &self.color_map,
            glitch_map: &self.glitch_map,
            char_pool: &self.char_pool,
            previous_char_pool: &self.previous_char_pool,
            charset_wave_line,
            color_wave_line,
            mouse_col: self.mouse_col,
            mouse_line: self.mouse_line,
            flash_col: self.flash_col,
            flash_line: self.flash_line,
            flash_time: self.flash_time,
        };

        for d in &mut self.droplets {
            let needs_tail_cleanup = !d.is_alive
                && d.bound_col != u16::MAX
                && d.tail_put_line.is_some_and(|tp| d.tail_cur_line != tp);

            if d.is_alive || needs_tail_cleanup {
                d.draw(&ctx, frame, now, draw_everything);
            }

            if !d.is_alive {
                d.bound_col = u16::MAX;
            }
        }

        if !self.message.is_empty() {
            self.draw_message(frame);
        }

        // --- Phosphor persistence post-process ---
        // Scale phosphor decay elapsed by resume_blend so afterglow fades at
        // the same rate as the rain wakes up. Without this, phosphor trails
        // vanish at full speed while droplets move in slow motion — creating
        // temporal inconsistency that feels "spiky" during resume.
        let phosphor_elapsed = now
            .saturating_duration_since(self.last_phosphor_time)
            .as_secs_f32()
            * self.resume_blend;
        self.last_phosphor_time = now;
        self.phosphor_decay_pass(frame, phosphor_elapsed);

        // --- Rare anomaly events ---
        // Check for new anomaly spawn. The product of multipliers creates a
        // positive feedback loop (more anomalies → higher instability → more
        // anomalies). Cap the effective rate at 3× base to prevent visual
        // overload while preserving atmospheric dynamics.
        let anomaly_chance = (ANOMALY_CHANCE_PER_SEC
            * self.profile_current.anomaly_freq_mult as f64
            * (1.0 + self.atmosphere.anomaly_offset as f64)
            * (1.0 + self.memory.instability_pressure as f64))
            .min(ANOMALY_CHANCE_PER_SEC * 3.0);
        if phosphor_elapsed > 0.0
            && (self.rand_chance.sample(&mut self.mt) as f64)
                <= anomaly_chance * phosphor_elapsed as f64
        {
            self.spawn_anomaly(now);
        }
        // Expire old anomaly zones
        self.anomaly_zones.retain(|z| {
            now.saturating_duration_since(z.start_time).as_secs_f32() < ANOMALY_DURATION_SECS
        });
        // Apply anomaly effects to frame
        self.apply_anomalies(frame, now);

        // --- Phase 3: Autonomous cinematic ecosystem tick ---
        // 1. Color ecosystem drift
        if let Some(new_scheme) = self
            .color_ecosystem
            .tick(now, &mut self.mt, self.color_scheme)
        {
            self.set_color_scheme(new_scheme);
        }

        // 2. Atmospheric evolution
        self.atmosphere.tick(now, self.profile_current.entropy_rate);

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
        if let Some(kind) = self.storytelling.tick(
            now,
            &mut self.mt,
            &self.atmosphere,
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

        // 5. Profile interpolation (smooth transition)
        if let Some(transition_start) = self.profile_transition_start {
            let elapsed = now
                .saturating_duration_since(transition_start)
                .as_secs_f32();
            let t = (elapsed / PROFILE_TRANSITION_SECS).min(1.0);
            // Smooth step interpolation
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

        // 7. Apply Phase 3 global atmospheric frame effects (post-process)
        self.apply_atmospheric_frame_effects(frame, now);

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
        // Expire flash effect after duration
        if let Some(flash_time) = self.flash_time {
            if now.saturating_duration_since(flash_time).as_secs_f32() >= MOUSE_FLASH_DURATION_SECS
            {
                self.flash_time = None;
            }
        }
    }
}
