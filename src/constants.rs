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

// v17: DENSITY_BASE_LINES removed — auto-density is now width-only.
// v17: DENSITY_AUTO_MAX removed — auto factor capped at 1.0 (identity).

/// Auto-density clamp range (min factor).
/// v17: the auto-density factor is now a width-only dampener
/// (clamp(cols/80, DENSITY_AUTO_MIN, 1.0)). It never amplifies above
/// 1.0 — the old sqrt(area) amplifier was removed.
pub const DENSITY_AUTO_MIN: f32 = 0.5;

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

/// Minimum droplet trail length (cells). Cinematic final polish: every
/// droplet must have at least 1 head + 1 body + 2 tail cells so the
/// trail has visible fade-out structure. Without this floor, short
/// back-layer droplets (length=1 or 2) appeared as bare heads with no
/// tail — visually reading as "stuck pixels" rather than rain streaks.
/// 4 is the smallest length that produces a recognizable head→body→tail
/// gradient; smaller values collapse the gradient into a single cell.
pub const MIN_DROPLET_LENGTH: u16 = 4;

/// Maximum droplet trail length cap (cells). Sanity ceiling to prevent
/// degenerate values when `lines` is very large (e.g. 8K UHD bench with
/// 4320 lines). A 4320-cell droplet would saturate the column for many
/// seconds, blocking new spawns in that column and creating visual
/// "stalactites". 200 cells is well above any natural droplet length
/// (typical max is ~80 cells on a 50-line terminal) while still
/// bounding the worst-case phosphor footprint.
pub const MAX_DROPLET_LENGTH_CAP: u16 = 200;

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

/// Density step for `[`/`]` keys.
pub const DENSITY_STEP: f32 = 0.25;

/// Watchdog check interval in seconds.
pub const WATCHDOG_INTERVAL_SECS: u64 = 5;

// Terminal / rendering

/// Dirty threshold ratio: if dirty cells >= total/N, do full redraw.
pub const DIRTY_THRESHOLD_RATIO: usize = 3;

/// Graceful shutdown timeout in seconds (force-exit if flush blocks).
pub const SHUTDOWN_TIMEOUT_SECS: u64 = 2;

/// Maximum allowed terminal width (columns) for interactive mode.
/// Prevents OOM from wildly misreported terminal sizes (e.g. 65535 × 65535 → hundreds of GiB).
/// 1024 cols × 500 lines × ~48 bytes/cell ≈ 24 MiB — still comfortable.
pub const MAX_TERMINAL_COLS: u16 = 1024;

/// Maximum allowed terminal height (lines) for interactive mode.  Same rationale as above.
pub const MAX_TERMINAL_LINES: u16 = 500;

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
pub const BENCH_MAX_COLS: u16 = 7680;

/// Maximum screen size for benchmark mode (lines). See `BENCH_MAX_COLS`.
///
/// 4320 = 8K UHD height. Same rationale: largest meaningful stress resolution
/// before the cell-grid allocation becomes the bottleneck instead of the
/// renderer itself.
pub const BENCH_MAX_LINES: u16 = 4320;

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
/// Exponential decay constant for trail brightness (distance from head).
///
/// v17 mastery: lowered from 1.8 to 1.2. The old value made the body
/// fade too quickly — at 50% down the stream, brightness was only 41%
/// (exp(-0.9)). The new value gives 55% at midpoint, producing a
/// gradual head→body→tail fade that's clearly visible at all positions.
/// Head is brightest, body is medium-bright, tail is dim — the cinematic
/// hierarchy the owner wants: "head paling terang, body agak kurang,
/// ekor redup".
pub const TRAIL_EXPONENTIAL_K: f64 = 1.2;

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
///
/// v18 mastery: raised from 500 to 700. The longer ramp gives scene
/// transitions a more deliberate, cinematic fill-in. Combined with the
/// lower min scale (0.15), the first second of a new scene is a gradual
/// bloom of rain rather than a quick pop.
pub const GLYPH_ENTRY_RAMP_DURATION_MS: u32 = 700;

