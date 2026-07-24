// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v20: Modular intro system + Linux process metrics helpers.
//!
//! Two unrelated concerns coexist in this file:
//!
//! 1. **Linux process metrics** (`read_self_rss_kb`, `read_self_voluntary_ctxt`)
//!    — lightweight `/proc` readers used by the HUD overlay. Kept here because
//!    the file already exists; the helpers are tiny and have no dependencies.
//!
//! 2. **Modular intro dispatcher** (`run_intro`, `IntroType`) — a cinematic
//!    studio-logo-style animation played before the rain engine takes over.
//!    Triggered by `cosmostrix --intro <type>`. The actual phase logic lives
//!    in sibling modules:
//!    * [`super::intro_cosmic`] — Cosmic Burst (singularity → explosion → morph → rain)
//!    * [`super::intro_logo`]   — Cosmostrix Logo (fade in → ignition → dissolve → rain)
//!
//!    This file owns the shared particle infrastructure (pool, RNG, lerp) and
//!    the dispatcher that routes `IntroType` to the correct submodule's
//!    `run_*_intro` entry point.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crossterm::event::Event;
use crossterm::style::Color;

use crate::cell::Cell;
use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::palette::color_to_rgb;
use crate::terminal::Terminal;

use super::watchdog::{FRAME_COUNTER, GRACEFUL_SHUTDOWN};

// ─────────────────────────────────────────────────────────────────────────────
// Linux process metrics (unchanged from v17)
// ─────────────────────────────────────────────────────────────────────────────

/// Read this process's current RSS from `/proc/self/status` (Linux only).
#[cfg(target_os = "linux")]
pub(crate) fn read_self_rss_kb() -> u64 {
    // Read VmRSS from /proc/self/status. Lightweight: single line match.
    use std::io::Read;
    let mut file = match std::fs::File::open("/proc/self/status") {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut buf = [0u8; 8192];
    let n = file.read(&mut buf).unwrap_or(0);
    let text = std::str::from_utf8(&buf[..n]).unwrap_or("");
    for line in text.split('\n') {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let trimmed = rest.trim();
            let digits_end = trimmed
                .bytes()
                .position(|b| !b.is_ascii_digit())
                .unwrap_or(trimmed.len());
            if digits_end > 0 {
                return trimmed[..digits_end].parse().unwrap_or(0);
            }
        }
    }
    0
}

