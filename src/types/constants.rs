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

// ─── Power management: single source of truth ──────────────────────────────
//
// All power / perf / adaptive / thermal / xterm.js-tier-2 constants live
// in `central_control_dragon_power.rs`. This re-export keeps every
// existing `use crate::constants::*;` call site working unchanged.
// To fine-tune any power parameter, edit `central_control_dragon_power.rs`.
pub(crate) use crate::central_control_dragon_power::*;

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

// Terminal / rendering (power constants now in central_control_dragon_power.rs)

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
///
/// v50 masterclass retune: lifespan extended from 0.8s → 2.5s. Three
/// overlapping clicks (60 active) can now coexist with one extra click
/// in flight (20) → 80 worst case. Pool raised from 64 → 96 to absorb
/// this without silent drops during rapid multi-click bursts.
pub(crate) const QUANTUM_RIPPLE_POOL_SIZE: usize = 96;

/// Particles spawned per click (fixed 20 for determinism).
pub(crate) const QUANTUM_RIPPLE_PARTICLE_COUNT: usize = 20;

/// Particle lifespan in seconds.
///
/// v50 masterclass retune (owner feedback 8/10): the original 0.8s
/// lifespan was reported as "too fast dead" — the cohort vanished
/// before the eye could register the ricochet trajectory. Raised to
/// 2.5s so a click produces a multi-bounce shower that the eye reads
/// as a deliberate visual effect, then fades out gracefully via the
/// smoothstep + tail brightness curve (see `apply_quantum_ripple`).
///
/// 2.5s was chosen so that:
///  - At the default speed (30 cells/sec) the particle travels ~75
///    cells in its lifetime — enough to cross a wide viewport multiple
///    times with ricochets, matching the "masterclass" aesthetic.
///  - Three rapid clicks (60 active particles) all coexist for most
///    of their lifespan without exhausting the 96-slot pool.
///  - The fade curve's "tail" segment (last 30% of life = 0.75s) is
///    long enough to read as a graceful decay rather than a flicker.
///
/// Lower values (1.0–1.5s) restore the "too fast dead" complaint.
/// Higher values (3.5–5s) make the effect linger after the user has
/// mentally moved on, becoming visual noise.
/// v50 masterclass: 4.0s — longer life so particles coast to a natural
/// stop and fade smoothly. Combined with QUANTUM_RIPPLE_VELOCITY_DECAY,
/// particles decelerate exponentially and drift to a halt over ~3s,
/// then fade out over the remaining ~1s. The previous 2.5s was too
/// short — particles disappeared "gone fast" per owner feedback.
pub(crate) const QUANTUM_RIPPLE_LIFETIME_SECS: f32 = 4.0;

/// Particle outward radial speed (cells/sec).
///
/// v50 speed evolution: 18.0 (too fast/blur) -> 9.0 (too slow/snow)
/// -> 12.0 (better but still stuttery at 0.2 cells/frame) -> 30.0.
///
/// 30.0 cells/sec was chosen because at 60 FPS the particle moves
/// 0.5 cells/frame — it changes terminal cell every 2 frames instead
/// of every 5 frames (12.0) or every 7 frames (9.0). This is the
/// minimum speed where discrete cell-to-cell rendering feels smooth
/// in a terminal emulator. Above 60 cells/sec (1 cell/frame) the
/// motion reads as a blur again; below 12 cells/sec the stutter is
/// visually distracting.
///
/// Combined with the narrower spawn-speed variance (0.9..1.1 instead
/// of 0.8..1.2), this produces a visually coherent cohort where all
/// particles travel at similar perceived speeds — the "masterclass"
/// look the owner requested.
pub(crate) const QUANTUM_RIPPLE_SPEED: f32 = 30.0;

// ── Border-Touch Splash Crown Spark (F2) ─────────────────────────────────
// See docs/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md §3.2.
// "Rain drop hitting a glass ceiling" — 6 particles, 350ms, 1-cell trail,
// upward semicircle fan. Shares the QuantumParticle pool with quantum
// ripples (zero new allocation). Triggered by detect_border_touch on
// non-corner border cells only (LTS invariant: no lone bright heads at
// top corners).

