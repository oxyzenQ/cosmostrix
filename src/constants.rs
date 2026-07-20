// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Centralized named constants for the entire codebase.
//!
//! All magic numbers are extracted here to avoid duplication and
//! provide a single source of truth for tuning parameters.

// Density & sizing

/// Default cols for density auto-calculation in bench mode.
pub const DENSITY_AUTO_DEFAULT_COLS: u16 = 120;

/// Default lines for density auto-calculation in bench mode.
pub const DENSITY_AUTO_DEFAULT_LINES: u16 = 40;

/// Base terminal width for auto-density scaling.
pub const DENSITY_BASE_COLS: f32 = 80.0;

/// Base terminal height for auto-density scaling.
pub const DENSITY_BASE_LINES: f32 = 25.0;

/// Auto-density clamp range (min factor).
pub const DENSITY_AUTO_MIN: f32 = 0.5;

/// Auto-density clamp range (max factor).
pub const DENSITY_AUTO_MAX: f32 = 2.0;

/// Absolute density clamp range (min).
pub const DENSITY_CLAMP_MIN: f32 = 0.01;

/// Absolute density clamp range (max).
pub const DENSITY_CLAMP_MAX: f32 = 5.0;

/// Minimum user-facing rain speed for CLI, config, and keyboard controls.
pub const SPEED_MIN: f32 = 1.0;

/// Maximum user-facing rain speed for CLI, config, and keyboard controls.
pub const SPEED_MAX: f32 = 100.0;

/// Minimum runtime speed reachable through keyboard controls.
pub const RUNTIME_SPEED_MIN: f32 = SPEED_MIN;

/// Maximum runtime speed reachable through keyboard controls.
pub const RUNTIME_SPEED_MAX: f32 = SPEED_MAX;

/// Maximum effective Monolith speed, including CLI/config values.
pub const MONOLITH_EFFECTIVE_SPEED_MAX: f32 = SPEED_MAX;

// Performance tuning (shared between interactive & cloud)

/// Pressure spawn scaling factor: reduces spawn rate under perf pressure.
pub const PERF_PRESSURE_SPAWN_FACTOR: f32 = 0.75;

/// Minimum spawn scale under pressure.
pub const PERF_SPAWN_SCALE_MIN: f32 = 0.25;

/// Glitch threshold: disable glitch rendering when pressure exceeds this.
pub const GLITCH_THRESHOLD: f32 = 0.35;

/// Glitch brightness threshold ratio (first N% of glitch cycle = bright).
pub const GLITCH_BRIGHT_RATIO: f64 = 0.25;

/// Glitch dim threshold ratio (last N% of glitch cycle = dim).
pub const GLITCH_DIM_RATIO: f64 = 0.75;

/// Performance pressure increment per overshoot frame.
pub const PERF_PRESSURE_INCREMENT: f32 = 0.25;

/// Performance pressure decay per normal frame.
pub const PERF_PRESSURE_DECAY: f32 = 0.02;

// Cloud internals

/// Initial RNG seed.
pub const RNG_INITIAL_SEED: u64 = 0x0123_4567;

/// Droplet count multiplier (N * columns).
pub const DROPLET_COUNT_FACTOR: f32 = 1.5;

/// Character pool size.
pub const CHAR_POOL_SIZE: usize = 2048;

/// Glitch pool size.
pub const GLITCH_POOL_SIZE: usize = 1024;

/// Max char pool index used in distributions (CHAR_POOL_SIZE - 1).
pub const MAX_CHAR_POOL_IDX: u16 = 2047;

/// Re-seed interval for RNG in seconds (~10 minutes).
pub const RNG_RESEED_INTERVAL_SECS: u64 = 600;

/// Head linger brightness duration in milliseconds.
/// Raised from 100 to 300 for a smoother, more cinematic head fadeout that
/// keeps the head visually prominent for longer after it stops advancing.
/// The old 100ms was too abrupt — the bright head snapped to near-zero
/// within a few frames, undermining the head > body hierarchy.
pub const HEAD_LINGER_BRIGHTNESS_MS: u64 = 300;

// Interactive mode tuning

/// Monotonic clock jump guard: skip frame if elapsed exceeds this.
pub const CLOCK_JUMP_GUARD_SECS: f64 = 10.0;

/// Pause polling period in milliseconds.
pub const PAUSE_PERIOD_MS: u64 = 250;

/// Simulation pressure scaling factor (multiplier for base sim time).
pub const SIM_PRESSURE_SCALE_FACTOR: f64 = 0.7;

/// Minimum simulation time as fraction of frame period.
pub const SIM_MIN_FRACTION: f64 = 0.5;

