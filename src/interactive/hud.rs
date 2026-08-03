// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live HUD overlay for interactive mode.
//!
//! Toggle with `?`. When visible, writes a compact 5-line overlay into
//! the frame buffer (before `term.draw()`) showing real-time FPS, p99,
//! max frame time, RSS, and session uptime. Press `H` to toggle
//! position between left and right corners.
//!
//! ## Design constraints
//! - **Zero cost when off**: `visible == false` short-circuits all work.
//! - **Metrics at 1 Hz**: p99 sort + string formatting only every 1000ms.
//!   1 Hz is the world-class standard for live HUDs (htop, mangoHUD,
//!   Steam FPS counter, nvidia-smi) — calm enough that the eye reads
//!   numbers as stable, fast enough to catch any real spike. The
//!   previous 4 Hz cadence made FPS/p99 visibly flicker 4×/sec which
//!   read as "wasteful" even though CPU cost was negligible (~30 µs/s).
//! - **Frame buffer integration**: HUD cells written via `frame.set()`
//!   (not `set_force`) so unchanged cells are NOT marked dirty — the
//!   terminal skips re-sending them. When metrics are stable, only
//!   the uptime seconds change between frames.
//! - **Dynamic palette colors**: HUD colors come from the active theme,
//!   brightened 50% with white for readability on black background.
//! - **Auto-reset max**: max_ms resets every 60s to show recent peaks,
//!   not a startup spike from 10 minutes ago.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use crate::cpustat;
use crate::interactive::activity::FrameTimeTracker;
use crate::memstat;

/// Minimum interval between HUD metric recomputation (1 Hz).
///
/// 1 Hz is the sweet spot for live HUD overlays — matches htop,
/// mangoHUD, Steam FPS counter, and `nvidia-smi`. Faster rates (e.g.
/// the previous 250 ms / 4 Hz) cause visible number flicker without
/// improving diagnostic value, since the human eye can't correlate
/// sub-second FPS changes to root causes anyway. Slower rates (e.g.
/// 3 s) would hide real spikes that the user wants to catch.
///
/// This interval also aligns with `HUD_RSS_INTERVAL` so both metric
/// and RSS updates fire on the same tick — halving the per-frame
/// timestamp-comparison overhead on the fast path.
const HUD_METRIC_INTERVAL: Duration = Duration::from_millis(1000);

/// Interval between RSS samples in interactive mode (1 Hz).
const HUD_RSS_INTERVAL: Duration = Duration::from_millis(1000);

/// Interval between CPU% samples in interactive mode (1 Hz).
/// Aligned with `HUD_RSS_INTERVAL` so both fire on the same tick.
/// At 1 Hz the per-sample cost is one `cpustat::current_cpu_ns()` call
/// (~2 KiB `/proc` read on Linux, one `task_info` syscall on macOS,
/// one `getrusage` syscall on BSD/Android) — well under 0.1% CPU.
const HUD_CPU_INTERVAL: Duration = Duration::from_millis(1000);

/// How often to reset max_ms (seconds). Prevents a startup spike from
/// dominating the max display forever.
const MAX_RESET_INTERVAL_SECS: u64 = 60;

/// Minimum width of the HUD overlay (for short values).
/// The actual width is dynamic — grows when values are long (e.g. high FPS).
const HUD_MIN_WIDTH: u16 = 12;

/// Maximum width cap (prevents HUD from eating the whole terminal).
/// Bumped from 20 → 22 in v30 to fit the new `cpu: 100.00%` line
/// (max-width value: ` cpu: 100.00%` = 13 chars + 1 leading space = 14,
/// but other lines like ` p99: 9999.999ms` already exceed that, so
/// the practical cap is set by the longest existing line). The bump
/// ensures the cpu line never gets truncated when fps is high (which
/// would make ` p99` wrap visually).
const HUD_MAX_WIDTH: u16 = 22;

/// HUD position: left or right corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HudPosition {
    Left,
    Right,
}

