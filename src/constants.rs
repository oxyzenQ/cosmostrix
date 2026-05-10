// Copyright (c) 2026 rezky_nightky

//! Centralized named constants for the entire codebase.
//!
//! All magic numbers are extracted here to avoid duplication and
//! provide a single source of truth for tuning parameters.

// ---------------------------------------------------------------------------
// Density & sizing
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Performance tuning (shared between interactive & cloud)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cloud internals
// ---------------------------------------------------------------------------

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
pub const HEAD_LINGER_BRIGHTNESS_MS: u64 = 100;

// ---------------------------------------------------------------------------
// Interactive mode tuning
// ---------------------------------------------------------------------------

/// Monotonic clock jump guard: skip frame if elapsed exceeds this.
pub const CLOCK_JUMP_GUARD_SECS: f64 = 10.0;

/// Pause polling period in milliseconds.
pub const PAUSE_PERIOD_MS: u64 = 250;

/// Simulation pressure scaling factor (multiplier for base sim time).
pub const SIM_PRESSURE_SCALE_FACTOR: f64 = 0.7;

/// Minimum simulation time as fraction of frame period.
pub const SIM_MIN_FRACTION: f64 = 0.5;

/// Maximum simulation cap in seconds.
pub const SIM_MAX_CAP_SECS: f64 = 0.5;

/// Multiplier for frame_period to get sim_base.
pub const SIM_BASE_MULTIPLIER: f64 = 3.0;

/// Glitch percent step for Left/Right keys.
pub const GLITCH_PCT_STEP: f32 = 0.05;

/// Density step for `[`/`]` keys.
pub const DENSITY_STEP: f32 = 0.25;

/// Watchdog check interval in seconds.
pub const WATCHDOG_INTERVAL_SECS: u64 = 5;

// ---------------------------------------------------------------------------
// Terminal / rendering
// ---------------------------------------------------------------------------

/// Dirty threshold ratio: if dirty cells >= total/N, do full redraw.
pub const DIRTY_THRESHOLD_RATIO: usize = 3;

/// Graceful shutdown timeout in seconds (force-exit if flush blocks).
pub const SHUTDOWN_TIMEOUT_SECS: u64 = 2;

// ---------------------------------------------------------------------------
// Benchmark
// ---------------------------------------------------------------------------

/// Minimum elapsed seconds denominator to avoid division by zero in bench.
pub const BENCH_ELAPSED_MIN_S: f64 = 0.000_001;

// ---------------------------------------------------------------------------
// Config file
// ---------------------------------------------------------------------------

/// Config file directory name under XDG_CONFIG_HOME or ~/.config.
pub const CONFIG_DIR_NAME: &str = "cosmostrix";

/// Config file name.
pub const CONFIG_FILE_NAME: &str = "config";

/// Default frame dirty capacity pre-allocation (1/4 of total cells).
pub const DIRTY_CAPACITY_DIVISOR: usize = 4;

// ---------------------------------------------------------------------------
// Exponential trail fade & head bloom
// ---------------------------------------------------------------------------

/// Exponential decay rate for trail fading (higher = faster fade near head).
pub const TRAIL_EXPONENTIAL_K: f64 = 3.0;

/// Number of cells behind the head that get bloom glow effect.
pub const HEAD_BLOOM_CELLS: u16 = 3;

/// Bloom glow intensity (0.0 = off, 1.0 = full white blend).
pub const HEAD_BLOOM_INTENSITY: f32 = 0.4;

// ---------------------------------------------------------------------------
// Color scheme crossfade
// ---------------------------------------------------------------------------

/// Duration of color scheme crossfade in seconds.
pub const COLOR_CROSSFADE_DURATION_SECS: f64 = 1.5;

// ---------------------------------------------------------------------------
// Gravity acceleration
// ---------------------------------------------------------------------------

/// Gravity acceleration for droplets (chars/s²).
pub const DROPLET_GRAVITY: f32 = 2.0;

/// Terminal velocity multiplier (fraction of chars_per_sec).
pub const DROPLET_TERMINAL_VELOCITY_MULT: f32 = 1.8;

// ---------------------------------------------------------------------------
// Depth fog vignette
// ---------------------------------------------------------------------------

/// Number of rows at top and bottom for fog vignette effect.
pub const FOG_ROWS: u16 = 4;

/// Minimum brightness factor at fog edges (0.0 = invisible, 1.0 = full).
pub const FOG_MIN_FACTOR: f32 = 0.25;

// ---------------------------------------------------------------------------
// Mouse interaction
// ---------------------------------------------------------------------------

/// Mouse interaction: radius around cursor (in columns) where droplets avoid.
pub const MOUSE_AVOID_RADIUS_COLS: u16 = 5;

/// Cursor glow: horizontal radius (in columns) for the brightness boost.
pub const MOUSE_GLOW_RADIUS_COLS: f32 = 8.0;

/// Cursor glow: vertical radius (in lines) for the brightness boost.
pub const MOUSE_GLOW_RADIUS_LINES: f32 = 6.0;

/// Cursor glow: peak intensity at cursor center (0.0 = off, 1.0 = full white).
pub const MOUSE_GLOW_INTENSITY: f32 = 0.35;

/// Click flash: ring expansion speed (columns per second).
pub const MOUSE_FLASH_SPEED: f32 = 25.0;

/// Click flash: thickness of the glowing ring (in columns).
pub const MOUSE_FLASH_RING_WIDTH: f32 = 3.0;

/// Click flash: peak intensity at ring center (0.0 = off, 1.0 = full white).
pub const MOUSE_FLASH_INTENSITY: f32 = 0.6;

/// Click flash: total duration of the ripple effect in seconds.
pub const MOUSE_FLASH_DURATION_SECS: f32 = 0.5;

// ---------------------------------------------------------------------------
// Parallax depth layers
// ---------------------------------------------------------------------------

/// Number of parallax depth layers.
pub const PARALLAX_LAYERS: usize = 3;

/// Per-layer speed multiplier (layer 0 = far, 2 = near).
pub const PARALLAX_SPEED_MULT: [f32; PARALLAX_LAYERS] = [0.35, 1.0, 1.7];

/// Per-layer brightness multiplier (layer 0 = dim, 2 = bright).
pub const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.35, 0.8, 1.0];

/// Per-layer length multiplier (layer 0 = short, 2 = long).
pub const PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.5, 1.0, 1.4];
