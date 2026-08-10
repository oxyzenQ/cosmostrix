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
//! - **Dynamic palette colors**: HUD colors come from the active rain
//!   palette, hue-preserving brightened via HSV value scaling so the
//!   HUD follows the rain's actual color scheme (green rain → green
//!   HUD, amber rain → amber HUD) instead of washing out to grey.
//!   Color refresh is split out of the 1 Hz metric tick — `refresh_colors`
//!   runs every frame so a runtime palette change (`c`/`C` key, auto-color-
//!   drift, live-config reload) is reflected on the very next frame, with
//!   no perceptible delay. The 1 Hz rate limit only governs text
//!   reformatting (p99 sort, format! calls, RSS string).
//! - **Rain-aesthetic color gradient**: the HUD's 8 lines form a vertical
//!   brightness gradient that mirrors a falling rain droplet — the bottom
//!   line (screensize) is the brightest `head` (palette last-stop, the
//!   rain's leading bright character), the top line (fps) is the dimmest
//!   `tail` (palette index 1, the rain's trailing fade). Mid lines span
//!   `trail` and `mid` so the eye reads the HUD as a small rain column
//!   hanging in the corner, not as a flat block of text. This inverts
//!   the original mapping where `fps`/`tgt`/`max` were the brightest —
//!   the user explicitly flagged the inversion: 'rain tail is dim head
//!   is white' (head leads at the bottom of a falling stream).
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

