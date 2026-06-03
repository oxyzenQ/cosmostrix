// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Spawn, reset, and column management methods for Cloud.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
    SeedableRng,
};

use crate::constants::*;
use crate::droplet::Droplet;

use super::state::{ColumnStatus, DropletSpawnSpec};

use super::ecosystem::{AtmosphericEvolution, ColorEcosystem, RendererMemory, StorytellingState};

use super::Cloud;

impl Cloud {
    pub fn reset(&mut self, cols: u16, lines: u16) {
        // Defense in depth: clamp even though callers should clamp before
        // calling. Prevents degenerate sizes from reaching buffer allocation
        // or Uniform::new_inclusive construction.
        self.cols = cols.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
        self.lines = lines.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);

        let pool_size = (DROPLET_COUNT_FACTOR * self.cols as f32).round() as usize;
        self.droplets.clear();
        self.droplets.resize_with(pool_size, Droplet::new);
        self.spawn_scan_idx = 0;

        let max_line = lines.saturating_sub(2);
        let max_len = max_line.max(1);
        self.rand_line = Uniform::new_inclusive(0, max_line).expect("rand_line: max_line >= 0");
        self.rand_len =
            Uniform::new_inclusive(1, max_len).expect("rand_len: max_len >= 1 after max(1)");
        self.rand_col =
            Uniform::new_inclusive(0, cols.saturating_sub(1)).expect("rand_col: cols-1 >= 0");
        self.rand_cpidx = Uniform::new_inclusive(0, MAX_CHAR_POOL_IDX)
            .expect("rand_cpidx: [0,2047] always valid");

        self.recalc_droplets_per_sec();

        self.col_stat.clear();
        self.col_stat.resize(
            cols as usize,
            ColumnStatus {
                max_speed_pct: 1.0,
                num_droplets: 0,
                can_spawn: true,
            },
        );

        // Initialize palette generation system for current terminal size
        self.palette_table[self.active_palette_slot as usize] = Some(self.palette.clone());
        self.column_palette_slot.clear();
        self.column_palette_slot
            .resize(cols as usize, self.active_palette_slot);
        self.column_transition_delay_ms.clear();
        self.column_transition_delay_ms.resize(cols as usize, 0);
        self.transition_start = None;
        self.previous_char_pool.clear();
        self.charset_transition_start = None;

        self.fill_glitch_map();
        self.fill_color_map();
        self.set_column_speeds();
        self.update_droplet_speeds();

        // Reset phosphor state for new terminal size
        let total = (cols as usize) * (lines as usize);
        self.phosphor.clear();
        self.phosphor.resize(total, 0);
        self.phosphor_base_fg.clear();
        self.phosphor_base_fg.resize(total, None);
        self.phosphor_base_ch.clear();
        self.phosphor_base_ch.resize(total, '\0');
        self.phosphor_layer.clear();
        self.phosphor_layer.resize(total, 0);
        self.phosphor_fresh.clear();
        self.phosphor_fresh.resize(total, false);

        // Reset anomaly zones on terminal resize
        self.anomaly_zones.clear();

        if self.message_text.is_some() {
            self.reset_message();
        }

        let now = Instant::now();
        self.last_glitch_time = now;
        self.next_glitch_time =
            now + Duration::from_millis(self.rand_glitch_ms.sample(&mut self.mt) as u64);
        self.last_spawn_time = now;
        self.spawn_remainder = 0.0;
        self.force_draw_everything = true;
        self.frames_since_full_redraw = 0;
        self.last_reseed_time = now;
        self.last_phosphor_time = now;

