// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v19: Cosmic Burst intro + Linux process metrics helpers.
//!
//! Two unrelated concerns coexist in this file:
//!
//! 1. **Linux process metrics** (`read_self_rss_kb`, `read_self_voluntary_ctxt`)
//!    — lightweight `/proc` readers used by the HUD overlay. Kept here because
//!    the file already exists; the helpers are tiny and have no dependencies.
//!
//! 2. **Cosmic Burst intro** (`run_intro`) — a cinematic studio-logo-style
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
// Cosmic Burst intro
// ─────────────────────────────────────────────────────────────────────────────
//
// A four-phase cinematic studio logo for cosmostrix. Triggered by `--intro`.
//
// ## Phases
//
// ```text
// Phase 1: Singularity  (0    – 1000 ms)  A point of light pulses at center.
// Phase 2: Burst         (1000 – 2500 ms)  Particles explode outward + spiral.
// Phase 3: Morph         (2500 – 4000 ms)  Particles slow + turn downward,
//                                           colors shift cosmic → palette.
// Phase 4: Rain Handoff  (4000 – 5000 ms)  Particles fade; rain engine takes
//                                           over seamlessly.
// ```
//
// Total: ~5 s. Any key (q / Enter / etc.) skips instantly. The intro is
// skipped entirely on terminals smaller than 80×24 with a stderr notice.
//
// ## Constraints
//
// * Zero per-frame heap allocation — the particle pool is pre-allocated and
//   reused via a free-list stack.
// * Reuses the existing `Terminal` / `Frame` / `Cell` pipeline — no separate
//   renderer.
// * `FRAME_COUNTER` is bumped each frame so the watchdog doesn't kill us
//   during the 5 s cinematic.

/// Minimum terminal size for the intro to play. Below this, skip with a
/// stderr notice. Matches the classic 80×24 VT100 baseline.
const MIN_INTRO_COLS: u16 = 80;
const MIN_INTRO_LINES: u16 = 24;

/// Phase boundaries (milliseconds from intro start).
const PHASE1_SINGULARITY_END_MS: u64 = 1_000;
const PHASE2_BURST_END_MS: u64 = 2_500;
const PHASE3_MORPH_END_MS: u64 = 4_000;
const PHASE4_RAIN_END_MS: u64 = 5_000;

/// Frame period for the intro animation. ~30 FPS — the intro is mostly
/// particle motion, so 30 FPS is smooth without burning CPU.
const INTRO_FRAME_PERIOD: Duration = Duration::from_millis(33);

/// Particle pool capacity. Pre-allocated once; reused via free-list.
/// 512 × 48 B = 24 KiB — negligible. The peak concurrent particle count
/// during Phase 2 (burst) is ~120, leaving ample headroom.
const PARTICLE_POOL_SIZE: usize = 512;

/// Burst particle characters — varied glyphs so the explosion looks like
/// cosmic debris rather than a uniform dotted cloud.
const BURST_CHARS: [char; 6] = ['*', '+', '#', '%', '&', '@'];

/// Cosmic color stops (RGB). Sampled by per-particle random index — the
/// burst alternates gold (energy), purple (brand), and cyan (plasma).
const COSMIC_COLORS_RGB: [(u8, u8, u8); 3] = [
    (255, 200, 0),  // bright gold
    (168, 85, 247), // purple (brand)
    (0, 255, 255),  // cyan
];

/// Singularity color — pure white-hot at the center of the burst.
const SINGULARITY_RGB: (u8, u8, u8) = (255, 255, 255);

/// Particle lifetime in seconds. Short enough to feel like a phosphor
/// afterglow; long enough to leave a visible trail during the burst.
const PARTICLE_LIFE_SECS: f32 = 1.4;

/// Burst particle speed range (cells per second). Particles fan out at
/// random speeds within this range.
const BURST_SPEED_MIN: f32 = 10.0;
const BURST_SPEED_MAX: f32 = 30.0;

/// Morph-phase target speed range. Particles decelerate to this range
/// as they transition to rain behavior.
const MORPH_SPEED_MIN: f32 = 5.0;
const MORPH_SPEED_MAX: f32 = 15.0;

