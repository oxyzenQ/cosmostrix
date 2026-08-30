// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD metrics update — extracted from `hud/mod.rs` to keep that file
//! under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `HudState::update_metrics()` — the 1 Hz metric recompute that
//! refreshes all HUD text fields (fps, target_fps, rss, cpu%, p99, dirty
//! cells, vmode, mode, scene, color, density, speed, ehs, dsty, charset,
//! cid, screensize, build) + recomputes the chroma gradient + resizes
//! the cached_lines buffer to fit the new content width.
//!
//! Implemented as a separate `impl HudState` block (Rust allows
//! multiple impl blocks across files for the same type).

use std::time::Instant;

use super::{
    compute_chroma_gradient_22, format_rss_kb, FrameMode, HudState, HUD_MAX_WIDTH,
    HUD_METRIC_INTERVAL, HUD_MIN_WIDTH,
};

impl HudState {
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
        let colors = compute_chroma_gradient_22(palette_colors);

        // Session uptime: compound time format.
        // < 1h:  MM:SS    e.g. 59:03
        // < 1d:  Xh:MM    e.g. 1h:03
        // >= 1d: Xd:YYh   e.g. 2d:03h
        let uptime_secs = self.session_start.elapsed().as_secs();
        let uptime_str = if uptime_secs < 3600 {
            format!("{:02}:{:02}", uptime_secs / 60, uptime_secs % 60)
        } else if uptime_secs < 86_400 {
            format!("{}h:{:02}", uptime_secs / 3600, (uptime_secs % 3600) / 60)
        } else {
            format!(
                "{}d:{:02}h",
                uptime_secs / 86_400,
                (uptime_secs % 86_400) / 3600
            )
        };

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
        let fps_str = if fps >= 10_000.0 {
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
        // v50 (2026-08-17) HUD expansion — populate the 7 new owner-mandated
        // metrics at rows 6-12. All values come from HudState fields that
        // are written by the corresponding setters (called by event_loop).
        // The text is rebuilt here at the 1 Hz tick so number flicker is
        // avoided (matches the fps/p99/max/rss cadence). Color refresh is
        // handled separately by `refresh_colors` every frame.
        //
        // ehs (Endurance Health Score): 0-100 integer. Shows long-endurance
        // process stability. 100 = perfectly stable, <50 = degraded.
        let ehs_val = self.endurance_health_score.round() as i32;
        self.cached_lines[6] = (colors[6], format!(" ehs: {ehs_val}"));
        // prs (Effective Pressure): 0.00-1.00, 2 decimals. Drives spawn rate
        // + sim factor + self-healer. 0.0 = no pressure, 1.0 = max throttle.
        let prs_clamped = self.effective_pressure.clamp(0.0, 1.0);
        self.cached_lines[7] = (colors[7], format!(" prs: {prs_clamped:.2}"));
        // sped (chars-per-sec speed): 1 decimal. User adjusts via ↑/↓.
        let sped_val = self.chars_per_sec;
        self.cached_lines[8] = (colors[8], format!(" sped: {sped_val:.1}"));
        // dsty (droplet density multiplier): 2 decimals. User adjusts via
        // [/]. Owner explicitly mandated `dsty` label (NOT `den`).
        //
        // v50.0.0-beta.6 Option D: when power-dragon is ON, dsty is DYNAMIC
        // — it reflects the effective density after power-dragon throttle.
        // The throttle reduces spawn scale based on CPU pressure (via
        // compute_spawn_scale, the same function used in rain_at()). When
        // power-dragon is OFF, dsty is STATIC (shows the user's configured
        // density, no throttle applied).
        //
        // CLI `--density` is the ceiling: the throttle only reduces below
        // the user's value (scale ≤ 1.0), never above it. So `--density 1.0`
        // with max pressure shows `dsty: 0.25` (1.0 * 0.25 floor), not 1.0.
        let dsty_val = if self.power_dragon_on {
            let scale = crate::central_control_rains::compute_spawn_scale(
                self.effective_pressure,
                self.aggressive_throttle,
            );
            self.droplet_density * scale
        } else {
            self.droplet_density
        };
        self.cached_lines[9] = (colors[9], format!(" dsty: {dsty_val:.2}"));
        // scn (scene name): string, no format. User cycles via `x`.
        self.cached_lines[10] = (colors[10], format!(" scn: {}", self.scene_name));
        // chr (charset preset): string, no format. User cycles via `s`/`S`.
        self.cached_lines[11] = (colors[11], format!(" chr: {}", self.charset_preset));
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
        self.cached_lines[12] = (colors[12], clr_line);
        // v50 (2026-08-17) HUD expansion reorder: up/screensize/cid moved
        // from rows 6/7/8 to rows 13/14/15 per owner's Option S mandate.
        // cid stays static (set in `new()`); only up + screensize are
        // rewritten here on the 1 Hz tick (uptime changes every second,
        // screensize changes only on terminal resize).
        self.cached_lines[13] = (colors[13], format!(" up: {uptime_str}"));
        let (sw, sh, is_fixed) = self.screen_size;
        let mode = if is_fixed { "fix" } else { "auto" };
        self.cached_lines[14] = (colors[14], format!(" {sw}x{sh} {mode}"));
        // v50.0.0-beta.6: dragon on/off indicators at rows 15-16. These
        // reflect the LIVE runtime state (set by set_power_dragon /
        // set_crystal_dragon, called every frame from event_loop with
        // cfg.power_dragon / cfg.crystal_dragon). When the user
        // live-reloads the config, the HUD shows the new state on the
        // next 1 Hz tick. Renders as " prdr: on" / " prdr: off" and
        // " crdr: on" / " crdr: off" (lowercase to match the existing
        // HUD label convention: fps/tgt/max/p99/cpu/rss/ehs/prs/etc).
        let prdr_val = if self.power_dragon_on { "on" } else { "off" };
        self.cached_lines[15] = (colors[15], format!(" prdr: {prdr_val}"));
        let crdr_val = if self.crystal_dragon_on { "on" } else { "off" };
        self.cached_lines[16] = (colors[16], format!(" crdr: {crdr_val}"));

        // v50.0.0-beta.7 Option C expansion — 4 new owner-mandated metrics.
        // ambt: ambient on/off (auto-detected from config.toml ambient.HH-MM entries).
        let ambt_val = if self.ambient_on { "on" } else { "off" };
        self.cached_lines[17] = (colors[17], format!(" ambt: {ambt_val}"));
        // glth: glitch level (none/subtle/default/intense).
        let glth_val = match self.glitch_level {
            crate::config::GlitchLevel::None => "none",
            crate::config::GlitchLevel::Subtle => "subtle",
            crate::config::GlitchLevel::Default => "default",
            crate::config::GlitchLevel::Intense => "intense",
        };
        self.cached_lines[18] = (colors[18], format!(" glth: {glth_val}"));
        // ctun: color tuning default/custom (custom when any field ≠ 1.0).
        let ctun_val = if self.color_tune_custom {
            "custom"
        } else {
            "default"
        };
        self.cached_lines[19] = (colors[19], format!(" ctun: {ctun_val}"));
        // mnst: monolith size (small/normal/large) or "unknown" for
        // non-monolith scenes.
        let mnst_val = match self.monolith_size {
            Some(crate::runtime::MonolithSize::Small) => "small",
            Some(crate::runtime::MonolithSize::Normal) => "normal",
            Some(crate::runtime::MonolithSize::Large) => "large",
            None => "unknown",
        };
        self.cached_lines[20] = (colors[20], format!(" mnst: {mnst_val}"));

        // cid (row 21) is static — set once in new(), never rewritten
        // here. Only its color is refreshed by refresh_colors every frame.

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