/// Maximum simulation cap in seconds.
/// Capping at one 30fps frame prevents visible catch-up jumps after stalls
/// while still allowing brief scheduling hiccups to recover gracefully.
pub const SIM_MAX_CAP_SECS: f64 = 1.0 / 30.0;

/// Multiplier for frame_period to get sim_base.
pub const SIM_BASE_MULTIPLIER: f64 = 3.0;

/// Glitch percent step for Left/Right keys.
pub const GLITCH_PCT_STEP: f32 = 0.05;

/// Density step for `[`/`]` keys.
pub const DENSITY_STEP: f32 = 0.25;

/// Watchdog check interval in seconds.
pub const WATCHDOG_INTERVAL_SECS: u64 = 5;

// Terminal / rendering

/// Dirty threshold ratio: if dirty cells >= total/N, do full redraw.
pub const DIRTY_THRESHOLD_RATIO: usize = 3;

/// Graceful shutdown timeout in seconds (force-exit if flush blocks).
pub const SHUTDOWN_TIMEOUT_SECS: u64 = 2;

/// Maximum allowed terminal width (columns).  Prevents OOM from wildly
/// misreported terminal sizes (e.g. 65535 × 65535 → hundreds of GiB).
/// 1024 cols × 500 lines × ~48 bytes/cell ≈ 24 MiB — still comfortable.
pub const MAX_TERMINAL_COLS: u16 = 1024;

/// Maximum allowed terminal height (lines).  Same rationale as above.
pub const MAX_TERMINAL_LINES: u16 = 500;

/// Minimum usable terminal width (columns). Below this, the renderer
/// refuses to start to avoid degenerate edge cases (empty frame, zero
/// droplets, divide-by-zero in column math).
pub const MIN_TERMINAL_COLS: u16 = 4;

/// Minimum usable terminal height (lines). Same rationale as above.
pub const MIN_TERMINAL_LINES: u16 = 4;

/// Resize debounce window in milliseconds. Rapid resize events within this
/// window are coalesced into a single application, preventing redundant
/// full resets and visual thrashing during window drag.
pub const RESIZE_DEBOUNCE_MS: u64 = 150;

/// Seconds of no user input before entering idle mode. In idle mode the
/// effective FPS target is reduced to conserve CPU/battery, and
/// atmospheric subsystem tick rates are lowered. Any input event instantly
/// restores full performance.
pub const IDLE_THRESHOLD_SECS: f64 = 30.0;

/// Effective FPS multiplier while idle. Applied on top of the user's
/// configured FPS target to reduce update pressure during inactivity.
/// Raised from 0.25 to 0.5 (30 FPS at 60 target) to keep phosphor decay
/// and shimmer visually smooth even during idle — the old 15 FPS felt
/// choppy and undermined the cinematic smoothness improvements.
pub const IDLE_FPS_FACTOR: f64 = 0.5;

/// Wall-clock interval for one-shot full redraws while idle. This keeps
/// terminal/compositor state synchronized even when idle FPS makes the
/// frame-count drift correction too sparse in real time.
pub const IDLE_REDRAW_RESYNC_INTERVAL_SECS: f64 = 20.0;

// Benchmark

/// Minimum elapsed seconds denominator to avoid division by zero in bench.
pub const BENCH_ELAPSED_MIN_S: f64 = 0.000_001;

/// Estimated ANSI overhead bytes per drawn cell in steady-state rendering.
/// Accounts for run-encoded style changes amortized across the terminal:
/// ~19 bytes = (5-byte SGR reset + ~6-byte fg escape + ~6-byte bg escape
/// + 1-byte char) × ~0.65 run-compression factor. This is a rough estimate
///   used for throughput reporting in the benchmark, not for frame pacing.
pub const ANSI_BYTES_PER_CELL_ESTIMATE: u64 = 19;

// Config file

/// Config file directory name under XDG_CONFIG_HOME or ~/.config.
pub const CONFIG_DIR_NAME: &str = "cosmostrix";

/// Config file name.
/// Changed from "config" to "config.toml" in v10.0.0 for consistency
/// with the template file and standard TOML convention.
/// Backward compat: configfile.rs falls back to "config" if "config.toml"
/// doesn't exist (for users upgrading from pre-v10).
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// Legacy config file name (pre-v10.0.0). Used as fallback in
/// default_config_file_path() if config.toml doesn't exist.
pub const CONFIG_FILE_NAME_LEGACY: &str = "config";

/// Default frame dirty capacity pre-allocation.  One Nth of total cells.
/// 8 is conservative enough for 1024×500 terminals (≈64K pre-alloc) while
/// still covering most frames without a heap spill.
pub const DIRTY_CAPACITY_DIVISOR: usize = 8;