/// Particles per border-touch spark (F2 Splash Crown).
/// 6 particles = visible "plash" without competing with message text.
pub(crate) const BORDER_SPARK_PARTICLE_COUNT: usize = 6;

/// Spark particle lifetime in seconds. 350ms = brief but visible.
pub(crate) const BORDER_SPARK_LIFETIME_SECS: f32 = 0.35;

/// Spark particle speed in cells/second. 12.0 = 40% of quantum ripple
/// (30.0) — sparks are smaller/faster than click bursts.
pub(crate) const BORDER_SPARK_SPEED: f32 = 12.0;

/// Upward semicircle fan: -180° (left) through -90° (up) to 0° (right).
/// In terminal coords, negative Y = upward. The border is a ceiling,
/// so sparks deflect up + sideways (crown splash pattern).
pub(crate) const BORDER_SPARK_ANGLE_MIN_RAD: f32 = -std::f32::consts::PI; // -180°
pub(crate) const BORDER_SPARK_ANGLE_MAX_RAD: f32 = 0.0; // 0°

/// Max trail entries per spark particle. 1 = single-cell streak
/// (vs quantum ripple's 6). Gives "spray" feel without trailing noise.
pub(crate) const BORDER_SPARK_TRAIL_LEN: usize = 1;

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

/// Velocity retention factor applied to a quantum ripple particle each
/// time it bounces off a screen edge.
///
/// Owner-requested behavior (v50 stabilization): ripple particles used
/// to die as soon as they crossed the screen border (`col >= cols` or
/// `line >= lines`). On small viewports — or when the user clicks near
/// an edge — most of the 20-particle cohort would expire in the first
/// few frames, making the burst feel clipped instead of radiating
/// outward. The requested fix is to make the particles **bounce** off
/// the four screen edges, so a click anywhere produces a visible shower
/// that ricochets around the viewport until the age-based lifespan
/// (`QUANTUM_RIPPLE_LIFETIME_SECS`) expires.
///
/// To keep the visual organic and avoid a perpetual elastic ping-pong,
/// each bounce multiplies the velocity component by this damping
/// factor. The factor applies only to the axis that bounced — the
/// perpendicular component is untouched, so the post-bounce direction
/// is the mirror of the pre-bounce direction (true specular reflection)
/// scaled by `DAMPING`.
///
/// `0.78` was chosen so that (with the v50 masterclass 2.5s lifespan):
///  - After 1 bounce, 78% speed retained — still clearly energetic.
///  - After 3 bounces, ~47% — slowing down enough that the eye reads
///    the trajectory as decaying without it abruptly stopping.
///  - After 5 bounces, ~29% — combined with the smoothstep+tail
///    brightness fade over the 2.5s lifespan, the particle is both
///    dim AND slow, naturally fading out by end-of-life.
///
/// The previous value (0.85) was tuned for the old 0.8s lifespan —
/// with 2.5s the cohort would ricochet too elastically, masking the
/// underlying rain. 0.78 keeps the ricochet visible but visibly
/// decelerating across the longer lifespan.
///
/// Lower values (0.6–0.7) make bounces die too quickly — the second
/// bounce already reads as a stop, wasting the 2.5s lifespan budget.
/// Higher values (0.9–1.0) make the cohort ricochet nearly elastically
/// for the full 2.5s, which feels chaotic and can mask the rain on
/// small viewports.
///
/// See `cloud::rain_post::apply_quantum_ripple` for the bounce math.
pub(crate) const QUANTUM_RIPPLE_BOUNCE_DAMPING: f32 = 0.78;

