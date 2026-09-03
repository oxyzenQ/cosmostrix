// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD metrics update — extracted from `hud/mod.rs` to keep that file
//! under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `HudState::update_metrics()` — the 1 Hz metric recompute that
//! refreshes all HUD text fields (fps, tgt, max, p99, cpu, rss, ehs,
//! prs, scn, chr, clr, sped, dsty, prdr, crdr, ambt, glth, ctun,
//! mnst, dcel, tcel, up, screensize — cid is static, set once in
//! `new()` at row 21) + recomputes the chroma gradient + resizes the
//! cached_lines buffer to fit the new content width.
//!
//! v80.0.0-beta.1 row order (owner mandate 2026-08-31, "reorder/tidying HUD
//! metrics") + Z-master-1X round 5 (dcel/tcel added): identity/scene
//! lines moved up next to the health core (scn/chr/clr at rows 8-10,
//! before the user-adjustable sped/dsty at 11-12), dragon + tuning
//! state follows (prdr/crdr/ambt/glth/ctun/mnst at rows 13-18), cell
//! efficiency (dcel/tcel at rows 19-20), and the session footer closes
//! the dashboard (cid 21, up 22, screensize 23 — the build identity
//! keeps a prominent position and the terminal size stays the visual
//! anchor at the bottom).
//!
//! Also owns `HudState::set_metrics_paused()` — the v80.0.0-beta.1 pause-freeze
//! contract (owner bug fix 2026-08-30): while the rain is paused (or
//! decelerating toward pause), every running metric STOPS — uptime,
//! fps, max, p99, cpu, rss, ehs, prs all hold their last active value,
//! and sampling is suppressed so paused 4 Hz input-poll ticks cannot
//! contaminate the windows. On resume everything continues exactly
//! where it froze (uptime excludes paused time via the accumulated
//! `paused_total` + the open `pause_started_at` segment; the CPU
//! baseline stays warm so the first post-resume delta stays precise).
//! The `tgt:` line is the ONE live element during pause — its ` paused`
//! suffix must keep rendering so the user sees WHY the dashboard froze.
//!
//! Implemented as a separate `impl HudState` block (Rust allows
//! multiple impl blocks across files for the same type).

use std::time::Instant;

use super::{
    compute_chroma_gradient_24, format_rss_kb, FrameMode, HudState, HUD_MAX_WIDTH,
    HUD_METRIC_INTERVAL, HUD_MIN_WIDTH,
};

impl HudState {
    /// Announce the pause state for metric freezing (v80.0.0-beta.1, owner bug fix
    /// 2026-08-30). Called every frame by the event loop with
    /// `cloud.is_paused_or_decelerating()` — the SAME predicate the
    /// keybinding pause guard and the mouse click-wave guard use, so the
    /// freeze window exactly matches the interaction-freeze window.
    ///
    /// Transitions:
    /// - `false → true`: opens a pause segment (`pause_started_at = now`).
    /// - `true → false`: closes it into `paused_total`, so uptime math
    ///   excludes the whole paused span with sub-second precision.
    ///
    /// The freeze itself is enforced inside the samplers (`push_frame_time`,
    /// `maybe_sample_rss`, `maybe_sample_cpu`, `set_effective_pressure`,
    /// `set_endurance_health_score`) via the `metrics_paused` field.
    pub(crate) fn set_metrics_paused(&mut self, paused: bool) {
        if paused == self.metrics_paused {
            return;
        }
        if paused {
            self.pause_started_at = Some(Instant::now());
        } else if let Some(start) = self.pause_started_at.take() {
            self.paused_total += start.elapsed();
        }
        self.metrics_paused = paused;
    }

    #[inline]
    pub(crate) fn update_metrics(&mut self, palette_colors: &[crossterm::style::Color]) {
        if !self.visible {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_metric_update) < HUD_METRIC_INTERVAL {
            return;
        }
        self.last_metric_update = now;

        // Recompute p99 from the ring buffer (stack-allocated sort,
        // ~300ns, called once per second).
        self.p99_ms = self.frame_times.p99_ms();

        let avg_ms = self.frame_times.rolling_avg_ms();
        let fps = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };
        let rss_str = self
            .last_rss_kb
            .map(format_rss_kb)
            .unwrap_or_else(|| "—".to_string());