/// Hard cap on dirty-vec pre-allocation in cells (≈8 KiB worth of usize).
/// Prevents wasting memory when terminal is very large.
pub const DIRTY_CAPACITY_CAP: usize = 8192;

// Exponential trail fade & head bloom

/// Exponential decay rate for trail fading (higher = faster fade near head).
/// Lowered from 3.0 to 1.8 for improved body glyph readability: at K=3.0,
/// 75% of the trail was below 22% brightness (perceptually invisible); at
/// K=1.8, the same region is at 41% — clearly visible while still fading
/// smoothly toward the tail. This preserves the head > body > tail hierarchy
/// without making the trail body muddy and invisible.
pub const TRAIL_EXPONENTIAL_K: f64 = 1.8;

/// Hard cap on spawn remainder to prevent spawn debt accumulation
/// at high speeds or after timing spikes. Without this, a long stall
/// could dump hundreds of droplets into the same frame, causing
/// visual chaos and bottom-row "concrete wall" accumulation.
pub const SPAWN_REMAINDER_CAP: f32 = 4.0;

/// Absolute maximum head position (in rows from top) for the fresh-entry
/// warm-start when switching to a glyph scene at runtime. Combined with
/// the `lines/4` bound, this ensures warm-started droplets appear in the
/// upper portion of the viewport — looking freshly entered rather than
/// already in progress halfway down the screen.
pub const WARM_START_MAX_HEAD: u16 = 8;

/// Fraction of terminal columns to seed during glyph fresh-entry warm-start.
/// Lower values produce sparser initial scenes that fill naturally via the
/// spawn system and scene-entry ramp, creating a cinematic top-entry cascade
/// instead of an instant wall of rain.
pub const WARM_START_SEED_FRACTION: f32 = 0.12;

/// Minimum number of seed lanes for fresh-entry warm-start. Ensures at
/// least a few visible droplets on the first frame after switching to a
/// glyph scene, preventing blank screens on very narrow terminals.
pub const WARM_START_SEED_MIN: usize = 3;

/// Maximum number of seed lanes for fresh-entry warm-start. Prevents
/// excessive initial density on very wide terminals where 12% of columns
/// would otherwise produce too many simultaneous streams.
pub const WARM_START_SEED_MAX: usize = 12;

/// Initial spawn remainder set after glyph scene-entry warm-start. Much
/// lower than SPAWN_REMAINDER_CAP to avoid flooding the first frame with
/// natural spawn — the scene-entry ramp handles gradual fill-in instead.
pub const WARM_START_SPAWN_DEBT: f32 = 0.5;

/// Duration of the glyph scene-entry spawn ramp (ms). During this period
/// after switching to a glyph scene, spawn rate gradually increases from
/// GLYPH_ENTRY_RAMP_MIN_SCALE to full speed via smoothstep interpolation,
/// creating a cinematic top-entry cascade instead of an instant wall.
pub const GLYPH_ENTRY_RAMP_DURATION_MS: u32 = 500;

/// Minimum spawn scale at the start of the glyph entry ramp (fraction of
/// normal spawn rate). At the moment of scene switch, spawn begins at this
/// rate and smoothly ramps to 1.0 over GLYPH_ENTRY_RAMP_DURATION_MS.
pub const GLYPH_ENTRY_RAMP_MIN_SCALE: f32 = 0.25;

/// Hard cap on droplet advance remainder per frame. Without this,
/// high speed settings can cause a single advance() call to move
/// a droplet many rows at once, dumping many cells into the same
/// bottom rows and creating permanent blocky residue.
pub const ADVANCE_REMAINDER_CAP: f32 = 3.0;

// Cinematic color transition (generation-based palette propagation)

/// Maximum number of concurrent palette generations tracked simultaneously.
/// Old palettes are kept alive until all droplets using them have died,
/// at which point the slot can be reused. 4 slots covers rapid cycling
/// (by which time old droplets will have expired naturally).
pub const MAX_PALETTE_SLOTS: usize = 4;

/// Palette transition duration in milliseconds.
/// Fast enough that a keypress feels confirmed immediately while still leaving
/// room for a visible cinematic cascade.
pub const COLOR_TRANSITION_DURATION_MS: u16 = 150;

/// Minimum fraction of columns that adopt a new palette on the first
/// transition frame. This avoids the "one tiny column changed" perception.
pub const COLOR_TRANSITION_INITIAL_VISIBLE_PCT: f32 = 0.12;