/// Minimum spawn scale at the start of the glyph entry ramp (fraction of
/// normal spawn rate). At the moment of scene switch, spawn begins at this
/// rate and smoothly ramps to 1.0 over GLYPH_ENTRY_RAMP_DURATION_MS.
///
/// v18 mastery: lowered from 0.25 to 0.15. Scene entry now starts even
/// sparser, making the ramp more visible and the fill-in more graceful.
pub const GLYPH_ENTRY_RAMP_MIN_SCALE: f32 = 0.15;

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
///
/// v18 mastery: lowered from 0.05 to 0.03. Streams now start even slower,
/// giving the rain a softer "fade in from stillness" feel rather than an
/// instant jump. Combined with the longer ease tau (0.3s), the first
/// second of each stream's life is a graceful acceleration.
pub const STARTUP_VELOCITY_FRACTION: f32 = 0.03;

/// Time in seconds for startup easing to reach ~95% of full velocity.
/// Uses exponential ease: v = target × (1 - e^(-t/tau)).
///
/// v18 mastery: raised from 0.15 to 0.30. The old 0.15s tau meant streams
/// reached full speed in 0.45s — too fast, read as an instant start. The
/// new 0.30s tau gives 0.9s to reach 95% velocity, creating a visible
/// cinematic acceleration curve. New streams now "emerge" from the top
/// of the screen with a gentle ramp instead of snapping to full speed.
pub const STARTUP_EASE_TAU: f32 = 0.30;

// Head bloom (exponential gaussian falloff)

/// Bloom sigma (spread) for exponential gaussian head glow.
/// Higher = wider, softer glow. 1.5 gives a natural falloff over ~3 cells.
pub const HEAD_BLOOM_SIGMA: f32 = 1.5;

/// Bloom glow intensity at the head cell itself (0.0 = off, 1.0 = full white blend).
///
/// v17 mastery: raised from 0.4 to 0.55. Stronger head bloom for vivid
/// high-contrast head visibility. Combined with HEAD_WF=0.55, the head
/// cell is dramatically brighter than body/tail.
pub const HEAD_BLOOM_INTENSITY: f32 = 0.55;

/// Number of cells behind the head that receive bloom glow effect.
pub const HEAD_BLOOM_CELLS: u16 = 3;

// Depth fog vignette

/// Number of rows at top and bottom for fog vignette effect.
pub const FOG_ROWS: u16 = 4;

/// Minimum brightness factor at fog edges (0.0 = invisible, 1.0 = full).
/// Raised from 0.25 to 0.35 — the old value made edge-row glyphs perceptually
/// ~5% (effectively invisible). 0.35 (~14% perceptual) preserves the vignette
/// effect while keeping edge glyphs faintly visible rather than lost entirely.
pub const FOG_MIN_FACTOR: f32 = 0.65;

// Cinematic vignette (radial edge darkening)

/// Maximum dimming intensity at the screen corners (0.0 = no dimming,
/// 1.0 = full black). 0.4 means corner cells are dimmed to 60% of their
/// post-effects brightness — a soft photographic vignette that draws the
/// eye toward the center of the frame without darkening the focused
/// middle region. Matches the look of a real anamorphic lens.
pub const VIGNETTE_INTENSITY: f32 = 0.4;

/// Normalized radius at which vignette dimming begins (0.0 = center,
/// 1.0 = corner). 0.7 means the inner 70% of the screen (by Euclidean
/// distance from center) is unmodified; dimming ramps smoothly from
/// there to the corner via smoothstep. This preserves full readability
/// of the focused center while darkening only the periphery.
pub const VIGNETTE_INNER_RADIUS: f32 = 0.7;

// Rain shadow (bottom quadratic fade-out)

/// Fraction of screen height (from the bottom) covered by the rain
/// shadow. 0.20 means the bottom 20% of rows fade out quadratically —
/// a longer, softer fade than EDGE_FADE_BOTTOM (which is a sharp 12-row
/// lip). The shadow reads as "rain dissipating into shadow at the
/// ground" rather than "rain hitting a wall", giving the frame depth.
pub const RAIN_SHADOW_PCT: f32 = 0.20;

// Mouse interaction (v17: always-on, --mouse flag deleted)
// v17: MOUSE_AVOID_RADIUS_COLS removed — spawn avoidance deleted.
// MOUSE_GLOW_INTENSITY = 0.0 — hover glow disabled (dim/dark default).