/// Frame pacing mode announced by the event loop to the HUD.
///
/// v30 (2026-08-05): added so the HUD `tgt:` line can show whether the
/// user's configured --fps cap is actually in effect, or whether the loop
/// is currently running at the idle throttle (target_fps * IDLE_FPS_FACTOR)
/// or paused. Without this, `--fps 30` + 30s idle made the HUD `fps:` line
/// drop to ~15 with no explanation — the user had no way to tell whether
/// the renderer was broken or just idle-throttled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum FrameMode {
    /// Normal active rendering at `target_fps`.
    #[default]
    Active,
    /// Adaptive idle throttle is engaged (no input for IDLE_THRESHOLD_SECS).
    /// Effective FPS = target_fps * IDLE_FPS_FACTOR (typically 0.5x).
    Idle,
    /// User pressed Space/P to pause. Loop ticks at PAUSE_PERIOD_MS.
    Paused,
}

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
    /// Auto-resates every MAX_RESET_INTERVAL_SECS to prevent startup
    /// spikes from dominating forever.
    max_ms: f64,
    /// When max_ms was last reset. Used for auto-reset.
    max_reset_at: Instant,
    /// Cached p99 frame time (ms) for display. Updated at 1 Hz.
    p99_ms: f64,
    /// Target FPS as configured by --fps / config.toml `fps =`.
    /// Set via `set_target_fps()` from the event loop. Displayed as the
    /// `tgt:` line so the user can distinguish their configured cap from
    /// the `fps:` line (which is render-work throughput = 1000/work_ms,
    /// often much higher than the cap because the loop sleeps between
    /// frames). v30 (2026-08-05): added because users were confused that
    /// `--fps 30` produced `fps: 11000` in the HUD.
    target_fps: f64,
    /// Current frame pacing mode. Announced by the event loop each frame
    /// so the HUD can show whether the user's target_fps is actually in
    /// effect (active), throttled to idle_fps_factor (idle), or paused.
    /// v30 (2026-08-05): added alongside target_fps.
    frame_mode: FrameMode,
    /// Screen size for HUD display. Updated by event_loop when terminal
    /// resizes or --screen-size is set. Format: (width, height, is_fixed).
    screen_size: (u16, u16, bool),
    /// Cached display strings — reformatted only at 1 Hz, written to
    /// frame buffer every frame via write_to_frame().
    /// 8 lines: fps / tgt / p99 / max / rss / cpu / up / screensize.
    cached_lines: [(Color, String); 8],
    /// Current dynamic HUD width (in terminal columns). Recomputed
    /// every metric update to fit the longest line. Grows when FPS
    /// or RSS values are long, shrinks when they're short.
    current_width: u16,
    /// HB-01 (HUD residual 'e' bug fix): tracks the previous frame's
    /// `current_width` so `write_to_frame` can pad to `max(current_width,
    /// prev_width)`. Without this, when the `tgt:` line drops its ` idle`
    /// suffix on idle→active transition, the cell at the old column holds
    /// a residual char (visible `e` of `idle`) until rain passes through.
    prev_width: u16,
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
            // v30 (2026-08-05): default target_fps to 60.0 — the same default
            // as Args::fps. The event loop calls set_target_fps() at startup
            // with the resolved value (which may be lower on VSCode, etc.).
            target_fps: 60.0,
            frame_mode: FrameMode::Active,
            screen_size: (0, 0, false),
            cached_lines: [
                (Color::Cyan, String::new()),
                (Color::Cyan, String::new()), // tgt line — uses `dim` (tail) at runtime
                (Color::Yellow, String::new()),
                (Color::Magenta, String::new()),
                (Color::Green, String::new()),
                (Color::Cyan, String::new()), // cpu line — uses `mid` at runtime
                (Color::DarkCyan, String::new()),
                (Color::DarkCyan, String::new()),
            ],
            current_width: HUD_MIN_WIDTH,
            prev_width: HUD_MIN_WIDTH,
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

    /// Set the user-configured target FPS. Called by event_loop at startup
    /// with the resolved --fps / config.toml `fps =` value (after VSCode
    /// capping and any other terminal-cap adjustments). Displayed on the
    /// `tgt:` line so the user can distinguish their configured cap from
    /// the `fps:` line (render-work throughput, often much higher).
    /// v30 (2026-08-05): added to fix the "--fps 30 → HUD shows 11k fps"
    /// confusion — the user can now see `tgt: 30` alongside `fps: 11000`
    /// and understand the loop is sleeping to maintain the cap.
    pub(crate) fn set_target_fps(&mut self, fps: f64) {
        self.target_fps = fps;
    }

    /// Announce the current frame pacing mode. Called by event_loop every
    /// frame so the HUD can show whether `target_fps` is actually in effect
    /// (Active), throttled by the idle detector (Idle), or paused (Paused).
    /// v30 (2026-08-05): added alongside set_target_fps.
    pub(crate) fn set_frame_mode(&mut self, mode: FrameMode) {
        self.frame_mode = mode;
    }

    /// Refresh HUD line colors from the current palette. Called every
    /// frame when visible — cheap (4 `brighten_color` calls ≈ 2 µs) so
    /// the HUD tracks palette changes (`c`/`C` key cycle, auto-color-
    /// drift, live-config reload, scene transitions) on the very next
    /// frame, with no perceptible delay.
    ///
    /// ## Why this is split out of `update_metrics`
    /// Previously, colors were computed inside `update_metrics` (1 Hz
    /// rate-limited), so a runtime palette change took up to 1 second to
    /// appear in the HUD — the rain had already adopted the new palette
    /// while the HUD still showed the old colors. The owner explicitly
    /// flagged this as 'slight delay every owner changes colors at runtime'.
    /// Splitting the cheap color refresh out of the rate-limited metric
    /// path eliminates the delay: text reformatting (p99 sort, format!
    /// calls, RSS string) stays at 1 Hz to avoid number flicker, but
    /// colors update every frame.
    ///
    /// ## Rain-aesthetic gradient (top dim → bottom bright)
    /// The 8 HUD lines form a vertical brightness gradient that mirrors
    /// a falling rain droplet. In the rain visual, the leading character
    /// (the `head`) is the bright white at the BOTTOM of the stream, and
    /// the trailing fade (the `tail`) is dim at the TOP. The HUD adopts
    /// the same orientation:
    ///
    /// ```text
    ///   row 0  fps          ← dim      (tail — palette index 1)
    ///   row 1  tgt          ← dim
    ///   row 2  p99          ← trail    (palette index n/4)
    ///   row 3  max          ← trail
    ///   row 4  rss          ← mid      (palette index n/2)
    ///   row 5  cpu          ← mid
    ///   row 6  up           ← head     (palette last stop, brightest)
    ///   row 7  screensize   ← head     (rain head — leading white)
    /// ```
    ///
    /// This inverts the original mapping (where `fps`/`tgt`/`max` were
    /// brightest at the TOP). The owner explicitly flagged the inversion:
    /// 'rain tail is dim head is white' — the bright head must lead at
    /// the bottom, matching a real falling rain stream. The eye now reads
    /// the HUD as a small rain column hanging in the corner, not as a
    /// flat block of equally-bright text.
    ///
    /// ## Readability guarantee
    /// `brighten_color` applies HSV value scaling (TARGET_V = 200) to every
    /// palette stop, including the dim tail. This guarantees the dim lines
    /// are still readable on a black background — a dark green palette
    /// stop RGB(0,50,0) is boosted to RGB(0,200,0), preserving the hue
    /// while meeting the readability floor. Pure black falls back to a
    /// neutral grey RGB(120,120,120).
    #[inline]
    pub(crate) fn refresh_colors(&mut self, palette_colors: &[crossterm::style::Color]) {
        if !self.visible {
            return;
        }
        let (head, mid, trail, dim) = compute_rain_gradient(palette_colors);
        // Rain-aesthetic gradient: dim at top → head (bright) at bottom.
        // Each color level spans 2 consecutive lines for a smooth visual
        // gradient (4 levels × 2 lines = 8 lines). The bottom pair uses
        // `head` so the HUD reads as a rain column with the leading bright
        // character at the bottom — matching the rain's actual orientation.
        self.cached_lines[0].0 = dim; // fps         (top — tail)
        self.cached_lines[1].0 = dim; // tgt
        self.cached_lines[2].0 = trail; // p99
        self.cached_lines[3].0 = trail; // max
        self.cached_lines[4].0 = mid; // rss
        self.cached_lines[5].0 = mid; // cpu
        self.cached_lines[6].0 = head; // up
        self.cached_lines[7].0 = head; // screensize  (bottom — head)
    }

    /// Recompute HUD metrics (rate-limited at 1 Hz). Called every frame
    /// from the event loop. Cheap on the fast path (one timestamp
    /// comparison + early return). When the interval elapses, reformats
    /// the cached display strings.
    ///
    /// Note: this method ONLY reformats text. Color refresh is split out
    /// into `refresh_colors` (called every frame) so runtime palette
    /// changes appear on the very next frame instead of waiting up to 1
    /// second for the next metric tick. See `refresh_colors` docs for the
    /// rain-aesthetic gradient rationale (dim at top → bright head at
    /// bottom, mirroring a falling rain droplet).
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
        let (head, mid, trail, dim) = compute_rain_gradient(palette_colors);

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
        self.cached_lines[0] = (dim, format!(" fps: {fps_str}"));
        // v30 (2026-08-05): tgt line shows the user-configured --fps cap
        // alongside the current frame pacing mode. This disambiguates the
        // common confusion where `--fps 30` produces `fps: 11000` in the
        // HUD (because `fps:` is render-work throughput = 1000/work_ms,
        // not the loop's frame-period cap). The mode suffix tells the user
        // whether the cap is actually in effect:
        //   ` tgt: 30`        — active, loop targeting 30 FPS
        //   ` tgt: 30 idle`   — adaptive idle throttle engaged (effective ~15)
        //   ` tgt: 30 paused` — user pressed Space/P, loop ticking at 4 Hz
        // Format chosen to be compact (≤14 chars) so HUD width stays ≤22.
        let tgt_str = if self.target_fps >= 100.0 {
            format!("{:.0}", self.target_fps)
        } else {
            format!("{:.1}", self.target_fps)
        };
        let mode_suffix = match self.frame_mode {
            FrameMode::Active => String::new(),
            FrameMode::Idle => " idle".to_string(),
            FrameMode::Paused => " paused".to_string(),
        };
        self.cached_lines[1] = (dim, format!(" tgt: {tgt_str}{mode_suffix}"));
        self.cached_lines[2] = (trail, format!(" p99: {:.3}ms", self.p99_ms));
        self.cached_lines[3] = (trail, format!(" max: {:.3}ms", self.max_ms));
        self.cached_lines[4] = (mid, format!(" rss: {rss_str}"));
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
        self.cached_lines[5] = (mid, format!(" cpu: {cpu_str}"));
        self.cached_lines[6] = (head, format!(" up: {uptime_str}"));
        let (sw, sh, is_fixed) = self.screen_size;
        let mode = if is_fixed { "fix" } else { "auto" };
        self.cached_lines[7] = (head, format!(" {sw}x{sh} {mode}"));

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

