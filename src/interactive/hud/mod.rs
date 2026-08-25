// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live HUD overlay for interactive mode.
//!
//! Toggle with `i`. When visible, writes a compact 5-line overlay into
//! the frame buffer (before `term.draw()`) showing real-time FPS, p99,
//! max frame time, RSS, and session uptime.
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
//! - **Rain-aesthetic color gradient**: the HUD's 16 lines form a vertical
//!   brightness gradient that mirrors a falling rain droplet — the bottom
//!   line (cid) is the brightest `head` (palette last-stop, the rain's
//!   leading bright character), the top line (fps) is the dimmest `tail`
//!   (palette index 1, the rain's trailing fade). Mid lines span `trail`
//!   and `mid` so the eye reads the HUD as a small rain column hanging in
//!   the corner, not as a flat block of text. This inverts the original
//!   mapping where `fps`/`tgt`/`max` were the brightest — the user
//!   explicitly flagged the inversion: 'rain tail is dim head is white'
//!   (head leads at the bottom of a falling stream).
//! - **v50 (2026-08-17) HUD expansion**: rows 9-15 are reserved for the 7
//!   owner-mandated metrics (scene / color / density / speed / endurance-
//!   health-score / effective-pressure / charset). The structural bump
//!   (array 9 -> 16 + chroma gradient 9-stop -> 16-stop) lands as a
//!   no-behavior-change micro-commit — `write_to_frame` skips empty-text
//!   rows so the 7 reserved entries render nothing until populated by the
//!   follow-up data-plumbing commit.
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
    /// User pressed `p` to pause. Loop ticks at PAUSE_PERIOD_MS.
    Paused,
}

