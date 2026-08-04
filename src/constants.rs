// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Centralized named constants for the entire codebase.
//!
//! All magic numbers are extracted here to avoid duplication and
//! provide a single source of truth for tuning parameters.

// ─── Rain visual stack: single source of truth ─────────────────────────────
//
// All rain / parallax / phosphor / depth-layer / atmospheric / cinematic
// constants live in `central_control_rains.rs`. This re-export keeps
// every existing `use crate::constants::*;` call site working unchanged.
// To fine-tune any rain parameter, edit `central_control_rains.rs`.
pub(crate) use crate::central_control_rains::*;

// Density & sizing

/// Default cols for density auto-calculation in bench mode.
pub(crate) const DENSITY_AUTO_DEFAULT_COLS: u16 = 120;

/// Default lines for density auto-calculation in bench mode.
pub(crate) const DENSITY_AUTO_DEFAULT_LINES: u16 = 40;

/// Base terminal width for auto-density scaling.
pub(crate) const DENSITY_BASE_COLS: f32 = 80.0;

// v17: DENSITY_BASE_LINES removed — auto-density is now width-only.
// v17: DENSITY_AUTO_MAX removed — auto factor capped at 1.0 (identity).

/// Auto-density clamp range (min factor).
/// v17: the auto-density factor is now a width-only dampener
/// (clamp(cols/80, DENSITY_AUTO_MIN, 1.0)). It never amplifies above
/// 1.0 — the old sqrt(area) amplifier was removed.
pub(crate) const DENSITY_AUTO_MIN: f32 = 0.5;

/// Absolute density clamp range (min).
pub(crate) const DENSITY_CLAMP_MIN: f32 = 0.01;

/// Absolute density clamp range (max).
pub(crate) const DENSITY_CLAMP_MAX: f32 = 5.0;

/// Minimum user-facing rain speed for CLI, config, and keyboard controls.
pub(crate) const SPEED_MIN: f32 = 1.0;

/// Maximum user-facing rain speed for CLI, config, and keyboard controls.
pub(crate) const SPEED_MAX: f32 = 100.0;

/// Minimum runtime speed reachable through keyboard controls.
pub(crate) const RUNTIME_SPEED_MIN: f32 = SPEED_MIN;

/// Maximum runtime speed reachable through keyboard controls.
pub(crate) const RUNTIME_SPEED_MAX: f32 = SPEED_MAX;

/// Maximum effective Monolith speed, including CLI/config values.
pub(crate) const MONOLITH_EFFECTIVE_SPEED_MAX: f32 = SPEED_MAX;

// Performance tuning (shared between interactive & cloud)

/// Pressure spawn scaling factor: reduces spawn rate under perf pressure.
pub(crate) const PERF_PRESSURE_SPAWN_FACTOR: f32 = 0.75;

/// Performance pressure increment per overshoot frame.
pub(crate) const PERF_PRESSURE_INCREMENT: f32 = 0.25;

/// Performance pressure decay per normal frame.
pub(crate) const PERF_PRESSURE_DECAY: f32 = 0.02;

// Interactive mode tuning

/// Monotonic clock jump guard: skip frame if elapsed exceeds this.
pub(crate) const CLOCK_JUMP_GUARD_SECS: f64 = 10.0;

/// Pause polling period in milliseconds.
pub(crate) const PAUSE_PERIOD_MS: u64 = 250;

/// Perf-pressure classification threshold: below this = "low" pressure.
///
/// Used in the post-run perf report to bucket the average pressure into
/// low/medium/high. 0.05 = 5% average overshoot — anything below is
/// effectively running at full speed with no frame drops.
pub(crate) const PERF_PRESSURE_CLASS_LOW: f64 = 0.05;

/// Perf-pressure classification threshold: below this = "medium" pressure.
///
/// 0.30 = 30% average overshoot — frames are taking ~30% longer than the
/// target period, indicating sustained mild overload. Above this is "high".
pub(crate) const PERF_PRESSURE_CLASS_MEDIUM: f64 = 0.30;

// ── Adaptive resync interval tiers ───────────────────────────────────────────
//
// v25.15 (perf audit): the adaptive resync interval had four magic numbers
// inline in adaptive.rs (`3600.0`, `14400.0`, `60.0`, `120.0`). Promoted
// here so the idle-tier ladder is visible at a glance and tunable as a set.

