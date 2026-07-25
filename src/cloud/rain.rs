// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Main render loop: rain() and rain_at().

use std::time::Instant;

use crossterm::style::Color;
use rand::distr::Distribution;

use crate::constants::*;
use crate::droplet_prediction::{
    adaptive_prediction_horizon, is_prediction_disabled_by_context, PredictionContext,
    MAX_PREDICTED_CLEAN_FRAMES,
};
use crate::frame::Frame;
use crate::rain_style::RainStyle;

use super::ecosystem::EmergentMoment;
use super::monolith::{MonolithCleanup, MonolithRandom, MonolithSpawnParams};
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

        // v17 mastery: pause ease-OUT (deceleration).
        // If pause_start is set, compute pause_blend ramping 1→0 over
        // PAUSE_EASE_DURATION_SECS. Scale the effective resume_blend by
        // pause_blend so the rain decelerates smoothly. When pause_blend
        // reaches 0, set self.pause = true (fully frozen).
        if let Some(ps) = self.pause_start {
            let t = now.saturating_duration_since(ps).as_secs_f32();
            let normalized = (t / PAUSE_EASE_DURATION_SECS).min(1.0);
            // Smootherstep: 6t⁵ - 15t⁴ + 10t³ (C2 continuous).
            let smoother = normalized
                * normalized
                * normalized
                * (normalized * (normalized * 6.0 - 15.0) + 10.0);
            // pause_blend goes 1→0 (deceleration). Multiply with resume_blend
            // (which is 1.0 during active play) to get the effective time scale.
            let pause_blend = 1.0 - smoother;
            self.resume_blend = pause_blend;
            if normalized >= 1.0 {
                // Deceleration complete — fully paused now.
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
        // P1 optimization: t0 is just `now` (the caller already captured
        // it). The two extra Instant::now() calls (t1, t2) are gated behind
        // enable_component_timing — only the benchmark and --perf-stats
        // paths need them. Interactive mode skips them for ~40ns/frame
        // savings (2 calls × ~20ns each).
        let t0 = now;
        let enable_timing = self.enable_component_timing;

        // ── Atmospheric Event Engine: evaluate triggers ──
        let anomaly_density = self.anomaly_zones.len() as f32 / ANOMALY_MAX_ZONES.max(1) as f32;
        let in_transition = self.transition_start.is_some()
            || self.charset_transition_start.is_some()
            || self.profile_transition_start.is_some();
        let palette_last = self.palette.colors.last().copied();
        self.event_manager.evaluate_triggers(
            now,
            self.perf_pressure,
            self.cols,
            self.lines,
            anomaly_density,
            palette_last,
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

        // Advance cinematic resume easing: smoothstep S-curve from 0→1 over
        // RESUME_EASE_DURATION_SECS after unpause. Unlike exponential
        // easing or position-delta scaling, this interpolates the simulation
        // time scale itself — the physics clock runs in slow motion during
        // the transition, producing genuine inertia recovery rather than a
        // frozen-then-unfrozen snap.
        if let Some(rs) = self.resume_start {
            let t = now.saturating_duration_since(rs).as_secs_f32();
            let normalized = (t / RESUME_EASE_DURATION_SECS).min(1.0);
            // v17 mastery: smootherstep (C2 continuous) — 6t⁵ - 15t⁴ + 10t³.
            // Smoother than smoothstep (C1) — zero velocity AND zero acceleration
            // at start/end, eliminating all perceptual discontinuity.
            let smoother = normalized
                * normalized
                * normalized
                * (normalized * (normalized * 6.0 - 15.0) + 10.0);
            self.resume_blend = smoother;
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
        // Apply wind-gust multiplier (1.0 when idle, up to GUST_PEAK_MAX
        // during a gust). Independent of `atmosphere.density_offset`
        // (slow entropy cycle) — gusts are short, sharp surges.
        spawn_scale *= self.gust.tick(now, &mut self.mt);
        // Apply emergent density boost
        spawn_scale += self.storytelling.active_effects(now).density_boost;
        // Apply resume time-scale easing: spawn rate ramps with the smoothstep
        // curve so new streams appear gradually during the inertia recovery.
        spawn_scale *= self.resume_blend;
        // Glyph scene-entry ramp: gradually increase spawn rate after switching
        // to a glyph scene. During the ramp period, spawn starts at a reduced
        // rate and smoothly accelerates to full speed via smoothstep, creating
        // a cinematic top-entry cascade instead of an instant wall of rain.
        if let Some(entry) = self.glyph_entry_time {
            let elapsed_ms = now.saturating_duration_since(entry).as_millis() as f32;
            let ramp_dur = GLYPH_ENTRY_RAMP_DURATION_MS as f32;
            if elapsed_ms < ramp_dur {
                let t = (elapsed_ms / ramp_dur).clamp(0.0, 1.0);
                // Smoothstep: 3t² - 2t³ — slow start, fast middle, slow end.
                let ramp = t * t * (3.0 - 2.0 * t);
                spawn_scale *=
                    GLYPH_ENTRY_RAMP_MIN_SCALE + ramp * (1.0 - GLYPH_ENTRY_RAMP_MIN_SCALE);
            } else {
                self.glyph_entry_time = None; // Ramp complete
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
                full_width: self.full_width,
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
        let allow_glitch = glitch_due && self.perf_pressure < GLITCH_THRESHOLD;
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
            // dragon-temporal: simulation FPS estimate for path prediction.
            // We don't have a hard-coded FPS target — the simulation runs as
            // fast as the terminal refresh allows. We estimate from the
            // median frame interval seen by the bench loop, but for the
            // experiment we approximate with the common 60 FPS target. The
            // prediction's job is to detect "no movement this frame", which
            // is robust to FPS estimation error because the tolerance
            // absorbs ±1 cell of rounding noise.
            const PREDICTION_FPS: f32 = 60.0;

            // ── adaptive temporal prediction: build context once per frame ──
            //
            // `PredictionContext` snapshots all the state the prediction
            // engine needs to decide whether a droplet MAY skip a frame:
            // screen dimensions, mouse cursor position, active click wave.
            // Building it once per frame (rather than per-droplet) keeps the
            // simulation loop tight — every droplet just reads from the
            // snapshot via `is_prediction_disabled_by_context()`.
            let pred_ctx = PredictionContext {
                cols: self.cols,
                lines: self.lines,
                mouse_enabled: self.mouse_enabled,
                mouse_col: self.mouse_col,
                mouse_line: self.mouse_line,
                flash_col: self.flash_col,
                flash_line: self.flash_line,
                flash_time: self.flash_time,
                now,
            };

            for i in 0..self.droplets.len() {
                if !self.droplets[i].is_alive {
                    continue;
                }

                // ── adaptive temporal prediction: ALWAYS advance() ──
                //
                // Forensic fix (Fix 1): the previous implementation skipped
                // `advance()` along with `draw()` when a droplet was
                // "predicted clean". This froze `advance_remainder`, which
                // in turn froze the per-frame `head_brightness()` ramp and
                // the `frac_progress`-modulated head bloom cascade —
                // producing the "long white head" + stuttering motion that
                // the dragon suffered.
                //
                // The fix is to ALWAYS call `advance()`. The function is
                // cheap (just math — gravity, turbulence, fractional
                // accumulator update — no drawing). It grows
                // `advance_remainder` every frame, keeping the brightness
                // + bloom pulse alive even on frames where the head doesn't
                // cross a cell boundary.
                //
                // The prediction is still consulted AFTER `advance()` to
                // decide whether `draw()` may be skipped: if the head
                // position hasn't moved to a new row (prediction matches
                // actual trajectory) AND the surrounding context (mouse
                // halo, click wave, rain shadow) permits skipping, the
                // draw pass skips `d.draw()` for this droplet this frame.
                // The head cell's brightness still pulses because
                // `advance_remainder` grew — the next time `draw()` runs
                // (on cell-crossing), it paints the new brightness value.
                //
                // Context check uses the droplet's CURRENT head position
                // (head_cur_line is the visible row; bound_col is the
                // column). head_cur_line tracks the rendered position
                // even when the head is crawling, so it's the right
                // reference for "is this droplet inside the mouse halo
                // right now?".
                let (col, start_line, hp, cp_idx, free_col, died) = {
                    let d = &mut self.droplets[i];
                    // Always respect the max_sim_delta cap. We never skip
                    // advance() now, so there's no accumulated elapsed to
                    // bypass — every frame's elapsed is exactly 1 frame.
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
                    let start_line = d.tail_put_line.map(|v| v.saturating_add(1)).unwrap_or(0);
                    let hp = d.head_put_line;
                    let cp_idx = d.char_pool_idx;
                    let died = !d.is_alive;

                    // After advance(), decide whether the draw pass may
                    // skip this droplet. Four cases (forensic Task 2 adds
                    // the chars_advanced + staleness checks):
                    //   1. Context forbids (mouse halo / click wave / rain
                    //      shadow): force a real draw() this frame. Clear
                    //      any stale prediction.
                    //   2. Head moved this frame (last_chars_advanced > 0):
                    //      force a real draw() so the new head cell gets
                    //      painted with head color/bloom, and the previous
                    //      head cell transitions to body color. Skipping
                    //      draw() here was the root cause of the "putus-putus"
                    //      (fragmented) rain + "longer white head" symptom —
                    //      prediction_matches_actual() tolerated drift of up
                    //      to PREDICTION_DRIFT_TOLERANCE cells, so a 1-cell
                    //      head advance still counted as "predicted clean"
                    //      and skipped the paint.
                    //   3. Staleness cap exceeded (frames_since_last_draw >=
                    //      MAX_PREDICTED_CLEAN_FRAMES): force a redraw to
                    //      refresh cell colors that may have decayed via
                    //      phosphor pass + head brightness modulation
                    //      between actual cell crossings.
                    //   4. Prediction matches actual trajectory AND still
                    //      has frames_remaining: mark `predicted_clean`
                    //      so draw pass skips `d.draw()`. Decrement
                    //      frames_remaining so the prediction eventually
                    //      expires and recalibrates.
                    //   5. Prediction invalid (head crossed a cell, or
                    //      expired): clear it and re-predict with an
                    //      adaptive horizon tuned to layer + speed. The
                    //      draw pass will run because predicted_clean is
                    //      false.
                    let in_context_forbid = is_prediction_disabled_by_context(
                        &pred_ctx,
                        d.bound_col,
                        d.head_cur_line,
                        d.layer,
                    );
                    let head_moved = d.last_chars_advanced > 0;
                    let stale = d.frames_since_last_draw >= MAX_PREDICTED_CLEAN_FRAMES;
                    if in_context_forbid || head_moved || stale {
                        // Force draw() this frame. Reset the staleness
                        // counter because draw() will run. Clear any stale
                        // prediction so we don't accidentally keep using a
                        // trajectory that no longer reflects reality.
                        d.predicted_state = None;
                        d.predicted_clean = false;
                        d.frames_since_last_draw = 0;
                        // Recompute prediction so the next frame's check
                        // has a fresh trajectory to compare against.
                        let horizon = adaptive_prediction_horizon(d.layer, d.chars_per_sec);
                        d.predicted_state = d.predict_droplet_path(horizon, PREDICTION_FPS);
                    } else if d.prediction_matches_actual() {
                        // Decrement frames_remaining to bound staleness.
                        // Do NOT touch `was_skipped` — we never skip
                        // advance(), so the cap-bypass flag is irrelevant.
                        if let Some(ps) = d.predicted_state.as_mut() {
                            ps.frames_remaining = ps.frames_remaining.saturating_sub(1);
                        }
                        d.predicted_clean = true;
                        // Increment the staleness counter — draw() will be
                        // skipped this frame, so the cell content is one
                        // frame staler than the previous draw.
                        d.frames_since_last_draw = d.frames_since_last_draw.saturating_add(1);
                    } else {
                        // Prediction invalid — the head drifted off the
                        // predicted trajectory (or the prediction expired).
                        // Force a draw() this frame and re-predict.
                        d.predicted_state = None;
                        d.predicted_clean = false;
                        d.frames_since_last_draw = 0;
                        let horizon = adaptive_prediction_horizon(d.layer, d.chars_per_sec);
                        d.predicted_state = d.predict_droplet_path(horizon, PREDICTION_FPS);
                    }

                    (col, start_line, hp, cp_idx, free_col, died)
                };

                if died {
                    // Dragon egg #12: direct indexing — col comes from d.col which
                    // is guaranteed < cols (checked at spawn). col_stat is resized
                    // to cols in spawn.rs.
                    let cs = &mut self.col_stat[col as usize];
                    cs.num_droplets = cs.num_droplets.saturating_sub(1);
                    cs.can_spawn = true;
                    // Return the dead droplet's index to the free-list so
                    // spawn_droplets can reuse it in O(1) on the next spawn.
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
            let palette_slice_pre: &[Color] = &self.palette.colors;
            let pre_ctx = crate::cloud::atmospheric_events::EventCtx {
                cols: self.cols,
                lines: self.lines,
                bg: self.palette.bg,
                palette_colors: palette_slice_pre,
                now,
                message_bounds: None,
                has_message: false,
            };
            self.event_manager.render_pre_rain(&pre_ctx, frame);
        }

        let ctx = DrawCtx {
            lines: self.lines,
            cols: self.cols,
            full_width: self.full_width,
            shading_distance: self.shading_distance,
            bg: self.palette.bg,
            color_mode: self.color_mode,
            bold_mode: self.bold_mode,
            glitchy: self.glitchy,
            last_glitch_time: self.last_glitch_time,
            next_glitch_time: self.next_glitch_time,
            glitch_inv_between,
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
            charset_wave_line,
            color_wave_line,
            mouse_col: self.mouse_col,
            mouse_line: self.mouse_line,
            flash_col: self.flash_col,
            flash_line: self.flash_line,
            flash_time: self.flash_time,
            flash_elapsed: self.flash_time.and_then(|ft| {
                let e = ft.elapsed().as_secs_f32();
                if e < MOUSE_FLASH_DURATION_SECS {
                    Some(e)
                } else {
                    None
                }
            }),
            pool_is_binary,
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

                // dragon-temporal: skip draw() for droplets whose simulation
                // was skipped this frame. Their visible cells haven't moved
                // since the previous frame's draw(), so Frame::set's
                // content-aware dirty check would early-return on every cell
                // anyway (no dirty marks, no redraw). Skipping the draw call
                // entirely saves the per-cell loop iteration cost — the
                // actual dirty-cell reduction comes from the cells never
                // being touched in the first place.
                //
                // We CANNOT skip draw() for droplets that need tail cleanup
                // (just died this frame) — those need their tail cells
                // cleared, which always produces dirty marks.
                if d.predicted_clean && d.is_alive {
                    // Predicted clean: cells unchanged, no draw needed.
                    continue;
                }

                if d.is_alive || needs_tail_cleanup {
                    // dragon-temporal peak (Lever 2 + 3): when the droplet
                    // advanced by a small number of cells this frame, use
                    // the cheap `draw_recent_only()` path. It only iterates
                    // over the new head cell(s) + the previous head cell
                    // (which transitions from Head to Body loc), skipping
                    // the full trail iteration. Body cells retain their
                    // previous frame's content (slightly stale head bloom,
                    // imperceptible at 60 FPS).
                    //
                    // Threshold: last_chars_advanced <= DRAW_RECENT_THRESHOLD.
                    // Above this, the advance is large enough that the full
                    // trail needs refreshing (multiple body cells changed
                    // content, and the stale bloom would be too visible).
                    // We also fall back to full draw() when:
                    //   - draw_everything is true (forced full redraw —
                    //     e.g., resume from pause, palette transition)
                    //   - the droplet needs tail cleanup (just died) —
                    //     draw_recent_only does handle tail cleanup, but
                    //     dying droplets may have other state transitions
                    //     that benefit from a full refresh
                    //   - ctx is in a transitioning state (palette/charset
                    //     transition) — full draw ensures all cells get
                    //     the transition effect applied consistently
                    const DRAW_RECENT_THRESHOLD: u16 = 3;
                    let use_recent = d.is_alive
                        && !draw_everything
                        && !needs_tail_cleanup
                        && !ctx.transitioning
                        && !ctx.charset_transitioning()
                        && d.last_chars_advanced <= DRAW_RECENT_THRESHOLD;
                    if use_recent {
                        d.draw_recent_only(&ctx, frame, now, draw_everything);
                    } else {
                        d.draw(&ctx, frame, now, draw_everything);
                    }
                }

                if !d.is_alive {
                    d.bound_col = u16::MAX;
                }
            }

            // dragon-temporal: reset predicted_clean for the next frame.
            // The flag is set in the simulation pass above; it must be
            // cleared before the next frame's simulation pass runs, so
            // that a droplet which becomes "dirty" next frame (prediction
            // invalidated) doesn't accidentally skip draw() based on the
            // stale flag.
            //
            // Doing this in the draw pass (rather than at the start of the
            // next simulation pass) keeps the flag's lifecycle contained
            // within a single rain_at() call: set in sim pass, read in
            // draw pass, cleared at the end.
            for d in &mut self.droplets {
                d.predicted_clean = false;
            }
        }

        // ── forensic fix (Fix 2): interactive glow post-process ──
        //
        // The mouse cursor halo and click wave ripple used to live INSIDE
        // `Droplet::draw()`. When temporal prediction skipped `draw()` for
        // a "predicted clean" droplet, the halo + ripple were skipped too
        // — making the glow appear patchy (only on currently-advancing
        // droplets) and entirely absent on background cells between
        // droplets.
        //
        // Moving the glow to a global post-process pass fixes both:
        //   1. The halo is always applied every frame, regardless of
        //      which droplets were skipped by temporal prediction.
        //   2. The halo extends to ALL cells in the glow region —
        //      including background cells (blank between droplets) that
        //      were previously unreachable from per-droplet rendering.
        //
        // Runs AFTER the droplet draw pass (so it overlays on top of
        // freshly painted droplet cells) and BEFORE the phosphor decay
        // pass (so the glow doesn't get baked into phosphor afterglow,
        // which would persist after the cursor moves away).
        self.apply_interactive_glow(frame, now);

        // Message box drawn AFTER phosphor/anomaly/atmospheric effects
        // so it survives all post-processing — glow + typewriter reveal.

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

        // ── Component timing: sim → render boundary ────────────────────
        // Everything above this line is "simulation" (atmosphere events,
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

        // ── Atmospheric Event Engine: render active events ──
        if !self.event_manager.is_empty() {
            // Compute message bounds if a message is active
            let msg_bounds = if self.message_text.is_some() {
                let min_col = self.message.iter().map(|m| m.col).min();
                let max_col = self.message.iter().map(|m| m.col).max();
                let min_line = self.message.iter().map(|m| m.line).min();
                let max_line = self.message.iter().map(|m| m.line).max();
                if let (Some(mx), Some(mx2), Some(my), Some(my2)) =
                    (min_col, max_col, min_line, max_line)
                {
                    Some((
                        mx,
                        my,
                        mx2.saturating_sub(mx).saturating_add(1),
                        my2.saturating_sub(my).saturating_add(1),
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            let palette_slice: &[Color] = &self.palette.colors;
            let event_ctx = crate::cloud::atmospheric_events::EventCtx {
                cols: self.cols,
                lines: self.lines,
                bg: self.palette.bg,
                palette_colors: palette_slice,
                now,
                message_bounds: msg_bounds,
                has_message: self.message_text.is_some(),
            };
            self.event_manager.render(&event_ctx, frame);

            // Update event states (phosphor seeding on Decay entry)
            self.event_manager.update(
                now,
                &mut self.phosphor,
                &mut self.phosphor_base_fg,
                &mut self.phosphor_base_ch,
                self.cols,
                self.lines,
            );
        }

        // --- Autonomous cinematic ecosystem tick ---
        // 1. Color ecosystem drift
        // The ecosystem always ticks for luminance/saturation/hue climate drift
        // (safe — only modulates rendering params, not the palette scheme).
        // Autonomous *palette* drift (scheme replacement) is gated behind
        // `auto_color_drift` so that explicit CLI/config/profile color
        // remains sticky by default.
        let maybe_drift = self
            .color_ecosystem
            .tick(now, &mut self.mt, self.color_scheme);
        if self.auto_color_drift {
            if let Some(new_scheme) = maybe_drift {
                self.set_color_scheme(new_scheme);
            }
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

        // 7. Apply global atmospheric frame effects (post-process)
        self.apply_atmospheric_frame_effects(frame, now);

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

        // ── Atmospheric Event Engine: clean stale phosphor residue ──
        {
            let total = (self.cols as usize) * (self.lines as usize);
            self.event_manager.clean_stale_phosphor(
                &mut self.phosphor,
                &mut self.phosphor_base_fg,
                &mut self.phosphor_base_ch,
                &mut self.phosphor_active,
                total,
            );
        }

        // Expire flash effect after duration
        if let Some(flash_time) = self.flash_time {
            if now.saturating_duration_since(flash_time).as_secs_f32() >= MOUSE_FLASH_DURATION_SECS
            {
                self.flash_time = None;
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

    /// Apply mouse cursor halo + click wave ripple as a global post-process.
    ///
    /// Forensic fix (Fix 2): the cursor halo and click flash used to live
    /// inside `Droplet::draw()`, applied per-cell as the droplet's trail
    /// was painted. When temporal prediction skipped `draw()` for a
    /// "predicted clean" droplet, the halo + ripple were skipped too —
    /// making the glow appear patchy (only on currently-advancing
    /// droplets) and entirely absent on background cells between
    /// droplets.
    ///
    /// This post-process pass fixes both problems by iterating over the
    /// screen region around the cursor / click origin and applying the
    /// brightness boost to every cell in range — regardless of which
    /// droplet owns it (or whether any droplet owns it at all).
    ///
    /// The math mirrors the original per-cell logic in `Droplet::draw()`
    /// (now removed): elliptical Chebyshev-distance falloff for the
    /// cursor halo, dual-ring euclidean-distance ripple for the click
    /// wave. The only difference is the iteration driver: instead of
    /// "for each droplet, for each trail cell", it's "for each cell in
    /// the glow bounding box".
    ///
    /// Runs AFTER the droplet draw pass (so it overlays freshly painted
    /// cells) and BEFORE phosphor decay (so the glow doesn't get baked
    /// into afterglow, which would persist after the cursor moves away).
    fn apply_interactive_glow(&mut self, frame: &mut Frame, now: Instant) {
        // Snapshot the interactive state we need. Borrowed immutably via
        // the local snapshot so we can mutably borrow `frame` for set()
        // calls without conflict.
        let mouse_enabled = self.mouse_enabled;
        let mouse_col = self.mouse_col;
        let mouse_line = self.mouse_line;
        let flash_col = self.flash_col;
        let flash_line = self.flash_line;
        let flash_time = self.flash_time;
        let cols = self.cols;
        let lines = self.lines;
        let bg = self.palette.bg;

        // Compute the click wave's elapsed time once. If no flash is
        // active, skip the click-wave branch entirely.
        let flash_elapsed_secs =
            flash_time.map(|ft| now.saturating_duration_since(ft).as_secs_f32());
        let flash_active = flash_elapsed_secs
            .map(|e| e < MOUSE_FLASH_DURATION_SECS)
            .unwrap_or(false);

        // Determine the bounding box of cells that need to be touched.
        // The cursor halo covers a (2*MOUSE_GLOW_RADIUS_COLS+1) ×
        // (2*MOUSE_GLOW_RADIUS_LINES+1) rectangle around the cursor.
        // The click wave at peak expansion reaches
        // `MOUSE_FLASH_DURATION_SECS × MOUSE_FLASH_SPEED` cells —
        // we compute the active radius from the remaining fade window.
        //
        // We union both regions into one bounding box so we only iterate
        // the affected cells once (a cell in both regions gets both
        // contributions in a single pass).
        let mut min_col = cols;
        let mut max_col = 0u16;
        let mut min_line = lines;
        let mut max_line = 0u16;

        let glow_rc = MOUSE_GLOW_RADIUS_COLS.ceil() as u16;
        let glow_rl = MOUSE_GLOW_RADIUS_LINES.ceil() as u16;
        // Forensic fix (Task 1): only include the cursor halo in the
        // bounding box when it can actually change cell colors. The
        // default `MOUSE_GLOW_INTENSITY = 0.0` (dim cinematic mode) means
        // the halo adds zero brightness — but the previous code still
        // iterated the 15×11 box around the cursor every frame, dirty-
        // marking blank cells by rewriting their `fg: None` to
        // `fg: Some(Color::Rgb{bg_color})` (same visual, different Cell
        // representation, but equality check fails so the dirty mark
        // fires). Skipping the branch entirely when intensity is 0
        // eliminates the phantom dirty marks AND the per-frame bounding-
        // box iteration cost.
        let glow_active = mouse_enabled
            && mouse_col != u16::MAX
            && mouse_line != u16::MAX
            && MOUSE_GLOW_INTENSITY > 0.0;
        if glow_active {
            min_col = min_col.min(mouse_col.saturating_sub(glow_rc));
            max_col = max_col.max(mouse_col.saturating_add(glow_rc));
            min_line = min_line.min(mouse_line.saturating_sub(glow_rl));
            max_line = max_line.max(mouse_line.saturating_add(glow_rl));
        }

        if flash_active {
            let elapsed = flash_elapsed_secs.unwrap_or(0.0);
            let raw_fade = (1.0 - elapsed / MOUSE_FLASH_DURATION_SECS).max(0.0);
            let fade = raw_fade * raw_fade.sqrt();
            // Active ring radius — primary ring is the leading edge, but
            // we touch the full disc from origin to (primary_radius +
            // ring_width) so trailing cells inside the disc still get the
            // squared-falloff contribution.
            let primary_radius = elapsed * MOUSE_FLASH_SPEED;
            let reach = (primary_radius + MOUSE_FLASH_RING_WIDTH).ceil() as u16;
            min_col = min_col.min(flash_col.saturating_sub(reach));
            max_col = max_col.max(flash_col.saturating_add(reach));
            min_line = min_line.min(flash_line.saturating_sub(reach));
            max_line = max_line.max(flash_line.saturating_add(reach));
            // Suppress unused-warning when fade isn't read elsewhere — the
            // per-cell branch below consumes it.
            let _ = fade;
        }

        // If neither region is active (no mouse, no flash), bail.
        if min_col > max_col || min_line > max_line {
            return;
        }

        // Clamp the bounding box to the actual screen.
        let c0 = min_col.min(cols);
        let c1 = max_col.min(cols.saturating_sub(1));
        let l0 = min_line.min(lines);
        let l1 = max_line.min(lines.saturating_sub(1));
        if c0 > c1 || l0 > l1 {
            return;
        }

        // Iterate the bounding box. For each cell, read the current
        // content, apply the cursor halo + click wave brightness boost
        // to the foreground color, and write back via Frame::set (which
        // performs the content-aware dirty check — cells whose RGB
        // didn't actually change after rounding won't be marked dirty).
        for line in l0..=l1 {
            for col in c0..=c1 {
                let idx = match frame.index(col, line) {
                    Some(i) => i,
                    None => continue,
                };
                let cell = frame.cell_at_index(idx);

                // Decode the cell's foreground color. If the cell has no
                // foreground (blank cell with only bg), treat its
                // foreground as the background color so the glow can
                // brighten background cells too — this is what makes
                // the halo visible on cells between droplets.
                let fg_color = cell.fg.or(bg);
                let Some(fg_color) = fg_color else {
                    continue;
                };
                let Some((mut r, mut g, mut b)) = crate::palette::decode_color(fg_color) else {
                    continue;
                };

                let mut touched = false;

                // Cursor halo: elliptical Chebyshev-style falloff (matches
                // the original per-cell formula in Droplet::draw()).
                // Skipped entirely when MOUSE_GLOW_INTENSITY = 0.0 (the
                // bounding-box check above already excluded this case, but
                // we guard here too for defense-in-depth).
                if glow_active {
                    let col_dist = if col > mouse_col {
                        (col - mouse_col) as f32
                    } else {
                        (mouse_col - col) as f32
                    };
                    let line_dist = if line > mouse_line {
                        (line - mouse_line) as f32
                    } else {
                        (mouse_line - line) as f32
                    };
                    let norm_col = col_dist / MOUSE_GLOW_RADIUS_COLS;
                    let norm_line = line_dist / MOUSE_GLOW_RADIUS_LINES;
                    let dist_sq = norm_col * norm_col + norm_line * norm_line;
                    if dist_sq < 1.0 {
                        let glow = (1.0 - dist_sq) * MOUSE_GLOW_INTENSITY;
                        let wf = (glow * 256.0) as i32;
                        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        // Only mark touched if the color actually changed
                        // (wf > 0). When intensity=0, wf=0 and the RGB is
                        // unchanged — but we still fall through to write
                        // back the cell with `fg: Some(Color::Rgb{...})`
                        // which would dirty-mark blank cells unnecessarily.
                        if wf > 0 {
                            touched = true;
                        }
                    }
                }

                // Click wave: dual-ring euclidean ripple.
                if flash_active && flash_col != u16::MAX && flash_line != u16::MAX {
                    let elapsed = flash_elapsed_secs.unwrap_or(0.0);
                    let col_dist = if col > flash_col {
                        (col - flash_col) as f32
                    } else {
                        (flash_col - col) as f32
                    };
                    let line_dist = if line > flash_line {
                        (line - flash_line) as f32
                    } else {
                        (flash_line - line) as f32
                    };
                    let euclidean = (col_dist * col_dist + line_dist * line_dist).sqrt();
                    let raw_fade = (1.0 - elapsed / MOUSE_FLASH_DURATION_SECS).max(0.0);
                    let fade = raw_fade * raw_fade.sqrt();

                    let primary_radius = elapsed * MOUSE_FLASH_SPEED;
                    let primary_dist = (euclidean - primary_radius).abs();
                    let mut factor = 0.0;
                    if primary_dist < MOUSE_FLASH_RING_WIDTH {
                        let t = 1.0 - primary_dist / MOUSE_FLASH_RING_WIDTH;
                        let t_smooth = t * t;
                        factor = t_smooth * MOUSE_FLASH_INTENSITY * fade;
                    }

                    let secondary_radius =
                        elapsed * MOUSE_FLASH_SPEED * MOUSE_FLASH_SECONDARY_SPEED_FRAC;
                    let secondary_dist = (euclidean - secondary_radius).abs();
                    if secondary_dist < MOUSE_FLASH_RING_WIDTH {
                        let t = 1.0 - secondary_dist / MOUSE_FLASH_RING_WIDTH;
                        let t_smooth = t * t;
                        factor +=
                            t_smooth * MOUSE_FLASH_INTENSITY * MOUSE_FLASH_SECONDARY_FRAC * fade;
                    }

                    if factor > 0.0 {
                        let wf = (factor * 256.0) as i32;
                        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        touched = true;
                    }
                }

                if !touched {
                    continue;
                }

                // Write the boosted color back. Preserve the cell's
                // character and bold flag — we only changed fg.
                let new_fg = Color::Rgb { r, g, b };
                let new_cell = crate::cell::Cell {
                    ch: cell.ch,
                    fg: Some(new_fg),
                    bg: cell.bg,
                    bold: cell.bold,
                };
                frame.set(col, line, new_cell);
            }
        }
    }
}