/// Charset transition duration in milliseconds.
/// Uses a top-to-bottom wave so glyph identity changes read as intentional
/// motion instead of an instant full-screen snap.
pub const CHARSET_TRANSITION_DURATION_MS: u16 = 240;

/// Velocity boost for new-generation streams during an active transition.
/// Creates a subtle feeling of an incoming wave (3-8% range, 5% default).
pub const TRANSITION_VELOCITY_BOOST: f32 = 0.05;

/// Duration (seconds) over which new-palette streams have enhanced energy.
/// Fresh streams start brighter and settle to normal over this period.
pub const TRANSITION_ENERGY_DURATION_SECS: f32 = 1.5;

/// Saturation/brightness boost for freshly spawned transition streams.
/// New-palette heads glow slightly brighter to visually "push" the old palette.
pub const TRANSITION_ENERGY_SATURATION_BOOST: f32 = 0.15;

/// Additional head bloom glow for new-palette droplets during transition.
/// Makes the leading edge of the new color ecosystem feel more energetic.
pub const TRANSITION_HEAD_GLOW_BOOST: f32 = 0.2;

// Gravity acceleration

/// Gravity acceleration for droplets (chars/s²).
pub const DROPLET_GRAVITY: f32 = 2.0;

/// Terminal velocity multiplier (fraction of chars_per_sec).
pub const DROPLET_TERMINAL_VELOCITY_MULT: f32 = 1.8;

// Cinematic startup easing

/// Initial velocity as fraction of chars_per_sec when a stream is born.
/// Much lower than the old 0.3 for a more gradual, organic appearance.
pub const STARTUP_VELOCITY_FRACTION: f32 = 0.05;

/// Time in seconds for startup easing to reach ~95% of full velocity.
/// Uses exponential ease: v = target × (1 - e^(-t/tau)).
pub const STARTUP_EASE_TAU: f32 = 0.15;

// Head bloom (exponential gaussian falloff)

/// Bloom sigma (spread) for exponential gaussian head glow.
/// Higher = wider, softer glow. 1.5 gives a natural falloff over ~3 cells.
pub const HEAD_BLOOM_SIGMA: f32 = 1.5;

/// Bloom glow intensity at the head cell itself (0.0 = off, 1.0 = full white blend).
pub const HEAD_BLOOM_INTENSITY: f32 = 0.4;

/// Number of cells behind the head that receive bloom glow effect.
pub const HEAD_BLOOM_CELLS: u16 = 3;

// Depth fog vignette

/// Number of rows at top and bottom for fog vignette effect.
pub const FOG_ROWS: u16 = 4;

/// Minimum brightness factor at fog edges (0.0 = invisible, 1.0 = full).
/// Raised from 0.25 to 0.35 — the old value made edge-row glyphs perceptually
/// ~5% (effectively invisible). 0.35 (~14% perceptual) preserves the vignette
/// effect while keeping edge glyphs faintly visible rather than lost entirely.
pub const FOG_MIN_FACTOR: f32 = 0.45;

// Mouse interaction

/// Mouse interaction: radius around cursor (in columns) where droplets avoid.
pub const MOUSE_AVOID_RADIUS_COLS: u16 = 5;

/// Cursor glow: horizontal radius (in columns) for the brightness boost.
pub const MOUSE_GLOW_RADIUS_COLS: f32 = 6.0;

/// Cursor glow: vertical radius (in lines) for the brightness boost.
pub const MOUSE_GLOW_RADIUS_LINES: f32 = 4.0;

/// Cursor glow: peak intensity at cursor center (0.0 = off, 1.0 = full white).
///
/// v17 audit: lowered from 0.35 to 0.15. The old value combined with the
/// head cell's 0.45 self-bloom produced ~64% white blend on head cells under
/// the cursor — brighter than the `storm` scene's `Intense` glitch. The new
/// value produces a subtle ambient halo (~53% combined) that reads as a
/// cursor cue rather than a "storm of bright comets". See research item 6.
pub const MOUSE_GLOW_INTENSITY: f32 = 0.15;

/// Click flash: ring expansion speed (columns per second).
pub const MOUSE_FLASH_SPEED: f32 = 25.0;

/// Click flash: thickness of the glowing ring (in columns).
pub const MOUSE_FLASH_RING_WIDTH: f32 = 3.0;

/// Click flash: peak intensity at ring center (0.0 = off, 1.0 = full white).
///
/// v17 audit: lowered from 0.60 to 0.30. The old value combined with cursor
/// glow + head self-bloom produced up to 86% white blend on click-under-cursor
/// cells — visually indistinguishable from a glitch flash. The new value
/// gives a visible but gentle ripple (~58% combined peak) that does not
/// compete with the rain's natural brightness hierarchy.
pub const MOUSE_FLASH_INTENSITY: f32 = 0.30;