/// Compute the 4-stop rain-aesthetic gradient (dim, trail, mid, head)
/// from the active palette, hue-preserving brightened for readability.
///
/// Returns the colors in **rain-tail → rain-head order** (dim → brightest):
/// - `dim`   — palette index 1 (rain tail, dimmest)
/// - `trail` — palette index `n/4`
/// - `mid`   — palette index `n/2` (palette body — the saturated hue
///   the eye reads as "the rain color")
/// - `head`  — palette last stop (rain head — leading bright white)
///
/// Callers assign these to HUD rows in **bottom-brightest order** to mirror
/// a falling rain droplet: `dim` at the top (row 0), `head` at the bottom
/// (row 7). See `HudState::refresh_colors` for the full rationale.
///
/// The palette ordering matches `crate::palette::Palette::colors`, which is
/// constructed dim-first → bright-last by `build_palette`. Picking by index
/// (not by sorted brightness) preserves the palette author's intended hue
/// progression — a `Cosmos` palette goes dim-blue → bright-white, and the
/// HUD gradient mirrors that exact progression rather than a re-sorted one.
///
/// ## Why a helper function
/// Both `refresh_colors` (every frame) and `update_metrics` (1 Hz tick)
/// need the same 4-stop gradient. Centralizing the index math + brighten
/// calls ensures they stay in sync — if one path is changed (e.g. a new
/// palette stop is added), the other updates automatically. The cost is
/// 4 `brighten_color` calls (≈ 2 µs total), acceptable on the per-frame
/// path.
fn compute_rain_gradient(
    palette_colors: &[crossterm::style::Color],
) -> (Color, Color, Color, Color) {
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
    (head, mid, trail, dim)
}