/// Spiral rate range (radians per second). Each particle's angle rotates
/// by a small random amount per frame, giving the explosion a cosmic
/// spiral feel rather than a straight radial spread.
const SPIRAL_RATE_MIN: f32 = 0.5; // ~29 deg/sec
const SPIRAL_RATE_MAX: f32 = 1.5; // ~86 deg/sec

/// Number of burst particles to spawn during Phase 2.
const BURST_PARTICLE_COUNT: u32 = 100;

/// Downward base velocity (cells per second) during the morph phase.
/// Gravity-like acceleration pulling particles toward the rain direction.
const MORPH_DOWNWARD_VY: f32 = 14.0;

/// Parsed particle representation. 48 bytes — fits ~1.3 per cache line.
/// `active` is the free-list flag; dead particles are skipped during
/// update and render.
///
/// `angle` and `speed` are the polar-coordinate form of velocity. We
/// store them alongside `vx`/`vy` because:
/// * Phase 2 (burst) needs `angle` for spiral motion (angle += spiral_rate).
/// * Phase 3 (morph) needs `speed` for deceleration.
/// * `vx`/`vy` are kept as the cartesian cache for rendering.
#[derive(Clone, Copy)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    ch: char,
    /// Particle color stored as RGB triple. Avoids `Color` enum tag overhead
    /// and lets us lerp between cosmic and palette colors trivially.
    r: u8,
    g: u8,
    b: u8,
    life: f32,
    max_life: f32,
    /// Current direction in radians (0 = right, π/2 = down). Updated each
    /// frame by `spiral_rate` during Phase 2.
    angle: f32,
    /// Current speed in cells per second. Decelerates during Phase 3.
    speed: f32,
    /// Per-particle angular velocity (radians per second). Sampled at
    /// spawn time from `[SPIRAL_RATE_MIN, SPIRAL_RATE_MAX)`.
    spiral_rate: f32,
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
        angle: 0.0,
        speed: 0.0,
        spiral_rate: 0.0,
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