/// Cursor glow: horizontal radius (in columns) for the brightness boost.
pub const MOUSE_GLOW_RADIUS_COLS: f32 = 7.0;

/// Cursor glow: vertical radius (in lines) for the brightness boost.
pub const MOUSE_GLOW_RADIUS_LINES: f32 = 5.0;

/// Cursor glow: peak intensity at cursor center (0.0 = off, 1.0 = full white).
///
/// v17 mastery: set to 0.0 — hover glow DISABLED. Owner reported bright
/// colors when moving mouse over rain ('warnanya jadi terang dan tinggi
/// sekali'). The glow made cells near the cursor visually bright, breaking
/// the dim/dark cinematic default. Click wave (MOUSE_FLASH_INTENSITY)
/// remains strong for click feedback; hover glow is removed entirely.
pub const MOUSE_GLOW_INTENSITY: f32 = 0.0;

/// Click flash: ring expansion speed (columns per second).
///
/// v18 mastery: lowered from 60.0 to 32.0. The previous 60 cells/s was too
/// fast — it read as a flicker, not a ripple. 32 cells/s gives a slow,
/// elegant water-drop propagation that reaches a 120-col terminal edge in
/// ~2 seconds, matching the natural pace of a stone dropped into still
/// water. Combined with the longer duration (1.8s) the wave now travels
/// 58 cells — enough to cross mid-size terminals with a graceful arc.
pub const MOUSE_FLASH_SPEED: f32 = 32.0;

/// Click flash: thickness of the glowing ring (in columns).
///
/// v18 mastery: raised from 6.0 to 8.0. A thicker wave front reads as a
/// more substantial glowing band rather than a thin line. The extra width
/// also gives the brightness ramp more room to breathe, so the glow fades
/// smoothly instead of cutting off.
pub const MOUSE_FLASH_RING_WIDTH: f32 = 8.0;

/// Click flash: peak intensity at ring center (0.0 = off, 1.0 = full white).
///
/// v18 mastery: raised from 0.65 to 0.85. Owner reported the wave was too
/// dim — now the ring peaks at 85% white blend, giving a bright luminous
/// glow that reads clearly against the dark cinematic background. The
/// quadratic fade ensures the bright peak doesn't blow out harshly.
pub const MOUSE_FLASH_INTENSITY: f32 = 0.85;

/// Click flash: total duration of the ripple effect in seconds.
///
/// v18 mastery: raised from 1.2 to 1.8. The longer duration gives the
/// slower wave (32 cells/s) time to propagate fully and dissolve naturally.
/// The quadratic fade curve means the last 0.6s is a gentle tail-off,
/// creating the "lingering shimmer" of a real water drop.
pub const MOUSE_FLASH_DURATION_SECS: f32 = 1.8;

/// v18 mastery: secondary ripple intensity (fraction of primary flash).
///
/// Raised from 0.35 to 0.45. The secondary echo ring is now brighter,
/// making the layered "stone in water" effect more visible. The secondary
/// ring still trails the primary at half speed, creating a two-wave
/// cascade that reads as a single elegant event.
pub const MOUSE_FLASH_SECONDARY_FRAC: f32 = 0.45;

/// v18 mastery: secondary ripple speed (fraction of primary speed).
///
/// Lowered from 0.5 to 0.4. The secondary ring now lags further behind
/// (40% of primary speed), giving the two rings more visual separation.
/// This makes the echo distinct from the primary wave rather than
/// blending into a single thick pulse.
pub const MOUSE_FLASH_SECONDARY_SPEED_FRAC: f32 = 0.4;

// Parallax depth layers

/// Number of parallax depth layers.
pub const PARALLAX_LAYERS: usize = 3;

/// Per-layer speed multiplier (layer 0 = far, 2 = near).
pub const PARALLAX_SPEED_MULT: [f32; PARALLAX_LAYERS] = [0.35, 1.0, 1.7];

