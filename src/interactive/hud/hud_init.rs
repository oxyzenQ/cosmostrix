// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD initialization + frame writing — extracted from
//! `hud/mod.rs` to keep that file under the 800-LOC cap.

use std::time::{Duration, Instant};

use crate::runtime::ColorScheme;
use crossterm::style::Color;

use super::{FrameMode, HUD_CPU_INTERVAL, HUD_METRIC_INTERVAL, HUD_MIN_WIDTH, HUD_RSS_INTERVAL};
use crate::interactive::activity::FrameTimeTracker;

impl super::HudState {
    pub(crate) fn new() -> Self {
        // Compile-time git short SHA injected by build.rs via the
        // `COSMOSTRIX_GIT_SHA` env var (see `git_short_sha()` in
        // build.rs — runs `git rev-parse --short=7 HEAD`). Falls back
        // to the literal "unknown" when the build had no `.git` dir
        // (e.g. a tarball release build) so the HUD never panics on a
        // missing env. The value is a `&'static str`, so embedding it
        // in the cached_lines String is a one-time alloc per session —
        // zero per-frame cost.
        let commit_sha = option_env!("COSMOSTRIX_GIT_SHA").unwrap_or("unknown");
        Self {
            visible: false,
            session_start: Instant::now(),
            // v51 pause-freeze: metrics stop while paused (owner bug fix
            // 2026-08-30). See set_metrics_paused() in metrics.rs.
            metrics_paused: false,
            pause_started_at: None,
            paused_total: Duration::ZERO,
            frame_times: FrameTimeTracker::new(),
            last_metric_update: Instant::now()
                .checked_sub(HUD_METRIC_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_rss_sample: Instant::now()
                .checked_sub(HUD_RSS_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_rss_kb: None,
            last_cpu_sample: Instant::now()
                .checked_sub(HUD_CPU_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_cpu_ns: None,
            cpu_percent: None,
            max_ms: 0.0,
            max_reset_at: Instant::now(),
            p99_ms: 0.0,
            // v30 (2026-08-05): default target_fps to 60.0 — the same default
            // as Args::fps. The event loop calls set_target_fps() at startup
            // with the resolved value (which may be lower on VSCode, etc.).
            target_fps: 60.0,
            frame_mode: FrameMode::Active,
            screen_size: (0, 0, false),
            // v50 (2026-08-17) HUD expansion — initialize the 7 new metrics
            // to neutral defaults. The event loop calls the setters at
            // startup with the resolved values (from cfg / power_manager /
            // endurance_health) so the HUD shows real data from frame 1.
            scene_name: String::new(),
            color_scheme: ColorScheme::Green,
            custom_palette_name: None,
            charset_preset: String::new(),
            droplet_density: 1.0,
            chars_per_sec: 8.0,
            endurance_health_score: 100.0,
            effective_pressure: 0.0,
            // v50.0.0-beta.6: dragon on/off indicators. Defaults match
            // CloudConfig defaults (power_dragon=true, crystal_dragon=false).
            // The event loop calls the setters every frame with the live
            // cfg values, so live-reload changes are reflected immediately.
            power_dragon_on: true,
            crystal_dragon_on: false,
            aggressive_throttle: false,
            // v50.0.0-beta.7 Option C expansion defaults.
            ambient_on: false,
            glitch_level: crate::config::GlitchLevel::None,
            color_tune_custom: false,
            monolith_size: None,
            cached_lines: [
                // ── Performance core (rows 0-5) — unchanged from v50 ──
                (Color::Cyan, String::new()),    // 0: fps
                (Color::Cyan, String::new()),    // 1: tgt — uses `dim` (tail) at runtime
                (Color::Magenta, String::new()), // 2: max line
                (Color::Yellow, String::new()),  // 3: p99 line
                (Color::Cyan, String::new()),    // 4: cpu line
                (Color::Green, String::new()),   // 5: rss line
                // ── Health / pressure (rows 6-7) ──
                (Color::Yellow, String::new()), // 6: ehs
                (Color::Yellow, String::new()), // 7: prs
                // ── Identity lines (rows 8-10) — v51 reorder: moved up
                //    above the user-adjustable controls ──
                (Color::Cyan, String::new()), // 8: scn
                (Color::Cyan, String::new()), // 9: chr
                (Color::Cyan, String::new()), // 10: clr
                // ── User-adjustable live controls (rows 11-12) ──
                (Color::Magenta, String::new()), // 11: sped
                (Color::Magenta, String::new()), // 12: dsty
                // ── Dragon + tuning state (rows 13-18) — v51 reorder:
                //    was rows 15-20 ──
                (Color::DarkCyan, String::new()), // 13: prdr
                (Color::DarkCyan, String::new()), // 14: crdr
                (Color::DarkCyan, String::new()), // 15: ambt
                (Color::DarkCyan, String::new()), // 16: glth
                (Color::DarkCyan, String::new()), // 17: ctun
                (Color::DarkCyan, String::new()), // 18: mnst
                // cid line — commit short SHA, static for the entire process
                // lifetime. Row 19 (v51 reorder: moved up from the last row
                // so the session footer — up + screensize — closes the
                // dashboard instead).
                (Color::DarkCyan, format!(" cid: {commit_sha}")),
                // ── Session footer (rows 20-21) ──
                (Color::DarkCyan, String::new()), // 20: up
                (Color::DarkCyan, String::new()), // 21: screensize
            ],
            current_width: HUD_MIN_WIDTH,
            prev_width: HUD_MIN_WIDTH,
        }
    }

    /// Render the HUD overlay. Called every frame when visible, but
    /// rate-limited to ~60 Hz (HUD_DISPLAY_MAX_HZ) to avoid wasted ANSI
    /// escapes at high target_fps. Rain continues at full target_fps.
    ///
    /// Does NOT clear entire lines — only writes current_width characters
    /// starting at start_col, so rain on the rest of the line is
    /// preserved. This was the root cause of the "blank space above
    /// rain" bug: \x1b[2K cleared all columns, not just the HUD area.
    /// Write HUD cells into the frame buffer. Called BEFORE term.draw()
    /// so the HUD is part of the same frame flush — eliminates flicker.
    ///
    /// Uses frame.set() (not set_force) so unchanged cells aren't marked
    /// dirty — when metrics are stable, only the changing cells (uptime
    /// seconds) get re-sent.
    pub(crate) fn write_to_frame(
        &mut self,
        frame: &mut crate::frame::Frame,
        cols: u16,
        bg: Option<Color>,
    ) {
        if !self.visible {
            return;
        }
        // HB-01: pad to max(current_width, prev_width) so cells from a
        // previously wider HUD (e.g., after the `tgt:` line drops its
        // ` idle` suffix on idle→active transition) are cleared. Without
        // this, the last character of the previously-longer text remains
        // in the Frame buffer until a rain droplet happens to overwrite
        // that exact cell — visible as a residual `e` for up to several
        // seconds. `Frame::set` short-circuits on content equality, so
        // cells already holding blanks incur zero dirty-mark overhead.
        let w = self.current_width.max(self.prev_width);
        // v50.0.0-beta.6: HUD always renders flush-left at column 0.
        // The previous HudPosition enum + toggle_position method +
        // start_col() helper have been purged — the 'h' shortkey that
        // toggled between Left and Right corners was completely removed
        // (no binding exists, silently ignored). The literal 0 here
        // replaces the `self.position.start_col(cols, w)` call.
        let start_col = 0u16;
        for (i, (color, text)) in self.cached_lines.iter().enumerate() {
            let row = i as u16;
            // v50 (2026-08-17) HUD expansion: skip rows whose text is still
            // empty. The 7 reserved rows 9-15 initialize as empty strings
            // and must NOT render as space-padded rows — otherwise the HUD
            // would grow vertically by 7 blank lines before the
            // data-plumbing commit populates them with real metrics. Once
            // a row is populated, this guard is a no-op (the text is
            // non-empty). The padding loop below only runs for rows that
            // already have visible content, so existing clear-on-shrink
            // behavior (HB-01) is preserved.
            if text.is_empty() {
                continue;
            }
            // Write the text characters.
            for (col_offset, ch) in text.chars().enumerate() {
                let x = start_col + col_offset as u16;
                if x >= cols {
                    break;
                }
                let cell = crate::cell::Cell {
                    ch,
                    fg: Some(*color),
                    bg,
                    bold: false,
                };
                frame.set(x, row, cell);
            }
            // Pad the rest of the line with spaces to the effective width
            // so the background covers the full HUD area consistently —
            // including any cells from a previously wider HUD footprint.
            let text_len = text.chars().count() as u16;
            for col_offset in text_len..w {
                let x = start_col + col_offset;
                if x >= cols {
                    break;
                }
                let cell = crate::cell::Cell {
                    ch: ' ',
                    fg: None,
                    bg,
                    bold: false,
                };
                frame.set(x, row, cell);
            }
        }
        // Track the previous width for the next frame's padding calculation.
        self.prev_width = self.current_width;
    }
}