        // Phase 3: Reset cinematic subsystems on terminal resize
        self.color_ecosystem = ColorEcosystem::new(now);
        self.atmosphere = AtmosphericEvolution::new(now);
        self.memory = RendererMemory::new(now);
        self.storytelling = StorytellingState::new(now);
        self.profile_transition_start = None;
        // Note: profile and profile params are preserved across resets
    }

    pub fn init_chars(&mut self, chars: Vec<char>) {
        self.rebuild_char_pools(chars);
        self.previous_char_pool.clear();
        self.charset_transition_start = None;

        self.reset_phosphor_state();

        // Flag semantic invalidation so the Terminal's LastFrame cache is
        // fully invalidated on the next rain_at() call. This eliminates stale
        // glyph residue that can persist when only dirty-region invalidation
        // is used — immediate charset initialization is a semantic mutation,
        // not a cell mutation.
        self.semantic_invalidate = true;
    }

    pub fn transition_chars(&mut self, chars: Vec<char>) {
        self.previous_char_pool = if self.char_pool.is_empty() {
            vec!['0', '1']
        } else {
            self.char_pool.clone()
        };
        self.rebuild_char_pools(chars);
        self.charset_transition_start = Some(Instant::now());
    }

    pub(super) fn charset_wave_line_at(&self, now: Instant) -> Option<f32> {
        let start = self.charset_transition_start?;
        let elapsed_ms = now.saturating_duration_since(start).as_millis() as f32;
        let progress = (elapsed_ms / CHARSET_TRANSITION_DURATION_MS as f32).clamp(0.0, 1.0);
        Some(progress * (self.lines as f32 + 1.0))
    }

    /// Compute the color transition wave line position at the given time.
    /// Returns None if no transition is active. The wave sweeps from 0 to
    /// lines+1 over COLOR_TRANSITION_DURATION_MS, with the first
    /// COLOR_TRANSITION_INITIAL_VISIBLE_PCT of rows adopting immediately
    /// for responsive first-frame feedback.
    pub(super) fn color_wave_line_at(&self, now: Instant) -> Option<f32> {
        let start = self.transition_start?;
        let elapsed_ms = now.saturating_duration_since(start).as_millis() as f32;
        let duration = COLOR_TRANSITION_DURATION_MS as f32;
        if elapsed_ms >= duration {
            return Some(self.lines as f32 + 1.0); // Wave complete
        }
        // The initial band of rows adopts immediately for first-frame feedback.
        // We do this by offsetting the wave start: the wave line already
        // includes the initial visible fraction at t=0.
        let initial_frac = COLOR_TRANSITION_INITIAL_VISIBLE_PCT;
        let progress = (elapsed_ms / duration).clamp(0.0, 1.0);
        // At progress=0, wave_line = initial_frac * lines → first band visible.
        // At progress=1, wave_line = lines + 1 → entire screen converted.
        let wave_line = initial_frac * self.lines as f32
            + progress * (1.0 - initial_frac) * (self.lines as f32 + 1.0);
        Some(wave_line)
    }

    pub(super) fn rebuild_char_pools(&mut self, chars: Vec<char>) {
        self.chars = chars;
        if self.chars.is_empty() {
            self.chars.push('0');
            self.chars.push('1');
        }

        self.char_pool.resize(CHAR_POOL_SIZE, '0');
        self.glitch_pool.resize(GLITCH_POOL_SIZE, '0');
        self.glitch_pool_idx = 0;

        let dist = Uniform::new_inclusive(0usize, self.chars.len().saturating_sub(1))
            .expect("char_pool: chars.len() >= 2 (guaranteed by empty check above)");
        for i in 0..self.char_pool.len() {
            let idx = dist.sample(&mut self.mt);
            self.char_pool[i] = self.chars[idx];
        }
        for i in 0..self.glitch_pool.len() {
            let idx = dist.sample(&mut self.mt);
            self.glitch_pool[i] = self.chars[idx];
        }
    }

    pub(super) fn reset_phosphor_state(&mut self) {
        let total = (self.cols as usize) * (self.lines as usize);
        self.phosphor.clear();
        self.phosphor.resize(total, 0);
        self.phosphor_base_fg.clear();
        self.phosphor_base_fg.resize(total, None);
        self.phosphor_base_ch.clear();
        self.phosphor_base_ch.resize(total, '\0');
        self.phosphor_layer.clear();
        self.phosphor_layer.resize(total, 0);
    }

    pub(super) fn recalc_droplets_per_sec(&mut self) {
        if self.lines == 0 || self.cols == 0 {
            self.droplets_per_sec = 0.0;
            return;
        }
        let droplet_seconds = (self.lines as f32) / self.chars_per_sec.max(0.001);
        if droplet_seconds <= 0.0 {
            self.droplets_per_sec = 0.0;
            return;
        }
        let dps = (self.cols as f32) * self.droplet_density / droplet_seconds;
        self.droplets_per_sec = if dps.is_finite() { dps.max(0.0) } else { 0.0 };
    }

    pub(super) fn fill_glitch_map(&mut self) {
        if !self.glitchy {
            self.glitch_map.clear();
            return;
        }
        let size = self.lines as usize * self.cols as usize;
        self.glitch_map.resize(size, false);
        for i in 0..size {
            self.glitch_map
                .set(i, self.rand_chance.sample(&mut self.mt) <= self.glitch_pct);
        }
    }

    pub(super) fn fill_color_map(&mut self) {
        let size = self.lines as usize * self.cols as usize;
        self.color_map.resize(size, 0);

        let n = self.palette.colors.len().max(1);
        let (low, high) = match n {
            0..=2 => (0, 0),
            3 => (1, 1),
            _ => (1, (n - 2) as u8),
        };
        let dist =
            Uniform::new_inclusive(low, high).expect("fill_color_map: low <= high by construction");

        for v in &mut self.color_map {
            *v = dist.sample(&mut self.mt);
        }
    }

    pub(super) fn set_column_spawn(&mut self, col: u16, b: bool) {
        if let Some(cs) = self.col_stat.get_mut(col as usize) {
            cs.can_spawn = b;
        }
    }

    pub(super) fn set_column_speeds(&mut self) {
        for cs in &mut self.col_stat {
            cs.max_speed_pct = if self.async_mode {
                self.rand_speed.sample(&mut self.mt)
            } else {
                1.0
            };
        }
    }

    pub(super) fn update_droplet_speeds(&mut self) {
        for d in &mut self.droplets {
            if !d.is_alive {
                continue;
            }
            if let Some(cs) = self.col_stat.get(d.bound_col as usize) {
                let layer_speed = PARALLAX_SPEED_MULT[d.layer as usize];
                d.chars_per_sec = cs.max_speed_pct * self.chars_per_sec * layer_speed;
                // Keep velocity clamped to new terminal velocity
                let terminal = d.chars_per_sec * DROPLET_TERMINAL_VELOCITY_MULT;
                d.velocity = d.velocity.min(terminal);
            }
        }
    }

    pub(super) fn time_for_glitch(&self, now: Instant) -> bool {
        self.glitchy && now >= self.next_glitch_time
    }

    #[must_use]
    #[inline]
    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        let idx = col as usize * self.lines as usize + line as usize;
        self.glitch_map.get(idx).is_some_and(|b| *b)
    }

    pub(super) fn do_glitch_span(&mut self, start_line: u16, hp: u16, col: u16, cp_idx: u16) {
        if !self.glitchy {
            return;
        }

        for line in start_line..=hp {
            if line >= self.lines {
                break;
            }
            if self.is_glitched(line, col) {
                let char_idx = ((cp_idx as usize) + (line as usize)) % self.char_pool.len();
                let repl = self.glitch_pool[self.glitch_pool_idx % self.glitch_pool.len()];
                self.char_pool[char_idx] = repl;
                self.glitch_pool_idx = (self.glitch_pool_idx + 1) % self.glitch_pool.len();
            }
        }
    }

    pub(super) fn build_droplet_spec(&mut self, col: u16) -> DropletSpawnSpec {
        let mut end_line = self.lines.saturating_sub(1);
        if self.rand_chance.sample(&mut self.mt) <= self.die_early_pct {
            end_line = self.rand_line.sample(&mut self.mt);
        }
        let cp_idx = self.rand_cpidx.sample(&mut self.mt);

        let mut len = self.lines;
        if self.rand_chance.sample(&mut self.mt) <= self.short_pct {
            len = self.rand_len.sample(&mut self.mt);
        }

        // Assign parallax layer (0=far, 1=mid, 2=near)
        // Weighted: more background, fewer foreground for depth
        let layer_roll = self.rand_chance.sample(&mut self.mt);
        let layer: u8 = if layer_roll < 0.35 {
            0
        } else if layer_roll < 0.75 {
            1
        } else {
            2
        };

        // Adjust length by parallax layer
        let len_mult = PARALLAX_LENGTH_MULT[layer as usize];
        len = ((len as f32) * len_mult).max(1.0) as u16;

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
        let layer_speed = PARALLAX_SPEED_MULT[layer as usize];
        let mut speed = self
            .col_stat
            .get(col as usize)
            .map(|cs| cs.max_speed_pct)
            .unwrap_or(1.0)
            * self.chars_per_sec
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
            palette_slot,
            turb_phase,
        }
    }

    pub(super) fn maybe_reseed_rng(&mut self, now: Instant) {
        if now.saturating_duration_since(self.last_reseed_time)
            >= Duration::from_secs(RNG_RESEED_INTERVAL_SECS)
        {
            let elapsed = now.elapsed();
            let seed = elapsed.as_nanos() as u64 ^ elapsed.as_secs();
            self.mt = StdRng::seed_from_u64(seed);
            self.last_reseed_time = now;
        }
    }

    pub(super) fn spawn_droplets(&mut self, now: Instant, scale: f32) {
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

        let len = self.droplets.len();
        if len == 0 {
            return;
        }

        for _ in 0..to_spawn {
            let mut col = self.rand_col.sample(&mut self.mt);
            if self.full_width {
                col &= 0xFFFE;
            }

            if col as usize >= self.col_stat.len() {
                continue;
            }

            if !self.col_stat[col as usize].can_spawn
                || self.col_stat[col as usize].num_droplets >= self.max_droplets_per_column
            {
                continue;
            }

            // Mouse avoidance: skip spawning near cursor
            if self.mouse_enabled && self.mouse_col != u16::MAX {
                let col_dist = col.abs_diff(self.mouse_col);
                if col_dist <= MOUSE_AVOID_RADIUS_COLS {
                    continue;
                }
            }

            // Atmospheric depth: apply per-layer density control.
            // Pre-determine the layer for this spawn to check density.
            let layer_roll = self.rand_chance.sample(&mut self.mt);
            let layer: u8 = if layer_roll < 0.35 {
                0
            } else if layer_roll < 0.75 {
                1
            } else {
                2
            };
            // Far layer (0) spawns less frequently
            let density_mult = PARALLAX_DENSITY_MULT[layer as usize];
            if self.rand_chance.sample(&mut self.mt) > density_mult {
                continue;
            }

            let start = self.spawn_scan_idx.min(len.saturating_sub(1));
            let mut found = None;

            let mut idx = start;
            while idx < len {
                if !self.droplets[idx].is_alive {
                    found = Some(idx);
                    break;
                }
                idx += 1;
            }
            if found.is_none() {
                idx = 0;
                while idx < start {
                    if !self.droplets[idx].is_alive {
                        found = Some(idx);
                        break;
                    }
                    idx += 1;
                }
            }

            let Some(di) = found else {
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
            self.spawn_scan_idx = (di + 1) % len;

            self.col_stat[col as usize].can_spawn = false;
            self.col_stat[col as usize].num_droplets += 1;
        }
    }
}