/// Click flash: total duration of the ripple effect in seconds.
pub const MOUSE_FLASH_DURATION_SECS: f32 = 0.5;

// Parallax depth layers

/// Number of parallax depth layers.
pub const PARALLAX_LAYERS: usize = 3;

/// Per-layer speed multiplier (layer 0 = far, 2 = near).
pub const PARALLAX_SPEED_MULT: [f32; PARALLAX_LAYERS] = [0.35, 1.0, 1.7];

/// Per-layer brightness multiplier (layer 0 = dim, 2 = bright).
/// Raised from [0.55, 0.90, 1.0] to [0.70, 0.90, 1.0] for improved
/// background rain visibility. The old far-layer at 55% was perceptually
/// ~14% (nearly invisible after other dimming); 70% is perceptually ~18%
/// — still clearly dimmer than the near layer but actually visible.
pub const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.70, 0.90, 1.0];

/// Per-layer length multiplier (layer 0 = short, 2 = long).
pub const PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.5, 1.0, 1.4];

// Phosphor persistence (CRT afterglow)

/// Per-cell phosphor energy decay rate (higher = faster fade).
/// Raised from 3.0 to 5.0 for crisper, more energetic trail fade.
/// Film Matrix afterglow is ~200ms; at 5.0, afterglow lasts ~400ms
/// (still 2× film, but 2.7× faster than old 1094ms).
pub const PHOSPHOR_DECAY_RATE: f32 = 5.0;

/// Energy level when a cell's tail passes (starts the phosphor glow).
/// Lowered from 160 to 120 for crisper trail. At 120 (~47% brightness),
/// ghost cells are visible for ~400ms then fade — matching film Matrix
/// energy. The bottom-row "concrete wall" artifact is still prevented by
/// the PHOSPHOR_GLYPH_THRESHOLD (96) and PHOSPHOR_BOTTOM_DECAY_MULT (2.5).
pub const PHOSPHOR_TAIL_RESIDUAL: u8 = 120;

/// Below this energy, the cell is cleared to blank.
pub const PHOSPHOR_DEAD_THRESHOLD: u8 = 6;

/// Minimum phosphor energy for rendering the original character glyph in
/// ghost cells. Below this threshold, the ghost cell renders as a blank
/// space (or dim color-only patch) instead of the original character.
/// This prevents stale cells from filling the background with dark charset
/// glyphs — especially during force_draw_everything events (paste, focus
/// regain) where a full redraw would expose all ghost glyphs at once.
///
/// At 96 (~38% of max), ghost characters are visible for about the first
/// ~400ms of afterglow (from PHOSPHOR_TAIL_RESIDUAL=160), then the glyph
/// vanishes and only a dim color patch remains for the final ~600ms of
/// energy decay. This preserves the "fading text" cinematic effect for
/// recently passed trails while preventing stale background charset fill.
pub const PHOSPHOR_GLYPH_THRESHOLD: u8 = 96;

/// Per-layer phosphor decay rate multiplier (far=fast, near=slow).
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [1.6, 1.0, 0.7];

/// Number of rows from the bottom of the screen where phosphor decay is
/// accelerated. Ghost cells near the bottom accumulate into a static
/// "concrete wall" because droplets end there and fewer new streams
/// overwrite the residue. Accelerating decay in this region clears
/// afterglow faster without affecting the cinematic look elsewhere.
pub const PHOSPHOR_BOTTOM_ROWS: u16 = 8;

/// Phosphor decay rate multiplier applied to bottom rows. Combined with
/// PHOSPHOR_DECAY_RATE=3.0, this yields an effective rate of 7.5 at the
/// bottom, reducing afterglow duration from ~1.0s to ~0.4s.
pub const PHOSPHOR_BOTTOM_DECAY_MULT: f32 = 2.5;

// Atmospheric depth layering enhancements

/// Per-layer spawn density multiplier (far = sparse, near = dense).
pub const PARALLAX_DENSITY_MULT: [f32; PARALLAX_LAYERS] = [0.5, 1.0, 1.5];

/// Per-layer glyph simplicity: far layer chars are less visually dense.
/// Implemented as a brightness modifier on top of PARALLAX_BRIGHTNESS_MULT.
/// Raised from [0.7, 1.0, 1.0] to [0.85, 1.0, 1.0] — the far layer is
/// already dimmed significantly by PARALLAX_BRIGHTNESS_MULT; stacking an
/// additional 30% dim on top made it nearly invisible. 15% is enough to
/// create visual separation without making far-layer glyphs disappear.
pub const PARALLAX_GLYPH_DIM: [f32; PARALLAX_LAYERS] = [0.85, 1.0, 1.0];

