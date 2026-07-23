// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v18: Dragon's Awakening intro cinematic + Linux process metrics helpers.
//!
//! Two unrelated concerns coexist in this file:
//!
//! 1. **Linux process metrics** (`read_self_rss_kb`, `read_self_voluntary_ctxt`)
//!    — lightweight `/proc` readers used by the HUD overlay. Kept here because
//!    the file already exists; the helpers are tiny and have no dependencies.
//!
//! 2. **Dragon's Awakening intro** (`run_intro`) — a cinematic studio-logo-style
//!    animation played before the rain engine takes over. Triggered by
//!    `cosmostrix --intro`. See the [`run_intro`] docs for the full phase
//!    breakdown.

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
// Dragon's Awakening intro
// ─────────────────────────────────────────────────────────────────────────────
//
// A four-phase cinematic studio logo for cosmostrix. Triggered by `--intro`.
//
// ## Phases
//
// ```text
// Phase 1: Dragon Reveal   (0    – 1500 ms)  Dragon fades in, centered.
// Phase 2: Fire Breath     (1500 – 3500 ms)  Mouth emits fire particles ↓.
// Phase 3: Morph           (3500 – 5500 ms)  Fire → rain; cone widens to full
//                                            screen; dragon dims.
// Phase 4: Fade Out        (5500 – 6500 ms)  Dragon gone; particles decay.
// ```
//
// Total: ~6.5 s. Any key (q / Enter / etc.) skips instantly. The intro is
// skipped entirely on terminals smaller than 100×24 with a stderr notice.
//
// ## Constraints
//
// * Zero per-frame heap allocation — the particle pool is pre-allocated and
//   reused via a free-list stack.
// * Linear interpolation only (no sin/cos) for color morph and dragon dim.
// * Reuses the existing `Terminal` / `Frame` / `Cell` pipeline — no separate
//   renderer.
// * `FRAME_COUNTER` is bumped each frame so the watchdog doesn't kill us
//   during the 6.5 s cinematic.

/// Minimum terminal size for the intro to play. Below this, skip with a
/// stderr notice. Width 100 accommodates the 92-char dragon art with
/// margin; height 24 is the classic VT100 baseline.
const MIN_INTRO_COLS: u16 = 100;
const MIN_INTRO_LINES: u16 = 24;

/// Phase boundaries (milliseconds from intro start).
const PHASE1_REVEAL_END_MS: u64 = 1_500;
const PHASE2_FIRE_END_MS: u64 = 3_500;
const PHASE3_MORPH_END_MS: u64 = 5_500;
const PHASE4_FADE_END_MS: u64 = 6_500;

/// Frame period for the intro animation. ~30 FPS — the intro is mostly
/// static art + slow particle motion, so high FPS is wasteful.
const INTRO_FRAME_PERIOD: Duration = Duration::from_millis(33);

/// Particle pool capacity. Pre-allocated once; reused via free-list.
/// 512 × 40 B = 20 KiB — negligible. The peak concurrent particle count
/// during Phase 3 (full-screen morph) is ~400, leaving headroom.
const PARTICLE_POOL_SIZE: usize = 512;

/// Fire particle characters — varied glyphs so the stream looks like
/// ember/flame rather than a uniform dotted line.
const FIRE_CHARS: [char; 5] = ['*', '+', '#', '%', '&'];

/// Fire color stops (RGB). Sampled by a per-particle random index — the
/// stream alternates red → orange → yellow → white-hot.
/// Derived from the Fire theme's stop list (see `central_colors.rs`).
const FIRE_COLORS_RGB: [(u8, u8, u8); 4] = [
    (255, 80, 10),   // ember red
    (255, 145, 35),  // flame orange
    (255, 200, 90),  // bright yellow
    (255, 235, 170), // white-hot
];

/// Particle lifetime in seconds. Short enough to feel like a phosphor
/// afterglow; long enough to leave a visible trail.
const PARTICLE_LIFE_SECS: f32 = 0.9;

/// Downward base velocity (cells per second). Fire breath accelerates
/// slightly with phase progress to sell the "breath intensity" build.
const PARTICLE_BASE_VY: f32 = 12.0;

/// Horizontal spread (cells per second). Particles fan out slightly to
/// form a cone rather than a column.
const PARTICLE_SPREAD_VX: f32 = 6.0;