/// Per-layer brightness multiplier (layer 0 = far, 2 = near).
///
/// Cinematic final polish: back layer (0) dimmed further to 25% (was 35%)
/// because the head self-bloom (55% white blend toward white) was being
/// applied AFTER the brightness dimming, re-brightening back-layer heads
/// into visible "white dots" against the dark background. Lowering to 25%
/// pre-compensates so even after the self-bloom, back-layer heads stay
/// below the front-layer body visibility floor. Mid layer (1) at 65%
/// unchanged; near layer (2) at 100%.
///
/// Prior value [0.35, 0.65, 1.0] still let back-layer heads pop because
/// the head self-bloom was layer-agnostic. Combined with the new
/// PARALLAX_HEAD_SELFBLOOM_MULT (which scales the self-bloom itself),
/// back-layer heads are now triple-dimmed: brightness × self-bloom ×
/// saturation, killing the "white dot" artifact decisively.
pub const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.25, 0.65, 1.0];

/// Per-layer saturation multiplier (layer 0 = desaturated, 2 = full).
///
/// Cinematic final polish: back layer (0) desaturated further to 30%
/// (was 40%) for stronger atmospheric haze. Even after the head
/// self-bloom blends toward white, the low saturation means the head
/// color is still mostly gray-tinted rather than vivid neon — making it
/// read as "distant rain in fog" rather than "bright pixel artifact".
/// Mid layer (1) at 70% unchanged; near layer (2) at 100%.
///
/// Implemented in droplet.rs as a blend toward gray (luminance) by
/// `1.0 - saturation_mult`.
pub const PARALLAX_SATURATION_MULT: [f32; PARALLAX_LAYERS] = [0.30, 0.70, 1.0];

/// Per-layer head-bloom multiplier (layer 0 = suppressed, 2 = full).
///
/// Head bloom (HEAD_BLOOM_INTENSITY gaussian glow behind the head) is
/// normally the same across all layers. For depth-of-field, back-layer
/// heads should NOT glow as brightly — otherwise they become the
/// aforementioned "bright spots". This multiplier scales the bloom
/// gaussian factor before it's applied to RGB. Back layer at 0.40 means
/// the head glow is reduced by 60%; mid layer at 0.70 by 30%.
pub const PARALLAX_HEAD_BLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.40, 0.70, 1.0];

/// Per-layer head self-bloom multiplier (layer 0 = suppressed, 2 = full).
///
/// Cinematic final polish: the head self-bloom (HEAD_WF 55% white blend
/// applied to head cells) was previously layer-agnostic, which re-brightened
/// back-layer heads AFTER the brightness dimming had already brought them
/// down to 25%. This created the persistent "white dot" artifact: a
/// back-layer head would dim to ~25%, then get boosted back up to ~66%
/// by the white blend, popping out as a hot pixel.
///
/// This multiplier scales HEAD_WF per layer so the self-bloom is also
/// depth-aware. Back layer at 0.30 means head self-bloom is reduced by
/// 70% — a back-layer head now has effective self-bloom of ~17% (vs 55%
/// for front layer), keeping it firmly in the background. Mid layer at
/// 0.65; near layer at 1.0 (full cinematic head pop).
pub const PARALLAX_HEAD_SELFBLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.30, 0.65, 1.0];

/// Per-layer length multiplier (layer 0 = short, 2 = long).
pub const PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.5, 1.0, 1.4];

// Phosphor persistence (CRT afterglow)

/// Per-cell phosphor energy decay rate (higher = faster fade).
/// Raised from 3.0 to 5.0 for crisper, more energetic trail fade.
/// Film Matrix afterglow is ~200ms; at 5.0, afterglow lasts ~400ms
/// (still 2× film, but 2.7× faster than old 1094ms).
pub const PHOSPHOR_DECAY_RATE: f32 = 5.0;

/// Energy level when a cell's tail passes (starts the phosphor glow).
/// v17 mastery: raised from 120 to 160. Trail brightness at ~63% (was 47%).
/// Body cells are clearly visible as colored rain, not dim ghosts. The
/// head→body→tail hierarchy is: head = brightest (55% white blend), body
/// = medium-bright (palette color at 55-80% brightness via exp decay),
/// tail = dim (palette color at 0-30% brightness).
pub const PHOSPHOR_TAIL_RESIDUAL: u8 = 160;

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
///
/// Masterclass depth-of-field tuning: back layer decays at 2.2× the base
/// rate (was 1.6×) so back-layer head glow fades fast and doesn't linger
/// as a persistent "bright spot". Mid layer at 1.2× keeps a slight
/// recession. Near layer at 0.7× retains the long cinematic trail.
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [2.2, 1.2, 0.7];

