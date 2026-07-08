// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live HUD overlay for interactive mode.
//!
//! Toggle with `?`. When visible, renders a compact 4-line overlay in the
//! top-right corner showing real-time FPS, frame time, p99 frame time,
//! and RSS. The overlay is drawn AFTER `terminal.draw()` so it survives
//! differential redraws.
//!
//! ## Design constraints
//! - **Zero cost when off**: `visible == false` short-circuits all work.
//! - **Rate-limited when on**: HUD redraws at 4 Hz (every 250ms) regardless
//!   of frame rate, so 60 FPS rendering does not pay 60× HUD redraw cost.
//! - **Frame-time tracking reuses the existing `FrameTimeTracker`** from
//!   `activity.rs` — no duplicate ring buffer.
//! - **RSS sampling reuses `crate::memstat`** — same cross-platform logic
//!   as the benchmark, sampling at 1 Hz (slower than benchmark's 100ms
//!   because interactive mode has lower sampling budget).
//! - **ANSI-only output**: no `term.draw()` integration, no frame buffer
//!   mutation. The HUD writes directly to stdout via `execute!` and
//!   restores the cursor position. This keeps the frame's dirty tracking
//!   clean and prevents the HUD from polluting the rain renderer's
//!   differential redraw bookkeeping.

use std::io::Write;
use std::time::{Duration, Instant};

use crossterm::cursor::{MoveTo, RestorePosition, SavePosition};
use crossterm::execute;
use crossterm::style::{Color, Print, SetBackgroundColor, SetForegroundColor};

use crate::interactive::activity::FrameTimeTracker;
use crate::memstat;

/// Minimum interval between HUD metric recomputation (~4 Hz).
const HUD_METRIC_INTERVAL: Duration = Duration::from_millis(250);

/// Maximum HUD display redraw rate (~60 Hz). Even when target_fps is
/// 120 or 240, the HUD text is only rewritten at 60 Hz — the human eye
/// cannot read text faster than this, and capping reduces ANSI escape
/// overhead. Rain continues to render at full target_fps.
const HUD_DISPLAY_MAX_HZ: Duration = Duration::from_millis(16);

/// Interval between RSS samples in interactive mode (1 Hz).
const HUD_RSS_INTERVAL: Duration = Duration::from_millis(1000);

/// Width of the HUD overlay in terminal columns. Each line is padded
/// to exactly this width with spaces so the black background covers
/// only the HUD area — rain on the rest of the line stays intact.
const HUD_WIDTH: u16 = 15;

/// HUD position: left or right corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HudPosition {
    Left,
    Right,
}

impl HudPosition {
    /// Compute the start column for this position given terminal width.
    fn start_col(self, cols: u16) -> u16 {
        match self {
            // Left: flush against the edge (column 0).
            Self::Left => 0,
            // Right: flush against the right border.
            // The last HUD character sits at column cols-1.
            // Using saturating_sub(HUD_WIDTH) places the HUD so its
            // rightmost character is at cols-1 (the last column).
            Self::Right => cols.saturating_sub(HUD_WIDTH),
        }
    }
}

/// Live HUD overlay state.
pub(crate) struct HudState {
    visible: bool,
    position: HudPosition,
    /// Session start time for uptime display.
    session_start: Instant,
    frame_times: FrameTimeTracker,
    last_metric_update: Instant,
    last_display_update: Instant,
    last_rss_sample: Instant,
    last_rss_kb: Option<u64>,
    /// Cached max frame time (ms) for display. Updated on every push.
    max_ms: f64,
    /// Cached p99 frame time (ms) for display. Updated at 4 Hz.
    p99_ms: f64,
    /// Cached display strings — reformatted only at 4 Hz, written to
    /// stdout at up to 60 Hz to prevent rain flicker.
    cached_lines: [(Color, String); 5],
    /// Cached start column for the HUD. Recomputed when cols/position changes.
    cached_start_col: u16,
}

impl HudState {
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            position: HudPosition::Left,
            session_start: Instant::now(),
            frame_times: FrameTimeTracker::new(),
            last_metric_update: Instant::now()
                .checked_sub(HUD_METRIC_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_display_update: Instant::now()
                .checked_sub(HUD_DISPLAY_MAX_HZ)
                .unwrap_or_else(Instant::now),
            last_rss_sample: Instant::now()
                .checked_sub(HUD_RSS_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_rss_kb: None,
            max_ms: 0.0,
            p99_ms: 0.0,
            cached_lines: [
                (Color::Cyan, String::new()),
                (Color::Yellow, String::new()),
                (Color::Magenta, String::new()),
                (Color::Green, String::new()),
                (Color::DarkCyan, String::new()),
            ],
            cached_start_col: 0,
        }
    }

