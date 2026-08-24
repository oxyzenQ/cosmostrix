// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Spawn, reset, and column management methods for Cloud.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
    SeedableRng,
};

use crate::constants::*;
use crate::droplet::Droplet;
use crate::rain_style::RainStyle;

use super::state::{ColumnStatus, DropletSpawnSpec};

use super::ecosystem::{RendererMemory, StorytellingState};

use super::Cloud;

impl Cloud {
    pub fn reset(&mut self, cols: u16, lines: u16) {
        self.reset_with_bounds(cols, lines, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES);
    }

    /// Benchmark variant of [`Self::reset`]: clamps to the benchmark bounds
    /// (8K UHD by default) instead of the interactive bounds, mirroring
    /// `Frame::new_bench`. The scaling and stress benchmarks intentionally
    /// exceed the interactive safety cap to stress-test cell throughput —
    /// the interactive clamp would silently shrink the simulated rain area
    /// to the top-left corner of a 7680-column frame.
    ///
    /// Triple-engine LTS audit LOW-2 follow-up: previously the benchmark
    /// passed its oversized dimensions through `reset`, which clamped only
    /// `self.cols`/`self.lines` (plus the pool sizing and a few gates) while
    /// the RNG ranges, column tables, and per-cell LUTs were built from the
    /// RAW dimensions — an inconsistent hybrid state where rain spawned
    /// across the full bench width but glitch/color-map coverage stopped at
    /// the interactive cap. This variant keeps the benchmark fully
    /// consistent at bench-bounded dimensions.
    pub fn reset_bench(&mut self, cols: u16, lines: u16) {
        self.reset_with_bounds(cols, lines, BENCH_MAX_COLS, BENCH_MAX_LINES);
    }

