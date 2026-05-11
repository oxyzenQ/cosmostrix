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
/// 100ms is below the human perception threshold for smooth motion,
/// preventing visible teleporting of droplet heads during frame spikes.
pub const SIM_MAX_CAP_SECS: f64 = 0.1;

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

// ---------------------------------------------------------------------------
// Benchmark
// ---------------------------------------------------------------------------

/// Minimum elapsed seconds denominator to avoid division by zero in bench.
pub const BENCH_ELAPSED_MIN_S: f64 = 0.000_001;

/// Estimated ANSI overhead bytes per drawn cell in steady-state rendering.
/// Accounts for run-encoded style changes amortized across the terminal:
/// ~19 bytes = (5-byte SGR reset + ~6-byte fg escape + ~6-byte bg escape
/// + 1-byte char) × ~0.65 run-compression factor. This is a rough estimate
///   used for throughput reporting in the benchmark, not for frame pacing.
pub const ANSI_BYTES_PER_CELL_ESTIMATE: u64 = 19;

// ---------------------------------------------------------------------------
// Config file
// ---------------------------------------------------------------------------

/// Config file directory name under XDG_CONFIG_HOME or ~/.config.
pub const CONFIG_DIR_NAME: &str = "cosmostrix";

/// Config file name.
pub const CONFIG_FILE_NAME: &str = "config";

/// Default frame dirty capacity pre-allocation.  One Nth of total cells.
/// 8 is conservative enough for 1024×500 terminals (≈64K pre-alloc) while
/// still covering most frames without a heap spill.
pub const DIRTY_CAPACITY_DIVISOR: usize = 8;

/// Hard cap on dirty-vec pre-allocation in cells (≈8 KiB worth of usize).
/// Prevents wasting memory when terminal is very large.
pub const DIRTY_CAPACITY_CAP: usize = 8192;

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
// Cinematic color transition (generation-based palette propagation)
// ---------------------------------------------------------------------------

/// Maximum number of concurrent palette generations tracked simultaneously.
/// Old palettes are kept alive until all droplets using them have died,
/// at which point the slot can be reused. 4 slots covers rapid cycling
/// (by which time old droplets will have expired naturally).
pub const MAX_PALETTE_SLOTS: usize = 4;

/// Maximum stagger delay for column desynchronization (in milliseconds).
/// Each column adopts the new palette at a slightly different time,
/// creating an organic propagation wave instead of a robotic simultaneous switch.
pub const COLUMN_TRANSITION_STAGGER_MS: u16 = 700;

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

// ---------------------------------------------------------------------------
// Phosphor persistence (CRT afterglow)
// ---------------------------------------------------------------------------

/// Per-cell phosphor energy decay rate (higher = faster fade).
/// At 3.0, a cell at energy 200 decays to ~10 in ~1.3 seconds.
pub const PHOSPHOR_DECAY_RATE: f32 = 3.0;

/// Energy level when a cell's tail passes (starts the phosphor glow).
pub const PHOSPHOR_TAIL_RESIDUAL: u8 = 180;

/// Below this energy, the cell is cleared to blank.
pub const PHOSPHOR_DEAD_THRESHOLD: u8 = 6;

/// Per-layer phosphor decay rate multiplier (far=fast, near=slow).
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [1.6, 1.0, 0.7];

// ---------------------------------------------------------------------------
// Atmospheric depth layering enhancements
// ---------------------------------------------------------------------------

/// Per-layer spawn density multiplier (far = sparse, near = dense).
pub const PARALLAX_DENSITY_MULT: [f32; PARALLAX_LAYERS] = [0.5, 1.0, 1.5];

/// Per-layer glyph simplicity: far layer chars are less visually dense.
/// Implemented as a brightness modifier on top of PARALLAX_BRIGHTNESS_MULT.
pub const PARALLAX_GLYPH_DIM: [f32; PARALLAX_LAYERS] = [0.7, 1.0, 1.0];

// ---------------------------------------------------------------------------
// Velocity turbulence
// ---------------------------------------------------------------------------

/// Maximum velocity perturbation as fraction of base chars_per_sec.
pub const TURBULENCE_AMPLITUDE: f32 = 0.08;

/// Turbulence oscillation frequency (Hz). Controls how often drift changes.
pub const TURBULENCE_FREQ: f32 = 0.4;

// ---------------------------------------------------------------------------
// Rare anomaly events
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Phase 3: Temporal color ecosystems
// ---------------------------------------------------------------------------

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
pub const COLOR_LUMINANCE_CLIMATE_MIN: f32 = 0.6;
pub const COLOR_LUMINANCE_CLIMATE_MAX: f32 = 1.0;

/// Saturation climate bounds (min/max global saturation modifier).
pub const COLOR_SATURATION_CLIMATE_MIN: f32 = 0.5;
pub const COLOR_SATURATION_CLIMATE_MAX: f32 = 1.0;

/// Duration over which autonomous palette transitions occur (seconds).
/// Very long to feel like atmospheric evolution, not theme switching.
#[allow(dead_code)]
pub const AUTONOMOUS_PALETTE_TRANSITION_SECS: f32 = 120.0;

/// Probability per ecosystem tick that an autonomous palette drift occurs.
/// At 3s ticks, 0.03 ≈ one drift attempt every ~100 seconds.
pub const AUTONOMOUS_PALETTE_DRIFT_CHANCE: f32 = 0.03;

// ---------------------------------------------------------------------------
// Phase 3: Cinematic runtime behavior profiles
// ---------------------------------------------------------------------------

/// Number of cinematic behavior profiles.
#[allow(dead_code)]
pub const NUM_BEHAVIOR_PROFILES: usize = 7;

/// Duration for a profile transition (seconds).
pub const PROFILE_TRANSITION_SECS: f32 = 30.0;

/// How often the profile state interpolates toward target (in seconds).
pub const PROFILE_INTERPOLATION_RATE: f32 = 0.02;

// ---------------------------------------------------------------------------
// Phase 3: Autonomous atmospheric evolution
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Phase 3: Long-timescale renderer memory
// ---------------------------------------------------------------------------

/// Number of atmospheric history samples retained.
pub const MEMORY_HISTORY_SAMPLES: usize = 32;

/// How often a memory sample is recorded (in seconds).
pub const MEMORY_SAMPLE_INTERVAL_SECS: f32 = 30.0;

/// How much historical anomaly density increases instability pressure.
pub const MEMORY_ANOMALY_PRESSURE_WEIGHT: f32 = 0.3;

/// How much historical calm increases persistence richness.
pub const MEMORY_CALM_PERSISTENCE_BOOST: f32 = 0.15;

// ---------------------------------------------------------------------------
// Phase 3: Emergent visual storytelling
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Hardening: drift correction & terminal safety
// ---------------------------------------------------------------------------

/// Interval (in frames) between forced full screen redraws.
/// Prevents accumulated ANSI state desync over long sessions.
/// At 60fps this triggers roughly every 5 minutes — frequent enough to
/// catch drift but rare enough for zero perceptual impact.
pub const FULL_REDRAW_INTERVAL_FRAMES: u64 = 18000;