/// One hour in seconds — boundary between standard and 1-hour idle tier.
pub(crate) const SECS_PER_HOUR: f64 = 3600.0;

/// Four hours in seconds — boundary between 1-hour and 4-hour idle tier.
///
/// At 4+ hours of continuous idle, the resync interval doubles again to
/// `IDLE_RESYNC_TIER_3_SECS`. This mirrors the observed behavior of
/// long-running kiosk/screensaver deployments where 24-hour uptimes are
/// common and frequent redraws waste power for zero visual benefit.
pub(crate) const SECS_PER_4_HOURS: f64 = 14400.0;

/// Resync interval (seconds) for 1–4 hours of sustained idle.
///
/// 3× reduction from the standard 20 s interval. At this tier the user is
/// clearly away; we keep just enough redraws to refresh CRT-phosphor state
/// without burning CPU.
pub(crate) const IDLE_RESYNC_TIER_2_SECS: f64 = 60.0;

/// Resync interval (seconds) for >4 hours of sustained idle.
///
/// 6× reduction from the standard 20 s interval. Used for overnight or
/// weekend-idle kiosks. Below this, the redraw cadence becomes too sparse
/// to recover cleanly from terminal emulator state drift.
pub(crate) const IDLE_RESYNC_TIER_3_SECS: f64 = 120.0;

// Terminal / rendering

/// Dirty threshold ratio: if dirty cells >= total/N, do full redraw.
pub(crate) const DIRTY_THRESHOLD_RATIO: usize = 3;

/// Graceful shutdown timeout in seconds (force-exit if flush blocks).
pub(crate) const SHUTDOWN_TIMEOUT_SECS: u64 = 2;

/// Maximum allowed terminal width (columns) for interactive mode.
/// Prevents OOM from wildly misreported terminal sizes (e.g. 65535 × 65535 → hundreds of GiB).
/// 1024 cols × 500 lines × ~48 bytes/cell ≈ 24 MiB — still comfortable.
pub(crate) const MAX_TERMINAL_COLS: u16 = 1024;

/// Maximum allowed terminal height (lines) for interactive mode.  Same rationale as above.
pub(crate) const MAX_TERMINAL_LINES: u16 = 500;

/// Maximum screen size for benchmark mode (columns).
///
/// Set to 8K UHD width (7680). This is the largest *meaningful* benchmark
/// resolution for a CPU + stdout renderer:
///   - 8K UHD (7680 × 4320) = 33.2M cells × ~48 B/cell ≈ 1.6 GiB — pushes the
///     allocator and dirty-cell pipeline hard without entering OOM-killer territory.
///   - 4K UHD (3840 × 2160) = 8.3M cells — comfortable, but doesn't stress the
///     differential-renderer paths the way 8K does.
///   - 50000 × 50000 = 2.5 Gcells × ~48 B ≈ 120 GiB — impossible on any real
///     single machine; the benchmark would be measuring the OOM killer, not the
///     renderer. u16 nominally supports up to 65535, but the cell-grid allocation
///     is the hard floor.
///
/// Cosmic dragon verdict to "8k or 4k?": **8K UHD is the maximum.** 4K is the
/// recommended daily-driver; 8K is the ceiling for stress benchmarks. Anything
/// larger is a memory benchmark, not a render benchmark.
pub(crate) const BENCH_MAX_COLS: u16 = 7680;

/// Maximum screen size for benchmark mode (lines). See `BENCH_MAX_COLS`.
///
/// 4320 = 8K UHD height. Same rationale: largest meaningful stress resolution
/// before the cell-grid allocation becomes the bottleneck instead of the
/// renderer itself.
pub(crate) const BENCH_MAX_LINES: u16 = 4320;

/// Minimum usable terminal width (columns). Below this, the renderer
/// refuses to start to avoid degenerate edge cases (empty frame, zero
/// droplets, divide-by-zero in column math).
pub(crate) const MIN_TERMINAL_COLS: u16 = 4;

/// Minimum usable terminal height (lines). Same rationale as above.
pub(crate) const MIN_TERMINAL_LINES: u16 = 4;

/// Resize debounce window in milliseconds. Rapid resize events within this
/// window are coalesced into a single application, preventing redundant
/// full resets and visual thrashing during window drag.
pub(crate) const RESIZE_DEBOUNCE_MS: u64 = 150;