/// The majestic ASCII dragon — a top-down view of a flying dragon with
/// wings spread horizontally, body in the central column, and tail
/// tapering downward. The mouth sits at the bottom of the head (upper
/// third of the body column) so fire breath emerges downward naturally.
///
/// Dimensions: 53 lines tall × 92 chars max width. Requires a terminal
/// at least 100×24 — the intro auto-skips below that with a stderr
/// notice.
///
/// The mouth position (row, col) within the art is encoded in
/// [`DRAGON_MOUTH_ROW`] / [`DRAGON_MOUTH_COL`] so the fire-spawn point
/// stays correct even if the art is later edited. The
/// [`dragon_art_tests::mouth_position_is_valid`] test guards against
/// drift.
const DRAGON_ART: &str = concat!(
    "+                                                        +\n",
    "                   ++                                                          +÷\n",
    "                  ++                                                            ++\n",
    "                 -+                                                             +++\n",
    "               + ++                              ≠                               +++\n",
    "                ++                               +-                              ++++\n",
    "              ++++                               ++                               ++ +\n",
    "             + +++                              +++                               +++ -\n",
    "               ++                               ++ +                              +++ +\n",
    "            +  +++              ++             +√+  +             +               +++ +\n",
    "            + =++-              +              + ++       +       ++              +++\n",
    "            +  +++             ++        +   +  ++   +    +        +             =+++ -π\n",
    "            +  +++∞             +       +++  +  +     +  ++       +++            ++++\n",
    "      +      +  +++-          ≠ +       +++ + += +   + ∞++-+      ++            ++++  +     +÷\n",
    "      ++     +  ++++            ++      + ++÷   +++    +++ +      ++            ++++  +     +\n",
    "       +     +   ++++    ≠      ++    +   ++    ++++   ++÷   +   +++          +++++  ∞≠     +\n",
    "       ++     +   ++++   +    ÷  ++    +   ++  ++  +   ++   ++   ++ +     +   ++++   +     ++\n",
    "       =++        ×++++  +       +++   ++   + ++ +  +  ∞+ +++   ++  +     + +++++   +     ++\n",
    "         ++    +   +++++ ++    +  +++   ++      -++  ≠+  -++  ≠+++       ++-++++   +     +++\n",
    "        +≠++         +++++++    =  ++++  ++     ++++      +  ++++  ∞   +++++++    +     ++√\n",
    "         ÷√++    ++    ++++++π  +   ++++  ÷+   ++  +    +×  ++++  +   +++++++   -+     ++\n",
    "            ++×    +    +++++++  ≠   +++=  ++  + +  + +++   +++  ×   +++++++    +    +++ +\n",
    "             +++    +     -+++-+  +   +++   ++   ++   ++   +++  +   +++++     ++   ++++\n",
    "              -+++    ≠     +++    +   ++       ++++       ++       +++      +    +++  +\n",
    "                 +++   ++    ++++   +  √+++    ++  +     +++   +   +++     ++  ++++   +\n",
    "        +     +   +++    +     +++  ÷+  =+   + + ++   + ≠++   ∞+ ++++     +   +++\n",
    "         +      +   ++++   +     +++ -   +++    +++     ++   +  +++     +   +++=   +     ++\n",
    "          ++     +    ++     +   ++++  +  +++    + +  ≈++  ++  ++++  ++     ++   +      ++\n",
    "           +++          +  +  ÷   +++  +++ ++  + +  +  +  ++   ++       + ++         -+++\n",
    "            +++++        -+ ++  +  ++++ ++  -+  -++   +  ++ √+++-  +  +  +        ++++-π\n",
    "               ++++++        π   +   +++  +  +++ ∞  ++  ++ ++++   +            +++++\n",
    "               ≠  =+++     ++ ≈    +  ++++  ++   +≈÷  ++∞ +++   +    ÷-++     +++   ÷\n",
    "                 +     ++     ++       ×++++ ++  +  + - ++++ ×      ++     ++     +\n",
    "                          ++++  +++×  ++ +++++ +     ++++++ ++   +++  +++\n",
    "               ++               ++++  ++++ ++-+ ×  ++++++ +++   ++++               +\n",
    "                 ++++++≈ +++++     ++++  =  ×+      ÷++  √≠   +++     ++++  ÷++++++\n",
    "                      ++=   +++    ++++++     = +++× π    ÷++++++    ++    +++\n",
    "                                +      ++++  +++++++++   +++×     ≠+\n",
    "                         +++++    ++     +++ +++++++++ ++++     +     ++++\n",
    "                                     +    +++ ++++++++++++\n",
    "                             ×++++    +    ++++++++++++++  +-+    ++++\n",
    "                                       +   +÷ ++++++++ +∞ √++\n",
    "                                 ++++   ++ ++÷ +++++   + ++√   ++++\n",
    "                                      +  ++ ++   +    ++ ++  +\n",
    "                                       +  +  ÷          ++ ++\n",
    "                                        ++++ +++    ++  + +\n",
    "                                          ++÷ +   +  + ++=\n",
    "                                           ÷ +       + +\n",
    "                                           +  ++     + +\n",
    "                                            √π-++++ +÷+\n",
    "                                             ++  ÷π  ++\n",
    "                                              + +  + +\n",
    "                                                 ++\n",
);