/// Per-layer contrast reduction (depth-of-field perceptual blur).
/// Layer 0 (background) gets its foreground color blended toward the
/// background by this factor, creating a "foggy/out-of-focus" look.
/// 0.0 = no reduction (sharp), 0.5 = 50% blend toward bg (foggy).
/// Only layer 0 has contrast reduction — layers 1-2 stay sharp.
/// This is the terminal equivalent of depth-of-field blur: instead of
/// blurring pixels (impossible in text), we reduce fg-bg contrast so
/// the background rain reads as "behind a haze".
pub const PARALLAX_CONTRAST_REDUCTION: [f32; PARALLAX_LAYERS] = [0.35, 0.0, 0.0];

// Velocity turbulence

/// Maximum velocity perturbation as fraction of base chars_per_sec.
pub const TURBULENCE_AMPLITUDE: f32 = 0.08;

/// Turbulence oscillation frequency (Hz). Controls how often drift changes.
pub const TURBULENCE_FREQ: f32 = 0.4;

// Cinematic perceived smoothness (fractional advance & shimmer)

/// Fractional head brightness amplitude: how much the head cell brightness
/// varies based on fractional row progress between advances. 0.15 means the
/// head brightens up to 15% as it approaches the next row, creating a subtle
/// "energy building" pulse that makes every frame feel visually different even
/// when the head hasn't moved to a new row. This is the key to perceived
/// smoothness at default speed — without it, the head only changes every
/// ~8 frames (at 8 chars/sec, 60 FPS), making the rain feel like 8 FPS.
pub const FRACTIONAL_HEAD_BRIGHTNESS_AMP: f32 = 0.15;

/// Fractional bloom modulation: how much the head bloom glow intensifies
/// based on fractional progress. Works alongside FRACTIONAL_HEAD_BRIGHTNESS_AMP
/// to create a per-frame visual pulse that makes the leading edge feel alive.
pub const FRACTIONAL_BLOOM_AMP: f32 = 0.10;

/// Head glyph shimmer period in seconds. The head character cycles to a new
/// glyph from the char pool at this interval, creating subtle "churn" that
/// makes active cells feel alive without noisy flicker. At 0.12s, the head
/// changes character ~8 times per second — frequent enough to notice but
/// slow enough to avoid distraction.
pub const HEAD_SHIMMER_PERIOD_SECS: f32 = 0.10;

/// Whether to add random fractional phase offset when spawning a droplet.
/// When true, new droplets start with a random `advance_remainder` so they
/// don't all advance on the same frame cadence. This breaks the "robotic"
/// synchronized march where every stream moves its head on the same tick.
pub const SPAWN_PHASE_JITTER: bool = true;

/// Trail character cycling probability per decay step.
/// Each time a phosphor trail cell is re-rendered during decay, there is
/// this chance the character mutates to a new random glyph from the char
/// pool. At 0.02 (2%), roughly 1 in 50 trail cells change character per
/// decay step — subtle enough to feel organic, frequent enough to make
/// the rain feel "alive" throughout the trail, not just at the head.
/// This matches the film Matrix effect where background characters
/// subtly shift.
pub const TRAIL_CYCLE_PROBABILITY: f32 = 0.02;

// Rare anomaly events

/// Probability of an anomaly occurring per second (~1 every 60s).
pub const ANOMALY_CHANCE_PER_SEC: f64 = 0.017;

/// Duration of an anomaly event in seconds.
pub const ANOMALY_DURATION_SECS: f32 = 1.5;

/// Maximum number of active anomaly zones.
pub const ANOMALY_MAX_ZONES: usize = 3;

/// Anomaly intensity (brightness boost for luminance surge, 0.0-1.0).
pub const ANOMALY_LUMINANCE_INTENSITY: f32 = 0.3;

/// Anomaly corruption probability per cell in zone.
pub const ANOMALY_CORRUPTION_CHANCE: f32 = 0.4;

// Temporal color ecosystems

/// How often the color ecosystem evaluates a drift (in seconds).
/// Low-frequency — only checks every few seconds to avoid per-frame cost.
pub const COLOR_ECOSYSTEM_TICK_SECS: f32 = 3.0;

/// Maximum luminance climate shift per tick (0.0-1.0).
/// Very slow drift so changes are barely perceptible per tick.
pub const COLOR_CLIMATE_DRIFT_RATE: f32 = 0.008;

/// Maximum saturation climate shift per tick (0.0-1.0).
pub const COLOR_SATURATION_DRIFT_RATE: f32 = 0.005;