/// Number of rows from the bottom of the screen where phosphor decay is
/// accelerated. Ghost cells near the bottom accumulate into a static
/// "concrete wall" because droplets end there and fewer new streams
/// overwrite the residue. Accelerating decay in this region clears
/// afterglow faster without affecting the cinematic look elsewhere.
///
/// v17: extended from 8 to 12 to match EDGE_FADE_BOTTOM_ROWS. The bottom
/// shadow zone now spans 12 rows; phosphor trails in this zone decay at
/// 3.0× the normal rate (PHOSPHOR_BOTTOM_DECAY_MULT raised 2.5→3.0) so
/// ghost residue dissolves in sync with the visual fade.
pub const PHOSPHOR_BOTTOM_ROWS: u16 = 12;

/// Phosphor decay rate multiplier applied to bottom rows.
///
/// v17: raised from 2.5 to 3.0 to sync with the wider EDGE_FADE_BOTTOM_ROWS
/// zone. Combined with PHOSPHOR_DECAY_RATE=5.0, effective rate at the
/// bottom is 15.0, reducing afterglow duration from ~400ms to ~270ms —
/// closer to the film Matrix's ~200ms afterglow.
pub const PHOSPHOR_BOTTOM_DECAY_MULT: f32 = 3.0;

// Atmospheric depth layering enhancements

/// Per-layer spawn density multiplier (far = sparse, near = dense).
///
/// Masterclass depth-of-field tuning: back layer spawns at 30% of base
/// (was 50%) to thin out the "bright spot" population. Mid layer at 60%
/// (was 100%) — mid-layer rain should feel clearly less dense than front.
/// Near layer at 100% (was 150% — that oversaturated front and made the
/// whole frame feel crowded; 100% is the natural base rate).
///
/// The combined effect with PARALLAX_BRIGHTNESS_MULT and
/// PARALLAX_SATURATION_MULT: back-layer heads are now 30% as frequent,
/// 35% as bright, AND 40% as saturated — three independent reductions
/// stack to push them firmly into the background.
pub const PARALLAX_DENSITY_MULT: [f32; PARALLAX_LAYERS] = [0.30, 0.60, 1.0];

/// Per-layer glyph simplicity: far layer chars are less visually dense.
///
/// Set to [1.0, 1.0, 1.0] — glyph dimming is now subsumed by
/// PARALLAX_BRIGHTNESS_MULT and PARALLAX_SATURATION_MULT. Stacking a
/// third dimming pass on the back layer made it drop below the
/// visibility floor; the new brightness+saturation combo is sufficient.
pub const PARALLAX_GLYPH_DIM: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 1.0];

/// Per-layer contrast reduction (depth-of-field perceptual blur).
///
/// Masterclass depth-of-field tuning: back layer (0) at 0.55 — fg color
/// is blended 55% toward black (background), creating heavy haze. Mid
/// layer (1) at 0.20 — slight recession but still readable. Near layer
/// (2) at 0.0 — sharp foreground, no fog.
///
/// This is the terminal equivalent of depth-of-field blur: instead of
/// blurring pixels (impossible in text), we reduce fg-bg contrast so
/// background rain reads as "behind a haze".
pub const PARALLAX_CONTRAST_REDUCTION: [f32; PARALLAX_LAYERS] = [0.55, 0.20, 0.0];

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
/// v17 mastery: duration of the resume (unpause) smoothstep ramp.
///
/// Raised from 0.18 to 0.45 — the old 0.18s was only ~11 frames at 60fps,
/// too fast for cinematic smoothness. The new 0.45s (~27 frames) gives the
/// rain time to visibly accelerate from frozen to full speed, producing a
/// cinematic "inertia recovery" that feels like a film restarting in slow
/// motion. The easing curve is also upgraded from smoothstep (C1) to
/// smootherstep (C2 continuous — no velocity discontinuity at start/end).
pub const RESUME_EASE_DURATION_SECS: f32 = 0.45;