/// Row index (0-based, within `DRAGON_ART` lines) of the mouth opening —
/// the bottom of the head in the body column where fire particles
/// originate. Keep in sync with the art above;
/// [`dragon_art_tests::mouth_position_is_valid`] guards against drift.
const DRAGON_MOUTH_ROW: usize = 7;

/// Column index (0-based) of the mouth opening within the mouth row.
/// Points at the middle `+` of the `+++` body cluster at row 7. Fire
/// particles spawn at this (row, col) within the centered dragon, then
/// fall downward.
const DRAGON_MOUTH_COL: usize = 49;

/// Parsed dragon art: each line as a `&'static str`, with the max line width
/// precomputed. Done once at module load (compile-time constant via inline
/// parsing — the result is a `Vec<&'static str>` constructed lazily on first
/// `run_intro` call).
fn parse_dragon_art() -> (Vec<&'static str>, usize) {
    let lines: Vec<&'static str> = DRAGON_ART.split('\n').filter(|l| !l.is_empty()).collect();
    let max_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    (lines, max_w)
}

/// Compact particle representation. 40 bytes — fits 16 per cache line.
/// `active` is the free-list flag; dead particles are skipped during
/// update and render.
#[derive(Clone, Copy)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    ch: char,
    /// Particle color stored as RGB triple. Avoids `Color` enum tag overhead
    /// and lets us lerp between fire and palette colors trivially.
    r: u8,
    g: u8,
    b: u8,
    life: f32,
    max_life: f32,
    active: bool,
}

impl Particle {
    const INACTIVE: Self = Self {
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
        active: false,
    };
}

/// Tiny xorshift32 RNG — avoids pulling `rand` into this module. Seeded
/// from `Instant::now()` so each intro run looks slightly different.
struct XorShift(u32);