impl HudPosition {
    /// Compute the start column for this position given terminal width
    /// and the current dynamic HUD width.
    fn start_col(self, cols: u16, hud_width: u16) -> u16 {
        match self {
            // Left: flush against the edge (column 0).
            Self::Left => 0,
            // Right: flush against the right border.
            Self::Right => cols.saturating_sub(hud_width),
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
    last_rss_sample: Instant,
    last_rss_kb: Option<u64>,
    /// Last CPU% sample timestamp. Used to compute the wall-clock delta
    /// between two `cpustat::current_cpu_ns()` samples.
    last_cpu_sample: Instant,
    /// Previous process CPU ns reading. `None` until the first sample
    /// completes (we need two readings to compute a delta). On the
    /// very first tick we store the baseline and render `cpu: —` until
    /// the second tick arrives.
    last_cpu_ns: Option<u64>,
    /// Latest computed process CPU% (0.0 ..= 100.0 on single-threaded
    /// builds; can briefly exceed 100 if a frame spills onto another
    /// core, which we clamp at 999.99 for display width safety).
    /// `None` on unsupported platforms (non-unix) or before the first
    /// successful delta. Renders as `cpu: —` (em dash).
    cpu_percent: Option<f32>,
    /// Cached max frame time (ms) for display. Updated on every push.
    /// Auto-resets every MAX_RESET_INTERVAL_SECS to prevent startup
    /// spikes from dominating forever.
    max_ms: f64,
    /// When max_ms was last reset. Used for auto-reset.
    max_reset_at: Instant,
    /// Cached p99 frame time (ms) for display. Updated at 1 Hz.
    p99_ms: f64,
    /// Screen size for HUD display. Updated by event_loop when terminal
    /// resizes or --screen-size is set. Format: (width, height, is_fixed).
    screen_size: (u16, u16, bool),
    /// Cached display strings — reformatted only at 1 Hz, written to
    /// frame buffer every frame via write_to_frame().
    /// 7 lines: fps / p99 / max / rss / cpu / up / screensize.
    cached_lines: [(Color, String); 7],
    /// Current dynamic HUD width (in terminal columns). Recomputed
    /// every metric update to fit the longest line. Grows when FPS
    /// or RSS values are long, shrinks when they're short.
    current_width: u16,
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
            screen_size: (0, 0, false),
            cached_lines: [
                (Color::Cyan, String::new()),
                (Color::Yellow, String::new()),
                (Color::Magenta, String::new()),
                (Color::Green, String::new()),
                (Color::Cyan, String::new()), // cpu line — uses `mid` (bright) at runtime
                (Color::DarkCyan, String::new()),
                (Color::DarkCyan, String::new()),
            ],
            current_width: HUD_MIN_WIDTH,
        }
    }

