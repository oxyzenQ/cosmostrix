// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Main render loop: `rain_at()`.
//!
//! The no-arg `rain()` wrapper was removed in a deep zombie audit
//! (only tests called it; production calls `rain_at(...)` directly with
//! a captured `Instant`). Tests that need the convenience wrapper can
//! call `rain_at(frame, Instant::now())`.

use std::time::Instant;

use crossterm::style::Color;
use rand::distr::Distribution;

use crate::constants::*;
use crate::frame::Frame;
use crate::rain_style::RainStyle;

use super::ecosystem::EmergentMoment;
use super::monolith::{MonolithCleanup, MonolithRandom, MonolithSpawnParams};
use super::render::{DrawCtx, FlashWaveCtx};
use super::Cloud;
use smallvec::SmallVec;

// Phase 3-G: atmospheric ctx for integrated post-processing.
use crate::chroma_dragon_engine::post::climate::ClimateCtx;

impl Cloud {
    /// No-arg convenience wrapper around `rain_at`. Test-only — production
    /// callers pass an explicit `Instant` captured before the frame work
    /// begins (so the same instant is reused for the surrounding timing
    /// measurement, see `event_loop.rs::rain_at(frame, work_start)`).
    #[cfg(test)]
    pub fn rain(&mut self, frame: &mut Frame) {
        self.rain_at(frame, Instant::now());
    }

