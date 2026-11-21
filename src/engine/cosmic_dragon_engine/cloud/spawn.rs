// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Spawn, reset, and column management methods for Cloud.

use std::time::{Duration, Instant};

use rand::{
    SeedableRng,
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::*;
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
        // NIGHT-research-7 part 3: the DNA genesis is one-shot birth
        // choreography, not steady-state throughput — fast-forward
        // the bench to the formed molecule (the sequence would own
        // ~60 percent of a 10 s window at the default scene speed;
        // the Z-6 "critical path only" contract, and the genesis
        // tests pin the choreography frame by frame instead).
        self.dna_helix_rain.fast_forward_genesis();
        // NIGHT-research-8: same contract for the quasar ignition —
        // fast-forward the bench to the burning engine (the birth
        // is pinned by the ignition tests instead).
        self.quasar_rain.fast_forward_ignition();
        // NIGHT-research-9: same contract for the neural genesis —
        // fast-forward the bench to the trained machine (the birth
        // is pinned by the genesis tests instead).
        self.neural_rain.fast_forward_genesis();
        // Z-6: mark benchmark mode — rain_at skips message cosmetics
        // (draw_message + border-cross detection). Owner directive: bench
        // mode measures critical path only (rain + 3 dragons), not
        // cosmetics. NIGHT-perf-2 exception: the --bench-cosmetics
        // harness clears this flag AFTER reset_bench so the overlay
        // path gets its own dedicated measurement; every default bench
        // run still skips it.
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
        } else if matches!(self.rain_style, RainStyle::Flux) {
            self.flux_rain.clear_draw_history();
        } else if matches!(self.rain_style, RainStyle::Lorenz) {
            // NIGHT-research-4: lorenz — same structured-family reset
            // contract on charset switch (mirrors monolith/vortex/flux).
            self.lorenz_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Dragon) {
            // NIGHT-research-5: dragon — structured-family sibling,
            // same draw-history clear on charset switch.
            self.dragon_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Physarum) {
            // NIGHT-research-6: physarum — structured-family sibling,
            // same draw-history clear on charset switch.
            self.physarum_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::BlackHole) {
            // NIGHT-special-1: black hole — structured-family sibling.
            // clear_draw_history also arms a full glyph re-roll so the
            // ball surface never carries glyphs from the old pool.
            self.black_hole_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Aeolian) {
            // NIGHT-special-2: aeolian — structured-family sibling,
            // same draw-history clear on charset switch (string
            // cells re-pick glyphs from the new pool).
            self.aeolian_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::SolarFlare) {
            // NIGHT-special-4: solar_flare — structured-family sibling,
            // same draw-history clear on charset switch (arc cells
            // re-pick glyphs from the new pool).
            self.solar_flare_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::DnaHelix) {
            // NIGHT-research-7: dna_helix — structured-family sibling,
            // same draw-history clear on charset switch (bond cells
            // re-pick glyphs from the new pool; the base-pair ends
            // keep their A/T/G/C identity glyphs).
            self.dna_helix_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Murmuration) {
            // NIGHT-research-7: murmuration — structured-family
            // sibling, same draw-history clear on charset switch
            // (bird glyphs re-pick from the new pool).
            self.murmuration_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Quasar) {
            // NIGHT-research-8: quasar — structured-family
            // sibling, same draw-history clear on charset switch
            // (particle glyphs re-pick from the new pool).
            self.quasar_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Neural) {
            // NIGHT-research-9: neural — structured-family
            // sibling, same draw-history clear on charset switch
            // (cell glyphs re-pick from the new pool).
            self.neural_rain.clear_draw_history();
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
        } else if matches!(self.rain_style, RainStyle::Flux) {
            self.flux_rain.clear_draw_history();
        } else if matches!(self.rain_style, RainStyle::Lorenz) {
            // NIGHT-research-4: lorenz — structured-family sibling,
            // same draw-history clear on charset switch.
            self.lorenz_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Dragon) {
            // NIGHT-research-5: dragon — structured-family sibling,
            // same draw-history clear on palette change.
            self.dragon_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Physarum) {
            // NIGHT-research-6: physarum — structured-family sibling,
            // same draw-history clear on palette change.
            self.physarum_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::BlackHole) {
            // NIGHT-special-1: black hole — structured-family sibling,
            // same draw-history clear on palette change (the re-roll arm
            // covers the charset-switch half of this contract).
            self.black_hole_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Aeolian) {
            // NIGHT-special-2: aeolian — structured-family sibling,
            // same draw-history clear on palette change (string glyphs
            // re-pick under the transition wave).
            self.aeolian_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::SolarFlare) {
            // NIGHT-special-4: solar_flare — structured-family sibling,
            // same draw-history clear on palette change (arc glyphs
            // re-pick under the transition wave).
            self.solar_flare_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::DnaHelix) {
            // NIGHT-research-7: dna_helix — structured-family sibling,
            // same draw-history clear on palette change (bond glyphs
            // re-pick under the transition wave).
            self.dna_helix_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Murmuration) {
            // NIGHT-research-7: murmuration — structured-family
            // sibling, same draw-history clear on palette change
            // (bird glyphs re-pick under the transition wave).
            self.murmuration_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Quasar) {
            // NIGHT-research-8: quasar — structured-family
            // sibling, same draw-history clear on palette change
            // (particle glyphs re-pick under the transition wave).
            self.quasar_rain.clear_draw_history();
            self.reset_phosphor_state();
        } else if matches!(self.rain_style, RainStyle::Neural) {
            // NIGHT-research-9: neural — structured-family
            // sibling, same draw-history clear on palette change
            // (cell glyphs re-pick under the transition wave).
            self.neural_rain.clear_draw_history();
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
        // them. The cells stay at their last-drawn color → a visible "stain"
        // plus slow click effect. Cloud::reset() (Space key) already clears
        // these — this matches that behavior for scene switches.
        self.phosphor_fresh.fill(false);
        self.phosphor_in_active.fill(false);
        // S-master-HUNT-26: a full phosphor state rebuild voids any pending
        // thaw backlog (the cells it tracked no longer exist).
        self.phosphor_thaw_pending.fill(false);
        self.phosphor_thaw_pending_count = 0;
        // Z4b: the shockwave field rides the same one-shot reset surface —
        // resize to the current grid (no-op when the dims did not change)
        // and clear any in-flight wave (a scene switch voids the old
        // ripple's world, exactly like the pre-Z4b slot deactivate).
        self.wave_field.resize(self.cols, self.lines);
        self.wave_field.clear();
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

    /// NIGHT-depth-hardening-2: deterministic reseed ladder. Counter
    /// `counter` (1-based) steps through the golden-ratio constant so
    /// each interval reseeds onto a fresh independent stream while the
    /// full sequence stays reproducible from the single initial seed.
    pub(crate) fn reseed_ladder_seed(counter: u64) -> u64 {
        RNG_INITIAL_SEED ^ counter.wrapping_mul(RESEED_LADDER_STEP)
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
            // NIGHT-depth-hardening-2: the ladder seed used to be drawn
            // from wall-clock nanos, which broke the deterministic-seed
            // contract every RNG_RESEED_INTERVAL_SECS (long sessions
            // diverged per run and bench A/B baselines could not be
            // compared across the boundary). The ladder is a counter
            // stepped through the golden-ratio constant: every interval
            // still lands on a fresh independent stream, and the whole
            // session replays bit-for-bit for a given config.
            self.reseed_counter = self.reseed_counter.wrapping_add(1);
            self.mt = StdRng::seed_from_u64(Self::reseed_ladder_seed(self.reseed_counter));
            self.last_reseed_time = now;
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    //! NIGHT-depth-hardening-3: bounded model checking of the reseed
    //! ladder. The proof covers EVERY u64 counter value: the derivation
    //! cannot panic, and every nonzero rung lands off the launch seed
    //! (a rung that collapsed back onto it would restart the picture
    //! every RNG_RESEED_INTERVAL_SECS instead of showing a fresh
    //! arrangement).
    use super::*;

    #[kani::proof]
    fn reseed_ladder_is_total_and_fresh_per_rung() {
        let counter: u64 = kani::any();
        kani::assume(counter != 0);
        // The ladder uses only wrapping arithmetic, so this call is
        // panic-free for every input; the meaningful property is the
        // freshness invariant below.
        let seed = Cloud::reseed_ladder_seed(counter);
        assert!(seed != RNG_INITIAL_SEED);
    }
}
