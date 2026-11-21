// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

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

// ─── Power management: single source of truth ──────────────────────────────
//
// All power / perf / adaptive / thermal / xterm.js-tier-2 constants live
// in `central_control_power_dragon.rs`. This re-export keeps every
// existing `use crate::constants::*;` call site working unchanged.
// To fine-tune any power parameter, edit `central_control_power_dragon.rs`.
pub(crate) use crate::central_control_power_dragon::*;

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

// Terminal / rendering (power constants now in central_control_power_dragon.rs)

/// Dirty threshold ratio: if dirty cells >= total/N, do full redraw.
///
/// Dragon-fight experiment : bumped from 3 → 8 based on the
/// `threshold_sweep` cosmic dragon egg benchmark. The crossover point
/// where diff-path cost equals full-redraw cost is size-independent
/// at ~13% dirty (4 bytes/cell full-redraw vs 30 bytes/dirty diff).
/// The old `3` (33%) was 2.5× too permissive — diff path stayed active
/// even when full-redraw would be 7.5× cheaper. The new `8` (12.5%)
/// captures the crossover without adaptive `match terminal_size` logic
/// (the crossover is constant across sizes 4×4 through 300×80 because
/// the cost model is linear in cell count for both paths).
///
/// Benefit: 7.5× byte reduction at 25% dirty frames (e.g. 200×60:
/// 90KB → 48KB per frame). Zero visual change — same cells are drawn,
/// just via the cheaper path.
pub(crate) const DIRTY_THRESHOLD_RATIO: usize = 8;

/// Maximum allowed terminal width (columns) for interactive mode.
/// Prevents OOM from wildly misreported terminal sizes (e.g. 65535 × 65535 → hundreds of GiB).
/// 1024 cols × 500 lines × ~48 bytes/cell ≈ 24 MiB — still comfortable.
pub(crate) const MAX_TERMINAL_COLS: u16 = 1024;

/// Maximum allowed terminal height (lines) for interactive mode.  Same rationale as above.
pub(crate) const MAX_TERMINAL_LINES: u16 = 500;

/// Maximum screen size for benchmark mode (columns).
///
/// Set to 8K UHD width (7680). This is the largest meaningful benchmark
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
/// Cosmic dragon verdict to "8k or 4k?": 8K UHD is the maximum. 4K is the
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
/// refuses to start to avoid degenerate edge cases (zero droplets,
/// divide-by-zero in column math). 1x1 is the absolute floor — a single
/// cell is a valid (if trivial) render target.
pub(crate) const MIN_TERMINAL_COLS: u16 = 1;

/// Minimum usable terminal height (lines). Same rationale as above.
pub(crate) const MIN_TERMINAL_LINES: u16 = 1;

/// Resize debounce window in milliseconds. Rapid resize events within this
/// window are coalesced into a single application, preventing redundant
/// full resets and visual thrashing during window drag.
pub(crate) const RESIZE_DEBOUNCE_MS: u64 = 150;

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

/// Config file name.  removed the pre-v10 `config` (no extension)
/// fallback — users upgrading from pre-v10 must rename their file.
pub(crate) const CONFIG_FILE_NAME: &str = "config.toml";

/// Hard size cap for any config file read (S-master-3-v2 hardening).
///
/// Real configs are 1-10 KB; 1 MiB gives ~100x headroom while still
/// bounding the worst case. Applied by `config_io::read_config_capped`
/// to every config read path (startup load, /etc fallback, live-reload
/// watcher reparse, ambient ground-truth check) so a runaway or
/// maliciously large file in a whitelisted config dir cannot trigger
/// an unbounded memory read (OOM DoS) — the file is instead treated
/// as unreadable and defaults apply.
pub(crate) const CONFIG_FILE_MAX_BYTES: u64 = 1024 * 1024;

/// Default frame dirty capacity pre-allocation.  One Nth of total cells.
/// 8 is conservative enough for 1024×500 terminals (≈64K pre-alloc) while
/// still covering most frames without a heap spill.
pub(crate) const DIRTY_CAPACITY_DIVISOR: usize = 8;

