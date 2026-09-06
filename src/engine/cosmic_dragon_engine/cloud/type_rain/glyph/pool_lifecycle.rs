// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Glyph rain style — droplet pool lifecycle methods.
//!
//! Owns the three Glyph-specific `Cloud` methods that were
//! previously in `cloud/spawn.rs`:
//! - `recalc_droplets_per_sec` — per-frame droplet spawn rate
//!   recalculation (terminal-aware speed_mult).
//! - `update_droplet_speeds` — per-droplet velocity update
//!   (terminal-aware speed_mult + parallax layer multiplier).
//! - `ensure_glyph_pool_and_warm_start` — pool (re)allocation +
//!   sparse warm-start seeding for scene-entry (prevents the
//!   "blank black screen" on Glyph scene switch).
//!
//! These are Glyph-family methods (only called when
//! `RainStyle::is_droplet_family()` is true, i.e. the rain style
//! is Glyph). They were extracted from `cloud/spawn.rs` in
//! NIGHT-enhanced-hunt-G to complete the Glyph family
//! consolidation started in NIGHT-enhanced-1-fixup.
//!
//! The other methods in `cloud/spawn.rs` (reset, init_chars,
//! transition_chars, glitch management, quantum ripple, border
//! spark) are general Cloud lifecycle methods shared across all
//! rain styles and stay where they are.

use std::time::Instant;

use rand::distr::Distribution;

use crate::constants::*;
use crate::droplet::Droplet;

impl super::super::super::Cloud {
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

            // NIGHT-hunter-14: pop the slot from the free list instead of
            // direct-indexing `droplets[i]`. The free-list contract (see
            // type_rain/glyph/spawn_logic.rs) is "contains exactly the dead droplet
            // indices"; the previous direct index seeded ALIVE droplets at
            // 0..seed_limit while their indices stayed in the list, so
            // under pool pressure a later spawn could pop an alive index
            // and silently overwrite a live droplet mid-fall — the old
            // column's `col_stat.num_droplets` was never decremented
            // (decrements happen only on the OVERWRITTEN droplet's death,
            // which decrements the NEW column), leaking spawn budget from
            // the old column permanently (until the next reset or scene
            // switch) and thinning its rain density. Popping keeps the
            // invariant exact; `break` covers pool exhaustion.
            let Some(di) = self.droplet_free_list.pop() else {
                break;
            };

            let spec = self.build_droplet_spec(col);
            let end_line = spec.end_line;
            let d = &mut self.droplets[di];
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
}