/// Maximum hue rotation per tick (in radians, very small).
pub const COLOR_HUE_DRIFT_RATE: f32 = 0.015;

/// Probability per ecosystem tick that a drift direction changes.
pub const COLOR_DRIFT_REEVAL_CHANCE: f32 = 0.15;

/// Luminance climate bounds (min/max global brightness modifier).
/// Raised minimum from 0.6 to 0.75 — the old minimum of 0.6 combined with
/// profile luminance_offset could drop effective brightness to ~50%, making
/// active glyphs muddy and hard to read. 0.75 ensures the atmosphere never
/// dims below a clearly readable level.
pub const COLOR_LUMINANCE_CLIMATE_MIN: f32 = 0.75;
pub const COLOR_LUMINANCE_CLIMATE_MAX: f32 = 1.0;

/// Saturation climate bounds (min/max global saturation modifier).
/// Raised minimum from 0.5 to 0.7 — excessive desaturation makes colors
/// feel washed out and gray, undermining the premium green aesthetic.
pub const COLOR_SATURATION_CLIMATE_MIN: f32 = 0.7;
pub const COLOR_SATURATION_CLIMATE_MAX: f32 = 1.0;

/// Probability per ecosystem tick that an autonomous palette drift occurs.
/// At 3s ticks, 0.03 ≈ one drift attempt every ~100 seconds.
pub const AUTONOMOUS_PALETTE_DRIFT_CHANCE: f32 = 0.03;

/// Default for autonomous palette drift: disabled by default so that
/// explicit CLI/config/profile color remains sticky. Users who want
/// atmospheric color evolution can opt in via `--auto-color-drift` or
/// `auto-color-drift = true` in their config file.
pub const AUTO_COLOR_DRIFT_DEFAULT: bool = false;

// Cinematic runtime behavior profiles

/// Duration for a profile transition (seconds).
pub const PROFILE_TRANSITION_SECS: f32 = 30.0;

/// How often the profile state interpolates toward target (in seconds).
pub const PROFILE_INTERPOLATION_RATE: f32 = 0.02;

// Autonomous atmospheric evolution

/// How often the atmospheric evolution system ticks (in seconds).
pub const ATMOSPHERE_TICK_SECS: f32 = 5.0;

/// Duration of a full entropy cycle (calm → energetic → calm) in seconds.
pub const ENTROPY_CYCLE_SECS: f32 = 300.0;

/// Maximum density migration multiplier from atmospheric evolution.
pub const ATMOSPHERE_DENSITY_RANGE: f32 = 0.4;

/// Maximum luminance climate shift from atmospheric evolution.
pub const ATMOSPHERE_LUMINANCE_RANGE: f32 = 0.2;

/// Maximum anomaly pressure shift from atmospheric evolution.
pub const ATMOSPHERE_ANOMALY_RANGE: f32 = 0.5;

// Long-timescale renderer memory

/// Number of atmospheric history samples retained.
pub const MEMORY_HISTORY_SAMPLES: usize = 32;

/// How often a memory sample is recorded (in seconds).
pub const MEMORY_SAMPLE_INTERVAL_SECS: f32 = 30.0;

/// How much historical anomaly density increases instability pressure.
pub const MEMORY_ANOMALY_PRESSURE_WEIGHT: f32 = 0.3;

/// How much historical calm increases persistence richness.
pub const MEMORY_CALM_PERSISTENCE_BOOST: f32 = 0.15;

// Emergent visual storytelling

/// How often the storytelling system evaluates emergence (in seconds).
pub const STORYTELLING_TICK_SECS: f32 = 10.0;

/// Probability per storytelling tick that an emergent moment triggers,
/// given all convergence conditions are met. Very rare by design.
pub const EMERGENT_MOMENT_CHANCE: f32 = 0.08;

/// Duration of an emergent atmospheric moment (seconds).
pub const EMERGENT_MOMENT_DURATION_SECS: f32 = 8.0;

/// Maximum number of concurrent emergent moments.
pub const EMERGENT_MAX_MOMENTS: usize = 1;

/// Intensity of emergent luminance surge (subtle, not flashy).
pub const EMERGENT_LUMINANCE_INTENSITY: f32 = 0.12;

/// Intensity of emergent density pulse (subtle spawning surge).
pub const EMERGENT_DENSITY_INTENSITY: f32 = 0.25;

/// Intensity of emergent speed shift (slow-motion or acceleration).
pub const EMERGENT_SPEED_SHIFT: f32 = 0.15;

// Cinematic resume easing (pause → resume transition)

