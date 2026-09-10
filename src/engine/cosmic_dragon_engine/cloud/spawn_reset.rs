// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cloud reset with bounds — extracted from `cloud/spawn.rs` to keep
//! that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `Cloud::reset_with_bounds()` — the core initialization that
//! sets up columns, droplets, phosphor state, glitch maps, and column
//! spawn/speed tables. Called by reset() and reset_bench().
//!
//! Implemented as a separate `impl Cloud` block.

use std::time::{Duration, Instant};

#[allow(unused_imports)]
use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
    SeedableRng,
};

#[allow(unused_imports)]
use crate::constants::*;
#[allow(unused_imports)]
use crate::droplet::Droplet;
#[allow(unused_imports)]
use crate::rain_style::RainStyle;

#[allow(unused_imports)]
use super::ecosystem::{ColorEcosystem, EntropyDrift, RendererMemory, StorytellingState};
#[allow(unused_imports)]
use super::state::ColumnStatus;

impl super::Cloud {
    pub(super) fn reset_with_bounds(
        &mut self,
        cols: u16,
        lines: u16,
        max_cols: u16,
        max_lines: u16,
    ) {
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

        // Task-18/19 + NIGHT-research-4/5/6 + NIGHT-special-1/2/4
        // + NIGHT-research-7/8/9: structured styles (Monolith, Vortex,
        // Flux, Lorenz, Dragon, Physarum, BlackHole, Aeolian,
        // SolarFlare, DnaHelix, Murmuration, Quasar, Neural) keep the
        // droplet pool empty; the droplet-family style (Glyph)
        // allocates it. (Ripple was structured-but-droplet-family
        // in the old design — task-19 replaced it with
        // fully-structured Flux; NIGHT-research-4/5/6 added Lorenz,
        // Dragon and Physarum, which all share the Vortex
        // contract; NIGHT-special-1 added the black hole ball, which
        // shares it too; NIGHT-special-2 added the aeolian weave;
        // NIGHT-special-4 added the corona arcade; NIGHT-research-7
        // added the DNA helix and the murmuration flock;
        // NIGHT-research-8 added the quasar engine; NIGHT-research-9
        // added the neural network.)
        if self.rain_style.is_droplet_family() {
            let pool_size = (DROPLET_COUNT_FACTOR * self.cols as f32).round() as usize;
            self.droplets.clear();
            self.droplets.resize_with(pool_size, Droplet::new);
        } else {
            self.droplets.clear();
        }
        // All structured systems stay viewport-ready (style switch is a
        // pure field flip away); each takes its full reset here.
        self.monolith_rain.reset(self.cols);
        self.vortex_rain.reset(self.cols);
        self.flux_rain.reset(self.cols, self.lines);
        self.lorenz_rain.reset(self.cols);
        self.dragon_rain.reset(self.cols);
        self.physarum_rain.reset(self.cols);
        self.black_hole_rain.reset(self.cols, self.lines);
        self.aeolian_rain.reset(self.cols, self.lines);
        self.solar_flare_rain.reset(self.cols, self.lines);
        self.dna_helix_rain.reset(self.cols, self.lines);
        self.murmuration_rain.reset(self.cols, self.lines);
        self.quasar_rain.reset(self.cols, self.lines);
        self.neural_rain.reset(self.cols, self.lines);

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
        self.phosphor_thaw_pending.clear();
        self.phosphor_thaw_pending.resize(total, false);
        self.phosphor_thaw_pending_count = 0;
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
        // NIGHT-hunter-15: a hard reset is a semantic event for EVERY
        // style, not just the structured family. The 'r' shortkey calls
        // reset() + force_draw_everything(); for Glyph (droplet family)
        // the force flag used to be consumed by the HUNT-25 resync path
        // (Frame::force_repaint — re-emit current content, NO clear),
        // so the pre-restart glyphs stayed on screen while the droplet
        // pool and phosphor arrays were wiped: no droplet owned those
        // cells and the phosphor decay system no longer tracked them,
        // leaving the owner-reported permanent "bekas rain" residue
        // (restart appeared stuck, then rain resumed over the old
        // frame). Arming semantic_invalidate routes the first
        // post-restart frame through invalidate_semantic (full logical
        // clear + gen bump + terminal LastFrame resync) — the same
        // contract scene switches already honor
        // (transition_rain_style/apply_scene_runtime arm this for every
        // style). All other reset() callers rebuild the Frame fresh
        // (resize, live-reload, intro re-read, startup), where the
        // extra invalidation on an already-blank frame is a no-op.
        self.semantic_invalidate = true;
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

    /// NIGHT-lts-3: the full fresh-start restart (the 'r' shortkey).
    ///
    /// The owner contract: a restart must behave exactly like a fresh
    /// startup — a real start from zero. The 'r' handler previously
    /// ran only [`Self::reset`], whose semantics are the resize
    /// contract: it rebuilds geometry and empties the pools but
    /// deliberately preserves
    /// (a) each choreographed family's birth state — the black hole
    ///     popped in already formed, the DNA molecule already stood,
    ///     the quasar engine already burned, the neural machine
    ///     already trained, while a fresh construction is unborn and
    ///     plays the birth sequence (the owner's reported repro),
    /// (b) the deterministic RNG streams mid-course — every launch
    ///     seeds `StdRng` from `RNG_INITIAL_SEED`, so a relaunch replays
    ///     the same glyph rolls and spawn draws a restart must match,
    /// (c) the ecosystem/drift accumulators and the time anchor —
    ///     Phase D Bugs #8/#9 keep those across resize and live-reload
    ///     (an interrupt must not snap the visual climate); a restart
    ///     is a relaunch, not an interrupt, so from zero means from
    ///     the unevolved state,
    /// (d) any pause/resume easing in flight.
    ///
    /// This method layers the missing fresh-start state on top of the
    /// full reset and re-arms the birth choreography for the current
    /// style, so 'r' equals a same-config relaunch. The plain
    /// structured families need nothing beyond the full reset (pools
    /// vacant — identical to both startup and scene entry), and the
    /// glyph family matches a fresh launch by design: the empty pool
    /// fills through natural spawn, and the warm-start ramp belongs
    /// to style transitions, not launches.
    pub fn restart_from_zero(&mut self, cols: u16, lines: u16) {
        // Re-seed FIRST: reset() samples the glitch clock from the
        // RNG, and a fresh launch consumes the stream from position
        // zero — the re-seed must precede the reset's sample so the
        // restart draws the same values a startup drew.
        self.mt = StdRng::seed_from_u64(RNG_INITIAL_SEED);

        // The full reset: geometry, pools, maps, LUTs, message,
        // semantic invalidation, force redraw, subsystem clocks.
        self.reset(cols, lines);

        // Fresh anchor and drift accumulators (startup constructs
        // these at Cloud::new; resize/live-reload keep them by the
        // Phase D contract — a restart does not).
        let now = Instant::now();
        self.start_anchor = now;
        self.color_ecosystem = ColorEcosystem::new(now);
        self.entropy_drift = EntropyDrift::new(now);

        // The event scheduler's dedicated RNG replays the startup
        // event sequence (reset() only drops the active events).
        self.event_manager.restart_rng();

        // Pause/resume family: startup runs unpaused at full rate. A
        // restart during decel/resume easing must not carry the
        // blend — it would throttle the reborn rain for no reason.
        self.pause = false;
        self.pause_start = None;
        self.pause_time = None;
        self.resume_start = None;
        self.resume_blend = 1.0;
        self.resume_blend_start = 0.0;

        // The birth choreography (the scene-entry contract): re-arm
        // the intro sequences for the choreographed families exactly
        // as a scene entry would.
        match self.rain_style {
            RainStyle::BlackHole => self.black_hole_rain.begin_formation(),
            RainStyle::DnaHelix => self.dna_helix_rain.begin_genesis(),
            RainStyle::Quasar => self.quasar_rain.begin_ignition(),
            RainStyle::Neural => self.neural_rain.begin_genesis(),
            // The plain structured families and Glyph: the full reset
            // above already equals the startup state.
            _ => {}
        }
    }
}