/// v19: Cosmic Burst intro. Plays a ~5 s cinematic — a singularity
/// appears at center, pulses, explodes into cosmic particles, the
/// particles slow and morph into Matrix rain, and the rain engine
/// takes over seamlessly.
///
/// Skip with any key. On terminals smaller than 80×24 the intro is
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
    // Terminal-size guard. Below 80×24 the burst would clip badly;
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

    // Center of the screen — the singularity point.
    let center_x = w as f32 * 0.5;
    let center_y = h as f32 * 0.5;

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
        if elapsed_ms >= PHASE4_RAIN_END_MS {
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
        let (phase, phase_t) = if elapsed_ms < PHASE1_SINGULARITY_END_MS {
            (1u8, elapsed_ms as f32 / PHASE1_SINGULARITY_END_MS as f32)
        } else if elapsed_ms < PHASE2_BURST_END_MS {
            (
                2,
                (elapsed_ms - PHASE1_SINGULARITY_END_MS) as f32
                    / (PHASE2_BURST_END_MS - PHASE1_SINGULARITY_END_MS) as f32,
            )
        } else if elapsed_ms < PHASE3_MORPH_END_MS {
            (
                3,
                (elapsed_ms - PHASE2_BURST_END_MS) as f32
                    / (PHASE3_MORPH_END_MS - PHASE2_BURST_END_MS) as f32,
            )
        } else {
            (
                4,
                (elapsed_ms - PHASE3_MORPH_END_MS) as f32
                    / (PHASE4_RAIN_END_MS - PHASE3_MORPH_END_MS) as f32,
            )
        };

        // Spawn new particles for the current phase.
        let dt = INTRO_FRAME_PERIOD.as_secs_f32();
        match phase {
            1 => {
                // Phase 1: No particles yet — singularity is just appearing.
            }
            2 => {
                // Phase 2: Burst. Spawn all particles in the first 200 ms of
                // the phase so the explosion feels instantaneous.
                if phase_t < 0.08 {
                    spawn_burst(
                        &mut pool,
                        &mut rng,
                        center_x,
                        center_y,
                        BURST_PARTICLE_COUNT,
                    );
                }
            }
            3 => {
                // Phase 3: Morph. No new spawns; existing particles
                // decelerate and turn downward.
            }
            4 => {
                // Phase 4: Rain handoff. No new spawns; existing particles
                // fade out.
            }
            _ => {}
        }

        // Update + cull particles. Phase 3 applies morph deceleration.
        let morph_t = if phase == 3 {
            phase_t
        } else if phase == 4 {
            1.0
        } else {
            0.0
        };
        update_particles(&mut pool, dt, h as f32, morph_t, palette_rgb, &rain_chars);

        // ── Render ──────────────────────────────────────────────────────
        frame.clear_with_bg(palette_bg);

        // Singularity render (Phase 1 + early Phase 2). Brightness pulses
        // three times with increasing frequency during Phase 1, then fades
        // out as the burst takes over.
        let singularity_visible = phase == 1 || (phase == 2 && phase_t < 0.3);
        if singularity_visible {
            let brightness = if phase == 1 {
                // Triangle wave with chirped frequency: 3 Hz → 9 Hz over 1 s.
                let pulse_freq = 3.0 + 6.0 * phase_t;
                let phase_angle = (pulse_freq * phase_t).fract();
                if phase_angle < 0.5 {
                    0.5 + phase_angle
                } else {
                    1.5 - phase_angle
                }
            } else {
                // Phase 2: fade out 1.0 → 0.0 over the first 30% of burst.
                1.0 - (phase_t / 0.3)
            };
            let brightness = brightness.clamp(0.0, 1.0);
            let color = lerp_rgb((0, 0, 0), SINGULARITY_RGB, brightness);
            let cx = center_x as u16;
            let cy = center_y as u16;
            if cx < w && cy < h {
                frame.set_force(
                    cx,
                    cy,
                    Cell {
                        ch: '@',
                        fg: Some(Color::Rgb {
                            r: color.0,
                            g: color.1,
                            b: color.2,
                        }),
                        bg: palette_bg,
                        bold: true,
                    },
                );
            }
        }

        // Render particles. Each active particle becomes a single cell.
        // During Phase 2, render a 2-cell trail behind each particle for
        // a streaking effect.
        for p in pool.particles.iter() {
            if !p.active {
                continue;
            }
            let x = p.x as u16;
            let y = p.y as u16;
            // Fade particle alpha by remaining life ratio.
            let life_t = (p.life / p.max_life).clamp(0.0, 1.0);
            let faded = lerp_rgb((0, 0, 0), (p.r, p.g, p.b), life_t);
            if x < w && y < h {
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
            // Trail: 2 trailing cells behind the particle (only during
            // burst phase when particles are fast-moving). Trail cells
            // are dimmer and use the same glyph.
            if phase == 2 || (phase == 3 && phase_t < 0.5) {
                for trail_step in 1..=2u16 {
                    let tx = (p.x - p.vx * dt * trail_step as f32) as u16;
                    let ty = (p.y - p.vy * dt * trail_step as f32) as u16;
                    if tx < w && ty < h {
                        let trail_brightness = life_t * (0.4 / trail_step as f32);
                        let trail_color = lerp_rgb((0, 0, 0), (p.r, p.g, p.b), trail_brightness);
                        frame.set_force(
                            tx,
                            ty,
                            Cell {
                                ch: p.ch,
                                fg: Some(Color::Rgb {
                                    r: trail_color.0,
                                    g: trail_color.1,
                                    b: trail_color.2,
                                }),
                                bg: palette_bg,
                                bold: false,
                            },
                        );
                    }
                }
            }
        }

        term.draw(frame)?;
        FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(INTRO_FRAME_PERIOD);
    }

    Ok(())
}

