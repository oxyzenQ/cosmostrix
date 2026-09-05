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
use rand::distr::{Distribution, Uniform};

#[allow(unused_imports)]
use crate::constants::*;
#[allow(unused_imports)]
use crate::droplet::Droplet;
#[allow(unused_imports)]
use crate::rain_style::RainStyle;

#[allow(unused_imports)]
use super::ecosystem::{RendererMemory, StorytellingState};
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

        // Task-18/19 + NIGHT-research-4: structured styles (Monolith,
        // Vortex, Flux, Lorenz) keep the droplet pool empty; the
        // droplet-family style (Glyph) allocates it. (Ripple was
        // structured-but-droplet-family in the old design — task-19
        // replaced it with fully-structured Flux, and NIGHT-research-4
        // added Lorenz, which shares the Vortex contract.)
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
        if !self.rain_style.is_droplet_family() {
            // Structured styles carry drawn-cell diff history that must be
            // rebuilt after a hard reset.
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
}