/// Brightness curve segment boundary — end of the "head" segment where
/// the particle is at full brightness, start of the smoothstep ramp-down.
///
/// v50 masterclass retune (owner feedback 8/10): the original fade
/// curve was `fade * fade` (quadratic from 1.0 → 0 across the full
/// lifespan). At 0.8s lifespan this was acceptable — the cohort was
/// only visible for ~0.4s anyway. At 2.5s lifespan the quadratic
/// curve spends 50% of its time below 25% brightness, making the
/// particle effectively invisible for the second half of its life —
/// the "smoothness" complaint.
///
/// The new curve has three segments:
///  1. HEAD (0 → HEAD_END_FRAC of life): full brightness (1.0). The
///     particle is at peak visibility during the initial burst outward.
///  2. BODY (HEAD_END_FRAC → TAIL_START_FRAC): smoothstep from 1.0
///     down to ~0.35. The particle visibly dims but stays clearly
///     visible — the "drifting" phase.
///  3. TAIL (TAIL_START_FRAC → 1.0): linear fade from ~0.35 → 0. The
///     particle gracefully fades out — the "fade out gone" the owner
///     requested.
///
/// `0.15` means: the first 15% of life (0.375s at 2.5s lifespan) is
/// full brightness. After that the smoothstep ramp begins.
/// v50 masterclass: 0.10 — shorter peak (10% of life) so particles
/// start fading sooner, giving a longer BODY+TAIL for smooth decay.
pub(crate) const QUANTUM_RIPPLE_HEAD_END_FRAC: f32 = 0.10;

/// Brightness curve segment boundary — start of the TAIL segment
/// (linear fade to zero). See `QUANTUM_RIPPLE_HEAD_END_FRAC` for the
/// full curve description.
///
/// v50 masterclass: 0.50 — TAIL starts at 50% of life (was 70%),
/// giving a much longer linear fade-out. Combined with lower
/// TAIL_FLOOR (0.25), the fade from BODY to invisible is smoother
/// and more gradual. The previous 0.70 made the TAIL segment only 30%
/// of life, causing particles to disappear too quickly.
pub(crate) const QUANTUM_RIPPLE_TAIL_START_FRAC: f32 = 0.50;

/// v50 masterclass: velocity decay per second. Particles decelerate
/// exponentially: v *= (1 - decay * dt) each frame. At 0.35/sec,
/// particles lose ~35% velocity per second — they coast to a natural
/// stop over ~3s (fitting the 4.0s lifespan). This replaces the
/// previous behavior where particles maintained full speed until
/// they expired (only bounce damping slowed them).
pub(crate) const QUANTUM_RIPPLE_VELOCITY_DECAY: f32 = 0.35;

// ── v50 (2026-08-17) trail particles masterclass effect ──
//
// Owner-approved alternative masterclass effect for the quantum ripple:
// each particle leaves a "comet trail" of its last N positions, rendered
// with diminishing brightness + cycled color (from C7). The trail is
// pushed every frame in apply_quantum_ripple, creating a streaking
// effect behind the moving particle.

/// Number of past positions stored per particle for the trail effect.
/// 6 positions at 60 FPS = ~100ms of trail history — enough to read as
/// a streak without being so long that it clutters the screen during
/// rapid multi-click bursts. Larger values (8-10) make the trail
/// dominate the visual; smaller values (3-4) read as a "ghost" rather
/// than a streak.
pub(crate) const QUANTUM_RIPPLE_TRAIL_LEN: usize = 6;

/// Per-step brightness decay for trail positions. Each trail position i
/// (0 = most recent past, TRAIL_LEN-1 = oldest) renders at
/// `brightness * TRAIL_DECAY^(i+1)`. A value of 0.55 produces a clear
/// streak that fades smoothly to invisible by the trail's oldest entry.
/// Higher values (0.7+) make the trail too uniform; lower values (0.35)
/// make it disappear too quickly to read as a streak.
pub(crate) const QUANTUM_RIPPLE_TRAIL_DECAY: f32 = 0.55;

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
/// field at compile time. Format: `"cosmostrix v50.0.0-beta.3"`.
///
/// Kept as a function (not a `const`) because `format!` is not `const fn`
/// on stable Rust. The cost is one allocation per process startup, which
/// is invisible next to the rest of Cloud construction.
#[must_use]
pub(crate) fn default_message_text() -> String {
    format!(
        "Experience a masterpiece with cosmostrix v{}",
        env!("CARGO_PKG_VERSION")
    )
}
