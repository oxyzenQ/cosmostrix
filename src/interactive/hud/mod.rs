// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: HudState struct + 24-row cached_lines + setters + refresh_colors
// + DirtyCellTracker — the HUD is a single cohesive dashboard with 24 coupled
// metric rows. Splitting would scatter the struct fields and their invariants
// across files (each row's setter + render + color are tightly coupled).

//! Live HUD overlay for interactive mode.
//!
//! Toggle with `i`. When visible, writes a 24-row metric dashboard into
//! the frame buffer (before `term.draw()`): the health core (fps / tgt /
//! max / p99 / cpu / rss / ehs / prs), identity lines (scn / chr / clr),
//! user-adjustable controls (sped / dsty), dragon + tuning state (prdr /
//! crdr / ambt / glth / ctun / mnst), cell efficiency (dcel / tcel), and
//! the session footer (cid / up / screensize).
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
//!   runs every frame so a runtime palette change (`c`/`C` key, Crystal Dragon
//!   drift, live-config reload) is reflected on the very next frame, with
//!   no perceptible delay. The 1 Hz rate limit only governs text
//!   reformatting (p99 sort, format! calls, RSS string).
//! - **Rain-aesthetic color gradient**: the HUD's 24 lines form a vertical
//!   brightness gradient that mirrors a falling rain droplet — the bottom
//!   line (screensize, the terminal-size anchor) is the brightest `head`
//!   (palette last-stop, the rain's leading bright character), the top
//!   line (fps) is the dimmest `tail` (palette index 1, the rain's
//!   trailing fade). Mid lines span `trail` and `mid` so the eye reads
//!   the HUD as a small rain column hanging in the corner, not as a
//!   flat block of text. This inverts the original mapping where
//!   `fps`/`tgt`/`max` were the brightest — the user explicitly flagged
//!   the inversion: 'rain tail is dim head is white' (head leads at the
//!   bottom of a falling stream).
//! - **Row order (v80.0.0-beta.1 + Z-master-1X round 5)**: fps / tgt /
//!   max / p99 / cpu / rss / ehs / prs / scn / chr / clr / sped / dsty /
//!   prdr / crdr / ambt / glth / ctun / mnst / dcel / tcel / cid / up /
//!   screensize — identity lines above the controls, cell efficiency above
//!   the session footer at the bottom.
//! - **Auto-reset max**: max_ms resets every 60s to show recent peaks,
//!   not a startup spike from 10 minutes ago.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use crate::cpustat;
use crate::interactive::activity::FrameTimeTracker;
use crate::memstat;
use crate::runtime::ColorScheme;

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
const HUD_MAX_WIDTH: u16 = 24;

/// Z-master-1X round 5: rolling dirty-cell tracker for the dcel/tcel HUD
/// metrics. Stores the last 60 frames of (dirty_count, total_cells) so the
/// 1 Hz metric tick can render a rolling-average dirty cell ratio + the
/// current total cell count. Mirrors the `FrameTimeTracker` ring buffer
/// pattern (fixed 60-slot stack array, no allocation). Paused frames do
/// not push — matches the `push_frame_time` pause-freeze contract.
struct DirtyCellTracker {
    dirty: [u64; 60],
    total: [u64; 60],
    index: usize,
    count: usize,
}

impl DirtyCellTracker {
    const fn new() -> Self {
        Self {
            dirty: [0; 60],
            total: [0; 60],
            index: 0,
            count: 0,
        }
    }

    fn push(&mut self, dirty_count: u64, total_cells: u64) {
        self.dirty[self.index] = dirty_count;
        self.total[self.index] = total_cells;
        self.index = (self.index + 1) % 60;
        if self.count < 60 {
            self.count += 1;
        }
    }

    /// Rolling-average dirty cell count over the window.
    fn rolling_avg_dirty(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let sum: u64 = self.dirty[..self.count].iter().sum();
        sum as f64 / self.count as f64
    }

    /// Most recent total cell count (total cells is screen-size-driven and
    /// stable — no need for an average, just the latest sample).
    fn latest_total(&self) -> u64 {
        if self.count == 0 {
            return 0;
        }
        // The latest sample is at (index - 1 + 60) % 60, clamped to count.
        let latest_idx = if self.index == 0 {
            self.count - 1
        } else {
            self.index - 1
        };
        self.total[latest_idx]
    }
}