/// v17 mastery: duration of the pause (deceleration) smoothstep ramp.
///
/// When the user presses 'p' to pause, the rain doesn't snap to a halt —
/// it decelerates over PAUSE_EASE_DURATION_SECS using a smootherstep curve,
/// producing a cinematic "time slow" effect. This is the mirror of
/// RESUME_EASE_DURATION_SECS. Set to 0.0 for instant freeze (legacy behavior).
pub const PAUSE_EASE_DURATION_SECS: f32 = 0.30;

// Hardening: drift correction & terminal safety

/// Interval (in frames) between forced full screen redraws.
/// Prevents accumulated ANSI state desync over long sessions.
/// At 60fps this triggers roughly every 5 minutes — frequent enough to
/// catch drift but rare enough for zero perceptual impact.
pub const FULL_REDRAW_INTERVAL_FRAMES: u64 = 18000;

// Viewport edge fade (smooth entry/exit at terminal borders)

/// Number of rows from the TOP viewport edge for smooth entry fade.
/// Applied after all visual effects including head bloom, ensuring
/// the fade takes priority over head brightness at edges. The bottom
/// edge uses a separate, wider EDGE_FADE_BOTTOM_ROWS for the cinematic
/// dissolve-into-shadow effect (v17).
pub const EDGE_FADE_ROWS: u16 = 3;

/// v17: Number of rows from the BOTTOM viewport edge for the deep shadow
/// dissolve. Wider than EDGE_FADE_ROWS (top) because the cinematic
/// bottom-shadow effect needs a gradual fade across ~30% of the screen
/// height on a 40-line terminal. The fade curve is split into two zones:
/// Zone 1 (rows [lines-EDGE_FADE_BOTTOM_ROWS .. lines-EDGE_FADE_ROWS]):
///   gentle pre-fade from 1.0 down to EDGE_FADE_BOTTOM_LIP (smoothstep).
/// Zone 2 (rows [lines-EDGE_FADE_ROWS .. lines-1]):
///   sharp lip fade from EDGE_FADE_BOTTOM_LIP down to EDGE_FADE_BOTTOM_MIN
///   (linear).
/// This produces a film-like dissolve where rain visibly thins and darkens
/// before reaching the bottom border, eliminating the "concrete wall"
/// artifact where dying heads pile up at the last row.
pub const EDGE_FADE_BOTTOM_ROWS: u16 = 12;

/// v17: Brightness at the boundary between the gentle pre-fade zone and
/// the sharp lip zone (row lines-EDGE_FADE_ROWS). The pre-fade smoothsteps
/// from 1.0 down to this value; the lip then fades linearly to
/// EDGE_FADE_BOTTOM_MIN. Set to 0.75 so the pre-fade is subtle (rain still
/// clearly visible) and the lip does the heavy lifting.
pub const EDGE_FADE_BOTTOM_LIP: f32 = 0.75;

/// Minimum brightness at the very top edge (row 0).
/// At 0.55, the first visible row is moderately dimmed, creating a
/// subtle emergence effect as rain enters from just beyond the top
/// border. Combined with the existing fog vignette (FOG_MIN_FACTOR=0.35),
/// the effective brightness at row 0 is ~0.55 × 0.35 ≈ 0.19 — visible
/// but subdued, giving the cinematic "entering the frame" feel.
pub const EDGE_FADE_TOP_MIN: f32 = 0.70;

/// Minimum brightness at the very bottom edge (last row).
///
/// v17: lowered from 0.45 to 0.20 for the cinematic bottom-shadow
/// dissolve. The old 0.45 was "raised for visible bottom border" — a
/// counter-cinematic choice. The new 0.20 produces a deep shadow where
/// rain dissolves into darkness at the bottom of the frame, matching
/// the Matrix film's bottom-edge vignette. Combined with fog (0.45),
/// effective brightness at last row is ~0.20 × 0.45 ≈ 0.09 — near-black,
/// the rain visibly disappears before hitting the border.
pub const EDGE_FADE_BOTTOM_MIN: f32 = 0.35;

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