/// Seconds of no user input before entering idle mode. In idle mode the
/// effective FPS target is reduced to conserve CPU/battery, and
/// atmospheric subsystem tick rates are lowered. Any input event instantly
/// restores full performance.
pub(crate) const IDLE_THRESHOLD_SECS: f64 = 30.0;

/// Effective FPS multiplier while idle. Applied on top of the user's
/// configured FPS target to reduce update pressure during inactivity.
/// Raised from 0.25 to 0.5 (30 FPS at 60 target) to keep phosphor decay
/// and shimmer visually smooth even during idle — the old 15 FPS felt
/// choppy and undermined the cinematic smoothness improvements.
pub(crate) const IDLE_FPS_FACTOR: f64 = 0.5;

/// Wall-clock interval for one-shot full redraws while idle. This keeps
/// terminal/compositor state synchronized even when idle FPS makes the
/// frame-count drift correction too sparse in real time.
pub(crate) const IDLE_REDRAW_RESYNC_INTERVAL_SECS: f64 = 20.0;

// ── Performance self-healing (P1 + P2) ───────────────────────────────────────
//
// Two cooperating mechanisms that let cosmostrix proactively respond to
// sustained performance degradation without user intervention:
//
//   P1 — Auto scene downgrade: when perf_pressure stays high for a sustained
//        window, switch to a lighter scene (low-power) to shed load. When
//        pressure recovers for a sustained window, restore the prior scene.
//
//   P2 — Endurance-health mitigations: when the EnduranceHealth score
//        (RSS variance + frame jitter + context switches) drops into the
//        "investigate" band, trigger an immediate frame invalidate + memory
//        reclaim hint to clear potential stuck state.
//
// Both mechanisms gate on existing perf_pressure / EnduranceHealth
// infrastructure — zero per-frame overhead in steady state (just a counter
// and a couple of comparisons).

/// perf_pressure threshold above which sustained-pressure accumulation
/// counts toward the auto-downgrade trigger. Set below the phosphor skip
/// gate (0.7) so the downgrade fires *before* visual quality starts
/// degrading — the goal is to shed load while the experience is still
/// smooth, not after it's already choppy.
pub(crate) const SELF_HEAL_PRESSURE_HIGH: f32 = 0.6;

/// perf_pressure threshold below which sustained-pressure recovery counts
/// toward the auto-restore trigger. Hysteresis gap (0.6 → 0.3) prevents
/// oscillation when pressure hovers near the boundary.
pub(crate) const SELF_HEAL_PRESSURE_LOW: f32 = 0.3;

/// Seconds of sustained high perf_pressure before auto-downgrade fires.
/// 30 s is long enough to ride out transient spikes (compile jobs, window
/// drags, momentary GC pauses) but short enough that genuine sustained
/// overload is caught before the user gives up and kills the process.
pub(crate) const SELF_HEAL_DOWNGRADE_SECS: f64 = 30.0;

/// Seconds of sustained low perf_pressure before auto-restore fires.
/// Deliberately longer than the downgrade window (60 s vs 30 s) so the
/// restored scene gets a stable runway before potentially downgrading
/// again. Prevents flapping under borderline load.
pub(crate) const SELF_HEAL_RESTORE_SECS: f64 = 60.0;

/// EnduranceHealth score below which immediate mitigations fire.
/// Matches the "investigate" band from EnduranceHealth::classification()
/// (score < 60). When crossed, the self-healer forces a full redraw and
/// bypasses ReclaimState's 1 h min interval to issue an madvise hint.
pub(crate) const SELF_HEAL_HEALTH_INVESTIGATE: f64 = 60.0;

/// Minimum seconds between consecutive health-triggered mitigations.
/// Without this, a persistently unhealthy process would force-redraw every
/// recompute cycle (≈1 s) — burning more CPU and worsening the very
/// problem we're trying to fix. 30 s is a safe cooldown that still
/// catches genuine stuck state quickly.
pub(crate) const SELF_HEAL_HEALTH_COOLDOWN_SECS: f64 = 30.0;