    /// Toggle HUD visibility. Returns the new visibility state.
    pub(crate) fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        // When turning on, force immediate metric recompute + render.
        if self.visible {
            self.last_metric_update = Instant::now()
                .checked_sub(HUD_METRIC_INTERVAL * 2)
                .unwrap_or_else(Instant::now);
            self.last_display_update = Instant::now()
                .checked_sub(HUD_DISPLAY_MAX_HZ * 2)
                .unwrap_or_else(Instant::now);
        }
        self.visible
    }

    /// Toggle HUD position between left and right corners.
    /// Called when user presses 'H' (shift+h).
    /// Returns true to signal the event loop that a full redraw is
    /// needed to clear the old HUD position's residue from the frame.
    pub(crate) fn toggle_position(&mut self) -> bool {
        self.position = match self.position {
            HudPosition::Left => HudPosition::Right,
            HudPosition::Right => HudPosition::Left,
        };
        // Force recompute of start_col on next render.
        self.cached_start_col = u16::MAX;
        true
    }

    /// Whether the HUD is currently visible.
    #[allow(dead_code)]
    pub(crate) fn visible(&self) -> bool {
        self.visible
    }

    /// Record a frame time. Called every frame from the event loop.
    /// Cheap when the HUD is off (just one bool check + early return).
    #[inline]
    pub(crate) fn push_frame_time(&mut self, ms: f64) {
        if !self.visible {
            return;
        }
        self.frame_times.push(ms);
        if ms > self.max_ms {
            self.max_ms = ms;
        }
    }

    /// Maybe sample RSS (rate-limited). Called every frame.
    #[inline]
    pub(crate) fn maybe_sample_rss(&mut self) {
        if !self.visible {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_rss_sample) < HUD_RSS_INTERVAL {
            return;
        }
        self.last_rss_sample = now;
        self.last_rss_kb = memstat::current_rss_kb();
    }

    /// Recompute HUD metrics (rate-limited at 4 Hz). Called every frame
    /// from the event loop. Cheap on the fast path (one timestamp
    /// comparison + early return). When the interval elapses, reformats
    /// the cached display strings.
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

        // Recompute p99 from the ring buffer (~300ns, acceptable at 4 Hz).
        self.p99_ms = self.frame_times.p99_ms();

        let avg_ms = self.frame_times.rolling_avg_ms();
        let fps = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };
        let rss_str = self
            .last_rss_kb
            .map(format_rss_kb)
            .unwrap_or_else(|| "—".to_string());

        // Dynamic color selection from the active palette.
        // Pick colors from different positions to get visual variety:
        // head (brightest), mid, trail (dimmest).
        // Each color is brightened by blending with white to ensure
        // readability on the black background — some palette colors
        // (e.g. dark green trail) are too dim to read as HUD text.
        let n = palette_colors.len();
        let head = brighten_color(
            palette_colors
                .get(n.saturating_sub(1))
                .copied()
                .unwrap_or(Color::White),
        );
        let mid = brighten_color(palette_colors.get(n / 2).copied().unwrap_or(Color::Cyan));
        let trail = brighten_color(
            palette_colors
                .get(n / 4)
                .copied()
                .unwrap_or(Color::DarkCyan),
        );
        let dim = brighten_color(palette_colors.get(1).copied().unwrap_or(Color::DarkGrey));

        // Session uptime: mm:ss format.
        let uptime_secs = self.session_start.elapsed().as_secs();
        let uptime_str = format!("{:02}:{:02}", uptime_secs / 60, uptime_secs % 60);

        // 5-line HUD: fps (palette head), p99 (mid), max (head), rss (trail), uptime (dim).
        // avg is dropped — fps = 1000/avg, so it's redundant.
        // Format strings have NO trailing spaces — pad_hud_line handles
        // width consistency. This ensures the last visible character
        // sits flush against the border in right position.
        self.cached_lines[0] = (head, pad_hud_line(&format!(" fps: {:>7.0}", fps)));
        self.cached_lines[1] = (mid, pad_hud_line(&format!(" p99: {:>6.3}ms", self.p99_ms)));
        self.cached_lines[2] = (head, pad_hud_line(&format!(" max: {:>6.3}ms", self.max_ms)));
        self.cached_lines[3] = (trail, pad_hud_line(&format!(" rss: {:>8}", rss_str)));
        self.cached_lines[4] = (dim, pad_hud_line(&format!(" up: {:>5}", uptime_str)));
    }

    /// Render the HUD overlay. Called every frame when visible, but
    /// rate-limited to ~60 Hz (HUD_DISPLAY_MAX_HZ) to avoid wasted ANSI
    /// escapes at high target_fps. Rain continues at full target_fps.
    ///
    /// Does NOT clear entire lines — only writes HUD_WIDTH characters
    /// starting at start_col, so rain on the rest of the line is
    /// preserved. This was the root cause of the "blank space above
    /// rain" bug: \x1b[2K cleared all columns, not just the HUD area.
    /// Write HUD cells into the frame buffer. Called BEFORE term.draw()
    /// so the HUD is part of the same frame flush as the rain — this
    /// eliminates fullscreen flicker (two separate stdout writes were
    /// causing double-repaint in fullscreen mode).
    ///
    /// Writes HUD_WIDTH cells per line, starting at start_col, for 5
    /// lines. Each cell gets the line's fg color + black bg. Rain cells
    /// outside the HUD area are untouched.
    pub(crate) fn write_to_frame(&self, frame: &mut crate::frame::Frame, cols: u16) {
        if !self.visible {
            return;
        }
        let start_col = self.position.start_col(cols);
        for (i, (color, text)) in self.cached_lines.iter().enumerate() {
            let row = i as u16;
            for (col_offset, ch) in text.chars().enumerate() {
                let x = start_col + col_offset as u16;
                if x >= cols {
                    break;
                }
                let cell = crate::cell::Cell {
                    ch,
                    fg: Some(*color),
                    bg: Some(Color::Black),
                    bold: false,
                };
                frame.set_force(x, row, cell);
            }
        }
    }

    /// Legacy direct-stdout render. Kept for backward compat but no
    /// longer called by the event loop — use write_to_frame() instead.
    #[allow(dead_code)]
    pub(crate) fn render(&mut self, cols: u16) {
        if !self.visible {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_display_update) < HUD_DISPLAY_MAX_HZ {
            return;
        }
        self.last_display_update = now;

        let start_col = self.position.start_col(cols);
        if start_col != self.cached_start_col {
            self.cached_start_col = start_col;
        }

        let mut stdout = std::io::stdout();
        let _ = execute!(stdout, SavePosition);
        for (i, (color, text)) in self.cached_lines.iter().enumerate() {
            let row = i as u16;
            // Move to HUD start position, set bg=black, write padded text.
            // No line clear — only HUD_WIDTH chars are overwritten.
            let _ = execute!(
                stdout,
                MoveTo(self.cached_start_col, row),
                SetBackgroundColor(Color::Black),
                SetForegroundColor(*color),
                Print(text),
            );
        }
        let _ = execute!(
            stdout,
            SetBackgroundColor(Color::Reset),
            SetForegroundColor(Color::Reset),
            RestorePosition,
        );
        let _ = stdout.flush();
    }

    /// Reset max frame time. Called when the user wants to clear the
    /// peak (e.g. after a known spike like a resize).
    #[allow(dead_code)]
    pub(crate) fn reset_max(&mut self) {
        self.max_ms = 0.0;
    }
}