/// Hard cap on dirty-vec pre-allocation in cells (≈8 KiB worth of usize).
/// Prevents wasting memory when terminal is very large.
pub(crate) const DIRTY_CAPACITY_CAP: usize = 8192;

// ── Renderer buffer pre-allocation ( perf polish) ──────────────────────
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

// ── Benchmark warmup tuning ( perf polish) ─────────────────────────────

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

// ── msg-fill-style "engrave" spark (v80.0.0-beta.1) ─────────────────────────────────
// The engrave spark tuning constants (ENGRAVE_SPARK_POOL_SIZE /
// ENGRAVE_SPARKS_PER_HEAD / ENGRAVE_SPARK_LIFETIME_SECS /
// ENGRAVE_SPARK_SPEED) moved to `msg_fill_style/engrave.rs` in the
// one-file-per-style refactor — they are engrave-only tuning and belong
// with the rest of the engrave style, not in the shared engine
// constants.

// ── S-master-HUNT-22: real-time particle physics clock ──
//
// All transient particle systems (EngraveSpark, ScorchSmoke) integrate
// physics on REAL elapsed time, capped by this single anti-teleport bound. This
// replaces the old `min(dt_raw, 1/30, sim_cap)` chain, which silently
// dilated particle time on slow terminals (see the constant's doc
// comment below for the full failure analysis).

/// Anti-teleport cap for transient particle physics dt (seconds).
///
/// S-master-HUNT-22 (VTE particle stuck/hang, round 2). Particle systems
/// previously integrated `dt = min(dt_raw, 1/30, sim_cap)` per frame.
/// On slow CPU-rendered terminals (VTE: GNOME Terminal, Konsole) the
/// real frame interval is 67-200ms while the cap chain admits only
/// 15-33ms — so each frame advanced particles a fraction of the wall
/// clock that actually passed. The effects ran in permanent slow
/// motion: msg-fill sparks and smoke lingered far past their intended
/// lifetimes and drifted visibly behind the wall clock — the exact
/// "slow, then stuck, then vanishes by itself" symptom reported on
/// VTE.
///
/// The fix: particles integrate REAL time (like the co-spawned flash
/// wave, the border-touch pulse, and ghost events, which already age
/// by `now - birth`), with this generous anti-teleport bound for the
/// pathological cases the 1/30 clamp was originally aimed at:
/// focus loss, SIGSTOP, and the first frame after a stall. 250ms
/// lets a 10 FPS terminal advance a full 100ms of physics per frame,
/// so effects complete in their intended wall-clock duration.
///
/// Pause safety is unaffected: fully-paused runs never call the
/// particle update (rain_at early-returns), the pause/resume easing
/// still scales dt via `resume_blend`, and the pause bookkeeping
/// shifts the per-system `last_update` timestamps forward on unpause.
/// Rain/monolith keep the separate `max_sim_delta` dilated clock on
/// purpose: the rain is an ambient field where slow motion reads as
/// calm.
pub(crate) const PARTICLE_MAX_FRAME_DT_SECS: f32 = 0.25;

// Message overlay limits

/// Maximum message text length (characters). Prevents excessively long
/// messages from overflowing the terminal or causing layout issues.
/// 200 chars is enough for a sentence or short phrase — the message
/// box is a overlay, not a full-screen text editor.
pub(crate) const MESSAGE_MAX_LEN: usize = 200;

/// Default overlay message text shown when neither CLI (-m / -mb) nor
/// config (`message` / `message-border`) provides a message AND the
/// interactive mode is active (not benchmark).
///
/// Built dynamically from `env!("CARGO_PKG_VERSION")` so the version
/// number is never stale — it tracks the Cargo.toml `[package] version`
/// field at compile time. Format: `"Experience with cosmostrix v50.0.0-beta.3"`.
///
/// Kept as a function (not a `const`) because `format!` is not `const fn`
/// on stable Rust. The cost is one allocation per process startup, which
/// is invisible next to the rest of Cloud construction.
#[must_use]
pub(crate) fn default_message_text() -> String {
    format!("Experience with cosmostrix v{}", env!("CARGO_PKG_VERSION"))
}
