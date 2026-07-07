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

/// Minimum interval between HUD redraws (~4 Hz).
const HUD_UPDATE_INTERVAL: Duration = Duration::from_millis(250);

/// Interval between RSS samples in interactive mode (1 Hz).
/// Slower than the benchmark's 100ms because interactive mode runs
/// indefinitely and /proc reads have measurable overhead at high rates.
const HUD_RSS_INTERVAL: Duration = Duration::from_millis(1000);

/// Live HUD overlay state.
pub(crate) struct HudState {
    visible: bool,
    frame_times: FrameTimeTracker,
    last_update: Instant,
    last_rss_sample: Instant,
    last_rss_kb: Option<u64>,
    /// Cached max frame time (ms) for display. Updated on every push.
    max_ms: f64,
    /// Cached p99 frame time (ms) for display. Updated periodically by
    /// sorting a copy of the ring buffer.
    p99_ms: f64,
}

impl HudState {
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            frame_times: FrameTimeTracker::new(),
            // Allow the first update immediately.
            last_update: Instant::now()
                .checked_sub(HUD_UPDATE_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_rss_sample: Instant::now()
                .checked_sub(HUD_RSS_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_rss_kb: None,
            max_ms: 0.0,
            p99_ms: 0.0,
        }
    }

    /// Toggle HUD visibility. Returns the new visibility state.
    pub(crate) fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        // When turning on, force the next render() call to fire immediately
        // by backdating last_update past the rate-limit window.
        if self.visible {
            self.last_update = Instant::now()
                .checked_sub(HUD_UPDATE_INTERVAL * 2)
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

    /// Render the HUD overlay if visible and enough time has elapsed since
    /// the last redraw. Writes ANSI escape sequences directly to stdout.
    ///
    /// `cols` is the terminal width in columns, used to position the HUD
    /// in the top-right corner.
    pub(crate) fn render(&mut self, cols: u16) {
        if !self.visible {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_update) < HUD_UPDATE_INTERVAL {
            return;
        }
        self.last_update = now;

        // Recompute p99 from the ring buffer. FrameTimeTracker::p99_ms()
        // sorts a 60-element snapshot — ~300ns, negligible at 4 Hz.
        self.p99_ms = self.frame_times.p99_ms();

        let avg_ms = self.frame_times.rolling_avg_ms();
        let fps = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };
        let jitter = self.frame_times.jitter_classification();
        let rss_str = self
            .last_rss_kb
            .map(format_rss_kb)
            .unwrap_or_else(|| "—".to_string());

        // HUD layout: 4 lines, ~24 chars wide, top-right corner.
        // Line 1: FPS (bold green)
        // Line 2: avg frame time (ms)
        // Line 3: p99 frame time (ms) — highlights tail spikes
        // Line 4: RSS (KiB/MiB) — shows memory footprint
        // Line 5: jitter classification
        let lines: [(Color, &str); 5] = [
            (Color::Cyan, &format!(" fps: {:>7.0}  ", fps)),
            (Color::Yellow, &format!(" avg: {:>6.3}ms ", avg_ms)),
            (Color::Magenta, &format!(" p99: {:>6.3}ms ", self.p99_ms)),
            (Color::Green, &format!(" max: {:>6.3}ms ", self.max_ms)),
            (Color::DarkCyan, &format!(" rss: {:>8} ", rss_str)),
        ];
        let _ = jitter; // Reserved for future use; not rendered to keep HUD compact.

        // Position: top-right. The HUD is ~14 chars wide; place it at
        // cols - 16 to leave a 2-char right margin.
        let hud_width: u16 = 14;
        let start_col = cols.saturating_sub(hud_width + 2);

        let mut stdout = std::io::stdout();
        // Save cursor, move to top-right, draw the HUD, restore cursor.
        let _ = execute!(stdout, SavePosition);
        for (i, (color, text)) in lines.iter().enumerate() {
            let row = i as u16;
            let _ = execute!(
                stdout,
                MoveTo(start_col, row),
                SetBackgroundColor(Color::Black),
                SetForegroundColor(*color),
                Print(text),
                SetBackgroundColor(Color::Reset),
                SetForegroundColor(Color::Reset),
            );
        }
        let _ = execute!(stdout, RestorePosition);
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