// ── P3: stdout /dev/tty fallback ─────────────────────────────────────────────
//
// Mid-run stdout corruption (SSH disconnect, terminal emulator crash, parent
// process death) leaves cosmostrix's primary write fd invalid. Without a
// fallback, `flush_ansi` propagates the error → event loop exits → Drop
// cleanup tries to write to the same broken fd → partial cleanup.
//
// P3 mitigation: on a recoverable io::Error (BrokenPipe, EBADF, PermissionDenied)
// from `stdout.write_all()`, attempt to open `/dev/tty` (Unix) or `CONOUT$`
// (Windows) as a one-shot recovery channel, write the frame to it, and
// signal graceful shutdown. The process exits cleanly via the normal
// shutdown path rather than crashing on the next write attempt.
//
// Defensive cap on consecutive recoveries: if `/dev/tty` itself fails
// repeatedly (e.g., no controlling terminal under `setsid`), we stop
// trying and let the original error propagate.

/// Maximum number of consecutive /dev/tty fallback recoveries before
/// giving up and propagating the underlying stdout error. Each successful
/// recovery also fires `GRACEFUL_SHUTDOWN`, so the process is already
/// exiting — this cap exists purely as a defensive bound against a
/// pathological loop where shutdown is delayed (e.g., live-config save).
pub(crate) const STDOUT_FALLBACK_MAX_RECOVERIES: u32 = 3;

// ── P4: periodic stuck-cell sweep (debug mode only) ─────────────────────────
//
// A background watchdog that scans the frame buffer for "stuck" cells —
// cells that hold a glyph at the current generation but are not covered
// by any active droplet's tail_put_line..=head_put_line range AND have
// zero phosphor energy. These represent dirty-tracking edge cases that
// the phosphor system (which only handles cells with phosphor[i] > 0)
// cannot reach.
//
// The sweep is gated on `enable_component_timing` (i.e., `--perf-stats`)
// to avoid any per-frame cost in production interactive runs. Telemetry
// from the sweep is logged to stderr only when it finds stuck cells.
//
// Cost: O(W×H + droplets) every STUCK_CELL_SWEEP_INTERVAL_FRAMES frames.
// At 200×60 and ~100 active droplets, ≈12,100 ops every 60 s ≈ 200 ops/s.

/// Frames between stuck-cell sweeps. 3600 frames ≈ 60 s at 60 FPS.
/// Deliberately longer than FULL_REDRAW_INTERVAL_FRAMES (18000/5 min) —
/// the full redraw already catches most stuck cells, so the sweep only
/// fires to catch drift in the windows *between* full redraws.
pub(crate) const STUCK_CELL_SWEEP_INTERVAL_FRAMES: u64 = 3600;

/// Maximum number of stuck cells the sweep will clear per pass. Prevents
/// a pathological case (e.g., after a resize race) from clearing tens of
/// thousands of cells in one sweep — the next full redraw will catch the
/// rest. Logging is also capped to avoid stderr flooding.
pub(crate) const STUCK_CELL_MAX_PER_SWEEP: usize = 256;

// ── P5: periodic fd health probe ────────────────────────────────────────────
//
// A proactive `isatty(stdout)` check on a slow interval to detect fd
// corruption BEFORE a write fails. The reactive P3 path (write fails →
// route through /dev/tty) is sufficient for active rendering, but during
// idle periods (no redraws) stdout could break and we wouldn't notice
// until the next render attempt. P5 closes that window.
//
// The probe runs every FD_HEALTH_PROBE_INTERVAL_FRAMES frames, which is
// deliberately slow enough that the isatty syscall cost (≈1 μs) is
// negligible — roughly 0.0017 syscalls/sec at 60 FPS. The audit's
// concern about "per-frame syscall overhead" is solved by the interval:
// not per-frame, per-minute.
//
// When the probe detects fd corruption (isatty returns false), it reuses
// the P3 recovery path: calls `recover_to_tty(b"", BrokenPipe)` which
// opens /dev/tty, writes the (empty) buffer, sets GRACEFUL_SHUTDOWN, and
// logs to stderr. The process then exits cleanly via the normal shutdown
// path.

/// Frames between proactive stdout fd health probes. 3600 frames ≈ 60 s
/// at 60 FPS. Matches the P4 stuck-cell sweep cadence — both are
/// "background hygiene" passes that run on the same slow tick.
pub(crate) const FD_HEALTH_PROBE_INTERVAL_FRAMES: u64 = 3600;

// Benchmark

/// Minimum elapsed seconds denominator to avoid division by zero in bench.
pub(crate) const BENCH_ELAPSED_MIN_S: f64 = 0.000_001;