    /// Toggle HUD visibility. Returns the new visibility state.
    pub(crate) fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        if self.visible {
            // Force an immediate metric refresh on the next frame.
            self.last_metric_update = Instant::now()
                .checked_sub(HUD_METRIC_INTERVAL * 2)
                .unwrap_or_else(Instant::now);
            // Force an immediate RSS refresh too.
            self.last_rss_sample = Instant::now()
                .checked_sub(HUD_RSS_INTERVAL * 2)
                .unwrap_or_else(Instant::now);
            // CPU sampling: NO reset of last_cpu_ns / last_cpu_sample.
            // The CPU baseline is kept warm continuously (see
            // maybe_sample_cpu — it samples at 1 Hz even when the HUD is
            // off, costing one syscall/sec). This means on toggle-on,
            // we already have a recent baseline and can compute an
            // instant percent on the very next tick — no `cpu: —`
            // flash. The user explicitly requested this; other metrics
            // (fps/p99/rss) show data instantly, and CPU must too.
        }
        self.visible
    }

    /// Toggle HUD position between left and right corners.
    /// Returns true to signal the event loop that a full redraw is
    /// needed to clear the old HUD position's residue from the frame.
    pub(crate) fn toggle_position(&mut self) -> bool {
        self.position = match self.position {
            HudPosition::Left => HudPosition::Right,
            HudPosition::Right => HudPosition::Left,
        };
        true
    }

    /// Whether the HUD is currently visible. Test-only — production
    /// code reads the `visible` field directly (cheaper than a method
    /// call in the hot render path). Tests use this accessor to verify
    /// toggle behavior without reaching into private fields.
    #[cfg(test)]
    pub(crate) fn visible(&self) -> bool {
        self.visible
    }

    /// Record a frame time. Called every frame from the event loop.
    /// Cheap when the HUD is off (just one bool check + early return).
    /// Auto-resets max_ms every MAX_RESET_INTERVAL_SECS to prevent a
    /// startup spike from dominating the display forever.
    #[inline]
    pub(crate) fn push_frame_time(&mut self, ms: f64) {
        if !self.visible {
            return;
        }
        self.frame_times.push(ms);
        // Auto-reset max every 60s so the display shows recent peaks,
        // not a startup spike from 10 minutes ago.
        if self.max_reset_at.elapsed().as_secs() >= MAX_RESET_INTERVAL_SECS {
            self.max_ms = 0.0;
            self.max_reset_at = Instant::now();
        }
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

    /// Maybe sample process CPU% (rate-limited to 1 Hz). Called every frame.
    ///
    /// Computes `cpu_percent = (cpu_ns_delta / wall_ns_delta) * 100.0`
    /// using two consecutive `cpustat::current_cpu_ns()` readings.
    ///
    /// ## Keeps baseline warm even when HUD is off
    /// Unlike `maybe_sample_rss` (which short-circuits on invisible),
    /// this method samples at 1 Hz **regardless of HUD visibility**.
    /// The reason: CPU% requires a delta between two samples, so a cold
    /// baseline forces the HUD to show `cpu: —` for ~1 second after
    /// toggle-on. By keeping the baseline warm, toggle-on produces an
    /// instant percent on the very next tick — matching the UX of
    /// fps/p99/max/rss which all show data immediately.
    ///
    /// The cost is one `cpustat::current_cpu_ns()` call per second
    /// (~2 KiB `/proc` read on Linux, one syscall on macOS/BSD/Android)
    /// when the HUD is off — well under 0.1% CPU.
    ///
    /// When the underlying sampler returns `None` (non-unix targets, or
    /// a transient OS query failure), `cpu_percent` is set to `None` and
    /// the HUD renders `cpu: —` to honestly signal "metric unavailable"
    /// rather than misleadingly showing `0.00%`.
    ///
    /// ## Why not reuse `system_feeling.rs`?
    /// `system_feeling` is only active when `--auto-color-drift` is on.
    /// The HUD is independent (`i` toggles it any time) and must work
    /// without color drift. Decoupling also avoids sharing mutable state
    /// across subsystems on the hot frame path.
    #[inline]
    pub(crate) fn maybe_sample_cpu(&mut self) {
        // NOTE: samples at 1 Hz even when HUD is off, to keep the
        // baseline warm for instant percent on toggle-on.
        let now = Instant::now();
        if now.duration_since(self.last_cpu_sample) < HUD_CPU_INTERVAL {
            return;
        }
        let wall_delta = now.duration_since(self.last_cpu_sample);
        self.last_cpu_sample = now;

        let Some(cpu_ns_now) = cpustat::current_cpu_ns() else {
            // Sampler unsupported (non-unix) or transient OS failure.
            self.last_cpu_ns = None;
            self.cpu_percent = None;
            return;
        };

        match self.last_cpu_ns {
            None => {
                // First successful sample — establish baseline, no delta yet.
                // This only happens on the very first frame after process
                // start (not on toggle-on, because the baseline is kept
                // warm while the HUD is off).
                self.cpu_percent = None;
            }
            Some(prev_ns) => {
                let cpu_ns_delta = cpu_ns_now.saturating_sub(prev_ns);
                let wall_ns_delta = wall_delta.as_nanos() as u64;
                if wall_ns_delta == 0 {
                    // Degenerate: should never happen since we rate-limit
                    // to 1 Hz, but defend against division by zero.
                    self.cpu_percent = None;
                } else {
                    let pct = (cpu_ns_delta as f64 / wall_ns_delta as f64) * 100.0;
                    // Clamp to 999.99 for display width safety. On
                    // single-threaded builds this is effectively 0-100;
                    // multi-threaded spillover could exceed 100 briefly.
                    let clamped = pct.clamp(0.0, 999.99) as f32;
                    self.cpu_percent = Some(clamped);
                }
            }
        }
        self.last_cpu_ns = Some(cpu_ns_now);
    }

    /// Set the screen size for HUD display. Called by event_loop on init
    /// and resize. `is_fixed` = true when --screen-size was specified.
    pub(crate) fn set_screen_size(&mut self, w: u16, h: u16, is_fixed: bool) {
        self.screen_size = (w, h, is_fixed);
    }

    /// Recompute HUD metrics (rate-limited at 1 Hz). Called every frame
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

        // Recompute p99 from the ring buffer (stack-allocated sort,
        // ~300ns, called once per second).
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
        let fps_str = if fps >= 10_000.0 {
            crate::humanize::humanize_f64(fps)
        } else if fps >= 100.0 {
            format!("{fps:.0}")
        } else {
            format!("{fps:.1}")
        };
        self.cached_lines[0] = (head, format!(" fps: {fps_str}"));
        self.cached_lines[1] = (mid, format!(" p99: {:.3}ms", self.p99_ms));
        self.cached_lines[2] = (head, format!(" max: {:.3}ms", self.max_ms));
        self.cached_lines[3] = (trail, format!(" rss: {rss_str}"));
        // CPU% line: process CPU usage with 2-decimal precision.
        // Format: ` cpu: 0.45%` (single-threaded typical: 0-5%) or
        // ` cpu: —` when the sampler is unsupported (non-unix) or
        // waiting for the first delta to complete (first ~1s of process
        // lifetime only — baseline is kept warm while HUD is off, so
        // toggle-on shows instant percent).
        // The em dash is U+2014 (3 bytes UTF-8) but renders as 1 cell —
        // matches the existing `rss: —` fallback convention.
        //
        // Color: uses `mid` (palette_colors[n/2]) brightened — same as
        // the p99 line. This is intentional: cpu% is a metric the user
        // actively watches when investigating performance, so it
        // deserves a bright color. The `dim` color is reserved for
        // uptime/screensize which are informational only. Brightening
        // guarantees readability on dark rain palettes.
        let cpu_str = match self.cpu_percent {
            Some(pct) => format!("{pct:.2}%"),
            None => "—".to_string(),
        };
        self.cached_lines[4] = (mid, format!(" cpu: {cpu_str}"));
        self.cached_lines[5] = (dim, format!(" up: {uptime_str}"));
        let (sw, sh, is_fixed) = self.screen_size;
        let mode = if is_fixed { "fix" } else { "auto" };
        self.cached_lines[6] = (dim, format!(" {sw}x{sh} {mode}"));

        // Compute dynamic width: find the longest line, clamp to [min, max].
        let max_len = self
            .cached_lines
            .iter()
            .map(|(_, s)| s.chars().count())
            .max()
            .unwrap_or(HUD_MIN_WIDTH as usize) as u16;
        self.current_width = max_len.clamp(HUD_MIN_WIDTH, HUD_MAX_WIDTH);
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
    /// so the HUD is part of the same frame flush as the rain — this
    /// eliminates fullscreen flicker (two separate stdout writes were
    /// causing double-repaint in fullscreen mode).
    ///
    /// Uses frame.set() (not set_force) so cells that haven't changed
    /// since last frame are NOT marked dirty — the terminal skips
    /// re-sending them. This is the key overhead optimization: when
    /// metrics are stable (same fps/p99/max for 1s), only the
    /// changing cells (uptime seconds) get re-sent.
    pub(crate) fn write_to_frame(
        &self,
        frame: &mut crate::frame::Frame,
        cols: u16,
        bg: Option<Color>,
    ) {
        if !self.visible {
            return;
        }
        let w = self.current_width;
        let start_col = self.position.start_col(cols, w);
        for (i, (color, text)) in self.cached_lines.iter().enumerate() {
            let row = i as u16;
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
            // Pad the rest of the line with spaces to current_width
            // so the background covers the full HUD area consistently.
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

/// Brighten a crossterm Color by blending it with white.
/// Ensures HUD text is always readable on the black background,
/// even when the palette color is very dark (e.g. dark green trail).
///
/// ## Blend ratio
/// Uses an asymmetric blend that gives more weight to white than to
/// the source color — this guarantees readability even for very dark
/// rain palette colors (e.g. RGB(0,5,15) dark blue tail). The previous
/// 50/50 blend could still be too dim on some truecolor terminals
/// with low contrast curves. The current blend ensures every HUD line
/// stays clearly visible regardless of the active color scheme.
///
/// Non-RGB colors (AnsiValue, named) are returned as-is — they're
/// already bright enough in practice.
fn brighten_color(color: Color) -> Color {
    match color {
        Color::Rgb { r, g, b } => Color::Rgb {
            // Asymmetric blend: 35% source + 65% white. A pure black
            // RGB(0,0,0) becomes RGB(166,166,166) — clearly readable.
            // A dark RGB(10,5,20) becomes RGB(169,170,171).
            r: r * 35 / 100 + 166,
            g: g * 35 / 100 + 166,
            b: b * 35 / 100 + 166,
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
    fn hud_maybe_sample_cpu_keeps_baseline_warm_when_invisible() {
        // When the HUD is off, maybe_sample_cpu STILL samples at 1 Hz —
        // this is the warm-baseline design that lets toggle-on show an
        // instant percent (no `cpu: —` flash for 1 second).
        //
        // The cost is one syscall/sec when the HUD is off — well under
        // 0.1% CPU. This trade-off was explicitly requested by the user:
        // other metrics (fps/p99/rss) show data instantly on toggle-on,
        // and CPU must too.
        //
        // On unix platforms the sampler should produce Some(last_cpu_ns)
        // after a single call. On non-unix it stays None (sampler
        // unsupported). Both are valid per-platform outcomes.
        let mut h = HudState::new();
        h.maybe_sample_cpu();
        // We can't assert last_cpu_ns.is_some() unconditionally because
        // non-unix targets return None. But we CAN assert that the
        // function did NOT short-circuit on invisible — by checking that
        // last_cpu_sample was updated to ~now (i.e. the function ran to
        // completion past the visibility check).
        let now = Instant::now();
        let diff = now.duration_since(h.last_cpu_sample);
        assert!(
            diff.as_millis() < 1000,
            "maybe_sample_cpu must run even when invisible (warm baseline) — last_cpu_sample was not updated"
        );
    }

    #[test]
    fn hud_first_cpu_sample_establishes_baseline_only() {
        // On the very first CPU sample after HUD turns on, the function
        // must record the baseline ns but NOT compute a percent (no delta
        // yet). cpu_percent stays None and renders as `cpu: —`.
        let mut h = HudState::new();
        h.toggle(); // visible
        h.maybe_sample_cpu();
        // On supported platforms (unix), last_cpu_ns should now be Some.
        // On non-unix it stays None (sampler unsupported). Both are valid
        // per-platform outcomes — we just assert no percent is produced
        // (we can't compute a delta from one reading).
        assert!(
            h.cpu_percent.is_none(),
            "first CPU sample must not produce a percent (no delta yet)"
        );
    }

    #[test]
    fn hud_toggle_preserves_cpu_baseline_for_instant_reopen() {
        // When the HUD is toggled off then on again, the CPU baseline
        // must be PRESERVED (not cleared). This is the warm-baseline
        // design: maybe_sample_cpu samples at 1 Hz even while the HUD
        // is off, so on toggle-on we already have a recent baseline
        // and can compute an instant percent on the very next tick.
        //
        // Previously (commit ef8ab2a) the baseline was cleared on
        // toggle-on, forcing the HUD to show `cpu: —` for ~1 second.
        // The user explicitly flagged this as a UX inconsistency:
        // other metrics (fps/p99/rss) show data instantly, and CPU
        // must too.
        let mut h = HudState::new();
        h.toggle(); // on
        h.maybe_sample_cpu();
        // Stash the post-first-sample baseline (may be None on non-unix).
        let baseline_before_toggle_off = h.last_cpu_ns;
        // Toggle off then on.
        h.toggle(); // off
        h.toggle(); // on
        assert_eq!(
            h.last_cpu_ns, baseline_before_toggle_off,
            "toggling HUD on must PRESERVE the CPU baseline (warm-baseline design)"
        );
    }

    #[test]
    fn hud_cpu_line_renders_dash_when_unsupported() {
        // Verify the cached_lines[4] entry renders ` cpu: —` when
        // cpu_percent is None. This is the user-visible contract:
        // unsupported platforms (non-unix) and the brief pre-delta
        // window after HUD-on both show the em dash, not `0.00%`.
        let mut h = HudState::new();
        h.toggle(); // visible
                    // Force-update metrics without sampling — cpu_percent stays None.
                    // We need to bypass the rate-limit by directly calling update_metrics
                    // with an empty palette (the function recomputes cached_lines).
        h.update_metrics(&[]);
        assert!(
            h.cpu_percent.is_none(),
            "cpu_percent must be None before any sample"
        );
        // cached_lines[4] is the cpu line.
        let (_, cpu_line) = &h.cached_lines[4];
        assert!(
            cpu_line.contains('—'),
            "cpu line must render em dash when unsupported, got: {cpu_line:?}"
        );
    }

    #[test]
    fn hud_cpu_line_renders_percent_with_two_decimals_when_supported() {
        // Synthetic test: set cpu_percent directly (bypassing the sampler)
        // and verify update_metrics renders ` cpu: 12.34%` (2 decimals).
        // This locks in the display format independently of the sampler
        // behavior — if we later change to 1 decimal, this test fails.
        let mut h = HudState::new();
        h.toggle(); // visible
        h.cpu_percent = Some(12.3456); // should render as 12.35%
        h.update_metrics(&[]);
        let (_, cpu_line) = &h.cached_lines[4];
        assert!(
            cpu_line.contains("12.35%"),
            "cpu line must render 2-decimal percent, got: {cpu_line:?}"
        );
    }

    #[test]
    fn hud_has_seven_cached_lines_after_v30_cpu_addition() {
        // Regression guard: the HUD must have exactly 7 cached lines
        // (fps / p99 / max / rss / cpu / up / screensize). If a future
        // change adds or removes a line, this test will catch it.
        let h = HudState::new();
        assert_eq!(
            h.cached_lines.len(),
            7,
            "HUD must have 7 cached lines after the v30 CPU% addition"
        );
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