/// Boost a color's brightness while preserving its hue, so the HUD
/// follows the rain's actual color scheme instead of washing out to grey.
///
/// ## Why hue-preserving scaling (not white blend)
/// The previous implementation blended 35% source + 65% white, which
/// desaturated every color toward grey — a green rain produced a
/// grey-green HUD, an amber rain produced a washed-out amber. The user
/// explicitly flagged this: "HUD metrics colors too grey should be
/// dynamic follow the rain not hardcoded grey".
///
/// The new implementation uses HSV-style value scaling:
/// 1. Convert any Color variant to RGB via `palette::color_to_rgb`
///    (so AnsiValue + named colors also get processed — previously
///    they were returned as-is, which meant a 256-color palette stayed
///    at its native brightness even when too dim to read).
/// 2. Find the max channel (V in HSV).
/// 3. If V >= TARGET_V, the color is already bright enough — return
///    as-is to preserve the rain's vivid hue.
/// 4. If V < TARGET_V and V > 0, scale all channels by TARGET_V / V.
///    This preserves the hue ratio between channels — a dark green
///    RGB(0,50,0) becomes RGB(0,200,0), not a washed-out grey-green.
/// 5. If V == 0 (pure black), fall back to a neutral dim grey.
///    Scaling zero gives zero, so we need an explicit fallback.
///
/// TARGET_V = 200 ensures readability on a black background without
/// oversaturating. A vivid RGB(0,255,0) green is returned unchanged;
/// a dim RGB(0,80,0) green is boosted to RGB(0,200,0).
fn brighten_color(color: Color) -> Color {
    let (r, g, b) = crate::palette::color_to_rgb(color);
    const TARGET_V: u32 = 200;
    let max = r.max(g).max(b) as u32;
    if max >= TARGET_V {
        // Already bright enough — preserve the rain's vivid hue.
        Color::Rgb { r, g, b }
    } else if max == 0 {
        // Pure black — scaling zero gives zero, so fall back to a
        // neutral dim grey. This is the only case where we don't
        // preserve hue (there's no hue to preserve).
        Color::Rgb {
            r: 120,
            g: 120,
            b: 120,
        }
    } else {
        // Scale all channels by TARGET_V / max to boost brightness
        // while preserving the hue ratio between channels.
        // Uses integer math: scale = TARGET_V * 100 / max, then
        // (channel * scale) / 100. Min(255) guards against overflow
        // when the source channel is close to max but max < TARGET_V.
        //
        // SAFETY: max > 0 here because the `else if max == 0` branch
        // above caught the zero case. The debug_assert documents this
        // invariant for readers and catches logic regressions in dev
        // builds.
        debug_assert!(max > 0, "max must be > 0 here; zero case handled above");
        let scale = TARGET_V * 100 / max;
        Color::Rgb {
            r: ((r as u32 * scale) / 100).min(255) as u8,
            g: ((g as u32 * scale) / 100).min(255) as u8,
            b: ((b as u32 * scale) / 100).min(255) as u8,
        }
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
        // Verify the cached_lines[5] entry renders ` cpu: —` when
        // cpu_percent is None. This is the user-visible contract:
        // unsupported platforms (non-unix) and the brief pre-delta
        // window after HUD-on both show the em dash, not `0.00%`.
        // (v30 2026-08-05: index shifted from [4] to [5] because a new
        // `tgt:` line was inserted at index [1].)
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
        // cached_lines[5] is the cpu line (after v30 tgt line insertion).
        let (_, cpu_line) = &h.cached_lines[5];
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
        // (v30 2026-08-05: index shifted from [4] to [5] because a new
        // `tgt:` line was inserted at index [1].)
        let mut h = HudState::new();
        h.toggle(); // visible
        h.cpu_percent = Some(12.3456); // should render as 12.35%
        h.update_metrics(&[]);
        let (_, cpu_line) = &h.cached_lines[5];
        assert!(
            cpu_line.contains("12.35%"),
            "cpu line must render 2-decimal percent, got: {cpu_line:?}"
        );
    }

    #[test]
    fn hud_has_eight_cached_lines_after_v30_tgt_addition() {
        // Regression guard: the HUD must have exactly 8 cached lines
        // after the v30 (2026-08-05) addition of the `tgt:` line at
        // index [1]. The previous count was 7 (fps / p99 / max / rss /
        // cpu / up / screensize); now 8 (fps / tgt / p99 / max / rss /
        // cpu / up / screensize). If a future change adds or removes a
        // line, this test will catch it.
        let h = HudState::new();
        assert_eq!(
            h.cached_lines.len(),
            8,
            "HUD must have 8 cached lines after the v30 tgt-line addition"
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

    // ── Hue-preserving brighten_color tests ───────────────────────────
    //
    // The HUD must follow the rain's actual color scheme, not wash out
    // to grey. These tests lock in the hue-preserving behavior so a
    // future change back to a white-blend would fail loudly.

    #[test]
    fn brighten_color_preserves_vivid_green_hue() {
        // Vivid green RGB(0,255,0): max=255 >= TARGET_V(200), returned
        // as-is. The HUD line for this palette color must be vivid green,
        // not washed-out grey-green.
        let out = brighten_color(Color::Rgb { r: 0, g: 255, b: 0 });
        assert_eq!(out, Color::Rgb { r: 0, g: 255, b: 0 });
    }

    #[test]
    fn brighten_color_preserves_vivid_amber_hue() {
        // Amber/orange RGB(255,176,0): max=255 >= TARGET_V, returned as-is.
        // An amber rain palette must produce an amber HUD, not grey.
        let out = brighten_color(Color::Rgb {
            r: 255,
            g: 176,
            b: 0,
        });
        assert_eq!(
            out,
            Color::Rgb {
                r: 255,
                g: 176,
                b: 0
            }
        );
    }

    #[test]
    fn brighten_color_scales_dark_green_preserving_hue() {
        // Dark green RGB(0,50,0): max=50 < TARGET_V, scale=400.
        // Result must be RGB(0,200,0) — bright green, NOT grey-green.
        // The old white-blend produced RGB(166,183,166) (washed grey).
        let out = brighten_color(Color::Rgb { r: 0, g: 50, b: 0 });
        assert_eq!(out, Color::Rgb { r: 0, g: 200, b: 0 });
    }

    #[test]
    fn brighten_color_scales_dark_blue_preserving_hue_ratio() {
        // Dark blue RGB(50,100,150): max=150 < TARGET_V, scale=133
        // (integer: 200*100/150=133, truncated from 133.33).
        // Result: RGB(66,133,199) — preserves the blue hue ratio.
        // The old white-blend produced RGB(183,201,218) (washed grey-blue).
        // (199 not 200 because 150*133/100=199.5 → truncates to 199.)
        let out = brighten_color(Color::Rgb {
            r: 50,
            g: 100,
            b: 150,
        });
        assert_eq!(
            out,
            Color::Rgb {
                r: 66,
                g: 133,
                b: 199
            }
        );
    }

    #[test]
    fn brighten_color_pure_black_falls_back_to_neutral_grey() {
        // Pure black RGB(0,0,0): max=0, can't scale (0*x=0). Must fall
        // back to a neutral dim grey RGB(120,120,120) so the HUD is
        // still readable. This is the only case where hue is not
        // preserved (there's no hue to preserve in pure black).
        let out = brighten_color(Color::Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(
            out,
            Color::Rgb {
                r: 120,
                g: 120,
                b: 120
            }
        );
    }

    #[test]
    fn brighten_color_named_cyan_preserves_hue_when_bright_enough() {
        // Named Cyan = RGB(0,255,255): max=255 >= TARGET_V, returned as
        // RGB(0,255,255). The old code returned named colors as-is (no
        // conversion), which was fine for Cyan but broke for DarkCyan
        // (next test). This test locks in the conversion behavior.
        let out = brighten_color(Color::Cyan);
        assert_eq!(
            out,
            Color::Rgb {
                r: 0,
                g: 255,
                b: 255
            }
        );
    }

    #[test]
    fn brighten_color_named_darkcyan_gets_scaled_to_readable_cyan() {
        // Named DarkCyan = RGB(0,128,128): max=128 < TARGET_V, scale=156
        // (integer: 200*100/128=156, truncated from 156.25).
        // Result: RGB(0,199,199) — bright cyan, preserving the hue.
        // (199 not 200 because 128*156/100=199.68 → truncates to 199.)
        // The old code returned DarkCyan as-is (too dim on black bg).
        let out = brighten_color(Color::DarkCyan);
        assert_eq!(
            out,
            Color::Rgb {
                r: 0,
                g: 199,
                b: 199
            }
        );
    }

    #[test]
    fn brighten_color_does_not_wash_vivid_colors_to_grey() {
        // Regression guard: the user explicitly flagged "HUD metrics
        // colors too grey". The old 35% source + 65% white blend turned
        // RGB(0,255,0) into RGB(89,255,89) — a washed pale green. The
        // new code must return vivid colors unchanged. Verify the green
        // channel is NOT reduced and the red/blue channels stay at 0.
        let out = brighten_color(Color::Rgb { r: 0, g: 255, b: 0 });
        match out {
            Color::Rgb { r, g, b } => {
                assert_eq!(r, 0, "red channel must stay 0 for pure green");
                assert_eq!(b, 0, "blue channel must stay 0 for pure green");
                assert_eq!(g, 255, "green channel must stay 255 (not washed)");
            }
            other => panic!("expected Rgb, got {other:?}"),
        }
    }

    // ── refresh_colors + rain-aesthetic gradient tests ────────────────
    //
    // The HUD color refresh was split out of the 1 Hz `update_metrics`
    // tick so runtime palette changes (c/C key, auto-color-drift,
    // live-config reload) appear on the very next frame, not up to 1
    // second later. The gradient was also inverted to follow the rain
    // aesthetic: dim tail at top → bright head at bottom. These tests
    // lock in both behaviors.

    #[test]
    fn refresh_colors_is_noop_when_invisible() {
        // When the HUD is off, refresh_colors must short-circuit and
        // leave cached_lines untouched. This matches the zero-cost-when-off
        // design constraint — even the cheap 4-brighten_color calls are
        // skipped.
        let mut h = HudState::new();
        // Capture the initial colors (the defaults from new()).
        let initial_colors: Vec<_> = h.cached_lines.iter().map(|(c, _)| *c).collect();
        // Pass a non-empty palette — if refresh_colors ran, it would
        // overwrite the defaults with brightened palette colors.
        let palette = vec![
            Color::Rgb { r: 0, g: 50, b: 0 },
            Color::Rgb { r: 0, g: 255, b: 0 },
        ];
        h.refresh_colors(&palette);
        let after_colors: Vec<_> = h.cached_lines.iter().map(|(c, _)| *c).collect();
        assert_eq!(
            initial_colors, after_colors,
            "refresh_colors must be a no-op when HUD is invisible"
        );
    }

    #[test]
    fn refresh_colors_updates_colors_without_touching_text() {
        // When the HUD is visible, refresh_colors must update ONLY the
        // Color half of each cached_lines tuple — the String half (text)
        // is owned by the 1 Hz `update_metrics` tick and must be preserved
        // between ticks. This is what enables instant color refresh on
        // palette change without re-running the expensive format! calls.
        let mut h = HudState::new();
        h.toggle(); // visible
                    // Seed the cached_lines with sentinel text so we can verify it
                    // survives refresh_colors. (In production, update_metrics writes
                    // the real text — here we just verify refresh_colors doesn't
                    // touch it.)
        for slot in &mut h.cached_lines {
            slot.1 = "SENTINEL".to_string();
        }
        let palette = vec![
            Color::Rgb { r: 0, g: 50, b: 0 },  // dim (idx 1 = trail-ish)
            Color::Rgb { r: 0, g: 200, b: 0 }, // mid (n/2)
            Color::Rgb { r: 0, g: 255, b: 0 }, // head (last)
        ];
        h.refresh_colors(&palette);
        for (i, (color, text)) in h.cached_lines.iter().enumerate() {
            assert_eq!(
                text, "SENTINEL",
                "refresh_colors must NOT touch text (line {i}), got: {text:?}"
            );
            // Every color must have been overwritten to a non-default value.
            // The default new() colors include Color::Cyan, Color::Yellow,
            // Color::Magenta, Color::Green, Color::DarkCyan. After refresh,
            // all should be Rgb (brighten_color always returns Rgb except
            // for the named-color conversion path which still returns Rgb).
            assert!(
                matches!(color, Color::Rgb { .. }),
                "refresh_colors must overwrite line {i} color to Rgb, got: {color:?}"
            );
        }
    }

    #[test]
    fn refresh_colors_assigns_dim_to_top_and_head_to_bottom() {
        // Rain-aesthetic gradient: the BOTTOM row (screensize, idx 7) must
        // be the brightest `head` color (palette last stop), and the TOP
        // row (fps, idx 0) must be the dimmest `dim` color (palette idx 1).
        // This inverts the original mapping where fps/tgt/max were brightest
        // at the top — the owner explicitly flagged the inversion: 'rain
        // tail is dim head is white' (head leads at the bottom of a falling
        // stream).
        //
        // We use a palette where head is pure white RGB(255,255,255) so the
        // assertion is unambiguous, and dim is a dark green RGB(0,50,0)
        // that brightens to RGB(0,200,0). The bottom row must be white
        // (head); the top row must be green (dim).
        let mut h = HudState::new();
        h.toggle();
        let palette = vec![
            Color::Rgb { r: 0, g: 50, b: 0 },  // idx 0 (unused by gradient)
            Color::Rgb { r: 0, g: 50, b: 0 },  // idx 1 → dim
            Color::Rgb { r: 0, g: 100, b: 0 }, // idx 2 (n/4 = 1 for n=4) → trail
            Color::Rgb { r: 0, g: 200, b: 0 }, // idx 3 (n/2 = 2) → mid
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            }, // idx 4 (last) → head (white)
        ];
        h.refresh_colors(&palette);
        // Top row (fps, idx 0) = dim = RGB(0, 200, 0) (brightened from 0,50,0)
        assert_eq!(
            h.cached_lines[0].0,
            Color::Rgb { r: 0, g: 200, b: 0 },
            "top row (fps) must use `dim` (palette idx 1, brightened) — rain tail at top"
        );
        // Bottom row (screensize, idx 7) = head = RGB(255, 255, 255)
        assert_eq!(
            h.cached_lines[7].0,
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255
            },
            "bottom row (screensize) must use `head` (palette last stop) — rain head at bottom"
        );
        // The middle rows should NOT be white — they should be the
        // brightened trail/mid greens, not the head.
        assert_ne!(
            h.cached_lines[2].0,
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255
            },
            "row 2 (p99) must NOT use head — only bottom 2 rows get head"
        );
        assert_ne!(
            h.cached_lines[4].0,
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255
            },
            "row 4 (rss) must NOT use head — only bottom 2 rows get head"
        );
    }

    #[test]
    fn refresh_colors_picks_up_runtime_palette_change_immediately() {
        // This is the core regression test for the "slight delay" bug:
        // when the palette changes at runtime (c/C key, auto-color-drift,
        // live-config reload), refresh_colors must pick up the new palette
        // on the very next call — no rate limit, no 1-second wait.
        //
        // We simulate a palette change by calling refresh_colors twice
        // with different palettes and verifying the HUD colors update
        // on each call. With the old design (colors inside the 1 Hz
        // update_metrics), the second call would have been a no-op
        // because the rate limit hadn't elapsed.
        let mut h = HudState::new();
        h.toggle();
        // First palette: green head.
        let green_palette = vec![
            Color::Rgb { r: 0, g: 50, b: 0 },
            Color::Rgb { r: 0, g: 50, b: 0 },
            Color::Rgb { r: 0, g: 255, b: 0 },
        ];
        h.refresh_colors(&green_palette);
        let color_after_green = h.cached_lines[7].0; // bottom = head
        assert_eq!(
            color_after_green,
            Color::Rgb { r: 0, g: 255, b: 0 },
            "first refresh: bottom row must be green head"
        );
        // Second palette: amber head. NO time has elapsed — if refresh_colors
        // were rate-limited, this would be a no-op and the color would stay
        // green. With the split-out design, the color must update immediately.
        let amber_palette = vec![
            Color::Rgb { r: 50, g: 25, b: 0 },
            Color::Rgb { r: 50, g: 25, b: 0 },
            Color::Rgb {
                r: 255,
                g: 176,
                b: 0,
            },
        ];
        h.refresh_colors(&amber_palette);
        let color_after_amber = h.cached_lines[7].0;
        assert_eq!(
            color_after_amber,
            Color::Rgb {
                r: 255,
                g: 176,
                b: 0
            },
            "second refresh (immediate, no rate-limit): bottom row must update to amber head — \
             this is the core 'no delay on runtime palette change' guarantee"
        );
    }

    #[test]
    fn refresh_colors_gradient_uses_four_distinct_levels() {
        // The 8 HUD rows use 4 color levels (dim, trail, mid, head), each
        // spanning 2 consecutive rows. This test verifies the 4-level
        // structure by using a palette where each level is a distinct,
        // identifiable color, and asserting that:
        //   rows 0,1 == dim
        //   rows 2,3 == trail
        //   rows 4,5 == mid
        //   rows 6,7 == head
        //
        // Palette layout (8 stops, n=8) — chosen so the gradient indices
        // (1, n/4=2, n/2=4, last=7) are all distinct:
        //   idx 1 → dim    (distinct color: red)
        //   idx 2 → trail  (distinct color: green)
        //   idx 4 → mid    (distinct color: blue)
        //   idx 7 → head   (white)
        // All palette colors are already at TARGET_V or above, so
        // brighten_color returns them unchanged — this makes the
        // assertions exact (no integer-math surprise).
        let mut h = HudState::new();
        h.toggle();
        let mut palette = vec![Color::Rgb { r: 0, g: 0, b: 0 }; 8]; // idx 0 unused
        palette[1] = Color::Rgb { r: 200, g: 0, b: 0 }; // dim — already bright
        palette[2] = Color::Rgb { r: 0, g: 200, b: 0 }; // trail — already bright
        palette[4] = Color::Rgb { r: 0, g: 0, b: 200 }; // mid — already bright
        palette[7] = Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }; // head — white
        h.refresh_colors(&palette);
        // Pairs must match within each level.
        assert_eq!(
            h.cached_lines[0].0, h.cached_lines[1].0,
            "rows 0,1 must share `dim`"
        );
        assert_eq!(
            h.cached_lines[2].0, h.cached_lines[3].0,
            "rows 2,3 must share `trail`"
        );
        assert_eq!(
            h.cached_lines[4].0, h.cached_lines[5].0,
            "rows 4,5 must share `mid`"
        );
        assert_eq!(
            h.cached_lines[6].0, h.cached_lines[7].0,
            "rows 6,7 must share `head`"
        );
        // And the 4 levels must be distinct from each other.
        let dim = h.cached_lines[0].0;
        let trail = h.cached_lines[2].0;
        let mid = h.cached_lines[4].0;
        let head = h.cached_lines[6].0;
        assert_ne!(dim, trail, "dim and trail must be different colors");
        assert_ne!(trail, mid, "trail and mid must be different colors");
        assert_ne!(mid, head, "mid and head must be different colors");
        // Verify the actual colors match what we put in the palette.
        assert_eq!(
            dim,
            Color::Rgb { r: 200, g: 0, b: 0 },
            "dim must be palette idx 1 (red)"
        );
        assert_eq!(
            trail,
            Color::Rgb { r: 0, g: 200, b: 0 },
            "trail must be palette idx 2 (green)"
        );
        assert_eq!(
            mid,
            Color::Rgb { r: 0, g: 0, b: 200 },
            "mid must be palette idx 4 (blue)"
        );
        assert_eq!(
            head,
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255
            },
            "head must be palette idx 7 (white)"
        );
    }

    #[test]
    fn compute_rain_gradient_returns_dim_to_head_order() {
        // The helper returns (head, mid, trail, dim) in that tuple order
        // (head first, dim last) — callers unpack as `let (head, mid, trail, dim) = ...`.
        // This test verifies the tuple ordering is correct and the 4 stops
        // are picked from the right palette indices.
        let palette = vec![
            Color::Rgb { r: 0, g: 0, b: 0 },  // idx 0 (unused)
            Color::Rgb { r: 50, g: 0, b: 0 }, // idx 1 → dim
            Color::Rgb { r: 0, g: 50, b: 0 }, // idx 2 → trail (n/4 = 2 for n=8)
            Color::Rgb { r: 0, g: 0, b: 50 }, // idx 3 (unused)
            Color::Rgb {
                r: 100,
                g: 100,
                b: 0,
            }, // idx 4 → mid (n/2 = 4)
            Color::Rgb { r: 0, g: 0, b: 0 },  // idx 5 (unused)
            Color::Rgb { r: 0, g: 0, b: 0 },  // idx 6 (unused)
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            }, // idx 7 → head (last)
        ];
        let (head, mid, trail, dim) = compute_rain_gradient(&palette);
        // head = palette[7] = white (already bright, returned as-is).
        assert_eq!(
            head,
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
        // mid = palette[4] = RGB(100, 100, 0) — max=100 < TARGET_V(200),
        // scale = 200, result = RGB(200, 200, 0).
        assert_eq!(
            mid,
            Color::Rgb {
                r: 200,
                g: 200,
                b: 0
            }
        );
        // trail = palette[2] = RGB(0, 50, 0) — brightened to RGB(0, 200, 0).
        assert_eq!(trail, Color::Rgb { r: 0, g: 200, b: 0 });
        // dim = palette[1] = RGB(50, 0, 0) — brightened to RGB(200, 0, 0).
        assert_eq!(dim, Color::Rgb { r: 200, g: 0, b: 0 });
    }

    // HB-01 regression test: HUD width shrink (e.g., tgt drops " idle" suffix)
    // must clear the previously-occupied trailing cells immediately.
    #[test]
    fn hud_write_to_frame_clears_trailing_cells_when_width_shrinks() {
        let mut h = HudState::new();
        h.toggle(); // make visible
                    // 14-char string ending in "idle" (mirrors owner repro: " tgt: 144 idle").
        h.cached_lines[1].1 = " tgt: 144 idle".to_string();
        h.current_width = 14;
        h.prev_width = 14;

        let cols = 40u16;
        let mut frame = crate::frame::Frame::new(cols, 8, None);
        h.write_to_frame(&mut frame, cols, None);

        assert!(
            frame.get(13, 1).is_some(),
            "precondition: cell (13,1) must exist after wide write"
        );

        h.cached_lines[1].1 = " tgt: 144".to_string();
        h.current_width = 13; // shrunk

        h.write_to_frame(&mut frame, cols, None);

        let cleared = frame.get(13, 1).expect("cell must exist after shrink");
        assert_eq!(
            cleared.ch, ' ',
            "cell (13,1) must be blanked — was residual 'e' bug"
        );
        assert!(cleared.fg.is_none(), "cell (13,1) fg must be None");
    }
}