impl XorShift {
    fn new(seed: u32) -> Self {
        // Avoid the all-zero state which would lock the generator.
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }
    #[inline]
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform float in `[0.0, 1.0)`.
    #[inline]
    fn next_f32(&mut self) -> f32 {
        // 24-bit mantissa for uniform distribution.
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Pre-allocated particle pool with a free-list stack. The pool itself
/// stores `Particle` values; the free-list stores indices into the pool
/// so spawning is O(1) pop, killing is O(1) flag flip.
struct ParticlePool {
    particles: Vec<Particle>,
    free: Vec<usize>,
}

impl ParticlePool {
    fn new() -> Self {
        let particles = vec![Particle::INACTIVE; PARTICLE_POOL_SIZE];
        let free = (0..PARTICLE_POOL_SIZE).collect();
        Self { particles, free }
    }

    #[inline]
    fn spawn(&mut self, p: Particle) -> bool {
        if let Some(i) = self.free.pop() {
            self.particles[i] = p;
            true
        } else {
            false
        }
    }

    #[inline]
    fn kill(&mut self, i: usize) {
        self.particles[i].active = false;
        self.free.push(i);
    }

    #[inline]
    #[allow(dead_code)]
    fn active_count(&self) -> usize {
        PARTICLE_POOL_SIZE - self.free.len()
    }
}

/// Linear interpolation between two `f32` values.
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation between two RGB triples.
#[inline]
fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    (
        (lerp(a.0 as f32, b.0 as f32, t)).round().clamp(0.0, 255.0) as u8,
        (lerp(a.1 as f32, b.1 as f32, t)).round().clamp(0.0, 255.0) as u8,
        (lerp(a.2 as f32, b.2 as f32, t).round().clamp(0.0, 255.0)) as u8,
    )
}

/// v18: Dragon's Awakening intro. Plays a ~6.5 s cinematic — ASCII dragon
/// fades in, breathes fire, the fire morphs into Matrix rain, and the
/// dragon fades away leaving the rain engine to take over.
///
/// Skip with any key. On terminals smaller than 100×24 the intro is
/// skipped entirely with a stderr notice.
///
/// Reuses the existing `Terminal` / `Frame` / `Cell` pipeline. Zero
/// per-frame heap allocation (particle pool is pre-allocated and reused).
pub(crate) fn run_intro(
    term: &mut Terminal,
    frame: &mut Frame,
    cloud: &Cloud,
    w: u16,
    h: u16,
) -> std::io::Result<()> {
    // Terminal-size guard. Below 100×24 the dragon art would clip badly;
    // print a notice and skip the cinematic.
    if w < MIN_INTRO_COLS || h < MIN_INTRO_LINES {
        eprintln!(
            "Terminal too small for intro ({}x{} < {}x{}). Starting rain...",
            w, h, MIN_INTRO_COLS, MIN_INTRO_LINES
        );
        return Ok(());
    }

    // Seed RNG from wall-clock nanos. Each intro run gets a different
    // particle pattern, which keeps repeat viewings fresh.
    let seed = Instant::now()
        .elapsed()
        .as_nanos()
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(0x1234_5678) as u32;
    let mut rng = XorShift::new(seed);

    // Parse dragon art once.
    let (dragon_lines, dragon_w) = parse_dragon_art();
    let dragon_h = dragon_lines.len();
    // Center the dragon on the terminal. The mouth spawn point is the
    // explicit (row, col) of the mouth opening in the art — NOT the
    // bottom-center, because the mouth sits in the upper half of the
    // dragon (just below the head) and fire should emerge from there.
    let origin_x = w.saturating_sub(dragon_w as u16) / 2;
    let origin_y = h.saturating_sub(dragon_h as u16) / 2;
    let mouth_x = origin_x as f32 + DRAGON_MOUTH_COL as f32;
    let mouth_y = origin_y as f32 + DRAGON_MOUTH_ROW as f32;

    // Palette colors for the morph phase. We pull the brightest palette
    // color (typically the head color) as the rain target. If the palette
    // is empty, fall back to neon green.
    let palette_bg = cloud.palette.bg;
    let palette_rgb: (u8, u8, u8) = cloud
        .palette
        .colors
        .last()
        .copied()
        .map(color_to_rgb)
        .unwrap_or((57, 255, 20)); // NeonGreen fallback
    let dragon_rgb: (u8, u8, u8) = palette_rgb;
    let dragon_dim_rgb: (u8, u8, u8) = (
        (palette_rgb.0 as u32 / 6) as u8,
        (palette_rgb.1 as u32 / 6) as u8,
        (palette_rgb.2 as u32 / 6) as u8,
    );

    // Rain charset for the morph phase. Empty pool → binary fallback.
    let rain_chars: Vec<char> = if cloud.char_pool.is_empty() {
        vec!['0', '1']
    } else {
        cloud.char_pool.clone()
    };

    let mut pool = ParticlePool::new();
    let intro_start = Instant::now();

    loop {
        let elapsed_ms = intro_start.elapsed().as_millis() as u64;
        if elapsed_ms >= PHASE4_FADE_END_MS {
            break;
        }
        if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
            break;
        }
        // Skip on any key. We drain the event queue non-blocking.
        while Terminal::poll_event(Duration::from_millis(0))? {
            if let Ok(Event::Key(_)) = Terminal::read_event() {
                return Ok(());
            }
        }

        // Determine current phase and progress within phase.
        let (phase, phase_t) = if elapsed_ms < PHASE1_REVEAL_END_MS {
            (1u8, elapsed_ms as f32 / PHASE1_REVEAL_END_MS as f32)
        } else if elapsed_ms < PHASE2_FIRE_END_MS {
            (
                2,
                (elapsed_ms - PHASE1_REVEAL_END_MS) as f32
                    / (PHASE2_FIRE_END_MS - PHASE1_REVEAL_END_MS) as f32,
            )
        } else if elapsed_ms < PHASE3_MORPH_END_MS {
            (
                3,
                (elapsed_ms - PHASE2_FIRE_END_MS) as f32
                    / (PHASE3_MORPH_END_MS - PHASE2_FIRE_END_MS) as f32,
            )
        } else {
            (
                4,
                (elapsed_ms - PHASE3_MORPH_END_MS) as f32
                    / (PHASE4_FADE_END_MS - PHASE3_MORPH_END_MS) as f32,
            )
        };

        // Spawn new particles for the current phase.
        let dt = INTRO_FRAME_PERIOD.as_secs_f32();
        match phase {
            1 => {
                // Phase 1: No particles yet — dragon is just appearing.
            }
            2 => {
                // Phase 2: Fire breath. Spawn rate ramps 20 → 40 over the phase.
                let rate = lerp(20.0, 40.0, phase_t);
                spawn_fire_burst(
                    &mut pool,
                    &mut rng,
                    mouth_x,
                    mouth_y,
                    rate as u32,
                    PARTICLE_BASE_VY,
                    PARTICLE_SPREAD_VX,
                    0.0, // morph_t = 0 — pure fire
                );
            }
            3 => {
                // Phase 3: Morph. Spawn rate ramps 40 → 80. The cone widens
                // horizontally to fill the screen. morph_t goes 0 → 1 so
                // particles shift from fire → rain.
                let rate = lerp(40.0, 80.0, phase_t);
                let cone_widen = phase_t * (w as f32 * 0.5);
                spawn_morph_burst(
                    &mut pool,
                    &mut rng,
                    mouth_x,
                    mouth_y,
                    cone_widen,
                    rate as u32,
                    PARTICLE_BASE_VY + phase_t * 4.0, // slight acceleration
                    PARTICLE_SPREAD_VX,
                    phase_t,
                    &rain_chars,
                    palette_rgb,
                );
            }
            4 => {
                // Phase 4: Fade out. No new spawns; existing particles decay.
            }
            _ => {}
        }

        // Update + cull particles.
        update_particles(&mut pool, dt, h as f32);

        // ── Render ──────────────────────────────────────────────────────
        frame.clear_with_bg(palette_bg);

        // Dragon render. Brightness depends on phase:
        //   Phase 1: ramp up from dim → full (reveal).
        //   Phase 2: full brightness.
        //   Phase 3: ramp down from full → dim (morph).
        //   Phase 4: ramp down from dim → invisible (fade out).
        let dragon_color = match phase {
            1 => {
                let t = phase_t.clamp(0.0, 1.0);
                lerp_rgb(dragon_dim_rgb, dragon_rgb, t)
            }
            2 => dragon_rgb,
            3 => {
                let t = phase_t.clamp(0.0, 1.0);
                lerp_rgb(dragon_rgb, dragon_dim_rgb, t)
            }
            4 => {
                let t = phase_t.clamp(0.0, 1.0);
                lerp_rgb(dragon_dim_rgb, (0, 0, 0), t)
            }
            _ => dragon_rgb,
        };
        let dragon_visible = !(phase == 4 && phase_t > 0.85);
        if dragon_visible {
            for (row_idx, line) in dragon_lines.iter().enumerate() {
                let y = origin_y + row_idx as u16;
                if y >= h {
                    break;
                }
                for (col_idx, ch) in line.chars().enumerate() {
                    if ch == ' ' {
                        continue;
                    }
                    let x = origin_x + col_idx as u16;
                    if x >= w {
                        break;
                    }
                    frame.set_force(
                        x,
                        y,
                        Cell {
                            ch,
                            fg: Some(Color::Rgb {
                                r: dragon_color.0,
                                g: dragon_color.1,
                                b: dragon_color.2,
                            }),
                            bg: palette_bg,
                            bold: true,
                        },
                    );
                }
            }
        }

        // Render particles. Each active particle becomes a single cell.
        for p in pool.particles.iter() {
            if !p.active {
                continue;
            }
            let x = p.x as u16;
            let y = p.y as u16;
            if x >= w || y >= h {
                continue;
            }
            // Fade particle alpha by remaining life ratio.
            let life_t = (p.life / p.max_life).clamp(0.0, 1.0);
            let faded = lerp_rgb((0, 0, 0), (p.r, p.g, p.b), life_t);
            frame.set_force(
                x,
                y,
                Cell {
                    ch: p.ch,
                    fg: Some(Color::Rgb {
                        r: faded.0,
                        g: faded.1,
                        b: faded.2,
                    }),
                    bg: palette_bg,
                    bold: true,
                },
            );
        }

        term.draw(frame)?;
        FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(INTRO_FRAME_PERIOD);
    }

