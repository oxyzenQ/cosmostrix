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
//!    * [`super::intro_logo`]   — cosmostrix Logo (fade in → ignition → dissolve → rain)
//!
//!    This file owns the shared particle infrastructure (pool, RNG, lerp) and
//!    the dispatcher that routes `IntroType` to the correct submodule's
//!    `run_*_intro` entry point.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crossterm::event::Event;
use crossterm::style::Color;

use crate::cell::Cell;
use crate::chroma_dragon_engine::intro_colors::NEON_GREEN_FALLBACK;
use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::palette::color_to_rgb;
use crate::terminal::{is_terminal_gone, Terminal};

use super::input::is_unmodified_or_shift;
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
    // v50 audit C-4: use .nth(17) instead of collecting into Vec (saves
    // one heap allocation per call at 1 Hz cadence).
    after_paren
        .split_whitespace()
        .nth(17)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
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
/// stderr notice. v25 responsive: lowered from 80×24 to 10×5 — the
/// intros now dynamically scale their ASCII art to fit the terminal
/// (see intro_logo::scale_art and intro_cosmic::scale_cosmic_art), so
/// the hard floor is only for absurdly tiny terminals where even a
/// scaled-down logo would be unreadable.
pub(super) const MIN_INTRO_COLS: u16 = 10;
pub(super) const MIN_INTRO_LINES: u16 = 5;

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
/// # Skip behavior
///
/// The intro can be exited early by pressing **`q`** (case-insensitive)
/// or by sending SIGTERM / SIGHUP / SIGQUIT (handled via [`GRACEFUL_SHUTDOWN`]).
/// Ctrl+C (SIGINT) is deprecated — only 'q' exits cosmostrix.
/// No other key skips — the intro ignores stray keypresses so accidental
/// presses of space / enter / arrows don't cut the cinematic short.
///
/// # Benchmark mode
///
/// `--benchmark`, `--bench-frames`, and `--bench-all` all `return` from
/// `main.rs` before `interactive::run_interactive()` is ever called, so
/// this function is never reached in benchmark mode. The intro therefore
/// cannot perturb benchmark measurements — no extra CPU, no terminal
/// writes, no particle allocation.
///
/// Below `MIN_INTRO_COLS × MIN_INTRO_LINES`, the intro is skipped with
/// a stderr notice.
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
    logo_color: (u8, u8, u8),
) -> std::io::Result<()> {
    if intro_type == IntroType::None {
        return Ok(());
    }

    // Terminal-size guard. Below MIN_INTRO_COLS × MIN_INTRO_LINES the intros
    // clip badly, so skip the cinematic. The user-facing warning for this
    // case is emitted by the caller (event_loop.rs) BEFORE the alternate
    // screen is entered — printing here would leak into the rain matrix
    // (AB-10 rain-screen cleanliness).
    if w < MIN_INTRO_COLS || h < MIN_INTRO_LINES {
        return Ok(());
    }

    match intro_type {
        IntroType::Cosmic => {
            super::intro_cosmic::run_cosmic_intro(term, frame, cloud, w, h, logo_color)
        }
        IntroType::Logo => super::intro_logo::run_logo_intro(term, frame, cloud, w, h, logo_color),
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

    /// Number of particles currently active in the pool. Test-only —
    /// production rendering uses the free-list length directly when
    /// deciding whether to spawn. Tests use this to assert pool state
    /// after spawn/kill operations.
    #[inline]
    #[cfg(test)]
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
///
/// Routes through the Chroma Dragon OKLab perceptual blend pipeline
/// (`gradient::oklab_blend_rgb`) so the intro animation's color
/// transitions are perceptually uniform — same as the rain palette
/// gradients. This ensures the cinematic intro dissolves seamlessly
/// into the rain matrix without a visible color shift or muddiness.
///
/// Lightness L is linearly interpolated in OKLab. Chroma (a, b)
/// uses polar interpolation (shortest-arc hue rotation) to keep
/// saturation high through the midpoint — even on opposing-hue
/// transitions like purple → green.
///
/// Replaces the previous `chroma::legacy::blend_toward_rgb` (integer
/// sRGB linear blend) which produced muddy midpoints on
/// opposing-hue transitions.
///
/// Owner mandate: every color-processing site must route through the
/// chroma dragon pipeline (primary), with legacy fallback for non-
/// TrueColor terminals. The intro animation is the first thing the user
/// sees — its colors should be indistinguishable from the rain color
/// that follows, so the cinematic intro dissolves seamlessly into the
/// rain matrix without a visible color shift.
#[inline]
pub(super) fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    crate::chroma_dragon_engine::gradient::oklab_blend_rgb(a.0, a.1, a.2, b.0, b.1, b.2, t)
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
        .unwrap_or(NEON_GREEN_FALLBACK)
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

/// Returns `true` if a key event should cause the intro to skip.
///
/// Extracted from [`should_skip`] so the skip policy is unit-testable
/// without a real TTY: only `q` / `Q` (case-insensitive quit) skips.
/// Every other key — including modifiers, arrows, function keys, and
/// `Enter` / `Space` — is ignored.
///
/// Ctrl+C (SIGINT) is deprecated — only 'q' exits cosmostrix.
/// SIGTERM/SIGHUP/SIGQUIT are handled separately by the signal handler
/// setting [`GRACEFUL_SHUTDOWN`], so we don't need to match them here.
#[inline]
fn is_skip_key(key_event: &crossterm::event::KeyEvent) -> bool {
    // Modifier allowlist: accept ONLY bare 'q' (KeyModifiers::NONE) or
    // Shift+'Q' (KeyModifiers::SHIFT). Reject ALL other modifier bits:
    // CONTROL, ALT, SUPER, HYPER, META. Uses the canonical guard from
    // input.rs::is_unmodified_or_shift() so the rules stay consistent
    // across all key handling paths (handle_keybinding, HUD toggles, intro).
    //
    // Without this guard, Super+Q / Ctrl+Q / Alt+Q would skip the intro,
    // which is inconsistent with the main event loop where those combos
    // are silently rejected. The user must press bare 'q' (or Shift+Q
    // which produces 'Q' with SHIFT modifier) to skip.
    //
    // CapsLock is a keyboard state, NOT a KeyModifiers bit — it changes
    // which Char the terminal reports ('q' → 'Q' when CapsLock is on,
    // with modifiers=NONE). It is inherently allowed.
    if !is_unmodified_or_shift(key_event.modifiers) {
        return false;
    }
    if let crossterm::event::KeyCode::Char(c) = key_event.code {
        c == 'q' || c == 'Q'
    } else {
        false
    }
}

/// Drain the terminal event queue non-blocking. Returns `true` if the
/// intro should skip — but **only** when the user pressed `q` (case-
/// insensitive) or when [`GRACEFUL_SHUTDOWN`] is set (SIGTERM / SIGHUP / SIGQUIT).
///
/// All other key events are drained and ignored. This is deliberate: the
/// intros run for 5–6.25 s (Cosmic Burst ~5 s, Logo ~6.25 s), and accidental
/// presses of space / enter / arrow keys should not cut them short. The user
/// always has a fast exit via `q`. Ctrl+C (SIGINT) is deprecated.
///
/// # Why not "any key skips"?
///
/// * The intros are short enough that an "any key" skip is a footgun — a
///   stray keypress from a window manager focus change would abort it.
/// * `q` is the canonical "quit" key throughout cosmostrix's interactive
///   mode, so reusing it here keeps the mental model consistent.
/// * SIGTERM / SIGHUP / SIGQUIT remain hard exits for users who can't or
///   won't press `q` (e.g. piped input, scripted kills). Ctrl+C
///   (SIGINT) is deprecated — only 'q' exits cosmostrix.
///
/// # Terminal-gone guard
///
/// When the user force-closes the terminal during the intro, the PTY master
/// disappears. `poll_event(0)` returns `Ok(true)` instantly and forever
/// (POLLHUP makes the fd perpetually "readable"), and `read_event()` returns
/// `Err(EIO)`. The old loop used `if let Ok(...)` for `read_event`, which
/// silently swallowed the EIO — causing the loop to spin at 100% CPU for
/// 20 seconds until the watchdog fired.
///
/// The fix mirrors the main rain loop's drain logic (`event_loop.rs`):
/// detect EIO/EBADF/BrokenPipe from both `poll_event` and `read_event`, and
/// return `Ok(true)` (skip intro) so the normal shutdown path runs. This
/// drops post-SIGHUP CPU burn from 20s to < 1ms during the intro window.
pub(super) fn should_skip() -> std::io::Result<bool> {
    if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
        return Ok(true);
    }
    loop {
        match Terminal::poll_event(Duration::from_millis(0)) {
            Ok(false) => return Ok(false),
            Ok(true) => {}
            Err(e) if is_terminal_gone(&e) => return Ok(true),
            Err(e) => return Err(e),
        }
        match Terminal::read_event() {
            Ok(Event::Key(key_event)) => {
                if is_skip_key(&key_event) {
                    return Ok(true);
                }
                // All other keys are drained and ignored — the intro
                // continues playing. See `is_skip_key` for the rationale.
            }
            Ok(_) => {}
            Err(e) if is_terminal_gone(&e) => return Ok(true),
            Err(_) => {}
        }
        // Defensive: re-check the signal flag each iteration so a SIGHUP
        // arriving mid-drain breaks the loop within one iteration rather
        // than spinning until the next poll_event returns false.
        if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
            return Ok(true);
        }
    }
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
    fn min_intro_size_allows_responsive_scaling() {
        // v25 responsive: MIN_INTRO lowered from 80×24 to 10×5 so the
        // intros can play on small terminals via dynamic art scaling
        // (see intro_logo::scale_art). The hard floor is only for
        // absurdly tiny terminals where even a scaled-down logo would
        // be unreadable.
        assert_eq!(MIN_INTRO_COLS, 10);
        assert_eq!(MIN_INTRO_LINES, 5);
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
        // With OKLab perceptual blend, the midpoint is NOT the sRGB
        // linear midpoint (50, 100, 25). But endpoints are preserved
        // and the result is between a and b.
        let a = (0u8, 0u8, 0u8);
        let b = (100u8, 200u8, 50u8);
        let mid = lerp_rgb(a, b, 0.5);
        // Must be strictly between black and b (not equal to either).
        assert!(mid.0 > 0 && mid.0 < 100);
        assert!(mid.1 > 0 && mid.1 < 200);
        assert!(mid.2 > 0 && mid.2 < 50);
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

    // ── skip-key policy ──────────────────────────────────────────────────
    //
    // The intro must ONLY skip on `q` / `Q` (case-insensitive quit) —
    // every other key is drained and ignored. These tests pin the policy
    // so a future "make any key skip" refactor would fail loudly.

    fn key(code: crossterm::event::KeyCode) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn skip_key_accepts_lowercase_q() {
        assert!(is_skip_key(&key(crossterm::event::KeyCode::Char('q'))));
    }

    #[test]
    fn skip_key_accepts_uppercase_q() {
        assert!(is_skip_key(&key(crossterm::event::KeyCode::Char('Q'))));
    }

    #[test]
    fn skip_key_rejects_space() {
        assert!(!is_skip_key(&key(crossterm::event::KeyCode::Char(' '))));
    }

    #[test]
    fn skip_key_rejects_enter() {
        assert!(!is_skip_key(&key(crossterm::event::KeyCode::Enter)));
    }

    #[test]
    fn skip_key_rejects_escape() {
        assert!(!is_skip_key(&key(crossterm::event::KeyCode::Esc)));
    }

    #[test]
    fn skip_key_rejects_arrows() {
        use crossterm::event::KeyCode::*;
        assert!(!is_skip_key(&key(Up)));
        assert!(!is_skip_key(&key(Down)));
        assert!(!is_skip_key(&key(Left)));
        assert!(!is_skip_key(&key(Right)));
    }

    #[test]
    fn skip_key_rejects_other_letters() {
        // Sanity: only `q` / `Q` skip — not any other letter.
        for c in ['a', 'A', 'z', 'Z', 'x', 'X', 'p', 'P'] {
            assert!(
                !is_skip_key(&key(crossterm::event::KeyCode::Char(c))),
                "char {c:?} should NOT skip the intro"
            );
        }
    }

    #[test]
    fn skip_key_rejects_function_keys() {
        use crossterm::event::KeyCode::*;
        assert!(!is_skip_key(&key(F(1))));
        assert!(!is_skip_key(&key(F(12))));
    }

    #[test]
    fn skip_key_rejects_tab_and_backspace() {
        use crossterm::event::KeyCode::*;
        assert!(!is_skip_key(&key(Tab)));
        assert!(!is_skip_key(&key(Backspace)));
    }

    // ── modifier guard (allowlist: NONE | SHIFT only) ──────────────────
    //
    // Super+Q / Ctrl+Q / Alt+Q must NOT skip the intro. Only bare 'q'
    // or Shift+'Q' are allowed. This matches the main event loop's
    // is_unmodified_or_shift() guard.

    fn key_with_mod(
        code: crossterm::event::KeyCode,
        mods: crossterm::event::KeyModifiers,
    ) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(code, mods)
    }

    #[test]
    fn skip_key_rejects_super_q() {
        assert!(!is_skip_key(&key_with_mod(
            crossterm::event::KeyCode::Char('q'),
            crossterm::event::KeyModifiers::SUPER
        )));
    }

    #[test]
    fn skip_key_rejects_ctrl_q() {
        assert!(!is_skip_key(&key_with_mod(
            crossterm::event::KeyCode::Char('q'),
            crossterm::event::KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn skip_key_rejects_alt_q() {
        assert!(!is_skip_key(&key_with_mod(
            crossterm::event::KeyCode::Char('q'),
            crossterm::event::KeyModifiers::ALT
        )));
    }

    #[test]
    fn skip_key_accepts_shift_q() {
        // Shift+Q → 'Q' with SHIFT modifier — this IS allowed (CapsLock
        // equivalent on physical keyboard).
        assert!(is_skip_key(&key_with_mod(
            crossterm::event::KeyCode::Char('Q'),
            crossterm::event::KeyModifiers::SHIFT
        )));
    }

    #[test]
    fn skip_key_rejects_ctrl_shift_q() {
        // Ctrl+Shift+Q must NOT skip — CONTROL bit is present.
        assert!(!is_skip_key(&key_with_mod(
            crossterm::event::KeyCode::Char('Q'),
            crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT
        )));
    }

    // ── intro type bypass ────────────────────────────────────────────────
    //
    // `IntroType::None` must short-circuit before any rendering happens.
    // This is the contract `--benchmark` and friends rely on implicitly
    // (they bypass `run_interactive` entirely, but the intro entry point
    // must still be a no-op for `None` so a misconfigured config file
    // cannot accidentally invoke the cinematic).

    #[test]
    fn intro_type_none_equality_short_circuits() {
        // The run_intro early-out uses `==`. Confirm the enum supports it
        // and that `None` matches itself.
        assert!(IntroType::None == IntroType::None);
        assert!(IntroType::Logo != IntroType::None);
        assert!(IntroType::Cosmic != IntroType::None);
    }
}