/// Format a KiB value as a human-readable string (matches bench_report
/// formatting for consistency).
fn format_rss_kb(kib: u64) -> String {
    const MIB: u64 = 1024;
    if kib >= MIB {
        format!("{:.1}MiB", kib as f64 / MIB as f64)
    } else {
        format!("{kib}KiB")
    }
}

/// Pad a HUD line to exactly HUD_WIDTH characters with trailing spaces.
/// Truncate if longer (shouldn't happen with current format strings).
/// This ensures the black background covers a consistent area and rain
/// on the rest of the line is never touched.
fn pad_hud_line(s: &str) -> String {
    let w = HUD_WIDTH as usize;
    if s.len() >= w {
        s[..w].to_string()
    } else {
        let mut out = String::with_capacity(w);
        out.push_str(s);
        out.push_str(&" ".repeat(w - s.len()));
        out
    }
}

/// Brighten a crossterm Color by blending it 50% with white.
/// Ensures HUD text is always readable on the black background,
/// even when the palette color is very dark (e.g. dark green trail).
/// Non-RGB colors (AnsiValue, named) are returned as-is — they're
/// already bright enough in practice.
fn brighten_color(color: Color) -> Color {
    match color {
        Color::Rgb { r, g, b } => Color::Rgb {
            r: r / 2 + 128,
            g: g / 2 + 128,
            b: b / 2 + 128,
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_starts_invisible() {
        let h = HudState::new();
        assert!(!h.visible(), "HUD must start invisible");
    }

    #[test]
    fn hud_toggle_flips_visibility() {
        let mut h = HudState::new();
        assert!(!h.visible());
        assert!(h.toggle(), "first toggle must turn HUD on");
        assert!(h.visible());
        assert!(!h.toggle(), "second toggle must turn HUD off");
        assert!(!h.visible());
    }

    #[test]
    fn hud_push_frame_time_is_noop_when_invisible() {
        let mut h = HudState::new();
        h.push_frame_time(1.0);
        // max_ms should still be 0 because the HUD is off.
        assert_eq!(h.max_ms, 0.0, "invisible HUD must not record frame times");
    }

    #[test]
    fn hud_push_frame_time_records_when_visible() {
        let mut h = HudState::new();
        h.toggle();
        h.push_frame_time(1.0);
        h.push_frame_time(2.0);
        h.push_frame_time(0.5);
        assert_eq!(h.max_ms, 2.0, "max_ms must track the highest pushed value");
    }

    #[test]
    fn hud_maybe_sample_rss_is_noop_when_invisible() {
        let mut h = HudState::new();
        h.maybe_sample_rss();
        assert!(h.last_rss_kb.is_none(), "invisible HUD must not sample RSS");
    }

    #[test]
    fn format_rss_kb_renders_suffixes() {
        assert_eq!(format_rss_kb(0), "0KiB");
        assert_eq!(format_rss_kb(512), "512KiB");
        assert_eq!(format_rss_kb(1023), "1023KiB");
        assert_eq!(format_rss_kb(1024), "1.0MiB");
        assert_eq!(format_rss_kb(2048), "2.0MiB");
    }
}