        // Refresh colors alongside the text reformat — both stay in sync
        // on the 1 Hz tick. `refresh_colors` is ALSO called every frame
        // from the event loop (between metric ticks), so a runtime palette
        // change appears on the next frame instead of up to 1 second later.
        //
        // HD-01 (HUD chroma dragon integration): 18-stop sweep — each row
        // gets a distinct palette stop. Index math: `palette_colors`
        // sampled at `(i / 17.0 * (n-1)).round()` for i ∈ [0..18].
        let colors = compute_chroma_gradient_24(palette_colors);

        // Session uptime: tiered compound format (v80.0.0-alpha.1
        // S-master-HUNT-5, owner task 2026-09-03):
        //   < 1h:    MM:SS           e.g. 59:03
        //   < 1d:    Xh:MMm          e.g. 8h:01m   (owner reference)
        //   < 30d:   Xd:HHh:MMm      e.g. 1d:07h:22m
        //   < 365d:  Xmo:DDd:HHh:MMm e.g. 1mo:01d:22h:10m
        //   >= 365d: Xy:MOmo:DDd:HHh:MMm  (degrades past 19 chars)
        // Minutes survive past the day crossing; calendar-fixed units
        // (1mo = 30d, 1y = 365d) keep every boundary deterministic —
        // see `clock::format_uptime_tiered` for the full design contract
        // (zero-padding rationale, HUD width budget proof, degradation).
        //
        // v80.0.0-beta.1 pause freeze: paused time is EXCLUDED — the open segment
        // grows at the same rate as the elapsed clock while paused, so
        // the subtraction pins `up:` at the value it had when 'p' was
        // pressed; on resume it continues from exactly there.
        let mut uptime_secs = self.session_start.elapsed();
        uptime_secs = uptime_secs.saturating_sub(self.paused_total);
        if let Some(start) = self.pause_started_at {
            uptime_secs = uptime_secs.saturating_sub(start.elapsed());
        }
        let uptime_str = crate::clock::format_uptime_tiered(uptime_secs.as_secs());