/// Frame pacing mode announced by the event loop to the HUD.
///
/// v30 (2026-08-05): added so the HUD `tgt:` line can show whether the
/// user's configured --fps cap is actually in effect, or whether the loop
/// is currently running at the idle throttle (target_fps * IDLE_FPS_FACTOR)
/// or paused. Without this, `--fps 30` + 30s idle made the HUD `fps:` line
/// drop to ~15 with no explanation — the user had no way to tell whether
/// the renderer was broken or just idle-throttled.
///
/// S-master-HUNT-23: `Drain` announces the output drain backoff — the
/// terminal cannot drain our ANSI rate, so effective_fps is scaled toward
/// the terminal's sustainable cadence (see PowerManager::drain_backoff).
/// Shows on `tgt:` as e.g. `tgt: 36 drain`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum FrameMode {
    /// Normal active rendering at `target_fps`.
    #[default]
    Active,
    /// Adaptive idle throttle is engaged (no input for IDLE_THRESHOLD_SECS).
    /// Effective FPS = target_fps * IDLE_FPS_FACTOR (typically 0.5x).
    Idle,
    /// User pressed `p` to pause. Loop ticks at PAUSE_PERIOD_MS.
    Paused,
    /// Output drain backoff engaged (HUNT-23): the terminal's measured
    /// write latency exceeds its frame budget, so the cadence is scaled
    /// down toward the drain rate.
    Drain,
}