/// Estimated ANSI overhead bytes per drawn cell in steady-state rendering.
/// Accounts for run-encoded style changes amortized across the terminal:
/// ~19 bytes = (5-byte SGR reset + ~6-byte fg escape + ~6-byte bg escape
/// + 1-byte char) × ~0.65 run-compression factor. This is a rough estimate
///   used for throughput reporting in the benchmark, not for frame pacing.
pub(crate) const ANSI_BYTES_PER_CELL_ESTIMATE: u64 = 19;

// Config file

/// Config file directory name under XDG_CONFIG_HOME or ~/.config.
pub(crate) const CONFIG_DIR_NAME: &str = "cosmostrix";

/// Config file name. v20.1 removed the pre-v10 `config` (no extension)
/// fallback — users upgrading from pre-v10 must rename their file.
pub(crate) const CONFIG_FILE_NAME: &str = "config.toml";

/// Default frame dirty capacity pre-allocation.  One Nth of total cells.
/// 8 is conservative enough for 1024×500 terminals (≈64K pre-alloc) while
/// still covering most frames without a heap spill.
pub(crate) const DIRTY_CAPACITY_DIVISOR: usize = 8;

/// Hard cap on dirty-vec pre-allocation in cells (≈8 KiB worth of usize).
/// Prevents wasting memory when terminal is very large.
pub(crate) const DIRTY_CAPACITY_CAP: usize = 8192;

// ── Renderer buffer pre-allocation (v25.16 perf polish) ──────────────────────
//
// These constants tune the initial capacity of the per-Terminal reusable
// String/Vec buffers used by the diff- and full-redraw paths. The buffers
// grow dynamically if a frame exceeds the pre-allocation, so these values
// are pure performance hints — they trade a small fixed memory cost for
// avoiding heap allocations on the hot path.
//
// Advanced benchmarkers: if your terminal is much larger than typical
// (e.g. 8K UHD bench), raising these to match your expected worst-case
// frame size eliminates the one-time grow cost during the first few frames.

/// Initial capacity (bytes) for `Terminal::run_buf` — the diff-redraw
/// char-run accumulator. Holds one contiguous run of same-style chars
/// before flushing to `ansi_buf`. 256 bytes covers a full row on any
/// terminal up to 256 cols without growth; wider terminals trigger one
/// grow event during the first diff frame and then run at the new
/// capacity forever.
pub(crate) const RENDER_RUN_BUF_INIT_CAP: usize = 256;

/// Initial capacity (bytes) for `Terminal::row_buf` — the full-redraw
/// row accumulator. Holds one row of chars before flushing to
/// `ansi_buf`. 512 bytes covers terminals up to 512 cols without
/// growth. Larger than `RENDER_RUN_BUF_INIT_CAP` because full redraws
/// always iterate the entire row, while diff redraws may break a row
/// into multiple short style-runs.
pub(crate) const RENDER_ROW_BUF_INIT_CAP: usize = 512;

/// Initial capacity (bytes) for `Terminal::combined_flush_buf` — the
/// sync-output wrapper buffer. Holds SYNC_START + ansi_buf + SYNC_END
/// for the single-write flush path. 8 KiB covers most frames; dense
/// dirty-all frames at 200×40 (~140 KB ANSI) trigger one grow event
/// and then run at the new capacity. Only used when sync_output is
/// enabled (terminal capability detection).
pub(crate) const RENDER_COMBINED_FLUSH_INIT_CAP: usize = 8192;

// ── Benchmark warmup tuning (v25.16 perf polish) ─────────────────────────────

/// Fraction of total bench frames used as warmup. `(bench_frames /
/// BENCH_WARMUP_DIVISOR).clamp(BENCH_WARMUP_MIN_FRAMES,
/// BENCH_WARMUP_MAX_FRAMES)` produces the warmup frame count.
///
/// 10 = 10% of total. The warmup runs the renderer at full speed
/// without recording metrics, allowing the allocator to settle, the
/// CPU to ramp up to its max frequency, and branch predictors / I-cache
/// to warm. Without warmup, the first ~50 frames of every benchmark
/// run are ~30% slower than steady-state, polluting p99/max metrics.
pub(crate) const BENCH_WARMUP_DIVISOR: u64 = 10;

/// Minimum warmup frame count. Even for very short benchmarks (e.g.
/// `--bench-frames 50`), at least 10 warmup frames run so the allocator
/// and CPU have time to settle.
pub(crate) const BENCH_WARMUP_MIN_FRAMES: u64 = 10;