/// Read voluntary context switches from `/proc/self/stat` (Linux only).
#[cfg(target_os = "linux")]
pub(crate) fn read_self_voluntary_ctxt() -> u64 {
    let stat = match std::fs::read_to_string("/proc/self/stat") {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let after_paren = match stat.rfind(')') {
        Some(idx) => &stat[idx + 1..],
        None => return 0,
    };
    let fields: Vec<&str> = after_paren.split_whitespace().collect();
    fields.get(17).and_then(|s| s.parse().ok()).unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Intro type enum + dispatcher
// ─────────────────────────────────────────────────────────────────────────────

// `IntroType` itself lives in `crate::config` so clap can derive
// `ValueEnum` on it without `interactive` having to depend on clap.
// Re-exported here for convenience so callers in `interactive` don't
// have to spell out the full path each time.
pub(crate) use crate::config::IntroType;

/// Minimum terminal size for any intro to play. Below this, skip with a
/// stderr notice. Matches the classic 80×24 VT100 baseline.
pub(super) const MIN_INTRO_COLS: u16 = 80;
pub(super) const MIN_INTRO_LINES: u16 = 24;

/// Frame period for all intro animations. ~30 FPS — intros are mostly
/// particle motion, so 30 FPS is smooth without burning CPU.
pub(super) const INTRO_FRAME_PERIOD: Duration = Duration::from_millis(33);

/// Particle pool capacity. Pre-allocated once; reused via free-list.
/// 512 × 48 B = 24 KiB — negligible. Both Cosmic Burst and Logo intros
/// share this pool size; peak concurrent particle counts stay well below
/// 512 in either intro.
pub(super) const PARTICLE_POOL_SIZE: usize = 512;

/// Entry point — dispatch to the appropriate intro submodule based on
/// `intro_type`. Returns `Ok(())` immediately for `None` or when the
/// terminal is too small.
///
/// Skippable at any time by pressing any key. Below `MIN_INTRO_COLS ×
/// MIN_INTRO_LINES`, the intro is skipped with a stderr notice.
///
/// Reuses the existing `Terminal` / `Frame` / `Cell` pipeline. Zero
/// per-frame heap allocation (particle pool is pre-allocated and reused).
pub(crate) fn run_intro(
    term: &mut Terminal,
    frame: &mut Frame,
    cloud: &Cloud,
    w: u16,
    h: u16,
    intro_type: IntroType,
) -> std::io::Result<()> {
    if intro_type == IntroType::None {
        return Ok(());
    }

    // Terminal-size guard. Below 80×24 the intros clip badly; print a
    // notice and skip the cinematic.
    if w < MIN_INTRO_COLS || h < MIN_INTRO_LINES {
        eprintln!(
            "Terminal too small for intro ({}x{} < {}x{}). Starting rain...",
            w, h, MIN_INTRO_COLS, MIN_INTRO_LINES
        );
        return Ok(());
    }

    match intro_type {
        IntroType::Cosmic => super::intro_cosmic::run_cosmic_intro(term, frame, cloud, w, h),
        IntroType::Logo => super::intro_logo::run_logo_intro(term, frame, cloud, w, h),
        IntroType::None => Ok(()),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared particle infrastructure (used by intro_cosmic and intro_logo)
// ─────────────────────────────────────────────────────────────────────────────

/// Parsed particle representation. 48 bytes — fits ~1.3 per cache line.
/// `active` is the free-list flag; dead particles are skipped during
/// update and render.
///
/// `angle` and `speed` are the polar-coordinate form of velocity. We
/// store them alongside `vx`/`vy` because:
/// * Cosmic Burst Phase 2 (burst) needs `angle` for spiral motion (angle += spiral_rate).
/// * Cosmic Burst Phase 3 (morph) needs `speed` for deceleration.
/// * Logo dissolve phase uses `vx`/`vy` directly for rain-fall motion.
/// * `vx`/`vy` are kept as the cartesian cache for rendering.
#[derive(Clone, Copy)]
pub(super) struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub ch: char,
    /// Particle color stored as RGB triple. Avoids `Color` enum tag overhead
    /// and lets us lerp between cosmic and palette colors trivially.
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub life: f32,
    pub max_life: f32,
    /// Current direction in radians (0 = right, π/2 = down). Updated each
    /// frame by `spiral_rate` during Cosmic Burst Phase 2.
    pub angle: f32,
    /// Current speed in cells per second. Decelerates during Cosmic Burst
    /// Phase 3 morph. For Logo dissolve, holds the rain-fall speed.
    pub speed: f32,
    /// Per-particle angular velocity (radians per second). Sampled at
    /// spawn time from `[SPIRAL_RATE_MIN, SPIRAL_RATE_MAX)` by Cosmic Burst.
    /// Unused (zero) by Logo dissolve particles.
    pub spiral_rate: f32,
    pub active: bool,
}

impl Particle {
    pub(super) const INACTIVE: Self = Self {
        x: 0.0,
        y: 0.0,
        vx: 0.0,
        vy: 0.0,
        ch: ' ',
        r: 0,
        g: 0,
        b: 0,
        life: 0.0,
        max_life: 0.0,
        angle: 0.0,
        speed: 0.0,
        spiral_rate: 0.0,
        active: false,
    };
}

/// Tiny xorshift32 RNG — avoids pulling `rand` into this module. Seeded
/// from `Instant::now()` so each intro run looks slightly different.
pub(super) struct XorShift(pub u32);

impl XorShift {
    pub(super) fn new(seed: u32) -> Self {
        // Avoid the all-zero state which would lock the generator.
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }
    #[inline]
    pub(super) fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform float in `[0.0, 1.0)`.
    #[inline]
    pub(super) fn next_f32(&mut self) -> f32 {
        // 24-bit mantissa for uniform distribution.
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Pre-allocated particle pool with a free-list stack. The pool itself
/// stores `Particle` values; the free-list stores indices into the pool
/// so spawning is O(1) pop, killing is O(1) flag flip.
pub(super) struct ParticlePool {
    pub particles: Vec<Particle>,
    pub free: Vec<usize>,
}

impl ParticlePool {
    pub(super) fn new() -> Self {
        let particles = vec![Particle::INACTIVE; PARTICLE_POOL_SIZE];
        let free = (0..PARTICLE_POOL_SIZE).collect();
        Self { particles, free }
    }

    #[inline]
    pub(super) fn spawn(&mut self, p: Particle) -> bool {
        if let Some(i) = self.free.pop() {
            self.particles[i] = p;
            true
        } else {
            false
        }
    }

    #[inline]
    pub(super) fn kill(&mut self, i: usize) {
        self.particles[i].active = false;
        self.free.push(i);
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn active_count(&self) -> usize {
        PARTICLE_POOL_SIZE - self.free.len()
    }
}

/// Linear interpolation between two `f32` values.
#[inline]
pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation between two RGB triples.
#[inline]
pub(super) fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    (
        (lerp(a.0 as f32, b.0 as f32, t)).round().clamp(0.0, 255.0) as u8,
        (lerp(a.1 as f32, b.1 as f32, t)).round().clamp(0.0, 255.0) as u8,
        (lerp(a.2 as f32, b.2 as f32, t).round().clamp(0.0, 255.0)) as u8,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers used by both intro submodules
// ─────────────────────────────────────────────────────────────────────────────

/// Seed an [`XorShift`] RNG from wall-clock nanos. Each intro run gets a
/// different particle pattern, which keeps repeat viewings fresh.
pub(super) fn seed_rng() -> XorShift {
    let seed = Instant::now()
        .elapsed()
        .as_nanos()
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(0x1234_5678) as u32;
    XorShift::new(seed)
}

/// Pull the brightest palette color (typically the head color) as the
/// rain target. If the palette is empty, fall back to neon green.
pub(super) fn palette_target_rgb(cloud: &Cloud) -> (u8, u8, u8) {
    cloud
        .palette
        .colors
        .last()
        .copied()
        .map(color_to_rgb)
        .unwrap_or((57, 255, 20)) // NeonGreen fallback
}

/// Rain charset for the morph / dissolve phases. Empty pool → binary
/// fallback (`['0', '1']`). The returned Vec is owned because it's
/// computed once at intro start and then borrowed for the duration.
pub(super) fn rain_chars(cloud: &Cloud) -> Vec<char> {
    if cloud.char_pool.is_empty() {
        vec!['0', '1']
    } else {
        cloud.char_pool.clone()
    }
}

/// Deterministic pseudo-random float in `[0, 1)` used by glyph-swap paths
/// that don't have an RNG handle threaded through. Uses a per-call linear
/// congruential step seeded by an atomic counter, which is good enough
/// for cosmetic glyph variation.
#[inline]
pub(super) fn rng_freehand() -> f32 {
    use std::sync::atomic::{AtomicU32, Ordering as AOrdering};
    static STATE: AtomicU32 = AtomicU32::new(0x1357_9BDF);
    let mut s = STATE.load(AOrdering::Relaxed);
    if s == 0 {
        s = 0x2468_ACE0;
    }
    s ^= s << 13;
    s ^= s >> 17;
    s ^= s << 5;
    STATE.store(s, AOrdering::Relaxed);
    (s >> 8) as f32 / (1u32 << 24) as f32
}

/// Drain the terminal event queue non-blocking. Returns `true` if the
/// intro should skip (any key was pressed). Also returns `true` if the
/// graceful-shutdown flag is set.
pub(super) fn should_skip() -> std::io::Result<bool> {
    if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
        return Ok(true);
    }
    while Terminal::poll_event(Duration::from_millis(0))? {
        if let Ok(Event::Key(_)) = Terminal::read_event() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Render a single particle cell at `(x, y)` with the given color,
/// interpolating toward black by the inverse life ratio (so particles
/// fade as they age). `bold` controls the cell's bold flag.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_particle_cell(
    frame: &mut Frame,
    w: u16,
    h: u16,
    x: f32,
    y: f32,
    ch: char,
    rgb: (u8, u8, u8),
    bg: Option<Color>,
    life_t: f32,
    bold: bool,
) {
    let xi = x as u16;
    let yi = y as u16;
    if xi < w && yi < h {
        let faded = lerp_rgb((0, 0, 0), rgb, life_t);
        frame.set_force(
            xi,
            yi,
            Cell {
                ch,
                fg: Some(Color::Rgb {
                    r: faded.0,
                    g: faded.1,
                    b: faded.2,
                }),
                bg,
                bold,
            },
        );
    }
}

/// Bump the watchdog frame counter and sleep for one frame period.
/// Used by every intro submodule at the end of each frame loop iteration.
pub(super) fn end_frame(term: &mut Terminal, frame: &mut Frame) -> std::io::Result<()> {
    term.draw(frame)?;
    FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::thread::sleep(INTRO_FRAME_PERIOD);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests (shared infrastructure only — submodule-specific tests live in
// their respective files)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_intro_size_matches_vt100() {
        assert_eq!(MIN_INTRO_COLS, 80);
        assert_eq!(MIN_INTRO_LINES, 24);
    }

    #[test]
    fn xorshift_provides_varied_values() {
        let mut rng = XorShift::new(42);
        let a = rng.next_u32();
        let b = rng.next_u32();
        let c = rng.next_u32();
        assert_ne!(a, b, "consecutive u32 must differ");
        assert_ne!(b, c, "consecutive u32 must differ");
    }

    #[test]
    fn xorshift_next_f32_in_unit_range() {
        let mut rng = XorShift::new(7);
        for _ in 0..1000 {
            let f = rng.next_f32();
            assert!(
                (0.0..1.0).contains(&f),
                "next_f32 returned {f}, out of [0,1)"
            );
        }
    }

    #[test]
    fn xorshift_handles_zero_seed() {
        // Zero seed must not lock the generator.
        let mut rng = XorShift::new(0);
        let a = rng.next_u32();
        let b = rng.next_u32();
        assert_ne!(a, b);
    }

    #[test]
    fn lerp_interpolates_correctly() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-6);
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_rgb_interpolates_correctly() {
        let a = (0u8, 0u8, 0u8);
        let b = (100u8, 200u8, 50u8);
        let mid = lerp_rgb(a, b, 0.5);
        assert_eq!(mid, (50, 100, 25));
    }

    #[test]
    fn lerp_rgb_clamps_to_endpoints() {
        let a = (10u8, 20u8, 30u8);
        let b = (200u8, 100u8, 50u8);
        assert_eq!(lerp_rgb(a, b, 0.0), a);
        assert_eq!(lerp_rgb(a, b, 1.0), b);
    }

    #[test]
    fn particle_pool_starts_full_free_list() {
        let pool = ParticlePool::new();
        assert_eq!(pool.free.len(), PARTICLE_POOL_SIZE);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn particle_pool_spawn_and_kill_roundtrip() {
        let mut pool = ParticlePool::new();
        let initial_free = pool.free.len();
        let p = Particle {
            x: 1.0,
            y: 2.0,
            vx: 0.0,
            vy: 1.0,
            ch: '*',
            r: 255,
            g: 100,
            b: 50,
            life: 0.5,
            max_life: 0.5,
            angle: 0.0,
            speed: 10.0,
            spiral_rate: 1.0,
            active: true,
        };
        assert!(pool.spawn(p));
        assert_eq!(pool.free.len(), initial_free - 1);
        assert_eq!(pool.active_count(), 1);
        pool.kill(initial_free - 1);
        assert_eq!(pool.free.len(), initial_free);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn particle_pool_spawn_fails_when_full() {
        let mut pool = ParticlePool::new();
        // Drain the entire free-list.
        for _ in 0..PARTICLE_POOL_SIZE {
            assert!(pool.spawn(Particle::INACTIVE));
        }
        assert_eq!(pool.free.len(), 0);
        // Next spawn should fail (returns false).
        assert!(!pool.spawn(Particle::INACTIVE));
    }

    #[test]
    fn rng_freehand_returns_unit_range() {
        for _ in 0..1000 {
            let f = rng_freehand();
            assert!((0.0..1.0).contains(&f), "rng_freehand returned {f}");
        }
    }
}