/// Live HUD overlay state.
pub(crate) struct HudState {
    visible: bool,
    /// Session start time for uptime display.
    session_start: Instant,
    /// Pause-freeze tracking (owner bug fix 2026-08-30): while paused,
    /// uptime stops counting and samplers stop sampling; the segment +
    /// total exclude paused time from `up:` with sub-second precision.
    /// See `set_metrics_paused()` in metrics.rs.
    metrics_paused: bool,
    pause_started_at: Option<Instant>,
    paused_total: Duration,
    frame_times: FrameTimeTracker,
    /// Z-master-1X round 5: rolling dirty-cell tracker for the dcel/tcel
    /// HUD metrics. Pushed every frame via `set_dirty_cell_stats` (called
    /// after sim_draw in the event loop, when the dirty count is known).
    /// The rolling average (60-frame window, matching `frame_times`) is
    /// rendered at the 1 Hz metric tick as `dcel:` (dirty cell ratio %)
    /// and `tcel:` (total cells). Paused frames do not push (matches the
    /// `push_frame_time` pause-freeze contract).
    dirty_cell_tracker: DirtyCellTracker,
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
    // v50 (2026-08-17) HUD expansion — 7 new owner-mandated metrics.
    // All fed by setters called from the event loop whenever the value
    // changes (key press, scene cycle, color cycle, config reload) or
    // sampled every frame at 1 Hz (effective_pressure, ehs). The text
    // is rendered into cached_lines[6..=12] on the 1 Hz metric tick
    // (v80.0.0-beta.1 order: ehs/prs rows 6-7, scn/chr/clr rows 8-10, sped/dsty rows 11-12).
    /// Active scene name (e.g. "cinematic", "matrix", custom). Drives
    /// the `scn:` HUD line so the owner sees confirmation when cycling
    /// scenes with `x` — previously the user had to guess from visuals.
    scene_name: String,
    /// Active color scheme (e.g. NeonGreen, FancyDiamond). Rendered via
    /// Debug format (matches `verbose.rs` convention). Drives the `clr:`
    /// HUD line so the owner sees confirmation when cycling colors with
    /// `c` / `C`. When a custom palette is active, `custom_palette_name`
    /// is set and takes priority over the builtin scheme name.
    color_scheme: ColorScheme,
    /// Active custom palette name (e.g. "cyberpunk_2077", "tron_legacy").
    /// When `Some`, the `clr:` HUD line shows this name instead of the
    /// builtin `ColorScheme` Debug format. When `None` (no custom palette
    /// loaded, or user cycled to a builtin scheme), the Debug format is
    /// used as before. Set by event_loop when --colors-custom is active.
    custom_palette_name: Option<String>,
    /// Active charset preset name (e.g. "binary", "zen", custom). Drives
    /// the `chr:` HUD line for `s` / `S` cycle confirmation.
    charset_preset: String,
    /// Current droplet density multiplier (e.g. 1.0, 1.5, 2.0). Drives
    /// the `dsty:` HUD line for `[` / `]` adjustment feedback. Owner
    /// explicitly mandated the `dsty` label (NOT `den` — owner judged
    /// `den` as ugly/unsuitable for the density multiplier label).
    droplet_density: f32,
    /// Current chars-per-second speed (e.g. 14.0, 25.5). Drives the
    /// `sped:` HUD line for `↑` / `↓` adjustment feedback.
    chars_per_sec: f32,
    /// Endurance Health Score (0.0-100.0). Long-endurance process
    /// stability metric combining RSS variance, frame-time jitter, and
    /// context-switch rate. Drives the `ehs:` HUD line so the owner can
    /// answer "why is the rain behaving this way?" without quitting
    /// cosmostrix. Source: `EnduranceHealth::score()` called from the
    /// event loop's 1 Hz adaptive tick.
    endurance_health_score: f64,
    /// Effective pressure (0.0-1.0, clamped). Drives the spawn rate,
    /// sim factor, and self-healer. Drives the `prs:` HUD line so the
    /// owner can see when adaptive throttling is engaging. Source:
    /// `PowerManager::effective_pressure()` called from the event loop.
    effective_pressure: f32,
    // v50.0.0-beta.6: two new owner-mandated HUD metrics — power-dragon
    // and crystal-dragon on/off indicators. These reflect the LIVE
    // runtime state (not the startup config), so when the user toggles
    // power_dragon or crystal_dragon via config.toml live-reload, the
    // HUD immediately shows the new state on the next 1 Hz metric tick.
    /// Power Dragon on/off. When false, aggressive_throttle + idle FPS
    /// reduction are disabled (owner Option D). Drives the `prdr:` HUD
    /// line (row 13) so the owner can verify the live-reloaded state
    /// without quitting cosmostrix. Default: true (protection enabled).
    power_dragon_on: bool,
    /// Crystal Dragon on/off. When true, the palette drifts through the
    /// configured color range over time (ambient color morphing). Drives
    /// the `crdr:` HUD line (row 14) so the owner can verify the live-
    /// reloaded state. Default: false (drift off — palette is static).
    crystal_dragon_on: bool,
    /// v50.0.0-beta.6 Option D: aggressive-throttle flag (mirrors
    /// `cloud.aggressive_throttle`). When true, the self-healer has
    /// detected sustained high CPU pressure and is using the steeper
    /// spawn-scale curve. The `dsty:` HUD line uses this flag (via
    /// `compute_spawn_scale`) to show the effective density — the user
    /// sees density drop harder when aggressive throttle is active.
    aggressive_throttle: bool,
    // v50.0.0-beta.7 HUD Option C expansion — 4 new owner-mandated metrics.
    /// Ambient scheduler on/off (true when config.toml has ≥1 ambient.HH-MM
    /// entry). Drives the `ambt:` HUD line so the user knows automated
    /// scene/palette changes are happening.
    ambient_on: bool,
    /// Glitch level preset (none/subtle/default/intense). Drives the
    /// `glth:` HUD line.
    glitch_level: crate::config::GlitchLevel,
    /// Color tuning status: true when any ColorTune field ≠ 1.0 (custom),
    /// false when all fields = 1.0 (default/identity). Drives the
    /// `ctun:` HUD line.
    color_tune_custom: bool,
    /// Monolith size (small/normal/large). Only meaningful when scene is
    /// monolith-based. None = "unknown" (non-monolith scene). Drives the
    /// `mnst:` HUD line.
    monolith_size: Option<crate::runtime::MonolithSize>,
    /// Cached display strings — reformatted only at 1 Hz, written to
    /// frame buffer every frame via write_to_frame().
    ///
    /// 24 lines: fps / tgt / max / p99 / cpu / rss / ehs / prs / scn /
    /// chr / clr / sped / dsty / prdr / crdr / ambt / glth / ctun /
    /// mnst / dcel / tcel / cid / up / screensize (Z-master-1X round 5
    /// owner mandate 2026-08-31: added dcel + tcel above cid). The cid
    /// line is static (compile-time git SHA injected by build.rs via
    /// `COSMOSTRIX_GIT_SHA`) at row 21, set once in `new()`; only its
    /// color is refreshed by `refresh_colors` every frame.
    cached_lines: [(Color, String); 24],
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

    /// Whether the HUD is currently visible. Test-only — production
    /// code reads the `visible` field directly (cheaper than a method
    /// call in the hot render path). Tests use this accessor to verify
    /// toggle behavior without reaching into private fields.
    #[cfg(test)]
    pub(crate) fn visible(&self) -> bool {
        self.visible
    }

