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
/// The HUD *display* redraws every frame to prevent rain from
/// flickering through the overlay area; only the expensive metric
/// computation (p99 sort) is rate-limited.
const HUD_METRIC_INTERVAL: Duration = Duration::from_millis(250);

/// Interval between RSS samples in interactive mode (1 Hz).
const HUD_RSS_INTERVAL: Duration = Duration::from_millis(1000);

/// Live HUD overlay state.
pub(crate) struct HudState {
    visible: bool,
    frame_times: FrameTimeTracker,
    last_metric_update: Instant,
    last_rss_sample: Instant,
    last_rss_kb: Option<u64>,
    /// Cached max frame time (ms) for display. Updated on every push.
    max_ms: f64,
    /// Cached p99 frame time (ms) for display. Updated at 4 Hz.
    p99_ms: f64,
    /// Cached display strings — reformatted only at 4 Hz, written to
    /// stdout every frame to prevent rain flicker.
    cached_lines: [(Color, String); 5],
    /// Cached start column for the HUD. Recomputed when cols changes.
    cached_start_col: u16,
}

impl HudState {
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            frame_times: FrameTimeTracker::new(),
            last_metric_update: Instant::now()
                .checked_sub(HUD_METRIC_INTERVAL)
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
        }
        self.visible
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
    pub(crate) fn update_metrics(&mut self) {
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

        // Reformat cached display strings. These are written to stdout
        // every frame by render() to prevent rain from flickering
        // through the HUD area.
        self.cached_lines[0].1 = format!(" fps: {:>7.0}  ", fps);
        self.cached_lines[1].1 = format!(" avg: {:>6.3}ms ", avg_ms);
        self.cached_lines[2].1 = format!(" p99: {:>6.3}ms ", self.p99_ms);
        self.cached_lines[3].1 = format!(" max: {:>6.3}ms ", self.max_ms);
        self.cached_lines[4].1 = format!(" rss: {:>8} ", rss_str);
    }

    /// Render the HUD overlay. Called EVERY FRAME when visible to
    /// prevent rain from flickering through the overlay area. Writes
    /// cached display strings (updated at 4 Hz by update_metrics()).
    ///
    /// `cols` is the terminal width in columns, used to position the HUD
    /// in the top-right corner.
    pub(crate) fn render(&mut self, cols: u16) {
        if !self.visible {
            return;
        }

        // Recompute start column if terminal width changed.
        let hud_width: u16 = 14;
        let start_col = cols.saturating_sub(hud_width + 2);
        if start_col != self.cached_start_col {
            self.cached_start_col = start_col;
        }

        let mut stdout = std::io::stdout();
        // Save cursor position — restored after the HUD is drawn.
        let _ = execute!(stdout, SavePosition);
        for (i, (color, text)) in self.cached_lines.iter().enumerate() {
            let row = i as u16;
            // Clear the line first, then write. This prevents leftover
            // rain characters from showing through when text length
            // changes between metric updates.
            let _ = execute!(
                stdout,
                MoveTo(self.cached_start_col, row),
                SetBackgroundColor(Color::Black),
                SetForegroundColor(*color),
                Print("\x1b[2K"), // clear entire line
                Print(text),
            );
        }
        // Reset colors once after all lines are drawn.
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