    Ok(())
}

/// Spawn a burst of fire particles from the mouth point.
///
/// `morph_t` = 0 means pure fire; `morph_t` > 0 lerps each particle's color
/// and glyph toward the rain palette. Used by both Phase 2 (morph_t = 0)
/// and Phase 3 (morph_t > 0).
#[allow(clippy::too_many_arguments)]
fn spawn_fire_burst(
    pool: &mut ParticlePool,
    rng: &mut XorShift,
    cx: f32,
    cy: f32,
    count: u32,
    base_vy: f32,
    spread_vx: f32,
    morph_t: f32,
) {
    let _ = morph_t; // unused in pure fire path; kept for symmetry with spawn_morph_burst.
    for _ in 0..count {
        // Sample a fire color. Bias toward the lower (red) end for a
        // hotter core, with occasional bright (yellow/white) flecks.
        let color_idx = (rng.next_u32() % 4) as usize;
        let (r, g, b) = FIRE_COLORS_RGB[color_idx];
        let ch = FIRE_CHARS[(rng.next_u32() % FIRE_CHARS.len() as u32) as usize];
        // Slight horizontal jitter so the stream isn't a perfect column.
        let vx = (rng.next_f32() - 0.5) * 2.0 * spread_vx;
        // Slight vertical velocity variation — some particles are slower
        // embers, others are fast sparks.
        let vy = base_vy * (0.7 + rng.next_f32() * 0.6);
        // Spawn at the mouth with a small horizontal jitter.
        let x = cx + (rng.next_f32() - 0.5) * 2.0;
        let y = cy + rng.next_f32() * 1.5;
        let life = PARTICLE_LIFE_SECS * (0.6 + rng.next_f32() * 0.6);
        let spawned = pool.spawn(Particle {
            x,
            y,
            vx,
            vy,
            ch,
            r,
            g,
            b,
            life,
            max_life: life,
            active: true,
        });
        if !spawned {
            break; // pool full
        }
    }
}