/// Live HUD overlay state.
pub(crate) struct HudState {
    visible: bool,
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
    // v50 (2026-08-17) HUD expansion — 7 new owner-mandated metrics.
    // All fed by setters called from the event loop whenever the value
    // changes (key press, scene cycle, color cycle, config reload) or
    // sampled every frame at 1 Hz (effective_pressure, ehs). The text
    // is rendered into cached_lines[6..=12] on the 1 Hz metric tick.
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
    /// line (row 15) so the owner can verify the live-reloaded state
    /// without quitting cosmostrix. Default: true (protection enabled).
    power_dragon_on: bool,
    /// Crystal Dragon on/off. When true, the palette drifts through the
    /// configured color range over time (ambient color morphing). Drives
    /// the `crdr:` HUD line (row 16) so the owner can verify the live-
    /// reloaded state. Default: false (drift off — palette is static).
    crystal_dragon_on: bool,
    /// Cached display strings — reformatted only at 1 Hz, written to
    /// frame buffer every frame via write_to_frame().
    ///
    /// 18 lines: fps / tgt / max / p99 / cpu / rss / ehs / prs / sped /
    /// dsty / scn / chr / clr / up / screensize / prdr / crdr / cid.
    /// Rows 0-14 are the v50 layout (performance + health + live
    /// controls + session/screensize). Rows 15-16 are the v50.0.0-beta.6
    /// dragon on/off indicators (prdr, crdr). Row 17 is the cid line
    /// (commit short SHA, static for the entire process lifetime —
    /// owner-mandated bottom row). The cid line is static (compile-time
    /// git SHA injected by build.rs via `COSMOSTRIX_GIT_SHA`), so its
    /// text is set once in `new()` and only its color is refreshed by
    /// `refresh_colors` every frame.
    cached_lines: [(Color, String); 18],
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
            cached_lines: [
                // ── Performance core (rows 0-5) — unchanged from v50 ──
                (Color::Cyan, String::new()), // 0: fps
                (Color::Cyan, String::new()), // 1: tgt — uses `dim` (tail) at runtime
                // v50 (2026-08-15): rows 2-5 reordered intra-pair to match
                // htop/btop convention — extreme before representative
                // (max before p99), active before passive (cpu before rss).
                // Brightness gradient stop assignments are unchanged —
                // colors[i] still maps to cached_lines[i]. Only the content
                // at each index changed.
                (Color::Magenta, String::new()), // 2: max line (was row 3)
                (Color::Yellow, String::new()),  // 3: p99 line (was row 2)
                (Color::Cyan, String::new()),    // 4: cpu line — uses `mid` at runtime (was row 5)
                (Color::Green, String::new()),   // 5: rss line (was row 4)
                // ── Health / pressure (rows 6-7) — v50 HUD expansion ──
                // ehs (Endurance Health Score) before prs (Effective Pressure):
                // health is the summary, pressure is the live driver. Reading
                // order matches the diagnostic flow — "how is it" then "why".
                (Color::Yellow, String::new()), // 6: ehs  — endurance health score (NEW)
                (Color::Yellow, String::new()), // 7: prs  — effective pressure (NEW)
                // ── User-adjustable live controls (rows 8-12) — v50 HUD expansion ──
                // Ordering: speed → density → scene → charset → color.
                // Speed/density are numeric (adjust via arrows/brackets),
                // scene/charset/color are categorical (cycle via single keys).
                // Owner explicitly mandated `dsty` for density (NOT `den`).
                (Color::Magenta, String::new()), // 8: sped — chars/sec speed (↑↓) (NEW)
                (Color::Magenta, String::new()), // 9: dsty — density multiplier ([/]) (NEW)
                (Color::Cyan, String::new()),    // 10: scn  — scene name (x cycle) (NEW)
                (Color::Cyan, String::new()),    // 11: chr  — charset preset (s/S cycle) (NEW)
                (Color::Cyan, String::new()),    // 12: clr  — color scheme (c/C cycle) (NEW)
                // ── Session / diagnostic / build identity (rows 13-17) ──
                // v50 (2026-08-17): moved up/screensize/cid to the bottom
                // (cid was row 15 — owner-mandated). v50.0.0-beta.6: prdr
                // and crdr inserted above cid (rows 15-16), cid moved to
                // row 17 (still the last/bottom row per owner mandate:
                // "cid indicator keep last position metrics"). The chroma
                // gradient sweeps from dim tail (palette[0]) at the top to
                // bright head (palette[n-1]) at the bottom, so the build
                // identity still earns the brightest stop.
                (Color::DarkCyan, String::new()), // 13: up  — session uptime
                (Color::DarkCyan, String::new()), // 14: screensize
                // v50.0.0-beta.6: power-dragon on/off indicator. Text is
                // rebuilt at the 1 Hz tick in update_metrics (reads the
                // live power_dragon_on field set by set_power_dragon()).
                // Renders as " prdr: on" or " prdr: off".
                (Color::DarkCyan, String::new()), // 15: prdr — power-dragon on/off (NEW)
                // v50.0.0-beta.6: crystal-dragon on/off indicator. Same
                // pattern as prdr — live state from set_crystal_dragon().
                // Renders as " crdr: on" or " crdr: off".
                (Color::DarkCyan, String::new()), // 16: crdr — crystal-dragon on/off (NEW)
                // cid line — commit short SHA, static for the entire process
                // lifetime. Color is refreshed by `refresh_colors` (head stop,
                // brightest) every frame; the text never changes so
                // `update_metrics` skips it. v50: row 15 → v50.0.0-beta.6:
                // row 17 (still owner-mandated bottom row).
                (Color::DarkCyan, format!(" cid: {commit_sha}")),
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

    /// Set the active scene name. Drives the `scn:` HUD line (row 10) for
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

    /// Set the active color scheme. Drives the `clr:` HUD line (row 12)
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
    /// (row 11) for `s` / `S` cycle confirmation. Called by event_loop on
    /// init and whenever the user cycles charsets.
    ///
    /// v50 (2026-08-17) HUD metric stability: truncates the input to 14
    /// chars so a very long custom charset preset name cannot blow past
    /// the HUD_MAX_WIDTH (22 cols) budget. The ` chr: ` prefix is 6
    /// chars, so 6 + 14 = 20 ≤ 22.
    pub(crate) fn set_charset_preset(&mut self, preset: &str) {
        self.charset_preset.clear();
        const CHARSET_PRESET_MAX_CHARS: usize = 14;
        self.charset_preset
            .extend(preset.chars().take(CHARSET_PRESET_MAX_CHARS));
    }

    /// Set the current droplet density multiplier. Drives the `dsty:` HUD
    /// line (row 9) for `[` / `]` adjustment feedback. Called by event_loop
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
    /// (row 8) for `↑` / `↓` adjustment feedback. Called by event_loop on
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
        self.effective_pressure = if pressure.is_finite() {
            pressure.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    /// Set the power-dragon on/off state. Drives the `prdr:` HUD line
    /// (row 15) so the owner can verify the live-reloaded state without
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
    /// (row 16) so the owner can verify the live-reloaded state. Called
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
    ///   row 8   sped          ← mid      (chars/sec speed)
    ///   row 9   dsty          ← mid      (density multiplier)
    ///   row 10  scn           ← mid      (scene name)
    ///   row 11  chr           ← mid      (charset preset)
    ///   row 12  clr           ← mid      (color scheme)
    ///   row 13  up            ← head     (palette last stop, brightest)
    ///   row 14  screensize    ← head     (rain head — leading white)
    ///   row 15  prdr          ← head     (NEW: power-dragon on/off)
    ///   row 16  crdr          ← head     (NEW: crystal-dragon on/off)
    ///   row 17  cid           ← head     (build identity — same head stop)
    /// ```
    ///
    /// v50.0.0-beta.6 HUD expansion: rows 15-16 are the 2 new owner-mandated
    /// dragon on/off indicators (prdr / crdr) inserted above cid. Cid moved
    /// from row 15 to row 17 (still owner-mandated bottom row — "cid
    /// indicator keep last position metrics"). The chroma gradient sweeps
    /// continuously from palette[0] (dim tail) at the top to palette[n-1]
    /// (bright head) at the bottom. The cid line shares the head stop with
    /// screensize/prdr/crdr so the build identity is the most prominent
    /// entry — the owner needs to read the commit hash without quitting
    /// cosmostrix, so it earns the brightest position alongside the screen
    /// size and dragon indicators.
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
        // HD-01 (HUD chroma dragon integration): 16-stop sweep across the
        // active palette, mapping each of the 16 HUD rows to a distinct
        // palette stop. Row 0 (fps, top) → palette[0], row 15 (reserved clr,
        // bottom) → palette[n-1]. The full chroma dragon gradient is now
        // visible across the HUD — matching the border message gradient's
        // per-cell sweep philosophy, but applied per-LINE to preserve text
        // readability (each row keeps one consistent color).
        //
        // `brighten_color` floor (TARGET_V=200) guarantees every stop is
        // legible on a black background, including palette[0] which is
        // typically near-black start stop — it gets boosted to neutral
        // grey RGB(120,120,120) when pure black, preserving readability
        // without losing the palette's hue identity for non-black stops.
        let colors = compute_chroma_gradient_18(palette_colors);
        for (i, c) in colors.into_iter().enumerate() {
            self.cached_lines[i].0 = c;
        }
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
        //
        // HD-01 (HUD chroma dragon integration): 18-stop sweep — each row
        // gets a distinct palette stop. Index math: `palette_colors`
        // sampled at `(i / 17.0 * (n-1)).round()` for i ∈ [0..18].
        let colors = compute_chroma_gradient_18(palette_colors);

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
        let mode_suffix = match self.frame_mode {
            FrameMode::Active => String::new(),
            FrameMode::Idle => " idle".to_string(),
            FrameMode::Paused => " paused".to_string(),
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
        let dsty_val = self.droplet_density;
        self.cached_lines[9] = (colors[9], format!(" dsty: {dsty_val:.2}"));
        // scn (scene name): string, no format. User cycles via `x`.
        self.cached_lines[10] = (colors[10], format!(" scn: {}", self.scene_name));
        // chr (charset preset): string, no format. User cycles via `s`/`S`.
        self.cached_lines[11] = (colors[11], format!(" chr: {}", self.charset_preset));
        // clr (color scheme): Debug format (matches verbose.rs convention).
        // e.g. `NeonGreen`, `FancyDiamond`, `Cosmos`.
        // clr (color scheme): show custom palette name when active, otherwise
        // the builtin ColorScheme Debug format. This fixes the bug where
        // --colors-custom cyberpunk_2077 showed "clr: EnergyZen" (the
        // underlying builtin scheme) instead of the custom palette name.
        let clr_label = match &self.custom_palette_name {
            Some(name) => name.clone(),
            None => format!("{:?}", self.color_scheme),
        };
        self.cached_lines[12] = (colors[12], format!(" clr: {clr_label}"));
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
        // cid (row 17) is static — set once in new(), never rewritten
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
        // v50 (2026-08-17): HUD always renders flush-left at column 0.
        // The previous HudPosition enum + toggle_position method +
        // start_col() helper have been purged — the 'h' shortkey that
        // toggled between Left and Right corners was unused maintenance
        // cost per owner mandate. The literal 0 here replaces the
        // `self.position.start_col(cols, w)` call.
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

// HD-01 (HUD chroma dragon integration): the previous 4-stop
// `compute_rain_gradient` helper has been replaced by the 16-stop
// `compute_chroma_gradient_16` helper above. The old design paired 2 HUD
// rows per palette stop (dim/trail/mid/head × 2 = 8 rows); the new design
// gives each row its own palette stop, sweeping the full chroma dragon
// gradient top→bottom. This matches the border message's per-cell chroma
// sweep philosophy, applied per-LINE for text readability.
// v50 (2026-08-15): bumped from 8 → 9 stops after adding the `cid:`
// (commit id) line at row 8.
// v50 (2026-08-17): bumped from 9 → 16 stops to reserve rows 9-15 for
// the 7 owner-mandated HUD expansion metrics (scene / color / density /
// speed / endurance-health-score / effective-pressure / charset). The cid
// line shares the head stop (palette[n-1], brightest) with screensize —
// both are "definitive identity" lines that the owner reads to verify the
// build, so they earn the most prominent position.

/// HD-01 (HUD chroma dragon integration): compute an 18-stop chroma gradient
/// sweeping the active palette's full color range across all 18 HUD rows.
///
/// Each row `i ∈ [0..18]` samples `palette_colors` at interpolation
/// parameter `t = i / 17.0`, so row 0 (fps, top) → palette[0] and row 17
/// (cid, bottom) → palette[n-1]. This mirrors the border message's per-cell
/// clockwise sweep (`cloud/mod.rs::draw_message` BC-02) — applied per-LINE
/// here to preserve text readability (each row keeps one consistent color,
/// unlike per-cell which would rainbow-ize words).
///
/// v50 (2026-08-15): bumped from 8 → 9 stops after adding the `cid:`
/// (commit id) line at row 8.
/// v50 (2026-08-17): bumped from 9 → 16 stops to reserve rows 9-15 for the
/// 7 owner-mandated HUD expansion metrics.
/// v50.0.0-beta.6: bumped from 16 → 18 stops to add `prdr` (row 15) and
/// `crdr` (row 16) above cid (now row 17). Cid stays at the bottom
/// (owner-mandated last position); prdr/crdr sit directly above it so
/// the dragon on/off indicators are grouped with the build identity.
///
/// `brighten_color` floor (TARGET_V=200) guarantees every stop is legible on
/// a black background, including palette[0] which is typically a near-black
/// start stop — it gets boosted to neutral grey RGB(120,120,120) when pure
/// black, preserving readability without losing the palette's hue identity
/// for non-black stops.
///
/// Returns a fixed-size `[Color; 18]` array (no allocation, stack-only).
fn compute_chroma_gradient_18(palette_colors: &[crossterm::style::Color]) -> [Color; 18] {
    let n = palette_colors.len();
    let mut out = [
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
        Color::DarkGrey,
    ];
    if n == 0 {
        return out;
    }
    // v50 (2026-08-17) HUD chroma gradient smoothness fix: replace the
    // previous discrete sampling `palette_colors[(t * last).round()]` with
    // `interpolate_palette_color(palette_colors, t)` — the same linear-
    // interpolation helper used by the border message gradient (C4 fix).
    //
    // The previous discrete sampling produced visible bands when the palette
    // had fewer stops than the HUD has rows (e.g. a 3-stop palette + 18 HUD
    // rows → 6+ rows sharing the same color block). The owner explicitly
    // flagged this category as inconsistent with the chroma dragon smoothness
    // mandate: "audit which color-processing sites are not yet using
    // chroma dragon interpolation optimally ... can look inconsistent
    // if not unified". Every visible color surface must route through
    // the chroma dragon pipeline for consistency.
    //
    // With interpolation, every HUD row gets a smoothly-varying color even
    // when the palette is small — matching the border message's smooth
    // gradient behavior and the chroma dragon's per-cell sweep philosophy.
    // The `brighten_color` floor (TARGET_V=200) is still applied AFTER
    // interpolation to guarantee readability on a black background.
    //
    // LTS stability: `interpolate_palette_color` is NaN/Inf-safe (returns
    // the first stop defensively), so a future bug in upstream palette
    // generation cannot crash the HUD or produce garbage colors.
    // v50.0.0-beta.6: divisor is now 17.0 (was 15.0) since the array has
    // 18 entries (indices 0-17). Row 0 → t=0.0, row 17 → t=1.0.
    for (i, slot) in out.iter_mut().enumerate() {
        let t = i as f32 / 17.0;
        let interpolated = crate::cloud::interpolate_palette_color(palette_colors, t);
        *slot = brighten_color(interpolated.unwrap_or(Color::DarkGrey));
    }
    out
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
mod tests;

#[cfg(test)]
mod tests_brighten;

#[cfg(test)]
mod tests_dragon_indicators;
