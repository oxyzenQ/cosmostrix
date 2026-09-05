// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD initialization + frame writing — extracted from
//! `hud/mod.rs` to keep that file under the 800-LOC cap.

use std::time::{Duration, Instant};

use crate::runtime::ColorScheme;
use crossterm::style::Color;

use super::{
    DirtyCellTracker, FrameMode, HUD_CPU_INTERVAL, HUD_METRIC_INTERVAL, HUD_MIN_WIDTH,
    HUD_RSS_INTERVAL,
};
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
            // v80.0.0-beta.1 pause-freeze: metrics stop while paused (owner bug fix
            // 2026-08-30). See set_metrics_paused() in metrics.rs.
            metrics_paused: false,
            pause_started_at: None,
            paused_total: Duration::ZERO,
            frame_times: FrameTimeTracker::new(),
            // Z-master-1X round 5: dirty-cell tracker for dcel/tcel metrics.
            dirty_cell_tracker: DirtyCellTracker::new(),
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
            // NIGHT-hunter-9: rain style default — Glyph (the default
            // scene is `cinematic` which uses RainStyle::Glyph). The
            // event loop calls set_rain_style() every frame so this
            // initial value is overwritten on the first tick.
            rain_style: crate::rain_style::RainStyle::Glyph,
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
                // ── Identity lines (rows 8-10) — v80.0.0-beta.1 reorder: moved up
                //    above the user-adjustable controls ──
                (Color::Cyan, String::new()), // 8: scn
                (Color::Cyan, String::new()), // 9: chr
                (Color::Cyan, String::new()), // 10: clr
                // ── User-adjustable live controls (rows 11-12) ──
                (Color::Magenta, String::new()), // 11: sped
                (Color::Magenta, String::new()), // 12: dsty
                // ── Dragon + tuning state (rows 13-18) ──
                (Color::DarkCyan, String::new()), // 13: prdr
                (Color::DarkCyan, String::new()), // 14: crdr
                (Color::DarkCyan, String::new()), // 15: ambt
                (Color::DarkCyan, String::new()), // 16: glth
                (Color::DarkCyan, String::new()), // 17: ctun
                (Color::DarkCyan, String::new()), // 18: mnst
                // ── Rain style (row 19) — NIGHT-hunter-9 ──
                // rain: active rain style (glyph, monolith, vortex,
                // flux, lorenz, dragon, physarum). The owner-mandated
                // position is above `dcel:` so the user can read the
                // active motion DNA before the cell-efficiency metrics.
                (Color::DarkCyan, String::new()), // 19: rain
                // ── Cell efficiency (rows 20-21) — Z-master-1X round 5 ──
                // dcel: dirty cell ratio % (rolling avg over 60 frames).
                // NIGHT-hunter-9: shifted down from row 19 to row 20
                // to make room for the new `rain:` line above.
                (Color::DarkCyan, String::new()), // 20: dcel
                // tcel: total cells in the screen (latest sample).
                (Color::DarkCyan, String::new()), // 21: tcel
                // cid line — commit short SHA, static for the entire process
                // lifetime. Row 22 (Z-master-1X round 5: moved down from
                // row 19; NIGHT-hunter-9: moved down again from row 21
                // to make room for the new `rain:` line above `dcel:`).
                (Color::DarkCyan, format!(" cid: {commit_sha}")),
                // ── Session footer (rows 23-24) ──
                (Color::DarkCyan, String::new()), // 23: up
                (Color::DarkCyan, String::new()), // 24: screensize
            ],
            current_width: HUD_MIN_WIDTH,
            prev_width: HUD_MIN_WIDTH,
        }
    }

    /// Render the HUD overlay. Called every frame when visible; the
    /// METRIC recompute (p99 sort + formatting) is rate-limited to 1 Hz
    /// (HUD_METRIC_INTERVAL in mod.rs) — the per-frame work is only the
    /// cell write of the cached lines, so no ANSI escape is wasted at
    /// high target_fps. Rain continues at full target_fps.
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
    /// (v80.0.0-alpha.1 doc-drift fix: the old comment referenced a
    /// phantom "HUD_DISPLAY_MAX_HZ ~60 Hz" rate limiter that does not
    /// exist — the real rate limit is the 1 Hz metric tick.)
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
        // v80.0.0-beta.1 HUD chroma border (owner mandate 2026-09-02):
        // L-shape border on the right + bottom of the HUD area, using
        // the same chroma dragon palette integration as the message
        // border (cloud/message_draw.rs BC-01..05). Same simple function
        // as the message border, different position: the message border
        // is a full rectangle around the centered message box; the HUD
        // border is an L-shape closing the top-left HUD block (top +
        // left edges are implied by the screen edge at col 0, row 0).
        self.draw_border(frame, cols, bg);
        // Track the previous width for the next frame's padding calculation.
        self.prev_width = self.current_width;
    }

    /// v80.0.0-beta.1 HUD chroma border (owner mandate 2026-09-02):
    /// Draw an L-shape chroma dragon border on the right + bottom of
    /// the HUD area. Same palette integration as the message border
    /// (`cloud/message_draw.rs` BC-01..05), different position: the
    /// message border is a full rectangle around the centered message
    /// box; the HUD border is an L-shape closing the top-left HUD
    /// block (top + left edges are implied by the screen edge).
    ///
    /// Color sweep:
    /// - Right edge (rows 0..23, col = current_width): per-row chroma
    ///   color from `cached_lines`, sweeping dim tail at the top to
    ///   bright head at the bottom — mirrors the HUD's own 24-row
    ///   gradient and the message border's clockwise sweep philosophy.
    /// - Bottom edge (row 24, cols 0..=current_width): single bright
    ///   head color (`cached_lines[23].0`, the palette's last stop —
    ///   the rain's leading bright character) for a clean closing line.
    /// - Corner (col current_width, row 24): '╯' (light up-left corner)
    ///   in the bright head color, connecting the right + bottom edges.
    ///
    /// Edge fade (owner mandate 2026-09-02, "visual 9/10 — ujung border
    /// harus semi black/fade biar elegant"): the border edges fade
    /// toward the screen edge (top-left corner of screen) so the
    /// border "emerges from shadow" instead of popping in abruptly.
    /// This mirrors the message border's triangle-wave fade
    /// (`cloud/message_draw.rs` BD-02: dark→bright→dark around
    /// perimeter), applied per-edge with a linear ramp:
    /// - Right edge: row 0 (top, near screen top) = max fade (0.6
    ///   blend toward bg), row 23 (bottom, head anchor) = no fade.
    ///   factor = 0.6 * (1.0 - row / 23.0).
    /// - Bottom edge: col 0 (left, near screen left) = max fade,
    ///   col cur (corner anchor) = no fade.
    ///   factor = 0.6 * (1.0 - col / cur).
    ///
    /// The corner cell (col cur, row 24) is always full-bright (the
    /// anchor point). Uses `chroma_dragon_engine::palette::blend_toward_bg`
    /// for the fade — same blend helper the rain droplets use.
    ///
    /// Dynamic clean movement (owner bug report 2026-09-02, "visual
    /// rating 8/10 — residue/stain when border moves"): the border
    /// position tracks `current_width` directly (NOT `max(cur, prev)`),
    /// so it moves left/right immediately when metric values change
    /// width (e.g. `dcel` value grows/shrinks). When the HUD shrinks
    /// (`prev > cur`), the old border cells at col `prev` (right edge)
    /// and cols `cur+1..=prev` at row 24 (bottom edge + corner) are
    /// explicitly blanked BEFORE drawing the new border — the metrics
    /// padding loop only blanks cols `text_len..max(cur,prev)`, which
    /// excludes col `prev` itself (the old border column). Without
    /// this clearing, the border leaves a "stain" or "ghost" at its
    /// old position when it moves left.
    ///
    /// Uses `frame.set()` (not `set_force`) so unchanged border cells
    /// aren't marked dirty — when the HUD width is stable, the border
    /// is a one-time write that the terminal never re-sends. Frame's
    /// `set()` silently skips out-of-bounds cells, so a terminal too
    /// short for row 24 simply omits the bottom edge without panicking.
    fn draw_border(&self, frame: &mut crate::frame::Frame, cols: u16, bg: Option<Color>) {
        let cur = self.current_width;
        let prev = self.prev_width;
        if cur == 0 && prev == 0 {
            return;
        }
        // Bright head color (palette last stop, cached_lines row 23 =
        // screensize — the visual anchor at the bottom of the HUD).
        let head_color = self.cached_lines[23].0;

        // v80.0.0-beta.1 edge fade: blend toward bg (semi-black) at the
        // screen-edge ends of each border edge. bg defaults to black
        // when None (transparent terminal background).
        let bg_color = bg.unwrap_or(Color::Rgb { r: 0, g: 0, b: 0 });
        // Max fade factor at the screen-edge end (0.6 = 60% blend
        // toward bg = semi-black, owner "semi black/fade" mandate).
        const FADE_MAX: f32 = 0.6;

        // v80.0.0-beta.1 residue fix: when the HUD width shrinks
        // (prev > cur), the old border cells at col `prev` (right
        // edge, rows 0..24) and cols `cur+1..=prev` at row 24 (bottom
        // edge + corner) still hold border chars from the previous
        // frame. They MUST be blanked explicitly — the metrics padding
        // loop only blanks cols `text_len..max(cur,prev)`, which
        // excludes col `prev` itself. Without this, the border leaves
        // a visible "stain" at its old position when it moves left.
        if prev > cur {
            // Clear old right border column at `prev` (rows 0..24).
            if prev < cols {
                for row in 0..24u16 {
                    let blank = crate::cell::Cell {
                        ch: ' ',
                        fg: None,
                        bg,
                        bold: false,
                    };
                    frame.set(prev, row, blank);
                }
            }
            // Clear old bottom border cells at row 24. When cur > 0,
            // the new border covers cols 0..=cur, so only clear
            // cur+1..=prev. When cur == 0, no new border is drawn,
            // so clear the entire old range 0..=prev.
            let clear_from = if cur == 0 { 0 } else { cur + 1 };
            for col in clear_from..=prev {
                if col >= cols {
                    break;
                }
                let blank = crate::cell::Cell {
                    ch: ' ',
                    fg: None,
                    bg,
                    bold: false,
                };
                frame.set(col, 24, blank);
            }
        }

        // Draw new right border at `cur` (if cur > 0).
        // Edge fade: row 0 (top) = max fade toward bg, row 23 (bottom)
        // = no fade (bright head anchor). Linear ramp.
        if cur > 0 && cur < cols {
            for row in 0..24u16 {
                // Fade factor: 0.0 at row 23 (no fade), FADE_MAX at row 0.
                let fade = FADE_MAX * (1.0 - row as f32 / 23.0);
                let base_color = self.cached_lines[row as usize].0;
                let fg = if fade > 0.0 {
                    crate::chroma_dragon_engine::palette::blend_toward_bg(
                        base_color, bg_color, fade,
                    )
                } else {
                    base_color
                };
                let cell = crate::cell::Cell {
                    ch: '│',
                    fg: Some(fg),
                    bg,
                    bold: false,
                };
                frame.set(cur, row, cell);
            }
        }

        // Draw new bottom border at row 24, cols 0..=cur.
        // Edge fade: col 0 (left, near screen edge) = max fade toward
        // bg, col cur (corner) = no fade (bright head anchor). Linear
        // ramp. The corner cell (col == cur) is always full-bright.
        if cur > 0 {
            for col in 0..=cur {
                if col >= cols {
                    break;
                }
                let ch = if col == cur { '╯' } else { '─' };
                // Fade factor: 0.0 at col cur (no fade), FADE_MAX at col 0.
                let fade = FADE_MAX * (1.0 - col as f32 / cur as f32);
                let fg = if fade > 0.0 {
                    crate::chroma_dragon_engine::palette::blend_toward_bg(
                        head_color, bg_color, fade,
                    )
                } else {
                    head_color
                };
                let cell = crate::cell::Cell {
                    ch,
                    fg: Some(fg),
                    bg,
                    bold: false,
                };
                frame.set(col, 24, cell);
            }
        }
    }
}