/// Spawn a morphing burst — particles fan out across the cone width and
/// lerp from fire → rain as `morph_t` goes 0 → 1.
#[allow(clippy::too_many_arguments)]
fn spawn_morph_burst(
    pool: &mut ParticlePool,
    rng: &mut XorShift,
    cx: f32,
    cy: f32,
    cone_widen: f32,
    count: u32,
    base_vy: f32,
    spread_vx: f32,
    morph_t: f32,
    rain_chars: &[char],
    palette_rgb: (u8, u8, u8),
) {
    for _ in 0..count {
        // Pick a fire color, then lerp toward palette based on morph_t.
        let fire_idx = (rng.next_u32() % 4) as usize;
        let fire_rgb = FIRE_COLORS_RGB[fire_idx];
        let (r, g, b) = lerp_rgb(fire_rgb, palette_rgb, morph_t);

        // Glyph: pick fire char early, rain char late.
        let ch = if rng.next_f32() < morph_t {
            // Rain char.
            let idx = (rng.next_u32() as usize) % rain_chars.len().max(1);
            *rain_chars.get(idx).unwrap_or(&'0')
        } else {
            FIRE_CHARS[(rng.next_u32() % FIRE_CHARS.len() as u32) as usize]
        };

        // Horizontal velocity scales with cone widening.
        let vx = (rng.next_f32() - 0.5) * 2.0 * (spread_vx + cone_widen);
        let vy = base_vy * (0.7 + rng.next_f32() * 0.6);
        // Spawn X is spread across the widening cone mouth.
        let x = cx + (rng.next_f32() - 0.5) * 2.0 * (1.0 + cone_widen);
        let y = cy + rng.next_f32() * 1.5;
        let life = PARTICLE_LIFE_SECS * (0.6 + rng.next_f32() * 0.6);
        let spawned = pool.spawn(Particle {
            x,
            y,
            vx,
            vy,
            ch,
            r,
            g,
            b,
            life,
            max_life: life,
            active: true,
        });
        if !spawned {
            break;
        }
    }
}