/// Maximum warmup frame count. Caps warmup at 200 frames (~3.3s at 60
/// FPS) so long benchmarks (e.g. `--bench-frames 100000`) don't waste
/// disproportionate time on warmup. 200 frames is enough for any
/// realistic CPU / allocator to reach steady state.
pub(crate) const BENCH_WARMUP_MAX_FRAMES: u64 = 200;

// ── Quantum Ripple particle burst ──
//
// Quantum Ripple constants tune the mouse-click particle burst — a v25
// masterclass feature where each click spawns 20 outward-radiating glyphs
// that snapshot the active palette's body color and fade out over 0.8s.
//
// v30 masterclass: render-time tone-down
//
// `QUANTUM_BODY_TONE_DOWN` is applied at render time in
// `cloud::rain::apply_quantum_ripple` so the snapshot stored on each
// particle stays equal to the palette body stop exactly (preserving the
// crossfade and "snapshot matches body stop" regression-test contracts),
// while the rendered pixel is dimmed to match the rain's perceived
// average brightness rather than the saturated body stop alone.

/// Maximum concurrent Quantum Ripple particles. Pre-allocated once at
/// Cloud init; reused via free-list. 32 covers the peak case of 2-3
/// rapid clicks (each spawns up to 25) with overlap.
pub(crate) const QUANTUM_RIPPLE_POOL_SIZE: usize = 64;

/// Particles spawned per click (fixed 20 for determinism).
pub(crate) const QUANTUM_RIPPLE_PARTICLE_COUNT: usize = 20;

/// Particle lifespan in seconds (0.8s midpoint).
pub(crate) const QUANTUM_RIPPLE_LIFETIME_SECS: f32 = 0.8;

/// Particle outward radial speed (cells/sec).
pub(crate) const QUANTUM_RIPPLE_SPEED: f32 = 18.0;

/// Brand purple RGB (same as logo color) for Quantum effects.
pub(crate) const QUANTUM_BRAND_PURPLE_R: u8 = 168;
pub(crate) const QUANTUM_BRAND_PURPLE_G: u8 = 85;
pub(crate) const QUANTUM_BRAND_PURPLE_B: u8 = 247;

/// v30 masterclass: render-time tone-down applied to each particle's
/// snapshot of the palette body color.
///
/// Owner visual testing reported that ripple particles read as "too
/// bright" — a click on the Green scheme produced saturated
/// `(0, 220, 0)`-class pixels that visually out-shone the surrounding
/// rain (whose droplets are mostly head→body→tail gradient, so the
/// average cell the eye sees is much dimmer than the body stop alone).
///
/// Rather than change the snapshot itself (the snapshot must stay
/// equal to `palette.colors[len/2]` because the crossfade tests assert
/// that exact invariant), this constant is applied at RENDER time in
/// `apply_quantum_ripple`: the per-particle RGB used for blending is
/// `p.r * QUANTUM_BODY_TONE_DOWN`, etc.
///
/// `0.72` was chosen empirically: on Green it dims `(0, 220, 0)` to
/// `(0, 158, 0)` — still clearly green and well above the trail floor
/// (~131), but no longer competing with the head stop for visual
/// dominance. On Red it dims `(220, 0, 0)` to `(158, 0, 0)`. On dark
/// themes like Cosmos the dimmed body still sits comfortably above the
/// Phase 7 floor, so no theme regresses on visibility.
///
/// The snapshot stored on the particle (`p.r/g/b`) is unchanged —
/// palette-switch crossfade and the "snapshot matches the body stop"
/// regression tests still hold. Only the rendered pixel is dimmed.
///
/// Lower values (0.5–0.65) make ripples read as ambient sparks rather
/// than a hue burst — fine on bright themes but too dim on dark themes
/// like Cosmos/Nebula where the body is already low-luminance. Higher
/// values (0.85–1.0) restore the "too bright" complaint.
pub(crate) const QUANTUM_BODY_TONE_DOWN: f32 = 0.72;

// Message overlay limits

/// Maximum message text length (characters). Prevents excessively long
/// messages from overflowing the terminal or causing layout issues.
/// 200 chars is enough for a sentence or short phrase — the message
/// box is a overlay, not a full-screen text editor.
pub(crate) const MESSAGE_MAX_LEN: usize = 200;