    /// Read one metric line's rendered text (e.g. row 7 = ` prs:`).
    /// Test-only accessor for cross-module tests (event_loop_hud gate
    /// tests live outside this module and cannot read `cached_lines`
    /// directly). Panics on out-of-range index (24 rows).
    #[cfg(test)]
    pub(crate) fn test_metric_line(&self, row: usize) -> &str {
        &self.cached_lines[row].1
    }

    /// Record a frame time. Called every frame from the event loop.
    /// Cheap when the HUD is off (just one bool check + early return).
    /// Auto-resets max_ms every MAX_RESET_INTERVAL_SECS to prevent a
    /// startup spike from dominating the display forever.
    #[inline]
    pub(crate) fn push_frame_time(&mut self, ms: f64) {
        if !self.visible || self.metrics_paused {
            // Paused 4 Hz input-poll ticks must not contaminate fps/max/p99.
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

    /// Z-master-1X round 5: push dirty-cell + total-cell counts for the
    /// dcel/tcel HUD metrics. Called every frame from the event loop
    /// AFTER sim_draw (when `dirty_len` + `is_dirty_all` are known).
    /// Cheap when the HUD is off (bool check + early return). Paused
    /// frames do not push — matches the `push_frame_time` freeze contract.
    #[inline]
    pub(crate) fn set_dirty_cell_stats(&mut self, dirty_count: u64, total_cells: u64) {
        if !self.visible || self.metrics_paused {
            return;
        }
        self.dirty_cell_tracker.push(dirty_count, total_cells);
    }

    /// Maybe sample RSS (rate-limited). Called every frame.
    #[inline]
    pub(crate) fn maybe_sample_rss(&mut self) {
        if !self.visible || self.metrics_paused {
            // Holds the last active reading while paused (v80.0.0-beta.1 freeze).
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
    /// ## Crystal Dragon independence
    /// The HUD is independent from Crystal Dragon (`i` toggles it any time)
    /// and must work without color drift. Decoupling also avoids sharing mutable state
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

        // v80.0.0-beta.1 pause freeze: cpu% holds its last active value while
        // paused, but the baseline keeps ticking so the first post-resume
        // delta stays a ~1 s window (not the whole idle pause span).
        if self.metrics_paused {
            self.last_cpu_ns = Some(cpu_ns_now);
            return;
        }

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

    // ── v50 (2026-08-17) HUD expansion setters ───────────────────────────
    //
    // Each setter is called from the event loop whenever the corresponding
    // value changes (key press / scene cycle / color cycle / config reload
    // / 1 Hz adaptive tick). The text is rendered into cached_lines[6..=12]
    // on the 1 Hz metric tick (see `update_metrics` below). Setters are
    // cheap (one field write, no format! call) so the event loop can call
    // them on every frame without measurable cost.

    /// Set the active scene name. Drives the `scn:` HUD line (row 8) for
    /// `x` cycle confirmation. Called by event_loop on init and whenever
    /// the user cycles scenes.
    ///
    /// v50 (2026-08-17) HUD metric stability: truncates the input to 14
    /// chars (by char count, preserving UTF-8 boundaries) so a very
    /// long custom scene name cannot blow past the HUD_MAX_WIDTH (22
    /// cols) budget. The ` scn: ` prefix is 5 chars, so 5 + 14 = 19 ≤ 22.
    pub(crate) fn set_scene_name(&mut self, name: &str) {
        self.scene_name.clear();
        const SCENE_NAME_MAX_CHARS: usize = 14;
        self.scene_name
            .extend(name.chars().take(SCENE_NAME_MAX_CHARS));
    }

    /// Set the active color scheme. Drives the `clr:` HUD line (row 10)
    /// for `c` / `C` cycle confirmation. Called by event_loop on init and
    /// whenever the user cycles colors. Rendered via Debug format (matches
    /// `verbose.rs` convention — e.g. `NeonGreen`, `FancyDiamond`).
    pub(crate) fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.color_scheme = scheme;
        // Clear custom palette name — user cycled to a builtin scheme.
        self.custom_palette_name = None;
    }

    /// Set the active custom palette name. Takes priority over the builtin
    /// `ColorScheme` Debug format for the `clr:` HUD line. Called by
    /// event_loop when --colors-custom is active.
    pub(crate) fn set_custom_palette_name(&mut self, name: Option<&str>) {
        self.custom_palette_name = name.map(|s| s.to_string());
    }

    /// Set the active charset preset name. Drives the `chr:` HUD line
    /// (row 9) for `s` / `S` cycle confirmation. Called by event_loop on
    /// init and whenever the user cycles charsets.
    ///
    /// v50 (2026-08-17) HUD metric stability: truncates the input to 14
    /// chars so a very long custom charset preset name cannot blow past
    /// the HUD_MAX_WIDTH (24 cols) budget. The ` chr: ` prefix is 6
    /// chars, so 6 + 14 = 20 ≤ 24.
    /// (v80.0.0-alpha.1 doc-drift fix: comment said 22; the const is 24.)
    pub(crate) fn set_charset_preset(&mut self, preset: &str) {
        self.charset_preset.clear();
        const CHARSET_PRESET_MAX_CHARS: usize = 14;
        self.charset_preset
            .extend(preset.chars().take(CHARSET_PRESET_MAX_CHARS));
    }

    /// Set the current droplet density multiplier. Drives the `dsty:` HUD
    /// line (row 12) for `[` / `]` adjustment feedback. Called by event_loop
    /// on init and whenever the user adjusts density or live-config reloads.
    /// Owner explicitly mandated the `dsty` label (NOT `den`).
    ///
    /// v50 (2026-08-17) HUD metric stability: NaN, infinite, or negative
    /// values map to 0.0 so a runtime bug cannot produce a runaway
    /// density or garbage HUD output. Rendered as `dsty: 0.00` (visibly
    /// broken, forcing investigation rather than hiding the issue).
    pub(crate) fn set_droplet_density(&mut self, density: f32) {
        self.droplet_density = if density.is_finite() && density >= 0.0 {
            density
        } else {
            0.0
        };
    }

    /// Set the current chars-per-second speed. Drives the `sped:` HUD line
    /// (row 11) for `↑` / `↓` adjustment feedback. Called by event_loop on
    /// init and whenever the user adjusts speed or live-config reloads.
    ///
    /// v50 (2026-08-17) HUD metric stability: NaN, infinite, or negative
    /// values map to 0.0 so a runtime bug cannot produce a runaway
    /// speed or garbage HUD output. Rendered as `sped: 0.0` (visibly
    /// broken, forcing investigation rather than hiding the issue).
    pub(crate) fn set_chars_per_sec(&mut self, cps: f32) {
        self.chars_per_sec = if cps.is_finite() && cps >= 0.0 {
            cps
        } else {
            0.0
        };
    }

    /// Set the Endurance Health Score (0.0-100.0). Drives the `ehs:` HUD
    /// line (row 6) so the owner can answer "why is the rain behaving this
    /// way?" without quitting cosmostrix. Called by event_loop on the 1 Hz
    /// adaptive tick (alongside `endurance_health.recompute()`).
    ///
    /// v50 (2026-08-17) HUD metric stability: NaN, infinite, or
    /// out-of-range values map to 0.0 (rendered as `ehs: 0` — visibly
    /// degraded, forcing investigation rather than hiding the issue).
    /// In-range values are clamped to [0.0, 100.0].
    pub(crate) fn set_endurance_health_score(&mut self, score: f64) {
        // v80.0.0-beta.1 pause freeze: hold the last active reading while paused.
        if self.metrics_paused {
            return;
        }
        self.endurance_health_score = if score.is_finite() {
            score.clamp(0.0, 100.0)
        } else {
            0.0
        };
    }

    /// Set the effective pressure (0.0-1.0, clamped). Drives the `prs:` HUD
    /// line (row 7) so the owner can see when adaptive throttling engages.
    /// Called by event_loop every frame (cheap — one field write) so the
    /// pressure value tracks the live adaptive state with no perceptible
    /// delay. Source: `PowerManager::effective_pressure()`.
    ///
    /// v50 (2026-08-17) HUD metric stability: NaN, infinite, or
    /// out-of-range values map to 0.0 (rendered as `prs: 0.00`).
    /// `update_metrics` also clamps at format-time, so the dual clamp
    /// (setter + format) is defense-in-depth — a future code path that
    /// bypasses the setter still gets sanitized output.
    pub(crate) fn set_effective_pressure(&mut self, pressure: f32) {
        // v80.0.0-beta.1 pause freeze: hold the last active reading while paused
        // (pressure measured over 4 Hz input polls is meaningless).
        if self.metrics_paused {
            return;
        }
        self.effective_pressure = if pressure.is_finite() {
            pressure.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    /// Set the power-dragon on/off state. Drives the `prdr:` HUD line
    /// (row 13) so the owner can verify the live-reloaded state without
    /// quitting cosmostrix. Called by event_loop every frame with
    /// `cfg.power_dragon` — when the user edits `power_dragon = false`
    /// in config.toml and live-reload applies it, the HUD reflects the
    /// new state on the next 1 Hz metric tick. Renders as `prdr: on` or
    /// `prdr: off`.
    ///
    /// v50.0.0-beta.6: owner-mandated metric. The value is NOT hardcoded
    /// — it tracks the live runtime state, not the startup config. This
    /// matches the existing HUD metrics behavior (e.g. `scn:`, `clr:`
    /// all reflect the live state, not the startup value).
    pub(crate) fn set_power_dragon(&mut self, on: bool) {
        self.power_dragon_on = on;
    }

    /// Set the crystal-dragon on/off state. Drives the `crdr:` HUD line
    /// (row 14) so the owner can verify the live-reloaded state. Called
    /// by event_loop every frame with `cfg.crystal_dragon` — when the
    /// user edits `crystal_dragon = true` in config.toml and live-reload
    /// applies it, the HUD reflects the new state. Renders as `crdr: on`
    /// or `crdr: off`.
    ///
    /// v50.0.0-beta.6: owner-mandated metric. Same live-reload pattern
    /// as `set_power_dragon` — the value tracks the runtime state, not
    /// the startup config.
    pub(crate) fn set_crystal_dragon(&mut self, on: bool) {
        self.crystal_dragon_on = on;
    }

    /// v50.0.0-beta.6 Option D: set the aggressive-throttle flag.
    /// Mirrors `cloud.aggressive_throttle` (set by the self-healer on
    /// sustained high CPU pressure). The `dsty:` HUD line uses this flag
    /// via `compute_spawn_scale()` to show the effective density — when
    /// aggressive throttle is active, dsty drops harder (steeper curve +
    /// lower floor). Called by event_loop every frame alongside
    /// `set_effective_pressure`.
    pub(crate) fn set_aggressive_throttle(&mut self, on: bool) {
        self.aggressive_throttle = on;
    }

    // v50.0.0-beta.7 Option C expansion — 4 new owner-mandated metrics.
    /// Set ambient scheduler on/off. `on` = config.toml has ≥1 ambient.HH-MM
    /// entry (auto-detected by the event loop from the ambient schedule).
    pub(crate) fn set_ambient_on(&mut self, on: bool) {
        self.ambient_on = on;
    }

    /// Set glitch level preset (none/subtle/default/intense).
    pub(crate) fn set_glitch_level(&mut self, level: crate::config::GlitchLevel) {
        self.glitch_level = level;
    }

    /// Set color tuning status: `true` = custom (any ColorTune field ≠ 1.0),
    /// `false` = default (all fields = 1.0 / identity).
    pub(crate) fn set_color_tune_custom(&mut self, is_custom: bool) {
        self.color_tune_custom = is_custom;
    }

    /// Set monolith size (small/normal/large). Only meaningful when scene
    /// is monolith-based.
    pub(crate) fn set_monolith_size(&mut self, size: crate::runtime::MonolithSize) {
        self.monolith_size = Some(size);
    }

    /// Set monolith size to "unknown" — called when scene is NOT monolith-based.
    pub(crate) fn set_monolith_size_unknown(&mut self) {
        self.monolith_size = None;
    }

    /// Refresh HUD line colors from the current palette. Called every
    /// frame when visible — cheap (4 `brighten_color` calls ≈ 2 µs) so
    /// the HUD tracks palette changes (`c`/`C` key cycle, Crystal Dragon
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
    /// The 18 HUD rows form a vertical brightness gradient that mirrors a
    /// falling rain droplet. In the rain visual, the leading character
    /// (the `head`) is the bright white at the BOTTOM of the stream, and
    /// the trailing fade (the `tail`) is dim at the TOP. The HUD adopts
    /// the same orientation:
    ///
    /// ```text
    ///   row 0   fps           ← dim      (tail — palette index 1)
    ///   row 1   tgt           ← dim
    ///   row 2   max           ← trail    (palette index n/4)
    ///   row 3   p99           ← trail
    ///   row 4   cpu           ← mid      (palette index n/2)
    ///   row 5   rss           ← mid
    ///   row 6   ehs           ← mid      (endurance health score)
    ///   row 7   prs           ← trail    (effective pressure)
    ///   row 8   scn           ← mid      (scene name — v80.0.0-beta.1 reorder)
    ///   row 9   chr           ← mid      (charset preset — v80.0.0-beta.1 reorder)
    ///   row 10  clr           ← mid      (color scheme — v80.0.0-beta.1 reorder)
    ///   row 11  sped          ← mid      (chars/sec speed — v80.0.0-beta.1 reorder)
    ///   row 12  dsty          ← mid      (density multiplier — v80.0.0-beta.1 reorder)
    ///   row 13  prdr          ← head     (power-dragon on/off — v80.0.0-beta.1 reorder)
    ///   row 14  crdr          ← head     (crystal-dragon on/off — v80.0.0-beta.1 reorder)
    ///   row 15  ambt          ← head     (ambient on/off — v80.0.0-beta.1 reorder)
    ///   row 16  glth          ← head     (glitch level — v80.0.0-beta.1 reorder)
    ///   row 17  ctun          ← head     (color tuning — v80.0.0-beta.1 reorder)
    ///   row 18  mnst          ← head     (monolith size — v80.0.0-beta.1 reorder)
    ///   row 19  dcel          ← head     (dirty cell ratio % — Z-master-1X round 5)
    ///   row 20  tcel          ← head     (total cells — Z-master-1X round 5)
    ///   row 21  cid           ← head     (build identity — Z-master-1X round 5: moved down)
    ///   row 22  up            ← head     (session uptime — Z-master-1X round 5: moved down)
    ///   row 23  screensize    ← head     (rain head — Z-master-1X round 5: moved down)
    /// ```
    ///
    /// Z-master-1X round 5 (owner mandate 2026-08-31): added dcel + tcel
    /// at rows 19-20 (cell efficiency metrics — owner insight from the
    /// CELL EFFICIENCY benchmark section). cid moved from row 19 to 21,
    /// up from 20 to 22, screensize from 21 to 23. The session footer
    /// still closes the dashboard — cid keeps a head-band position (row 21),
    /// the terminal size stays the visual anchor at the bottom (row 23).
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
        // HD-01 (HUD chroma dragon integration): 24-stop sweep across the
        // active palette, mapping each of the 24 HUD rows to a distinct
        // palette stop. Row 0 (fps, top) → dim tail stop, row 23
        // (screensize, bottom) → bright head stop. The full chroma dragon
        // gradient is now visible across the HUD — matching the border
        // message gradient's per-cell sweep philosophy, but applied
        // per-LINE to preserve text readability (each row keeps one
        // consistent color). (v80.0.0-alpha.1 doc-drift fix: the old
        // comment described the pre-expansion 16-row layout.)
        //
        // `brighten_color` floor (TARGET_V=200) guarantees every stop is
        // legible on a black background, including palette[0] which is
        // typically near-black start stop — it gets boosted to neutral
        // grey RGB(120,120,120) when pure black, preserving readability
        // without losing the palette's hue identity for non-black stops.
        let colors = compute_chroma_gradient_24(palette_colors);
        for (i, c) in colors.into_iter().enumerate() {
            self.cached_lines[i].0 = c;
        }
    }

    // v50.0.0-beta.7 LOC refactor: update_metrics method extracted to
    // metrics.rs as a separate impl HudState block.
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

// v50.0.0-beta.7 LTS: compute_chroma_gradient_24 + brighten_color
// extracted to colors.rs to keep this file under the 800-LOC cap.
// Re-exported here so 'use super::*' glob in tests.rs + tests_brighten.rs
// resolves them unchanged. mod.rs only calls compute_chroma_gradient_24
// directly; brighten_color is re-exported purely for the test modules
// (tests_brighten.rs calls it directly), hence the allow(unused_imports).
mod colors;
mod hud_init;
#[allow(unused_imports)]
pub(crate) use colors::{brighten_color, compute_chroma_gradient_24};

// v50.0.0-beta.7 LOC refactor: update_metrics method extracted to
// metrics.rs as a separate impl HudState block.
mod metrics;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_chroma_metrics;
#[cfg(test)]
mod tests_pause_freeze;

#[cfg(test)]
mod tests_brighten;

#[cfg(test)]
mod tests_dragon_indicators;
