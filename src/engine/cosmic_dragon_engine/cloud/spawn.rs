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
        // Z-6: mark benchmark mode — rain_at skips message cosmetics
        // (draw_message + border-cross detection). Owner directive: bench
        // mode measures critical path only (rain + 3 dragons), not cosmetics.
        self.bench_mode = true;
    }

    // v50.0.0-beta.7 LOC refactor: reset_with_bounds extracted to
    // spawn_reset.rs as a separate impl Cloud block.

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
        } else if matches!(self.rain_style, RainStyle::Vortex) {
            self.vortex_rain.clear_draw_history();
            self.reset_phosphor_state();
        }
    }

    /// Start a 500ms charset transition wave from a previous charset.
    ///
    /// Used by live config reload when the charset changes: the Cloud
    /// rebuild already installed the new `char_pool` via `init_chars`
    /// inside `create_cloud`, but `init_chars` clears
    /// `charset_transition_start = None` (instant path). Without this
    /// call the transition would be an instant glyph jump — inconsistent
    /// with the `s`/`S` shortkey, which uses `transition_chars` to
    /// produce a 500ms top-to-bottom wave.
    ///
    /// This method mirrors `start_transition_from_previous_palette`
    /// (the color-subsystem counterpart): the caller captures the OLD
    /// `char_pool` BEFORE the Cloud rebuild, then passes it here so the
    /// shader can read both old and new pools during the 500ms wave
    /// (see rain.rs `charset_wave_line_at`).
    ///
    /// This method:
    /// 1. Stores `prev_chars` in `previous_char_pool` so the per-cell
    ///    `get_char()` can pick old-pool glyphs below the wave and
    ///    new-pool glyphs above it.
    /// 2. Sets `charset_transition_start = Some(now)` to activate the wave.
    /// 3. Sets `force_draw_everything` + `semantic_invalidate` so the
    ///    first frame redraws everything under the new charset wave
    ///    (mirrors `transition_chars` v18 cinematic unification).
    /// 4. Clears monolith draw history + phosphor state when the rain
    ///    style is Monolith (mirrors `transition_chars`).
    ///
    /// Precondition: the caller must ensure that `prev_chars` is
    /// genuinely different from `self.char_pool` (same-charset no-op
    /// guard is the caller's responsibility, matching the contract of
    /// `start_transition_from_previous_palette`). An empty `prev_chars`
    /// (very first reload on a fresh session) falls back to the binary
    /// default `['0', '1']` so the wave is visually meaningful rather
    /// than empty — but the caller's `!prev_chars.is_empty()` guard
    /// usually prevents this branch entirely.
    pub fn start_transition_from_previous_charset(&mut self, prev_chars: Vec<char>) {
        // Install the caller-provided previous chars as the transition
        // source. The new char_pool is already in place from Cloud
        // construction (create_cloud -> init_chars); we only need to seed
        // the "previous" pool and arm the wave start time.
        self.previous_char_pool = if prev_chars.is_empty() {
            vec!['0', '1']
        } else {
            prev_chars
        };

        // Activate the 500ms top-to-bottom wave transition.
        self.charset_transition_start = Some(Instant::now());

        // Force full redraw so the new charset wave is visible on the
        // next frame across all rain styles (mirrors transition_chars
        // v18 cinematic unification + start_transition_from_previous_palette
        // force_draw contract).
        self.force_draw_everything = true;
        self.semantic_invalidate = true;

        if matches!(self.rain_style, RainStyle::Monolith) {
            self.monolith_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Vortex) {
            self.vortex_rain.clear_draw_history();
            self.reset_phosphor_state();
        }
    }

    pub(crate) fn charset_wave_line_at(&self, now: Instant) -> Option<f32> {
        let start = self.charset_transition_start?;
        let elapsed_ms = now.saturating_duration_since(start).as_millis() as f32;
        let progress = (elapsed_ms / CHARSET_TRANSITION_DURATION_MS as f32).clamp(0.0, 1.0);
        // S-master-HUNT-10: smoothstep easing (3t^2 - 2t^3) replaces the
        // linear velocity sweep. The wave now eases in at the top,
        // accelerates through the middle, and eases out at the bottom —
        // a more organic, cinematic feel vs the previous mechanical
        // constant-velocity sweep. smoothstep(0)=0, smoothstep(1)=1,
        // monotonic on [0,1] — so the wave still starts at row 0, ends
        // at lines+1, and progresses strictly downward (LTS-safe: all
        // existing ordering/threshold/completion tests pass unchanged).
        let eased = progress * progress * (3.0 - 2.0 * progress);
        Some(eased * (self.lines as f32 + 1.0))
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
        // S-master-HUNT-10: smoothstep easing (3t^2 - 2t^3) on the
        // post-initial-band progress. The initial band (initial_frac) is
        // preserved at t=0 (smoothstep(0)=0 — no easing applied to the
        // initial offset). The remaining sweep (1 - initial_frac) eases
        // in/out for a cinematic feel vs the previous constant-velocity
        // linear sweep. smoothstep(1)=1 — full screen still converts at
        // t=duration (LTS-safe).
        let eased = progress * progress * (3.0 - 2.0 * progress);
        // At progress=0, wave_line = initial_frac * lines → first band visible.
        // At progress=1, wave_line = initial_frac * lines + (1 - initial_frac) * (lines + 1)
        //                = lines + 1 - initial_frac ≈ lines + 1 → entire screen converted.
        let wave_line = initial_frac * self.lines as f32
            + eased * (1.0 - initial_frac) * (self.lines as f32 + 1.0);
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
        // v50.0.0-beta.6: apply terminal-aware speed_mult so droplets
        // fall faster on slower-rendering terminals (VTE/xterm.js).
        let effective_cps = self.chars_per_sec * self.speed_mult;
        let droplet_seconds = (self.lines as f32) / effective_cps.max(0.001);
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
        // v50.0.0-beta.6: apply terminal-aware speed_mult to droplet
        // terminal velocity so droplets fall faster on VTE terminals.
        let effective_cps = self.chars_per_sec * self.speed_mult;
        for d in &mut self.droplets {
            if !d.is_alive {
                continue;
            }
            if let Some(cs) = self.col_stat.get(d.bound_col as usize) {
                let layer_speed = PARALLAX_SPEED_MULT[d.layer as usize];
                d.chars_per_sec = cs.max_speed_pct * effective_cps * layer_speed;
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
        // glitch_map is sized colslines. idx = collines + line.
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
    /// Seeded droplets get heads near the top rows (upper quarter of
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
        // PERF-4: --no-effects gate. No-op when effects are disabled.
        // (set_mouse_click also early-returns under --no-effects, so this
        // gate is a defense-in-depth no-op for production click paths —
        // kept for direct test/spawn calls that bypass set_mouse_click.)
        if !self.effects_enabled {
            return;
        }
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
            p.max_trail = QUANTUM_RIPPLE_TRAIL_LEN as u8;
            p.lifetime = QUANTUM_RIPPLE_LIFETIME_SECS;
            p.sim_age = 0.0;
            spawned += 1;
        }
        // Increment active count — tracked incrementally so
        // apply_quantum_ripple can O(1) early-out when none are active.
        self.quantum_active_count = self.quantum_active_count.saturating_add(spawned);
    }

    /// Spawn a border-touch splash crown spark at `(col, line)`.
    ///
    /// F2 Splash Crown variant — see
    /// `docs/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md` §3.2.
    ///
    /// Emits `BORDER_SPARK_PARTICLE_COUNT` (6) particles in an upward
    /// semicircle fan (-180° to 0°), mimicking a water-drop crown splash
    /// when a rain droplet's head touches the message-border top edge.
    /// Each particle uses `head_rgb` (palette last-stop, usually white)
    /// + fixed `·` (middle-dot) glyph + 1-cell trail (`max_trail = 1`)
    /// + 350ms lifetime.
    ///
    /// Particles share the existing `quantum_particles` pool with quantum
    /// ripples — zero new allocation. If the pool is full, the touch is
    /// silently dropped (same pattern as `spawn_quantum_ripple`).
    ///
    /// Called from `detect_border_touch` on non-corner border cells only
    /// (corner-skip guard preserves the "no lone bright heads at top
    /// corners" LTS invariant).
    pub(crate) fn spawn_border_spark(&mut self, col: u16, line: u16, head_rgb: (u8, u8, u8)) {
        // PERF-4: --no-effects gate. No-op when effects are disabled.
        if !self.effects_enabled {
            return;
        }
        let cx = col as f32 + 0.5;
        let cy = line as f32 + 0.5;
        let now = Instant::now();
        let mut spawned = 0usize;
        for p in &mut self.quantum_particles {
            if spawned >= BORDER_SPARK_PARTICLE_COUNT {
                break;
            }
            if p.active {
                continue;
            }
            // Upward semicircle fan: [-180°, 0°] (left through up to right).
            // In terminal coords, negative Y = upward. The border is a
            // ceiling, so sparks deflect up + sideways (crown splash).
            let angle = BORDER_SPARK_ANGLE_MIN_RAD
                + self.rand_chance.sample(&mut self.mt)
                    * (BORDER_SPARK_ANGLE_MAX_RAD - BORDER_SPARK_ANGLE_MIN_RAD);
            let speed = BORDER_SPARK_SPEED * (0.9 + self.rand_chance.sample(&mut self.mt) * 0.2);
            p.active = true;
            p.x = cx;
            p.y = cy;
            p.vx = speed * angle.cos();
            p.vy = speed * angle.sin();
            p.birth = now;
            p.ch = '·';
            p.r = head_rgb.0;
            p.g = head_rgb.1;
            p.b = head_rgb.2;
            p.trail_count = 0;
            p.max_trail = BORDER_SPARK_TRAIL_LEN as u8;
            p.lifetime = BORDER_SPARK_LIFETIME_SECS;
            p.sim_age = 0.0;
            spawned += 1;
        }
        self.quantum_active_count = self.quantum_active_count.saturating_add(spawned);
    }
}