    pub fn rain_at(&mut self, frame: &mut Frame, now: Instant) {
        // Defensive invariant (audit §8.6): pause_start and resume_start
        // are mutually exclusive. toggle_pause() guarantees this via
        // explicit `pause_start = None` / `resume_start = None` clears on
        // every branch, but a stale-state bug across rapid triple-taps
        // could in principle leave both set — which would cause both
        // easing blocks below to compute simultaneously, producing
        // nonsensical resume_blend values (decel fighting accel).
        // debug_assert! is zero-cost in release builds.
        debug_assert!(
            !(self.pause_start.is_some() && self.resume_start.is_some()),
            "pause_start and resume_start cannot coexist — toggle_pause() must clear one before setting the other"
        );

        if self.pause {
            return;
        }

        // v17 mastery → v50.0.0-beta.5 masterclass: pause ease-OUT (deceleration).
        // If pause_start is set, compute pause_blend via exponential decay
        // (exp(-k·t), k = PAUSE_EASE_DECAY_RATE) — physically motivated
        // drag with a long tail, matching the README's "exponential
        // deceleration (~3s coast-down)" promise. At k=1.2, blend reaches
        // PAUSE_EASE_SETTLE_FRAC (5%) at t≈2.5s, then snaps to fully
        // paused (avoids exp's asymptotic tail where the last 5% takes
        // forever to actually stop).
        //
        // Replaces the prior smootherstep S-curve (6t⁵-15t⁴+10t³, 0.30s)
        // which felt abrupt at the end-snap; the long tail here feels
        // like genuine inertia coast-down.
        if let Some(ps) = self.pause_start {
            let t = now.saturating_duration_since(ps).as_secs_f32();
            let pause_blend = (-PAUSE_EASE_DECAY_RATE * t).exp();
            // pause_blend goes 1→0 (deceleration). Store as resume_blend
            // so the rest of rain_at naturally scales with the coast-down
            // (spawn, advance, phosphor decay all multiply by resume_blend).
            self.resume_blend = pause_blend;
            if pause_blend <= PAUSE_EASE_SETTLE_FRAC {
                // Settled — fully paused now. Snap clean so other
                // subsystems see `self.pause = true` (monolith shift,
                // spawn_remainder reset, phosphor LUT, etc.).
                self.pause = true;
                self.pause_start = None;
                self.pause_time = Some(now);
                self.resume_blend = 0.0;
                return;
            }
        }

        // ── Per-frame component timing ──────────────────────────────────
        // t0 = start of rain_at. t1 will be captured just before the first
        // frame-mutating render step (phosphor_decay_pass). t2 = end of
        // rain_at. sim_ms = (t1 - t0), render_ms = (t2 - t1). The benchmark
        // reads these via last_sim_ms() / last_render_ms() to produce a
        // sub-component timing breakdown without external instrumentation.
        //
        // Bug fix (Strategy D root-cause): t0 was previously `now` (the
        // caller-passed parameter). In benchmark mode, the caller passes
        // `sim_now` — a synthetic time incremented by target_period each
        // frame. When bench FPS >> target FPS (e.g., 65K FPS vs 60 FPS
        // target), sim_now races ahead of real time by ~16.6ms per frame.
        // This made `t1.saturating_duration_since(t0)` saturate to 0,
        // reporting sim_ms = 0. The actual sim work (~9.4µs/frame in
        // monolith) was silently dumped into the benchmark's io_ms residual
        // bucket, creating a phantom "85% IO bottleneck" that was really
        // sim work mislabeled as IO.
        //
        // Fix: capture t0 = Instant::now() when enable_timing. This adds
        // ~20ns per frame in benchmark mode only (interactive mode skips
        // timing entirely). Interactive mode is unaffected because rain()
        // passes Instant::now() as `now`, so t0 ≈ now within ~20ns anyway.
        let enable_timing = self.enable_component_timing;
        let t0 = if enable_timing { Instant::now() } else { now };

        // ── Cinematic Event Engine: evaluate triggers ──
        let in_transition = self.transition_start.is_some()
            || self.charset_transition_start.is_some()
            || self.profile_transition_start.is_some();
        self.event_manager.evaluate_triggers(
            self.perf_pressure,
            self.cols,
            self.lines,
            self.pause,
            in_transition,
        );

        // Update color transition: during a palette transition, check if the
        // wave has completed (all rows have adopted the new palette).
        // The visual wave is driven by color_wave_line_at() in DrawCtx;
        // here we just detect completion and update droplet palette slots
        // for streams that are now fully above the wave.
        if let Some(transition_start) = self.transition_start {
            let elapsed_ms = now.saturating_duration_since(transition_start).as_millis() as u64;
            if elapsed_ms >= COLOR_TRANSITION_DURATION_MS as u64 {
                // Transition complete: all active streams adopt the new palette.
                if matches!(self.rain_style, RainStyle::Monolith) {
                    self.monolith_rain
                        .adopt_palette_slot(self.active_palette_slot);
                } else {
                    for d in &mut self.droplets {
                        if d.is_alive {
                            d.palette_slot = self.active_palette_slot;
                        }
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

        // Advance cinematic resume easing: exponential decay approach to
        // 1.0 (v50.0.0-beta.5 masterclass). At k = RESUME_EASE_DECAY_RATE (0.9/sec),
        // the blend rises to RESUME_EASE_SETTLE_FRAC (95%) at t≈3.3s,
        // then snaps to full speed. Slightly slower than the pause ramp
        // (k=1.2), preserving the asymmetric "pause feels snappy / resume
        // feels like a wake-up" feel from the prior 0.30s/0.45s duration
        // ratio. This interpolates the simulation time scale itself —
        // the physics clock runs in slow motion during the transition,
        // producing genuine inertia recovery rather than a frozen-then-
        // unfrozen snap.
        //
        // §8.4 preserved: interpolate from `resume_blend_start` → 1.0
        // rather than always 0 → 1.0 (lets aborted-decel resumes start
        // at e.g. 0.4 — the abort path snaps resume_blend to 1.0 in
        // toggle_pause() BRANCH 1, but the resume_blend_start field is
        // still consulted here for forward-compat with hybrid resume
        // paths that may set a partial-start value).
        //
        // The 0.05 floor is kept as a safety net for the very first
        // frame (exp decay has no flat-start window — it rises
        // immediately — but the floor guards against any future path
        // that leaves resume_blend_start at 0 with resume_start just
        // set, before the easing branch updates it).
        if let Some(rs) = self.resume_start {
            let t = now.saturating_duration_since(rs).as_secs_f32();
            // exp-IN: 1 - exp(-k*t). Rises from 0 (t=0) toward 1 asymptotically.
            let approach = 1.0 - (-RESUME_EASE_DECAY_RATE * t).exp();
            let start = self.resume_blend_start;
            self.resume_blend = (start + (1.0 - start) * approach).max(0.05);
            if self.resume_blend >= RESUME_EASE_SETTLE_FRAC {
                // Settled — snap to full speed (avoids exp's asymptotic tail).
                self.resume_blend = 1.0;
                self.resume_start = None;
            }
        }

        // BN-03 (Dragon Hunt v3): hoist `active_effects(now)` once — was
        // called twice (here + line ~520) with the same `now` and no
        // `self.storytelling` mutation between them (`tick()` is at line ~950,
        // strictly after both reads). The result is invariant, so a single
        // call + Copy-binding suffices.
        let emergent_effects = self.storytelling.active_effects(now);

        // AB-11 (dragon power audit, option 2): when the self-healer has set
        // aggressive_throttle, use a steeper curve (0.9 vs 0.75) + lower floor
        // (0.10 vs 0.25) to shed more spawn load. This does NOT touch the
        // user's density setting — only the spawn rate multiplier is affected.
        let (factor, floor) = if self.aggressive_throttle {
            (
                PERF_PRESSURE_SPAWN_FACTOR_AGGRESSIVE,
                PERF_SPAWN_SCALE_MIN_AGGRESSIVE,
            )
        } else {
            (PERF_PRESSURE_SPAWN_FACTOR, PERF_SPAWN_SCALE_MIN)
        };
        let mut spawn_scale = (1.0 - (factor * self.perf_pressure)).clamp(floor, 1.0);
        // Apply atmospheric density modulation
        spawn_scale *= 1.0 + self.entropy_drift.density_offset;
        // Apply profile density modulation
        spawn_scale *= self.profile_current.density_mult;
        // Apply wind-gust multiplier (1.0 when idle, up to GUST_PEAK_MAX
        // during a gust). Independent of `entropy_drift.density_offset`
        // (slow entropy cycle) — gusts are short, sharp surges.
        spawn_scale *= self.gust.tick(now, &mut self.mt);
        // Apply emergent density boost
        spawn_scale += emergent_effects.density_boost;
        // Apply resume time-scale easing: spawn rate ramps with the exp decay
        // approach curve (1 - exp(-k*t), k=RESUME_EASE_DECAY_RATE) so new
        // streams appear gradually during the inertia recovery. Multiplicatively
        // composed with glyph_entry_time ramp below (also exp decay, see
        // GLYPH_ENTRY_RAMP_DECAY_RATE) when both are active — compound easing
        // is intentional (scene-switch-during-resume = double soft entry).
        spawn_scale *= self.resume_blend;
        // Glyph scene-entry ramp: gradually increase spawn rate after switching
        // to a glyph scene. Exp approach (1 - exp(-k*t), k =
        // GLYPH_ENTRY_RAMP_DECAY_RATE = 4.28/s) gives an instant cascade that
        // asymptotes to full speed - the cinematic "top-entry cascade" feel
        // (replaces the prior smoothstep 3t^2-2t^3 over 700ms; the 700ms is
        // now the settle time at 95% threshold, not a fixed window).
        // Multiplicatively composed with resume_blend above (also exp decay)
        // when both are active - scene-switch-during-resume = double soft
        // entry, intentional.
        if let Some(entry) = self.glyph_entry_time {
            let elapsed_s = now.saturating_duration_since(entry).as_secs_f32();
            let approach = 1.0 - (-GLYPH_ENTRY_RAMP_DECAY_RATE * elapsed_s).exp();
            if approach >= GLYPH_ENTRY_RAMP_SETTLE_FRAC {
                // Settled - snap to full speed and clear the ramp state.
                self.glyph_entry_time = None;
            } else {
                spawn_scale *=
                    GLYPH_ENTRY_RAMP_MIN_SCALE + approach * (1.0 - GLYPH_ENTRY_RAMP_MIN_SCALE);
            }
        }
        spawn_scale = spawn_scale.clamp(0.0, 3.0);
        if matches!(self.rain_style, RainStyle::Monolith) {
            let mut elapsed = now.saturating_duration_since(self.last_spawn_time);
            if self.max_sim_delta > std::time::Duration::from_millis(0) {
                elapsed = elapsed.min(self.max_sim_delta);
            }
            self.last_spawn_time = now;

            let params = MonolithSpawnParams {
                cols: self.cols,
                lines: self.lines,
                density: self.droplet_density,
                size: self.monolith_size,
                active_palette_slot: self.active_palette_slot,
                spawn_scale,
                mouse_enabled: self.mouse_enabled,
                mouse_col: self.mouse_col,
                density_map: self.monolith_density_map,
            };
            let mut random = MonolithRandom {
                rng: &mut self.mt,
                rand_chance: &self.rand_chance,
                rand_col: &self.rand_col,
            };
            self.monolith_rain
                .spawn(now, elapsed, &mut self.spawn_remainder, params, &mut random);
        } else {
            self.spawn_droplets(now, spawn_scale);
        }

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
            if matches!(self.rain_style, RainStyle::Monolith) {
                self.monolith_rain.clear_draw_history();
                self.reset_phosphor_state();
            } else {
                for ch in self.phosphor_base_ch.iter_mut() {
                    *ch = '\0';
                }
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
            if matches!(self.rain_style, RainStyle::Monolith) {
                self.monolith_rain.clear_draw_history();
                self.reset_phosphor_state();
            } else {
                for ch in self.phosphor_base_ch.iter_mut() {
                    *ch = '\0';
                }
            }
            self.force_draw_everything = false;
        }

        let glitch_due = self.time_for_glitch(now);
        // AB-11: when aggressive_throttle is active, disable glitches entirely
        // (don't even check the threshold). This sheds the glitch computation
        // cost without touching the user's glitch_level setting — the setting
        // stays, glitches just don't fire while the throttle is active.
        let allow_glitch =
            glitch_due && !self.aggressive_throttle && self.perf_pressure < GLITCH_THRESHOLD;
        let time_for_glitch = allow_glitch;

        let max_sim_delta = self.max_sim_delta;
        let use_sim_cap = max_sim_delta > std::time::Duration::from_millis(0);

        // Update pass (mut self)
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.advance(
                now,
                self.lines,
                self.chars_per_sec,
                max_sim_delta,
                self.resume_blend,
            );
        } else {
            // sim path optimization: split the droplet advance loop into two
            // specialized paths based on `use_sim_cap` (loop-invariant). In
            // benchmark mode, max_sim_delta = 0 (set_max_sim_delta is a no-op
            // — see commit a34fcdb audit), so use_sim_cap = false and adv_now
            // is always just `now`. The original single-loop formulation
            // evaluated 3 per-iteration branches (use_sim_cap, last_time,
            // now > max_now) that were all dead in bench mode — branch
            // predictor handles them, but the dead code still occupies
            // instruction slots and register pressure. Splitting lets LLVM
            // generate a tighter loop for the bench path (no Instant add,
            // no comparison, no Option match).
            //
            // Both paths share identical post-advance logic (died → free-list,
            // free_col → set_column_spawn, time_for_glitch → do_glitch_span).
            // Duplication is ~12 lines; preferable to a macro with `continue`
            // (which is hard to read and debug) or a method extraction (which
            // would need 7+ params, exceeding clippy's too_many_arguments
            // threshold and adding call overhead).
            if use_sim_cap {
                for i in 0..self.droplets.len() {
                    if !self.droplets[i].is_alive {
                        continue;
                    }

                    let (col, start_line, hp, cp_idx, free_col, died) = {
                        let d = &mut self.droplets[i];
                        let adv_now = if let Some(last) = d.last_time {
                            let max_now = last + max_sim_delta;
                            if now > max_now {
                                max_now
                            } else {
                                now
                            }
                        } else {
                            now
                        };
                        let free_col = d.advance(adv_now, self.lines, self.resume_blend);
                        let col = d.bound_col;
                        let start_line = d.tail_put_line.map(|v| v.saturating_add(1)).unwrap_or(0);
                        let hp = d.head_put_line;
                        let cp_idx = d.char_pool_idx;
                        let died = !d.is_alive;
                        (col, start_line, hp, cp_idx, free_col, died)
                    };

                    if died {
                        let cs = &mut self.col_stat[col as usize];
                        cs.num_droplets = cs.num_droplets.saturating_sub(1);
                        cs.can_spawn = true;
                        self.droplet_free_list.push(i);
                        continue;
                    }

                    if free_col {
                        self.set_column_spawn(col, true);
                    }

                    if time_for_glitch {
                        self.do_glitch_span(start_line, hp, col, cp_idx);
                    }
                }
            } else {
                // Bench / uncapped path: adv_now = now for all droplets.
                // No per-iteration Instant arithmetic or Option match.
                for i in 0..self.droplets.len() {
                    if !self.droplets[i].is_alive {
                        continue;
                    }

                    let (col, start_line, hp, cp_idx, free_col, died) = {
                        let d = &mut self.droplets[i];
                        let free_col = d.advance(now, self.lines, self.resume_blend);
                        let col = d.bound_col;
                        let start_line = d.tail_put_line.map(|v| v.saturating_add(1)).unwrap_or(0);
                        let hp = d.head_put_line;
                        let cp_idx = d.char_pool_idx;
                        let died = !d.is_alive;
                        (col, start_line, hp, cp_idx, free_col, died)
                    };

                    if died {
                        let cs = &mut self.col_stat[col as usize];
                        cs.num_droplets = cs.num_droplets.saturating_sub(1);
                        cs.can_spawn = true;
                        self.droplet_free_list.push(i);
                        continue;
                    }

                    if free_col {
                        self.set_column_spawn(col, true);
                    }

                    if time_for_glitch {
                        self.do_glitch_span(start_line, hp, col, cp_idx);
                    }
                }
            }
        }

        // Build palette_slices for DrawCtx from the palette table.
        // Each slot either has a Palette (Some) or is empty (None) — use an
        // empty slice for empty slots so hot-path rendering stays branch-free.
        let mut palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [&[]; MAX_PALETTE_SLOTS];
        for (i, slot) in palette_slices.iter_mut().enumerate() {
            if let Some(ref p) = self.palette_table[i] {
                *slot = &p.colors;
            }
        }

        let transitioning = self.transition_start.is_some();
        let charset_wave_line = if self.charset_transition_start.is_some() {
            charset_wave_line
        } else {
            None
        };
        let color_wave_line = self.color_wave_line_at(now);

        // Phase 5: build the transition L table for perceptual L smoothing
        // at the palette transition wave line.
        //
        // Active only when `transition_start.is_some()` (a palette switch
        // is in progress) AND `color_wave_line.is_some()` (the wave hasn't
        // finished sweeping). The table pre-computes the OKLab L for each
        // stop index in both the old (previous slot) and new (active slot)
        // palettes, plus the current wave line position and smoothing
        // window. Built once per frame — the shader's `apply_l_smoothing`
        // borrows it through DrawCtx → ShaderCtx.
        //
        // `None` outside the transition window (most frames) — the shader
        // early-returns cheaply when table is None.
        //
        // The previous palette is read from `palette_table` at the slot
        // BEFORE `active_palette_slot` (circular buffer). If that slot is
        // None (no previous palette was set — e.g., first palette switch
        // after startup with only one palette in the table), the table
        // build returns None and no smoothing is applied for this frame.
        let transition_l_table =
            self.transition_start
                .zip(color_wave_line)
                .and_then(|(_, wave_line)| {
                    let prev_slot = ((self.active_palette_slot as usize + MAX_PALETTE_SLOTS - 1)
                        % MAX_PALETTE_SLOTS) as u8;
                    let prev_palette: &[Color] = self.palette_table[prev_slot as usize]
                        .as_ref()
                        .map(|p| p.colors.as_slice())
                        .unwrap_or(&[]);
                    if prev_palette.is_empty() {
                        return None;
                    }
                    crate::chroma_dragon_engine::shaders::transition::TransitionLTable::build(
                        prev_palette,
                        &self.palette.colors,
                        wave_line,
                        crate::chroma_dragon_engine::tuning::TRANSITION_L_SMOOTHING_WINDOW,
                    )
                });

        // Draw pass (split-borrows via DrawCtx)
        let draw_everything = force_draw_everything;
        // v16: pool_is_binary cached in Cloud, recomputed only on charset change.
        let pool_is_binary = self.char_pool_is_binary;

        let glitch_inv_between = {
            let between = self
                .next_glitch_time
                .saturating_duration_since(self.last_glitch_time)
                .as_nanos() as f64;
            if between > 0.0 {
                1.0 / between
            } else {
                0.0
            }
        };

        // PERF: precompute glitch bright/dim phase state once per frame.
        // Previously is_bright(now)/is_dim(now) were called per-cell from
        // DrawCtx::get_attr — but both depend only on `now` (not cell
        // position), so the result is identical across all cells in the
        // same frame. Caching saves ~100-300 Instant::saturating_duration_since
        // + as_nanos + float multiply ops per frame when glitchy.
        let glitch_bright = if now < self.last_glitch_time || glitch_inv_between <= 0.0 {
            false
        } else {
            let since = now
                .saturating_duration_since(self.last_glitch_time)
                .as_nanos() as f64;
            since * glitch_inv_between <= GLITCH_BRIGHT_RATIO
        };
        let glitch_dim = if now > self.next_glitch_time || glitch_inv_between <= 0.0 {
            true
        } else {
            let since = now
                .saturating_duration_since(self.last_glitch_time)
                .as_nanos() as f64;
            since * glitch_inv_between >= GLITCH_DIM_RATIO
        };

        // ── Pre-rain event render (ghosts, behind droplets) ──
        if !self.event_manager.is_empty() {
            // Phase 3-I: derive ghost base color from the current palette's
            // darkest stop. Replaces the hardcoded (18, 22, 18) in ghost.rs —
            // ghosts now match the scene's color scheme.
            let ghost_base_color =
                crate::chroma_dragon_engine::post::ghost::ghost_base_color(&self.palette.colors);
            let pre_ctx = crate::cloud::ghost_events::EventCtx {
                cols: self.cols,
                lines: self.lines,
                ghost_base_color,
                color_pipeline: self.color_pipeline,
                now,
            };
            self.event_manager.render_pre_rain(&pre_ctx, frame);
        }

        // Phase 3-G (Chroma Dragon Innovation G): build the per-frame
        // atmospheric ctx from Cloud state. This precomputes all
        // frame-invariant factors (dim/boost/saturation/persistence/instability
        // integers + now_secs) once, then passes them through DrawCtx →
        // ShaderCtx → resolve_cell_color where the shader applies them to
        // each cell's resolved color BEFORE encoding. (the old
        // post-hoc `apply_climate_frame_effects` pass was deleted; climate
        // is shader-only now, eliminating ~500 decode-encode-frame.set
        // cycles per frame.)
        //
        // The math here is identical to the pre-Phase-3-G post-hoc pass —
        // same thresholds, same integer fixed-point factors. The only
        // difference is WHEN it runs (shader vs post-hoc).
        let atmospheric = {
            let luminance = self.color_ecosystem.luminance_climate;
            let saturation = self.color_ecosystem.saturation_climate;
            let instability = self.memory.instability_pressure;
            let persistence = self.memory.persistence_richness;
            let emergent = emergent_effects;
            let profile = self.profile_current;

            let needs_luminance = (luminance - 1.0).abs() > 0.01
                || emergent.luminance_boost > 0.0
                || profile.luminance_offset.abs() > 0.01;
            let needs_saturation = (saturation - 1.0).abs() > 0.01;
            let needs_persistence = persistence.abs() > 0.01;

            if !needs_luminance && !needs_saturation && !needs_persistence {
                // All neutral — pass None so the shader's fast-path skips
                // entirely. The post-hoc pass also early-returns on this
                // condition (preserved behavior).
                None
            } else {
                // v30 Hinnant-style: use start_anchor-based elapsed instead of
                // `now.elapsed()` (which returned microseconds — frame-start to
                // now-during-rain_at — too small to drive a meaningful phase
                // seed). `start_anchor` is captured once at Cloud::new() and
                // inherited across live-reload.
                let now_secs = now.saturating_duration_since(self.start_anchor).as_secs() as u32;
                let total_lum = luminance + profile.luminance_offset + emergent.luminance_boost;
                let lum_fi = if total_lum < 1.0 {
                    Some((total_lum.clamp(0.0, 1.0) * 256.0) as i32)
                } else {
                    None
                };
                let lum_wf = if total_lum > 1.0 {
                    Some(((total_lum - 1.0).clamp(0.0, 0.3) * 256.0) as i32)
                } else {
                    None
                };
                let sat_ti = if needs_saturation && saturation < 1.0 {
                    Some((saturation.clamp(0.0, 1.0) * 256.0) as i32)
                } else {
                    None
                };
                let persist_wf = if needs_persistence && persistence > 0.0 {
                    Some(((persistence * 0.3).clamp(0.0, 1.0) * 256.0) as i32)
                } else {
                    None
                };
                let instability_threshold = if instability > 0.15 {
                    Some((instability * 50.0) as u32)
                } else {
                    None
                };
                let instability_wf = if instability > 0.15 {
                    Some(((instability * 0.1).clamp(0.0, 1.0) * 256.0) as i32)
                } else {
                    None
                };
                Some(ClimateCtx {
                    lum_fi,
                    lum_wf,
                    sat_ti,
                    persist_wf,
                    instability_threshold,
                    instability_wf,
                    now_secs,
                })
            }
        };

        // v30 fix: pre-compute active flash wave list once per frame.
        // The DrawCtx borrows this slice for the duration of the draw call.
        // Active = `active && elapsed < MOUSE_FLASH_DURATION_SECS`. Expired
        // waves are NOT removed here — they're swept by the update loop
        // (see expiry block at end of rain_at). We just skip them in the
        // precomputed slice so the renderer doesn't see stale waves on the
        // frame before the sweep runs.
        let mut flash_waves_buf: SmallVec<[FlashWaveCtx; MOUSE_FLASH_POOL_SIZE]> = SmallVec::new();
        for w in &self.flash_waves {
            if w.active {
                // v30 Hinnant-style: use injected `now` instead of
                // `w.birth.elapsed()` (which issued a hidden Instant::now()
                // syscall per active wave per frame). `now` is the same
                // Instant captured once at frame start in event_loop.rs.
                let e = now.saturating_duration_since(w.birth).as_secs_f32();
                if e < MOUSE_FLASH_DURATION_SECS {
                    // v30 optimize (MOUSE_EFFECTS_AUDIT.md Quick Win #2):
                    // precompute wave-invariant quantities here (once per wave)
                    // instead of per cell × per wave in droplet.rs hot path.
                    let primary_radius = e * MOUSE_FLASH_SPEED;
                    let secondary_radius = e * MOUSE_FLASH_SPEED * MOUSE_FLASH_SECONDARY_SPEED_FRAC;
                    let raw_fade = (1.0 - e / MOUSE_FLASH_DURATION_SECS).max(0.0);
                    let fade = raw_fade * raw_fade.sqrt();
                    // max_reach_sq: squared bounding-circle radius for early-out.
                    // Primary ring is always faster (secondary_speed_frac < 1.0),
                    // so primary_radius >= secondary_radius.
                    let max_reach = primary_radius + MOUSE_FLASH_RING_WIDTH;
                    flash_waves_buf.push(FlashWaveCtx {
                        col: w.col,
                        line: w.line,
                        primary_radius,
                        secondary_radius,
                        fade,
                        max_reach_sq: max_reach * max_reach,
                        // v50 audit C-1: precompute palette HEAD color
                        // once per wave instead of per-cell in droplet.rs.
                        head_rgb: self
                            .palette
                            .colors
                            .last()
                            .copied()
                            .and_then(crate::palette::decode_color)
                            .unwrap_or((255, 255, 255)),
                    });
                }
            }
        }

        // Phase D (hot-path): pre-compute the per-column hue-coherence LUT
        // once per frame. Was: per-cell `column_coherence_perturbation(phase,
        // col)` call inside `resolve_cell_color` (~12.9M Middle cells/sec at
        // 60 FPS → ~65-130M cycles/sec of sinf + round + cast). Now: a single
        // `cols`-length pass over `self.column_coherence_lut` (reused buffer —
        // no per-frame heap allocation), borrowed by `DrawCtx` for the frame,
        // and read by index in the shader hot path.
        //
        // The LUT is stored on Cloud (not built fresh each frame) to avoid
        // per-frame Vec allocation. Resize in place when `cols` changes
        // (terminal resize); otherwise just overwrite values.
        // v30 Hinnant-style: use start_anchor-based elapsed instead of
        // `now.elapsed()` (which returned microseconds — frame-start to
        // now-during-rain_at — making the phase essentially 0 and the
        // coherence pattern visually static). With COLUMN_COHERENCE_FREQ =
        // 0.105 rad/s, the phase now smoothly cycles every 2π/0.105 ≈ 60s,
        // which is the intended slow-drift behavior. Also removes a hidden
        // Instant::now() syscall per frame.
        let column_coherence_phase = now
            .saturating_duration_since(self.start_anchor)
            .as_secs_f32()
            * crate::chroma_dragon_engine::tuning::COLUMN_COHERENCE_FREQ;
        let cols_us = self.cols as usize;
        if self.column_coherence_lut.len() != cols_us {
            self.column_coherence_lut.resize(cols_us, 0);
        }
        for col in 0..cols_us {
            self.column_coherence_lut[col] =
                crate::chroma_dragon_engine::shaders::base::column_coherence_perturbation(
                    column_coherence_phase,
                    col as u16,
                );
        }

        let ctx = DrawCtx {
            lines: self.lines,
            cols: self.cols,
            shading_distance: self.shading_distance,
            bg: self.palette.bg,
            color_mode: self.color_mode,
            color_pipeline: self.color_pipeline,
            bold_mode: self.bold_mode,
            glitchy: self.glitchy,
            glitch_bright,
            glitch_dim,
            palette_slices,
            active_palette_slot: self.active_palette_slot,
            transitioning,
            color_map: &self.color_map,
            glitch_map: &self.glitch_map,
            char_pool: &self.char_pool,
            previous_char_pool: &self.previous_char_pool,
            edge_fade_lut: &self.edge_fade_lut,
            vignette_lut: &self.vignette_lut,
            vignette_lut_cols: self.vignette_lut_dims.0,
            charset_wave_line,
            color_wave_line,
            mouse_col: self.mouse_col,
            mouse_line: self.mouse_line,
            // v30 fix: pre-compute active flash wave list once per frame.
            // Was: single `flash_elapsed: Option<f32>` from one slot.
            // Now: up to MOUSE_FLASH_POOL_SIZE concurrent waves, each with its
            // own elapsed time. The slice borrows a stack-local SmallVec that
            // outlives the DrawCtx.
            flash_waves: &flash_waves_buf,
            pool_is_binary,
            atmospheric,
            // Phase 3-H + Phase C: activate ColorEcosystem.hue_drift — was
            // dead code (updated every tick, never read). Now passed through
            // DrawCtx → ShaderCtx → resolve_cell_color, where it applies a
            // slow global palette-stop offset to Middle cells.
            //
            // Phase C optimization: pre-compute the i32 offset ONCE per
            // frame here, so the per-cell hot path is a single integer add
            // (was: f32 div + mul + round + cast per cell). The drift value
            // is in [-π, π]; hue_drift_offset maps it to {-2,-1,0,+1,+2}.
            // At ~12.9M Middle cells/sec this saves ~65M cycles/sec.
            // Always Some in production — the value is meaningful even
            // when small (and 0.0 is a valid no-op).
            hue_drift_offset: Some(
                crate::chroma_dragon_engine::shaders::base::hue_drift_offset(
                    self.color_ecosystem.hue_drift,
                ),
            ),
            // Phase 4-A (Dragon Awakening) + Phase D (hot-path): temporal
            // column hue coherence, now as a precomputed LUT. The LUT is
            // built once per frame from the time phase (COLUMN_COHERENCE_FREQ
            // rad/s, ~60 s period) — see the `column_coherence_lut` filling
            // loop above. Always Some in production — the effect is a slow
            // sine and 0.0 is a valid phase (perturbation still varies by
            // col).
            column_coherence_lut: Some(&self.column_coherence_lut),
            // Phase 4-B (Dragon Awakening): activate subpixel hue jitter
            // (Innovation E). The shader logic landed in Phase 3-E but was
            // dormant (DrawCtx hard-coded None). Phase 4-B sets a
            // conservative amplitude (SUBPIXEL_JITTER_AMPLITUDE = 3) for
            // subtle film-grain texture. Always Some in production — the
            // jitter is deterministic per (line, col) so it doesn't strobe.
            subpixel_jitter_amplitude: Some(
                crate::chroma_dragon_engine::tuning::SUBPIXEL_JITTER_AMPLITUDE,
            ),
            // Phase 4-D (Dragon Awakening): activate head halo via background
            // blend (Innovation D). The blend_toward_bg helper landed in
            // Phase 3-D but had zero production callers. Phase 4-D wires it
            // into the shader's Head branch with a conservative factor
            // (HEAD_HALO_FACTOR = 0.15) so the head dissolves into the scene
            // background. Always Some in production — the shader auto-no-ops
            // when bg is None or Color::Reset.
            head_halo_factor: Some(crate::chroma_dragon_engine::tuning::HEAD_HALO_FACTOR),
            // Phase 5: perceptual L smoothing at the palette transition
            // wave line. Built once per frame when transition_start.is_some()
            // AND color_wave_line.is_some() — the table pre-computes the
            // OKLab L for each stop index in both the old and new palettes,
            // plus the current wave line position and smoothing window.
            // The shader's apply_l_smoothing uses it to blend each cell's
            // OKLab L toward the opposite palette's L within ±window lines
            // of the wave, eliminating the hard brightness step at the
            // wave line. None outside the transition window (most frames).
            transition_l_table: transition_l_table.as_ref(),
        };

        if matches!(self.rain_style, RainStyle::Monolith) {
            let mut cleanup = MonolithCleanup {
                lines: self.lines,
                bg: self.palette.bg,
                phosphor: &mut self.phosphor,
                phosphor_base_fg: &mut self.phosphor_base_fg,
                phosphor_base_ch: &mut self.phosphor_base_ch,
                phosphor_layer: &mut self.phosphor_layer,
            };
            self.monolith_rain.draw(&ctx, frame, &mut cleanup);
        } else {
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
        }

        // Message box drawn AFTER phosphor/anomaly/atmospheric effects
        // so it survives all post-processing — glow + typewriter reveal.

        // ── Bug 2: cinematic CRT vignette post-process ──
        //
        // Apply a subtle dim to the top and bottom CRT_VIGNETTE_HEIGHT
        // rows. Creates a retro CRT-glow feel — the screen edges look
        // slightly darker, drawing the eye toward the center where the
        // rain is densest. Eases out via smoothstep so the dim is
        // imperceptible at the inner boundary (row CRT_VIGNETTE_HEIGHT
        // from the edge), preventing a hard cutoff.
        //
        // Runs AFTER the droplet draw pass + rain shadow (already
        // applied per-cell inside Droplet::draw()), but BEFORE phosphor
        // decay — so the glow is also dimmed, preventing edge cells
        // from retaining afterglow when the cursor passes through them.
        //
        // Cost: O(cols × CRT_VIGNETTE_HEIGHT × 2) per frame. At
        // 200×60 with CRT_VIGNETTE_HEIGHT=5, that's 2000 cells/frame
        // — negligible vs the ~2200 dirty cells/frame average.
        self.apply_crt_vignette(frame);

        // ── Quantum Ripple particle update + render (v25 masterclass) ──
        //
        // Update active particles (move outward, expire by lifespan),
        // then render each as a brand-purple glyph with fade based on
        // age. Runs O(active_particles) per frame — typically 0-20.
        self.apply_quantum_ripple(frame, now);

        // --- Phosphor persistence post-process ---
        // Scale phosphor decay elapsed by resume_blend so afterglow fades at
        // the same rate as the rain wakes up. Without this, phosphor trails
        // vanish at full speed while droplets move in slow motion — creating
        // temporal inconsistency that feels "spiky" during resume.
        // clamp dt to 1/30 sec (matches droplet/quantum/spawn caps).
        // Without this, a frame timing spike (GC pause, OS stall) could
        // make phosphor decay up to 3x faster than droplets move in the
        // same frame — visible as a "brightness dip" on trails.
        let phosphor_elapsed = now
            .saturating_duration_since(self.last_phosphor_time)
            .as_secs_f32()
            .min(1.0 / 30.0)
            * self.resume_blend;
        self.last_phosphor_time = now;

        // ── Component timing: sim → render boundary ────────────────────
        // Everything above this line is "simulation" (cinematic events,
        // spawn rate, droplet physics). Everything below mutates the frame
        // buffer — that is "render" (phosphor decay, anomaly zones,
        // atmospheric post-processing, message box).
        //
        // P1: only capture t1 when component timing is enabled (benchmark
        // or --perf-stats). Interactive mode skips this Instant::now().
        let t1 = if enable_timing { Instant::now() } else { t0 };
        if enable_timing {
            self.last_sim_ms = t1.saturating_duration_since(t0).as_secs_f64() * 1000.0;
        }

        self.phosphor_decay_pass(frame, phosphor_elapsed);
        if matches!(self.rain_style, RainStyle::Monolith) {
            let mut cleanup = MonolithCleanup {
                lines: self.lines,
                bg: self.palette.bg,
                phosphor: &mut self.phosphor,
                phosphor_base_fg: &mut self.phosphor_base_fg,
                phosphor_base_ch: &mut self.phosphor_base_ch,
                phosphor_layer: &mut self.phosphor_layer,
            };
            self.monolith_rain.clear_spine_phosphor(&mut cleanup);
        }

        // P4: periodic stuck-cell sweep (debug mode only). Runs after
        // phosphor decay so it can observe cells that the phosphor system
        // failed to track. Gated on enable_component_timing internally —
        // zero cost in production interactive runs.
        self.stuck_cell_sweep(frame);

        // --- Rare anomaly events ---
        // Check for new anomaly spawn. The product of multipliers creates a
        // positive feedback loop (more anomalies → higher instability → more
        // anomalies). Cap the effective rate at 3× base to prevent visual
        // overload while preserving atmospheric dynamics.
        //
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
        if phosphor_elapsed > 0.0
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
        // Apply anomaly effects to frame
        self.apply_anomalies(frame, now);

        // ── Cinematic Event Engine: render active events ──
        if !self.event_manager.is_empty() {
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
        // ambient/crystal-dragon harmony: when ambient has asserted a palette
        // (`ambient_palette_locked`), Crystal Dragon drift is suppressed.
        // Ambient specifies the WHAT (which palette), Crystal Dragon
        // specifies the HOW (climate variation on top). When the user
        // manually overrides (presses 'c' or 'x'), the lock is cleared
        // and Crystal Dragon drift resumes until the next ambient fire.
        // See docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md §1.3 + §3.
        //
        // Note: custom_palette_active is NOT a drift gate. When the user
        // explicitly enables --crystal-dragon with a custom palette (-c
        // tron_legacy), drift is allowed — the first drift event replaces
        // the custom palette with a builtin one via set_color_scheme (which
        // clears custom_palette_active). If the user doesn't want drift,
        // they should not enable --crystal-dragon.
        self.color_ecosystem.tick(now, &mut self.mt);

        // 1b. Crystal Dragon Engine drift
        // When crystal_dragon is enabled (and ambient lock is not asserted),
        // tick the Crystal Dragon sensor and probabilistically select a new
        // color theme from the temperature group (Cold/Medium/Hot) matching
        // the current system point.
        if self.crystal_dragon && !self.ambient_palette_locked {
            if let Some(new_scheme) = self.crystal_dragon_tick(now) {
                self.set_color_scheme(new_scheme);
                self.user_override_since_ambient = true;
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
        // Glow (60% white blend) + typewriter reveal (30ms/char).
        if !self.message.is_empty() {
            self.draw_message(frame);
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