/// Spawn a burst of particles at `(cx, cy)` — the cosmic explosion.
///
/// Each particle gets a random angle (0..2π), random speed within
/// `[BURST_SPEED_MIN, BURST_SPEED_MAX)`, and a random spiral rate
/// within `[SPIRAL_RATE_MIN, SPIRAL_RATE_MAX)`. Color is sampled from
/// [`COSMIC_COLORS_RGB`]; glyph from [`BURST_CHARS`].
fn spawn_burst(pool: &mut ParticlePool, rng: &mut XorShift, cx: f32, cy: f32, count: u32) {
    for _ in 0..count {
        let angle = rng.next_f32() * std::f32::consts::TAU;
        let speed = lerp(BURST_SPEED_MIN, BURST_SPEED_MAX, rng.next_f32());
        let spiral_rate = lerp(SPIRAL_RATE_MIN, SPIRAL_RATE_MAX, rng.next_f32())
            * if rng.next_f32() < 0.5 { -1.0 } else { 1.0 };
        let (vx, vy) = (angle.cos() * speed, angle.sin() * speed);
        let color_idx = (rng.next_u32() % COSMIC_COLORS_RGB.len() as u32) as usize;
        let (r, g, b) = COSMIC_COLORS_RGB[color_idx];
        let ch = BURST_CHARS[(rng.next_u32() % BURST_CHARS.len() as u32) as usize];
        // Slight positional jitter so particles don't all overlap at spawn.
        let x = cx + (rng.next_f32() - 0.5) * 1.5;
        let y = cy + (rng.next_f32() - 0.5) * 1.5;
        let life = PARTICLE_LIFE_SECS * (0.7 + rng.next_f32() * 0.6);
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
            angle,
            speed,
            spiral_rate,
            active: true,
        });
        if !spawned {
            break; // pool full
        }
    }
}