    /// Core reset: clamps `cols`/`lines` to `[MIN, max]` and rebuilds every
    /// size-dependent structure. Factored out so the interactive and
    /// benchmark paths share one routine, mirroring
    /// `Frame::new_with_bounds`.
    fn reset_with_bounds(&mut self, cols: u16, lines: u16, max_cols: u16, max_lines: u16) {
        // Defense in depth: clamp even though callers should clamp before
        // calling. Prevents degenerate sizes from reaching buffer allocation
        // or Uniform::new_inclusive construction.
        //
        // Triple-engine LTS audit LOW-2 (2026-08-23): the clamped values now
        // shadow the raw parameters for the WHOLE function body. Previously
        // only `self.cols`/`self.lines` (and the droplet pool sizing) used
        // the clamped values, while the RNG ranges, column tables, and
        // per-cell LUTs below were built from the RAW parameters — panic-free
        // (saturating arithmetic + `Frame::set` bounds checks) but
        // inconsistent: an oversized caller could spawn droplets outside the
        // clamped grid while the glitch/color maps only covered the clamped
        // region. Shadowing makes every downstream consumer see the same
        // clamped dimensions.
        let cols = cols.clamp(MIN_TERMINAL_COLS, max_cols);
        let lines = lines.clamp(MIN_TERMINAL_LINES, max_lines);
        self.cols = cols;
        self.lines = lines;

        if matches!(self.rain_style, RainStyle::Monolith) {
            self.droplets.clear();
        } else {
            let pool_size = (DROPLET_COUNT_FACTOR * self.cols as f32).round() as usize;
            self.droplets.clear();
            self.droplets.resize_with(pool_size, Droplet::new);
        }
        self.monolith_rain.reset(self.cols);

        // Re-seed the droplet free-list: after clear+resize, all droplets
        // are dead (Droplet::new defaults is_alive=false), so every index
        // 0..len is free. This enables O(1) spawn slot lookup instead of
        // the previous linear scan.
        self.droplet_free_list.clear();
        self.droplet_free_list.extend(0..self.droplets.len());

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
        self.transition_start = None;
        self.previous_char_pool.clear();
        self.charset_transition_start = None;

        self.fill_glitch_map();
        self.fill_color_map();
        self.set_column_speeds();
        self.update_droplet_speeds();

        // Precompute viewport edge fade LUT for the new terminal height.
        // Index by `line`; value is the fade factor in [EDGE_FADE_BOTTOM_MIN, 1.0].
        // Eliminates per-cell float division in Droplet::draw and Monolith draw.
        self.edge_fade_lut.clear();
        self.edge_fade_lut.reserve(lines as usize);
        for line in 0..lines {
            self.edge_fade_lut
                .push(crate::droplet::viewport_edge_fade(line, lines));
        }

        // Pre-bake 2D vignette factor LUT (flat: `line * cols + col`).
        // Eliminates per-cell sqrt + smoothstep in Droplet::draw's hot path.
        // At 200×60 = 48 KiB, 105×64 ≈ 27 KiB — trivial memory cost.
        let vignette_total = (cols as usize) * (lines as usize);
        self.vignette_lut.clear();
        self.vignette_lut.reserve(vignette_total);
        for line in 0..lines {
            for col in 0..cols {
                self.vignette_lut
                    .push(crate::brightness_factors::vignette_factor(
                        col, line, cols, lines,
                    ));
            }
        }
        self.vignette_lut_dims = (cols, lines);

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
        self.phosphor_in_active.clear();
        self.phosphor_in_active.resize(total, false);
        self.phosphor_active.clear();

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
        if matches!(self.rain_style, RainStyle::Monolith) {
            self.semantic_invalidate = true;
        }
        self.frames_since_full_redraw = 0;
        self.frames_since_stuck_sweep = 0;
        self.last_reseed_time = now;
        self.last_phosphor_time = now;

        // Phase D Bug #8 + #9 fix: color_ecosystem + entropy_drift are drift
        // accumulators (luminance_climate, saturation_climate, hue_drift,
        // density_offset, etc.) — they are independent of terminal size.
        // Previously reset() re-initialized them to defaults, which caused:
        //   - Bug #9: visible brightness/saturation/hue discontinuity on
        //     every live-reload (config edit)
        //   - Bug #8: drift state lost on terminal resize
        // Both are wrong — drift state should persist across resize and
        // live-reload. The initial ColorEcosystem::new(now) + EntropyDrift::new(now)
        // in Cloud::new() handles fresh-start initialization; reset() should
        // NOT clobber accumulated drift.
        //
        // (memory + storytelling ARE reset here because they track
        // cell-grid-dependent state — stuck cells from the old grid are
        // meaningless after a resize.)
        self.memory = RendererMemory::new(now);
        self.storytelling = StorytellingState::new(now);
        self.profile_transition_start = None;
        self.event_manager.reset(now);
        self.gust = crate::cloud::living_rain::GustState::new(now);
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

        // v18 cinematic unification: force a full redraw on the next frame
        // so the charset wave is visible on EVERY rain style, not just
        // Monolith. Without this, glyph-mode cells only update when
        // droplets happen to pass through them — the wave stays invisible
        // and the screen appears to swap instantly. This mirrors the same
        // pattern used by `apply_new_palette()` for color transitions:
        // `force_draw_everything` clears the frame and wipes stale
        // phosphor_base_ch, then the per-cell `get_char()` consults
        // `charset_wave_line` to pick old-pool chars below the wave and
        // new-pool chars above it, producing the top-to-bottom sweep.
        self.force_draw_everything = true;
        self.semantic_invalidate = true;

        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.clear_draw_history();
            self.reset_phosphor_state();
        }
    }

    pub(crate) fn charset_wave_line_at(&self, now: Instant) -> Option<f32> {
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
    pub(crate) fn color_wave_line_at(&self, now: Instant) -> Option<f32> {
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

    pub(crate) fn rebuild_char_pools(&mut self, chars: Vec<char>) {
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

        // v16: Cache binary check — eliminates O(2048) scan per frame.
        self.char_pool_is_binary =
            !self.char_pool.is_empty() && self.char_pool.iter().all(|ch| matches!(ch, '0' | '1'));
    }

    pub(crate) fn reset_phosphor_state(&mut self) {
        let total = (self.cols as usize) * (self.lines as usize);
        self.phosphor.clear();
        self.phosphor.resize(total, 0);
        self.phosphor_base_fg.clear();
        self.phosphor_base_fg.resize(total, None);
        self.phosphor_base_ch.clear();
        self.phosphor_base_ch.resize(total, '\0');
        self.phosphor_layer.clear();
        self.phosphor_layer.resize(total, 0);
        self.phosphor_active.clear();
        // Also clear phosphor_last_fresh — stale indices from a previous
        // scene/charset transition could reference cells that are now in
        // a different state. Clearing prevents the decay pass from
        // unsetting phosphor_fresh on cells that were never set this frame.
        self.phosphor_last_fresh.clear();
        // ME-01 (mouse-effect state leak fix): also clear the two BitVecs.
        // Without this, scene switch via 'x' (which calls reset_phosphor_state
        // via transition_rain_style) leaves stale `true` bits in
        // phosphor_in_active — freshly-drawn cells in the new scene then fail
        // the `if !self.phosphor_in_active[pidx]` check in phosphor_decay_pass
        // and never get pushed onto phosphor_active, so Pass 3 never decays
        // them. The cells stay at their last-drawn color → visible "noda"
        // stain + slow click effect. Cloud::reset() (Space key) already clears
        // these — this matches that behavior for scene switches.
        self.phosphor_fresh.fill(false);
        self.phosphor_in_active.fill(false);
    }

    pub(crate) fn recalc_droplets_per_sec(&mut self) {
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

    pub(crate) fn fill_glitch_map(&mut self) {
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

    pub(crate) fn fill_color_map(&mut self) {
        let size = self.lines as usize * self.cols as usize;
        self.color_map.resize(size, 0);

        let n = self.palette.colors.len().max(1);
        // Guard: palette size must fit u8 index range for color_map.
        // Current max is 20 (Rainbow), but this prevents a latent panic
        // if a future palette exceeds 257 colors.
        debug_assert!(
            n <= 257,
            "palette too large for u8 color_map index: {n} colors"
        );
        let (low, high) = match n {
            0..=2 => (0, 0),
            3 => (1, 1),
            _ => (1, ((n - 2).min(255)) as u8),
        };
        let dist =
            Uniform::new_inclusive(low, high).expect("fill_color_map: low <= high by construction");

        for v in &mut self.color_map {
            *v = dist.sample(&mut self.mt);
        }
    }

    pub(crate) fn set_column_spawn(&mut self, col: u16, b: bool) {
        if let Some(cs) = self.col_stat.get_mut(col as usize) {
            cs.can_spawn = b;
        }
    }

    pub(crate) fn set_column_speeds(&mut self) {
        for cs in &mut self.col_stat {
            cs.max_speed_pct = if self.async_mode {
                // Organic speed distribution: take the max of two uniform
                // samples to get a triangular distribution skewed toward
                // 1.0 (mean ~0.78, min 0.33, max 1.0). Most columns run
                // near full speed with occasional slow streams — more
                // natural than a flat uniform distribution.
                let a = self.rand_speed.sample(&mut self.mt);
                let b = self.rand_speed.sample(&mut self.mt);
                a.max(b)
            } else {
                1.0
            };
        }
    }

    pub(crate) fn update_droplet_speeds(&mut self) {
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

    pub(crate) fn time_for_glitch(&self, now: Instant) -> bool {
        self.glitchy && now >= self.next_glitch_time
    }

    #[must_use]
    #[inline]
    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        // Cosmic Dragon egg #14: bounds-check + direct indexing instead of .get().
        // glitch_map is sized cols*lines. idx = col*lines + line.
        // Callers ensure line < lines (checked in do_glitch_span loop),
        // but col may not be checked. Use a single bounds check + direct index.
        let col_usize = col as usize;
        let line_usize = line as usize;
        if col_usize >= self.cols as usize || line_usize >= self.lines as usize {
            return false;
        }
        let idx = col_usize * self.lines as usize + line_usize;
        self.glitch_map[idx]
    }

    pub(crate) fn do_glitch_span(&mut self, start_line: u16, hp: u16, col: u16, cp_idx: u16) {
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

    pub(crate) fn build_droplet_spec(&mut self, col: u16) -> DropletSpawnSpec {
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
            tail_cells,
            palette_slot,
            turb_phase,
        }
    }

    pub(crate) fn maybe_reseed_rng(&mut self, now: Instant) {
        if now.saturating_duration_since(self.last_reseed_time)
            >= Duration::from_secs(RNG_RESEED_INTERVAL_SECS)
        {
            // CC-01: use saturating_duration_since(now) so bench-mode
            // synthetic sim_now (which races ahead of real time) degrades
            // to Duration::ZERO instead of returning a stale value via
            // elapsed() (which underflows on monotonic clocks). Mirrors
            // the rain.rs:84 timing-capture pattern.
            let elapsed = Instant::now().saturating_duration_since(now);
            let seed = elapsed.as_nanos() as u64 ^ elapsed.as_secs();
            self.mt = StdRng::seed_from_u64(seed);
            self.last_reseed_time = now;
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

    /// Re-allocate the glyph droplet pool and warm-start with a sparse set
    /// of pre-seeded droplets so the first post-switch frame has visible rain
    /// immediately, but not crowded.
    ///
    /// This is called by `transition_rain_style()` when switching from
    /// Monolith (or any style) to Glyph. Without warm-starting, the newly
    /// allocated pool would be empty and `spawn_droplets()` would need
    /// several frames to build visible density — producing a blank black
    /// screen for 100–500ms after the scene switch.
    ///
    /// ## Sparse fresh-entry semantics
    ///
    /// Only a small fraction of columns (WARM_START_SEED_FRACTION, bounded
    /// by WARM_START_SEED_MIN and WARM_START_SEED_MAX) are seeded with
    /// droplets. This prevents the "instant wall of rain" look while still
    /// ensuring visible content on the first frame.
    ///
    /// Seeded droplets get heads near the **top rows** (upper quarter of
    /// the viewport, capped at WARM_START_MAX_HEAD absolute rows) with
    /// short trails starting from row 0.
    ///
    /// The natural spawn system fills remaining columns over subsequent
    /// frames, gradually accelerated by the scene-entry ramp
    /// (glyph_entry_time) which scales spawn rate from
    /// `GLYPH_ENTRY_RAMP_MIN_SCALE` to 1.0 via exp approach
    /// (k = `GLYPH_ENTRY_RAMP_DECAY_RATE`), settling at
    /// `GLYPH_ENTRY_RAMP_SETTLE_FRAC` (95%) in
    /// `GLYPH_ENTRY_RAMP_DURATION_MS` (700ms).
    pub(crate) fn ensure_glyph_pool_and_warm_start(&mut self) {
        let pool_size = (DROPLET_COUNT_FACTOR * self.cols as f32).round() as usize;
        self.droplets.clear();
        self.droplets.resize_with(pool_size, Droplet::new);

        // Re-seed free-list: all fresh droplets are dead.
        self.droplet_free_list.clear();
        self.droplet_free_list.extend(0..self.droplets.len());

        // Reset column spawn state so all columns are eligible
        for cs in &mut self.col_stat {
            cs.can_spawn = true;
            cs.num_droplets = 0;
        }

        // Sparse seed: only a fraction of columns, not the full width.
        // This avoids the "instant wall of rain" over-density problem
        // while still providing visible content on the first frame.
        let now = Instant::now();
        let seed_limit = ((self.cols as f32 * WARM_START_SEED_FRACTION).round() as usize)
            .clamp(WARM_START_SEED_MIN, WARM_START_SEED_MAX);
        let head_cap = (self.lines / 4).clamp(2, WARM_START_MAX_HEAD);

        // Iterate columns with even spacing to maximize viewport coverage.
        // Column step = total_cols / seed_limit, so seeds are distributed
        // across the full width rather than clustered at the left edge.
        let col_step = (self.cols as usize / seed_limit.max(1)).max(1);
        for i in 0..seed_limit {
            let col = ((i * col_step) as u16).min(self.cols.saturating_sub(1));
            if col as usize >= self.col_stat.len() {
                continue;
            }
            if self.col_stat[col as usize].num_droplets >= self.max_droplets_per_column {
                continue;
            }

            let spec = self.build_droplet_spec(col);
            let end_line = spec.end_line;
            let d = &mut self.droplets[i];
            spec.apply_to(d);

            // Fresh-entry: head near the top, not scattered mid-screen.
            let head_line =
                (self.rand_chance.sample(&mut self.mt) * head_cap as f32).floor() as u16;
            let safe_head = head_line.min(end_line);
            d.head_put_line = safe_head;
            d.head_cur_line = safe_head;
            // Short trail: tail at row 0 so the visible trail is
            // 0..safe_head — compact, fresh, top-biased.
            d.tail_put_line = Some(0);
            d.tail_cur_line = 0;

            d.activate(now);

            self.col_stat[col as usize].num_droplets += 1;
            self.col_stat[col as usize].can_spawn = false;
        }

        // Start the scene-entry ramp: spawn rate gradually increases
        // from `GLYPH_ENTRY_RAMP_MIN_SCALE` to 1.0 via exp approach
        // (consistent with the pause/resume easing family).
        self.glyph_entry_time = Some(now);

        // Low spawn debt: let the ramp + natural spawn fill gradually
        // instead of flooding the first frame.
        self.spawn_remainder = WARM_START_SPAWN_DEBT;
    }

    /// Spawn a quantum-ripple particle burst at the click point (v17 mastery).
    ///
    /// Called by `Cloud::set_mouse_click` alongside the dual-ring flash wave.
    /// Up to `QUANTUM_RIPPLE_PARTICLE_COUNT` particles are activated per click
    /// (each in the first inactive pool slot). The particle pool is
    /// pre-allocated with `QUANTUM_RIPPLE_POOL_SIZE` slots — clicks beyond the
    /// pool capacity are silently dropped (the flash wave still spawns).
    pub(crate) fn spawn_quantum_ripple(&mut self, col: u16, line: u16) {
        let cx = col as f32 + 0.5;
        let cy = line as f32 + 0.5;
        let now = Instant::now();
        let chars = ['*', '+', '·'];
        // Snapshot the palette BODY color (mid-index of palette.colors)
        // once at click time. Avoid the head stop (last index) — it's
        // near-white across most schemes (gives droplets their bright
        // leading edge; using it for ripples made every click look white).
        // Each particle keeps this RGB even if the user switches palette
        // mid-flight → natural crossfade between old & new cohorts.
        //
        // (chroma audit, A1 spawn): the primary path uses
        // `palette::decode_color` -- a chroma engine helper that decodes
        // any Color variant to its RGB triple. The fallback constants
        // QUANTUM_BRAND_PURPLE_* are only hit when `palette.colors` is
        // empty OR `decode_color` returns None (Color::Reset), which are
        // degenerate cases that don't occur in production (build_palette
        // always produces ≥8 stops, none of which are Color::Reset).
        // The fallback is the legacy sRGB "purple brand color" and exists
        // solely so a unit test that constructs a Cloud without calling
        // build_palette doesn't panic on unwrap.
        let body_idx = self.palette.colors.len() / 2;
        let (body_r, body_g, body_b) = self
            .palette
            .colors
            .get(body_idx)
            .and_then(|c| crate::palette::decode_color(*c))
            .unwrap_or((
                QUANTUM_BRAND_PURPLE_R,
                QUANTUM_BRAND_PURPLE_G,
                QUANTUM_BRAND_PURPLE_B,
            ));
        let mut spawned = 0usize;
        for p in &mut self.quantum_particles {
            if spawned >= QUANTUM_RIPPLE_PARTICLE_COUNT {
                break;
            }
            if p.active {
                continue;
            }
            let angle: f32 = self.rand_chance.sample(&mut self.mt) * std::f32::consts::TAU;
            // v50 masterclass retune: narrower speed variance (0.9..1.1
            // instead of 0.8..1.2). At the old 0.8s lifespan the variance
            // didn't matter — particles died before speed differences
            // became visible. At the new 2.5s lifespan, particles with
            // 1.2x speed would visibly outpace 0.8x peers, breaking the
            // "coherent cohort" aesthetic. 0.9..1.1 keeps ±10% variance
            // (enough for organic feel) without visible stratification.
            let speed = QUANTUM_RIPPLE_SPEED * (0.9 + self.rand_chance.sample(&mut self.mt) * 0.2);

            // Pre-clamp: rand_chance returns [0, 1), but float rounding
            // on the multiply can theoretically reach chars.len(). The
            // .min() below also guards, but clamping the random value
            // first makes the intent explicit and avoids relying on the
            // downstream guard.
            let char_idx =
                (self.rand_chance.sample(&mut self.mt).min(0.999) * chars.len() as f32) as usize;
            p.active = true;
            p.x = cx;
            p.y = cy;
            p.vx = angle.cos() * speed;
            p.vy = angle.sin() * speed;
            p.birth = now;
            p.ch = chars[char_idx.min(chars.len() - 1)];
            p.r = body_r;
            p.g = body_g;
            p.b = body_b;
            // v50 (2026-08-17) trail particles: reset trail_count so the
            // reused pool slot starts with no trail. trail_x/trail_y
            // contents are stale from the previous spawn but will be
            // overwritten as the particle moves — trail_count=0 ensures
            // the trail render loop reads 0 entries on the first frame.
            p.trail_count = 0;
            spawned += 1;
        }
        // Increment active count — tracked incrementally so
        // apply_quantum_ripple can O(1) early-out when none are active.
        self.quantum_active_count = self.quantum_active_count.saturating_add(spawned);
    }
}