        // v16: Dynamic-width HUD. Lines are formatted WITHOUT fixed-width
        // padding — the HUD width grows/shrinks to fit the longest line.
        // This prevents truncation when FPS is high (e.g. "45132" needs
        // more space than "60") and avoids wasted space when values are short.
        //
        // Format: " label: value" (no trailing padding — pad is added
        // dynamically in write_to_frame based on current_width).
        //
        // Color assignment uses the rain-aesthetic gradient (dim at top →
        // head at bottom). See `refresh_colors` docs for the rationale.
        let fps_str = if fps >= 1_000.0 {
            crate::humanize::humanize_f64(fps)
        } else if fps >= 100.0 {
            format!("{fps:.0}")
        } else {
            format!("{fps:.1}")
        };
        self.cached_lines[0] = (colors[0], format!(" fps: {fps_str}"));
        // v30 (2026-08-05): tgt line shows the user-configured --fps cap
        // alongside the current frame pacing mode. This disambiguates the
        // common confusion where `--fps 30` produces `fps: 11000` in the
        // HUD (because `fps:` is render-work throughput = 1000/work_ms,
        // not the loop's frame-period cap). The mode suffix tells the user
        // whether the cap is actually in effect:
        //   ` tgt: 30`        — active, loop targeting 30 FPS
        //   ` tgt: 30 idle`   — adaptive idle throttle engaged (effective ~15)
        //   ` tgt: 30 paused` — user pressed `p`, loop ticking at 4 Hz
        // Format chosen to be compact (≤14 chars) so HUD width stays ≤22.
        let tgt_str = if self.target_fps >= 100.0 {
            format!("{:.0}", self.target_fps)
        } else {
            format!("{:.1}", self.target_fps)
        };
        // v50.0.0-beta.6 LTS audit: mode_suffix as &'static str avoids
        // a heap allocation every 1 Hz tick (was String::new() / .to_string()).
        // The format! macro accepts &str with no overhead.
        let mode_suffix: &'static str = match self.frame_mode {
            FrameMode::Active => "",
            FrameMode::Idle => " idle",
            FrameMode::Paused => " paused",
        };
        self.cached_lines[1] = (colors[1], format!(" tgt: {tgt_str}{mode_suffix}"));
        // v50 (2026-08-15): intra-pair swap — max before p99 (extreme
        // first), cpu before rss (active first). Matches htop/btop/mangoHUD
        // convention. Brightness gradient stop assignments (colors[i]) are
        // unchanged — only the content at each index moved.
        self.cached_lines[2] = (colors[2], format!(" max: {:.3}ms", self.max_ms));
        self.cached_lines[3] = (colors[3], format!(" p99: {:.3}ms", self.p99_ms));
        // CPU% line: process CPU usage with 2-decimal precision.
        // Format: ` cpu: 0.45%` (single-threaded typical: 0-5%) or
        // ` cpu: —` when the sampler is unsupported (non-unix) or
        // waiting for the first delta to complete (first ~1s of process
        // lifetime only — baseline is kept warm while HUD is off, so
        // toggle-on shows instant percent).
        // The em dash is U+2014 (3 bytes UTF-8) but renders as 1 cell —
        // matches the existing `rss: —` fallback convention.
        //
        // Color: uses `mid` (palette_colors[n/2]) brightened. This is
        // intentional: cpu% is a metric the user actively watches when
        // investigating performance, so it deserves a bright color. The
        // `dim` color is reserved for uptime/screensize which are
        // informational only. Brightening guarantees readability on dark
        // rain palettes.
        let cpu_str = match self.cpu_percent {
            Some(pct) => format!("{pct:.2}%"),
            None => "—".to_string(),
        };
        self.cached_lines[4] = (colors[4], format!(" cpu: {cpu_str}"));
        self.cached_lines[5] = (colors[5], format!(" rss: {rss_str}"));
        // v50 (2026-08-17) HUD expansion → v80.0.0-beta.1 reorder (owner mandate
        // 2026-08-31) → Z-master-1X round 5 (dcel/tcel added): populate
        // the metric rows in the current order — after the health pair
        // (ehs/prs, rows 6-7) come the identity lines (scn/chr/clr,
        // rows 8-10), then the user-adjustable controls (sped/dsty,
        // rows 11-12), then the dragon + tuning state (prdr/crdr/ambt/
        // glth/ctun/mnst, rows 13-18), then cell efficiency (dcel/tcel,
        // rows 19-20), then the session footer (cid 21 static, up 22,
        // screensize 23). All values come from HudState fields that
        // are written by the corresponding setters (called by
        // event_loop). The text is rebuilt here at the 1 Hz tick so
        // number flicker is avoided (matches the fps/p99/max/rss
        // cadence). Color refresh is handled separately by
        // `refresh_colors` every frame.
        //
        // ehs (Endurance Health Score): 0-100 integer. Shows long-endurance
        // process stability. 100 = perfectly stable, <50 = degraded.
        let ehs_val = self.endurance_health_score.round() as i32;
        self.cached_lines[6] = (colors[6], format!(" ehs: {ehs_val}"));
        // prs (Effective Pressure): 0.00-1.00, 2 decimals. Drives spawn rate
        // + sim factor + self-healer. 0.0 = no pressure, 1.0 = max throttle.
        let prs_clamped = self.effective_pressure.clamp(0.0, 1.0);
        self.cached_lines[7] = (colors[7], format!(" prs: {prs_clamped:.2}"));
        // scn (scene name): string, no format. User cycles via `x`.
        // v80.0.0-beta.1: moved up to row 8 — identity lines sit directly under the
        // health core (owner reorder mandate).
        self.cached_lines[8] = (colors[8], format!(" scn: {}", self.scene_name));
        // chr (charset preset): string, no format. User cycles via `s`/`S`.
        self.cached_lines[9] = (colors[9], format!(" chr: {}", self.charset_preset));
        // clr (color scheme): show custom palette name when active, otherwise
        // the builtin ColorScheme Debug format. This fixes the bug where
        // --colors-custom cyberpunk_2077 showed "clr: EnergyZen" (the
        // underlying builtin scheme) instead of the custom palette name.
        //
        // v50.0.0-beta.6 LTS audit: eliminated the intermediate clr_label
        // String clone. Previously: name.clone() -> format!(" clr: {clr_label}")
        // = 2 allocations per tick. Now: single format!() call that borrows
        // the name directly = 1 allocation per tick.
        let clr_line = match &self.custom_palette_name {
            Some(name) => format!(" clr: {name}"),
            None => format!(" clr: {:?}", self.color_scheme),
        };
        self.cached_lines[10] = (colors[10], clr_line);
        // sped (chars-per-sec speed): 1 decimal. User adjusts via ↑/↓.
        // v80.0.0-beta.1: moved down to row 11 — user-adjustable controls now follow
        // the identity lines (owner reorder mandate).
        let sped_val = self.chars_per_sec;
        self.cached_lines[11] = (colors[11], format!(" sped: {sped_val:.1}"));
        // dsty (droplet density multiplier): 2 decimals. User adjusts via
        // [/]. Owner explicitly mandated `dsty` label (NOT `den`).
        //
        // v50.0.0-beta.6 Option D + v80.0.0-beta.1 masterclass: when power-dragon is
        // ON, dsty is DYNAMIC — it reflects the effective density after the
        // power-dragon throttle. The v80.0.0-beta.1 banded throttle (via
        // compute_spawn_scale, the same function used in rain_at()) uses the
        // user's configured density as the CEILING: dead zone below 5%
        // pressure (dsty = configured density), low band 0.84-0.70, medium
        // 0.70-0.50, high (rare) 0.50-0.10 — pressure only ever reduces
        // below the configured value, never above it. When power-dragon is
        // OFF, dsty is STATIC (shows the user's configured density, no
        // throttle applied — the pressure feed itself is gated to 0.0 by
        // update_hud_state, so prs: and dsty: stay consistent).
        let dsty_val = if self.power_dragon_on {
            let scale = crate::central_control_rains::compute_spawn_scale(
                self.effective_pressure,
                self.aggressive_throttle,
                self.droplet_density,
            );
            self.droplet_density * scale
        } else {
            self.droplet_density
        };
        self.cached_lines[12] = (colors[12], format!(" dsty: {dsty_val:.2}"));
        // v50.0.0-beta.6: dragon on/off indicators. These
        // reflect the LIVE runtime state (set by set_power_dragon /
        // set_crystal_dragon, called every frame from event_loop with
        // cfg.power_dragon / cfg.crystal_dragon). When the user
        // live-reloads the config, the HUD shows the new state on the
        // next 1 Hz tick. Renders as " prdr: on" / " prdr: off" and
        // " crdr: on" / " crdr: off" (lowercase to match the existing
        // HUD label convention: fps/tgt/max/p99/cpu/rss/ehs/prs/etc).
        // v80.0.0-beta.1 reorder: rows 13-14 (were 15-16).
        let prdr_val = if self.power_dragon_on { "on" } else { "off" };
        self.cached_lines[13] = (colors[13], format!(" prdr: {prdr_val}"));
        let crdr_val = if self.crystal_dragon_on { "on" } else { "off" };
        self.cached_lines[14] = (colors[14], format!(" crdr: {crdr_val}"));

        // v50.0.0-beta.7 Option C expansion — 4 new owner-mandated metrics.
        // v80.0.0-beta.1 reorder: rows 15-18 (were 17-20).
        // ambt: ambient on/off (auto-detected from config.toml ambient.HH-MM entries).
        let ambt_val = if self.ambient_on { "on" } else { "off" };
        self.cached_lines[15] = (colors[15], format!(" ambt: {ambt_val}"));
        // glth: glitch level (none/subtle/default/intense).
        let glth_val = match self.glitch_level {
            crate::config::GlitchLevel::None => "none",
            crate::config::GlitchLevel::Subtle => "subtle",
            crate::config::GlitchLevel::Default => "default",
            crate::config::GlitchLevel::Intense => "intense",
        };
        self.cached_lines[16] = (colors[16], format!(" glth: {glth_val}"));
        // ctun: color tuning default/custom (custom when any field ≠ 1.0).
        let ctun_val = if self.color_tune_custom {
            "custom"
        } else {
            "default"
        };
        self.cached_lines[17] = (colors[17], format!(" ctun: {ctun_val}"));
        // mnst: monolith size (small/normal/large) or "unknown" for
        // non-monolith scenes.
        let mnst_val = match self.monolith_size {
            Some(crate::runtime::MonolithSize::Small) => "small",
            Some(crate::runtime::MonolithSize::Normal) => "normal",
            Some(crate::runtime::MonolithSize::Large) => "large",
            None => "unknown",
        };
        self.cached_lines[18] = (colors[18], format!(" mnst: {mnst_val}"));

        // ── Cell efficiency (rows 19-20) — Z-master-1X round 5 ──
        // dcel: dirty cell count + ratio %. Format: " dcel: 1.2K/10.2%"
        // where the count is humanized (1.2K for 1200, raw for <1000) and
        // the percentage is the dirty/total ratio. Owner mandate: combine
        // count + percentage so the user sees BOTH the absolute number
        // AND the ratio. Count uses the same humanize() helper as tcel
        // for consistency (e.g. 120 → "120", 1200 → "1.2K", 12000 → "12K").
        let avg_dirty = self.dirty_cell_tracker.rolling_avg_dirty();
        let latest_total = self.dirty_cell_tracker.latest_total();
        let dcel_pct = if latest_total > 0 {
            (avg_dirty / latest_total as f64) * 100.0
        } else {
            0.0
        };
        let dcel_count = avg_dirty.round() as u64;
        let dcel_count_str = crate::humanize::humanize(dcel_count);
        self.cached_lines[19] = (
            colors[19],
            format!(" dcel: {dcel_count_str}/{dcel_pct:.1}%"),
        );
        // tcel: total cells in the screen (width × height). Driven by
        // terminal size — stable between resizes. Rendered with the
        // same humanize helper as fps for compactness (e.g. 2.8K).
        let tcel_val = crate::humanize::humanize(latest_total);
        self.cached_lines[20] = (colors[20], format!(" tcel: {tcel_val}"));

        // cid (row 21) is static — set once in new(), never rewritten
        // here. Only its color is refreshed by refresh_colors every frame.
        // Z-master-1X round 5: cid moved from row 19 to row 21 to make
        // room for dcel/tcel above it (owner mandate).

        // Session footer (Z-master-1X round 5: up moved from row 20 to 22,
        // screensize from 21 to 23 — terminal size stays the visual anchor
        // at the very bottom of the dashboard).
        self.cached_lines[22] = (colors[22], format!(" up: {uptime_str}"));
        let (sw, sh, is_fixed) = self.screen_size;
        let mode = if is_fixed { "fix" } else { "auto" };
        self.cached_lines[23] = (colors[23], format!(" {sw}x{sh} {mode}"));

        // Compute dynamic width: find the longest line, clamp to [min, max].
        let max_len = self
            .cached_lines
            .iter()
            .map(|(_, s)| s.chars().count())
            .max()
            .unwrap_or(HUD_MIN_WIDTH as usize) as u16;
        self.current_width = max_len.clamp(HUD_MIN_WIDTH, HUD_MAX_WIDTH);
    }
}