/// Duration of the smoothstep resume ease-in curve (seconds).
/// The simulation time scale interpolates from 0.0 → 1.0 over this period
/// using a smoothstep S-curve, producing a cinematic inertia recovery that
/// starts gently (no snap) and ends smoothly (no jank at full speed).
/// 180ms is enough to eliminate catch-up harshness while keeping resume snappy.
pub const RESUME_EASE_DURATION_SECS: f32 = 0.18;

// Hardening: drift correction & terminal safety

/// Interval (in frames) between forced full screen redraws.
/// Prevents accumulated ANSI state desync over long sessions.
/// At 60fps this triggers roughly every 5 minutes — frequent enough to
/// catch drift but rare enough for zero perceptual impact.
pub const FULL_REDRAW_INTERVAL_FRAMES: u64 = 18000;

// Viewport edge fade (smooth entry/exit at terminal borders)

/// Number of rows from viewport edges for smooth entry/exit fade.
/// Applied after all visual effects including head bloom, ensuring
/// the fade takes priority over head brightness at edges. Covers a
/// smaller zone than FOG_ROWS so the two effects complement without
/// excessive stacking (fog handles rows 3; edge fade handles rows 0-2).
pub const EDGE_FADE_ROWS: u16 = 3;

/// Minimum brightness at the very top edge (row 0).
/// At 0.55, the first visible row is moderately dimmed, creating a
/// subtle emergence effect as rain enters from just beyond the top
/// border. Combined with the existing fog vignette (FOG_MIN_FACTOR=0.35),
/// the effective brightness at row 0 is ~0.55 × 0.35 ≈ 0.19 — visible
/// but subdued, giving the cinematic "entering the frame" feel.
pub const EDGE_FADE_TOP_MIN: f32 = 0.55;

/// Minimum brightness at the very bottom edge (last row).
/// Raised from 0.20 to 0.45 for visible bottom border. Combined with
/// fog (0.35), effective brightness at last row is ~0.45 × 0.35 ≈ 0.16 —
/// dim but visible, preserving cinema framing without near-invisible border.
pub const EDGE_FADE_BOTTOM_MIN: f32 = 0.45;

/// Threshold for bold suppression at viewport edges. When the edge
/// fade factor is below this value, bold is forced off to prevent bold
/// glyphs from creating harsh bright spots right at the border.
pub const EDGE_FADE_BOLD_THRESHOLD: f32 = 0.5;

/// Phosphor energy cap for cells in the bottom viewport edge zone.
/// Normally phosphor captures full energy (255) for freshly drawn cells,
/// but at the bottom edge this creates persistent bright ghost residue
/// from dying droplet heads. Capping at 64 (~25% brightness), then tapering
/// lower toward the final row, ensures ghost cells at the bottom fade out
/// quickly with the existing PHOSPHOR_BOTTOM_DECAY_MULT (2.5×) acceleration,
/// preventing the horizontal residue line artifact.
pub const PHOSPHOR_EDGE_ENERGY_CAP: u8 = 64;

/// Additional per-row phosphor cap reduction toward the final bottom row.
/// The upper edge-fade row keeps PHOSPHOR_EDGE_ENERGY_CAP, while lower rows
/// taper down slightly so the terminal border itself never carries the same
/// afterglow energy as the rows above it.
pub const PHOSPHOR_EDGE_ROW_TAPER: u8 = 8;

// Atmospheric Event Engine (v10.0.0)

/// XOR seed offset for the event RNG (derived from Cloud's RNG seed).
pub const EVENT_RNG_XOR: u64 = 0xCAFE_BABE_1337_0420;

/// Phosphor seeding energy for event afterglow.
pub const EVENT_PHOSPHOR_SEED_ENERGY: u8 = 160;

/// Maximum phosphor decay frames before event residue is force-cleared.
pub const EVENT_MAX_PHOSPHOR_DECAY_FRAMES: u64 = 90;

/// Trigger evaluation is skipped when perf_pressure exceeds this.
pub const EVENT_PERF_GATE: f32 = 0.5;

// Phosphor Ghost (v10.0.0 Flash Pivot)

/// Per-tick probability of spawning a phosphor ghost kanji character.
pub const GHOST_SPAWN_CHANCE_PER_TICK: f64 = 0.003;
/// Maximum number of active ghost events.
pub const GHOST_MAX_ACTIVE: usize = 1;

// Message overlay limits

/// Maximum message text length (characters). Prevents excessively long
/// messages from overflowing the terminal or causing layout issues.
/// 200 chars is enough for a sentence or short phrase — the message
/// box is a overlay, not a full-screen text editor.
pub const MESSAGE_MAX_LEN: usize = 200;