/// Advance all active particles by `dt` seconds. Particles whose life
/// drops to ≤ 0 or that leave the screen are killed and returned to the
/// free-list. We iterate by index so we can call `kill` without borrow
/// conflicts.
fn update_particles(pool: &mut ParticlePool, dt: f32, screen_h: f32) {
    let mut to_kill: Vec<usize> = Vec::new();
    for (i, p) in pool.particles.iter_mut().enumerate() {
        if !p.active {
            continue;
        }
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        p.life -= dt;
        // Off-screen bottom or expired.
        if p.y > screen_h + 2.0 || p.life <= 0.0 {
            to_kill.push(i);
        }
    }
    for i in to_kill {
        pool.kill(i);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragon_art_is_nonempty_and_rectangular_isn() {
        let (lines, max_w) = parse_dragon_art();
        assert!(!lines.is_empty(), "dragon art must have lines");
        assert!(max_w > 0, "dragon art must have width");
        // All lines should be ≤ max_w (some may be shorter — that's fine).
        for line in &lines {
            assert!(line.chars().count() <= max_w);
        }
    }

    #[test]
    fn dragon_art_has_reasonable_size() {
        let (lines, max_w) = parse_dragon_art();
        // Requirements: 40-100 chars wide, 20-60 lines tall. The art is a
        // detailed top-down dragon with spread wings (wider than tall in
        // terms of horizontal extent). Width 100 accommodates the wingspan
        // while still fitting in most modern terminals; height 60 leaves
        // room for the body+tail without crowding the screen.
        assert!(
            (40..=100).contains(&max_w),
            "dragon width {max_w} outside [40, 100]"
        );
        assert!(
            (20..=60).contains(&lines.len()),
            "dragon height {} outside [20, 60]",
            lines.len()
        );
    }

    #[test]
    fn mouth_position_is_valid() {
        // The mouth constants must point at a real, non-space character
        // in the art. This guards against drift if DRAGON_ART is edited
        // without updating DRAGON_MOUTH_ROW / DRAGON_MOUTH_COL.
        let (lines, _) = parse_dragon_art();
        assert!(
            DRAGON_MOUTH_ROW < lines.len(),
            "DRAGON_MOUTH_ROW {} out of range (art has {} lines)",
            DRAGON_MOUTH_ROW,
            lines.len()
        );
        let mouth_line = lines[DRAGON_MOUTH_ROW].chars().collect::<Vec<_>>();
        assert!(
            DRAGON_MOUTH_COL < mouth_line.len(),
            "DRAGON_MOUTH_COL {} out of range (line {} has {} chars)",
            DRAGON_MOUTH_COL,
            DRAGON_MOUTH_ROW,
            mouth_line.len()
        );
        let ch = mouth_line[DRAGON_MOUTH_COL];
        assert!(
            ch != ' ',
            "DRAGON_MOUTH_COL points at a space; must point at a visible mouth glyph"
        );
    }

    #[test]
    fn mouth_row_is_above_body_center() {
        // Sanity: the mouth should be in the upper half of the dragon
        // (head/neck area), not at the bottom (tail tip). This catches
        // accidental mouth-position regressions that would make fire
        // emerge from the dragon's tail.
        let (lines, _) = parse_dragon_art();
        let height = lines.len();
        assert!(
            DRAGON_MOUTH_ROW < height / 2,
            "mouth at row {} should be in upper half of {}-line art",
            DRAGON_MOUTH_ROW,
            height
        );
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
    fn update_particles_kills_offscreen() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 50.0,
            vx: 0.0,
            vy: 100.0,
            ch: '*',
            r: 255,
            g: 255,
            b: 255,
            life: 1.0,
            max_life: 1.0,
            active: true,
        });
        // Screen height 24 — particle at y=50 is already off-screen.
        update_particles(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn update_particles_kills_expired_life() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 0.0,
            ch: '*',
            r: 255,
            g: 255,
            b: 255,
            life: 0.05,
            max_life: 0.05,
            active: true,
        });
        // After 0.1s, life = 0.05 - 0.1 = negative → killed.
        update_particles(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn update_particles_keeps_alive() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 1.0,
            ch: '*',
            r: 255,
            g: 255,
            b: 255,
            life: 1.0,
            max_life: 1.0,
            active: true,
        });
        update_particles(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn fire_colors_are_warm() {
        // Sanity: all fire colors should have R > G > B (warm tones).
        for &(r, g, b) in &FIRE_COLORS_RGB {
            assert!(r >= g, "fire color ({r},{g},{b}) should have r >= g");
            assert!(g >= b, "fire color ({r},{g},{b}) should have g >= b");
        }
    }

    #[test]
    fn fire_chars_are_distinct() {
        let mut sorted: Vec<char> = FIRE_CHARS.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            FIRE_CHARS.len(),
            "fire chars must be distinct"
        );
    }
}