/// Advance all active particles by `dt` seconds.
///
/// During Phase 3 (`morph_t > 0`), each particle's `angle` rotates
/// toward the downward direction (π/2), its `speed` decelerates toward
/// the morph range, and its color/glyph lerp toward the rain palette.
/// Particles whose life drops to ≤ 0 or that leave the screen are
/// killed and returned to the free-list.
fn update_particles(
    pool: &mut ParticlePool,
    dt: f32,
    screen_h: f32,
    morph_t: f32,
    palette_rgb: (u8, u8, u8),
    rain_chars: &[char],
) {
    let mut to_kill: Vec<usize> = Vec::new();
    let downward_angle = std::f32::consts::FRAC_PI_2; // 90° = down
    for (i, p) in pool.particles.iter_mut().enumerate() {
        if !p.active {
            continue;
        }
        // Apply spiral motion (always; rate scales down during morph).
        let spiral_scale = 1.0 - morph_t * 0.7;
        p.angle += p.spiral_rate * spiral_scale * dt;
        // During morph, lerp angle toward downward direction.
        if morph_t > 0.0 {
            // Compute the shortest signed angular delta to downward.
            let mut delta = downward_angle - p.angle;
            // Wrap to [-π, π].
            while delta > std::f32::consts::PI {
                delta -= std::f32::consts::TAU;
            }
            while delta < -std::f32::consts::PI {
                delta += std::f32::consts::TAU;
            }
            p.angle += delta * morph_t * dt * 2.0;
            // Decelerate toward morph speed range.
            let target_speed = lerp(MORPH_SPEED_MIN, MORPH_SPEED_MAX, 0.5);
            p.speed = lerp(p.speed, target_speed, morph_t * dt * 2.0);
            // Lerp color toward palette.
            let cur_rgb = (p.r, p.g, p.b);
            let new_rgb = lerp_rgb(cur_rgb, palette_rgb, morph_t * dt * 1.5);
            (p.r, p.g, p.b) = new_rgb;
            // Occasionally swap glyph to a rain char.
            if morph_t > 0.5 && !rain_chars.is_empty() {
                let swap_chance = (morph_t - 0.5) * 2.0 * dt * 4.0;
                if rng_freehand() < swap_chance {
                    // Knuth multiplicative hash for a deterministic but
                    // well-distributed per-index glyph pick. wrapping_mul
                    // avoids overflow on large pool indices.
                    let idx = (i as u32).wrapping_mul(2654435761u32) as usize % rain_chars.len();
                    p.ch = *rain_chars.get(idx).unwrap_or(&'0');
                }
            }
        }
        // Recompute cartesian velocity from polar.
        p.vx = p.angle.cos() * p.speed;
        p.vy = p.angle.sin() * p.speed;
        // During morph, add a downward bias to vy so particles fall.
        if morph_t > 0.0 {
            p.vy += MORPH_DOWNWARD_VY * morph_t * dt * 4.0;
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

/// Deterministic pseudo-random float in `[0, 1)` used by the glyph-swap
/// path in [`update_particles`]. We avoid threading the RNG through
/// `update_particles` to keep the function signature small; this helper
/// uses a per-call linear congruential step seeded by `Instant` nanos,
/// which is good enough for cosmetic glyph variation.
#[inline]
fn rng_freehand() -> f32 {
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

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_chars_are_distinct() {
        let mut sorted: Vec<char> = BURST_CHARS.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            BURST_CHARS.len(),
            "burst chars must be distinct"
        );
    }

    #[test]
    fn cosmic_colors_are_valid() {
        // Sanity: cosmic colors should have at least one bright channel.
        for &(r, g, b) in &COSMIC_COLORS_RGB {
            let max = r.max(g).max(b);
            assert!(max >= 200, "cosmic color ({r},{g},{b}) should be bright");
        }
    }

    #[test]
    fn cosmic_colors_match_brand_palette() {
        // Verify the brand purple is exactly the spec'd RGB.
        assert_eq!(COSMIC_COLORS_RGB[1], (168, 85, 247));
        // Verify gold and cyan match spec.
        assert_eq!(COSMIC_COLORS_RGB[0], (255, 200, 0));
        assert_eq!(COSMIC_COLORS_RGB[2], (0, 255, 255));
    }

    #[test]
    fn phase_boundaries_are_monotonic() {
        const {
            assert!(PHASE1_SINGULARITY_END_MS < PHASE2_BURST_END_MS);
        }
        const {
            assert!(PHASE2_BURST_END_MS < PHASE3_MORPH_END_MS);
        }
        const {
            assert!(PHASE3_MORPH_END_MS < PHASE4_RAIN_END_MS);
        }
    }

    #[test]
    fn phase_boundaries_match_spec() {
        // Spec: 0-1s singularity, 1-2.5s burst, 2.5-4s morph, 4-5s handoff.
        assert_eq!(PHASE1_SINGULARITY_END_MS, 1_000);
        assert_eq!(PHASE2_BURST_END_MS, 2_500);
        assert_eq!(PHASE3_MORPH_END_MS, 4_000);
        assert_eq!(PHASE4_RAIN_END_MS, 5_000);
    }

    #[test]
    fn burst_speed_range_is_valid() {
        const {
            assert!(BURST_SPEED_MIN < BURST_SPEED_MAX);
            assert!(BURST_SPEED_MIN >= 1.0);
            assert!(BURST_SPEED_MAX <= 100.0);
        }
    }

    #[test]
    fn morph_speed_range_is_valid() {
        const {
            assert!(MORPH_SPEED_MIN < MORPH_SPEED_MAX);
            assert!(MORPH_SPEED_MIN < BURST_SPEED_MIN, "morph should be slower");
        }
    }

    #[test]
    fn spiral_rate_range_is_valid() {
        const {
            assert!(SPIRAL_RATE_MIN < SPIRAL_RATE_MAX);
            assert!(SPIRAL_RATE_MIN > 0.0);
        }
    }

    #[test]
    fn burst_particle_count_fits_pool() {
        const {
            assert!(
                BURST_PARTICLE_COUNT as usize <= PARTICLE_POOL_SIZE,
                "burst particle count must fit in pool"
            );
        }
    }

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
            angle: std::f32::consts::FRAC_PI_2,
            speed: 100.0,
            spiral_rate: 0.0,
            active: true,
        });
        // Screen height 24 — particle at y=50 is already off-screen.
        update_particles(&mut pool, 0.1, 24.0, 0.0, (57, 255, 20), &['0', '1']);
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
            angle: 0.0,
            speed: 0.0,
            spiral_rate: 0.0,
            active: true,
        });
        // After 0.1s, life = 0.05 - 0.1 = negative → killed.
        update_particles(&mut pool, 0.1, 24.0, 0.0, (57, 255, 20), &['0', '1']);
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
            angle: std::f32::consts::FRAC_PI_2,
            speed: 1.0,
            spiral_rate: 0.0,
            active: true,
        });
        update_particles(&mut pool, 0.1, 24.0, 0.0, (57, 255, 20), &['0', '1']);
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn update_particles_morph_shifts_color_toward_palette() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 10.0,
            vy: 0.0,
            ch: '*',
            r: 255,
            g: 200,
            b: 0, // gold
            life: 5.0,
            max_life: 5.0,
            angle: 0.0,
            speed: 10.0,
            spiral_rate: 0.0,
            active: true,
        });
        // Find the spawned particle's index (free-list pops from the end,
        // so first spawn goes to PARTICLE_POOL_SIZE - 1, not 0).
        let active_idx = pool
            .particles
            .iter()
            .position(|p| p.active)
            .expect("spawned particle should be active");
        let before = (
            pool.particles[active_idx].r,
            pool.particles[active_idx].g,
            pool.particles[active_idx].b,
        );
        // Run several iterations of morph at morph_t = 1.0 (full morph).
        for _ in 0..30 {
            update_particles(&mut pool, 0.05, 50.0, 1.0, (57, 255, 20), &['0', '1']);
        }
        let after = (
            pool.particles[active_idx].r,
            pool.particles[active_idx].g,
            pool.particles[active_idx].b,
        );
        // Color should have shifted away from pure gold.
        assert_ne!(before, after, "morph should change particle color");
        // The green channel should have increased (target is 57,255,20).
        assert!(after.1 > before.1, "green channel should increase");
    }

    #[test]
    fn update_particles_morph_adds_downward_bias() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 20.0,
            vy: 0.0, // moving right, not down
            ch: '*',
            r: 255,
            g: 200,
            b: 0,
            life: 5.0,
            max_life: 5.0,
            angle: 0.0,
            speed: 20.0,
            spiral_rate: 0.0,
            active: true,
        });
        let active_idx = pool
            .particles
            .iter()
            .position(|p| p.active)
            .expect("spawned particle should be active");
        let vy_before = pool.particles[active_idx].vy;
        update_particles(&mut pool, 0.1, 50.0, 1.0, (57, 255, 20), &['0', '1']);
        let vy_after = pool.particles[active_idx].vy;
        // vy should have increased (become more positive = more downward).
        assert!(
            vy_after > vy_before,
            "morph should add downward bias: before={vy_before}, after={vy_after}"
        );
    }

    #[test]
    fn rng_freehand_returns_unit_range() {
        for _ in 0..1000 {
            let f = rng_freehand();
            assert!((0.0..1.0).contains(&f), "rng_freehand returned {f}");
        }
    }

    #[test]
    fn spawn_burst_populates_pool() {
        let mut pool = ParticlePool::new();
        let mut rng = XorShift::new(123);
        spawn_burst(&mut pool, &mut rng, 40.0, 12.0, 50);
        assert_eq!(pool.active_count(), 50);
        // Each spawned particle should have valid polar + cartesian fields.
        for p in &pool.particles {
            if !p.active {
                continue;
            }
            assert!(p.speed >= BURST_SPEED_MIN * 0.95);
            assert!(p.speed <= BURST_SPEED_MAX * 1.05);
            // vx, vy should be consistent with angle/speed.
            let expected_vx = p.angle.cos() * p.speed;
            let expected_vy = p.angle.sin() * p.speed;
            assert!(
                (p.vx - expected_vx).abs() < 0.1,
                "vx inconsistent with angle/speed"
            );
            assert!(
                (p.vy - expected_vy).abs() < 0.1,
                "vy inconsistent with angle/speed"
            );
        }
    }

    #[test]
    fn spawn_burst_handles_full_pool() {
        let mut pool = ParticlePool::new();
        // Fill the pool.
        for _ in 0..PARTICLE_POOL_SIZE {
            assert!(pool.spawn(Particle::INACTIVE));
        }
        let mut rng = XorShift::new(456);
        // spawn_burst should silently bail when the pool is full.
        spawn_burst(&mut pool, &mut rng, 40.0, 12.0, 50);
        // No new particles spawned — i.e., no particle should have
        // `active == true` since we filled the pool with INACTIVE (which
        // has `active: false`) and spawn_burst couldn't replace any of them.
        let any_active = pool.particles.iter().any(|p| p.active);
        assert!(
            !any_active,
            "spawn_burst should not have spawned any active particles when pool is full"
        );
    }
}
