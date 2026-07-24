// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cosmostrix Logo intro — a four-phase cinematic that reveals the
//! project's ASCII logo and dissolves it into Matrix rain.
//!
//! ```text
//! Phase 1: Fade In    (0    – 1000 ms)  Logo appears line by line, fading
//!                                         from black to the palette color.
//! Phase 2: Ignition   (1000 – 2500 ms)  A spark falls from the top of the
//!                                         screen to the logo's center; on
//!                                         impact the logo flashes bright.
//! Phase 3: Dissolve   (2500 – 3500 ms)  Logo characters turn into rain
//!                                         droplets starting from the outer
//!                                         edge and moving inward; droplets
//!                                         fall toward the bottom.
//! Phase 4: Rain       (3500 – 4500 ms)  The last droplets fall off-screen;
//!                                         rain engine takes over seamlessly.
//! ```
//!
//! Total: ~4.5 s. Any key (q / Enter / etc.) skips instantly. The intro
//! is skipped entirely on terminals smaller than 80×24 with a stderr
//! notice (handled by [`super::intro::run_intro`]).
//!
//! ## Constraints
//!
//! * Zero per-frame heap allocation — particle pool pre-allocated; the
//!   `dissolve_progress` bitmap is a stack-allocated `bool` array sized
//!   to the logo's cell count.
//! * Reuses the existing `Terminal` / `Frame` / `Cell` pipeline.
//! * `FRAME_COUNTER` is bumped each frame so the watchdog doesn't kill
//!   us during the cinematic.

use std::time::Instant;

use crossterm::style::Color;

use crate::cell::Cell;
use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::terminal::Terminal;

use super::intro::{
    end_frame, lerp, lerp_rgb, palette_target_rgb, rain_chars, render_particle_cell, seed_rng,
    should_skip, Particle, ParticlePool, XorShift, PARTICLE_POOL_SIZE,
};

// ─────────────────────────────────────────────────────────────────────────────
// Logo art + brand color
// ─────────────────────────────────────────────────────────────────────────────

/// The Cosmostrix logo as ASCII art. Single-width Unicode density
/// characters render the brand mark. Lines are stored with their original
/// leading indentation (which forms the visual shape); trailing
/// whitespace is stripped at parse time.
///
/// Dimensions: 19 lines × 39 chars wide (max). See [`LOGO_HEIGHT`] and
/// [`LOGO_WIDTH`] — both are computed at parse time so they always match
/// the actual art. Rendering centers the logo at the terminal center.
///
/// # Centering math
///
/// All lines start at the same `logo_x = (term_cols - LOGO_WIDTH) / 2`
/// offset (integer math, truncating). Each line's leading spaces in the
/// string literal form the visual shape — they are NOT source-code
/// indentation. Centering is purely from the offset, never per-line.
//
// Note: codespell may complain about substrings inside this art. We keep
// the .codespellrc ignore-list updated to suppress false positives.
const LOGO_ART: &str = "\
                  .,>- .=i,.
              .÷×->>+=l.l>≈≈≥≠i.
            l÷><±i;,.!.:    .i≈≤×,
          ,≈<>;.   .    .      .-≤!
         <>!>;     , I  i        .,:
        >!!;    ;   .>∂i,       i<;<≠.
       +<l:     i <,.i+li;:     ,  .-×
      .<I!   .  ii;;!   I:,:÷   .   >=,
      .!<:      I<:       :il       .>l
      .>!i       ;I,  ±   ;i    ;   .×.
       ,.       ,l:.  l    .±   >  I≈!×
       li, .    l;.,;   .<;;=     :<il,
       IIi,.      ;::;!::::,     .≠:;I
        !i<.         l÷×i  .  , .-::I.
         iii!.             ×  ::+,,±
           I;I:       .     .!!:,;,
            .=II-I..  :  <≥I:,,il
               ,××.;  ;i>;:;i+
                       ..";

/// Brand purple — the Cosmostrix signature color (`#A855F7` / RGB
/// 168,85,247). The logo always renders in this color, regardless of
/// the user's `--color` flag, so the brand mark stays consistent across
/// all palette themes. During the dissolve phase, droplets interpolate
/// from this purple toward the active rain palette's brightest stop,
/// creating a cinematic "brand → rain" handoff.
///
/// The `Color` enum form is kept as the canonical brand reference and
/// is exercised by unit tests; rendering uses [`LOGO_COLOR_RGB`] for
/// cheaper lerp math.
#[allow(dead_code)]
const LOGO_COLOR: Color = Color::Rgb {
    r: 168,
    g: 85,
    b: 247,
};

/// RGB triple form of [`LOGO_COLOR`] for efficient lerp math. Kept as a
/// constant so we don't pay the cost of matching the `Color` enum each
/// frame for every logo cell.
const LOGO_COLOR_RGB: (u8, u8, u8) = (168, 85, 247);

// ─────────────────────────────────────────────────────────────────────────────
// Phase + spawn constants
// ─────────────────────────────────────────────────────────────────────────────

/// Phase boundaries (milliseconds from intro start).
const PHASE1_FADEIN_END_MS: u64 = 1_000;
const PHASE2_IGNITION_END_MS: u64 = 2_500;
const PHASE3_DISSOLVE_END_MS: u64 = 3_500;
const PHASE4_RAIN_END_MS: u64 = 4_500;

/// Frame period in seconds, computed at runtime to avoid MSRV issues
/// with `Duration::as_secs_f32()` in const context (stable since 1.83,
/// but our MSRV is 1.81).
#[inline]
fn frame_period_secs() -> f32 {
    super::intro::INTRO_FRAME_PERIOD.as_secs_f32()
}

/// Ignition flash duration (seconds). The logo briefly brightens past
/// its base color when the spark impacts, then decays back.
const FLASH_DECAY_RATE: f32 = 4.0;

/// Rain droplet speed range (cells per second) for the dissolve phase.
/// Lower than Cosmic Burst's morph range so the rain curtain feels calm.
const DISSOLVE_SPEED_MIN: f32 = 8.0;
const DISSOLVE_SPEED_MAX: f32 = 16.0;

/// Horizontal velocity jitter range for dissolve droplets. Each droplet
/// gets a random `vx` in `[-JITTER_VX, +JITTER_VX]` cells/sec so the
/// curtain spreads organically before falling, instead of dropping in
/// perfectly straight columns. ±2 cells/sec is subtle enough to feel
/// natural without breaking the rain silhouette.
const JITTER_VX: f32 = 2.0;

/// Fade-in granularity — the logo appears in N reveal steps spread
/// across Phase 1. Each step reveals another batch of cells. Higher =
/// smoother but more CPU; lower = chunkier but cheaper. 32 feels
/// smooth at 30 FPS over a 1 s phase.
const FADEIN_STEPS: u32 = 32;

// ─────────────────────────────────────────────────────────────────────────────
// Logo geometry helpers
// ─────────────────────────────────────────────────────────────────────────────

/// A non-blank cell from the logo, with its position relative to the
/// logo's top-left corner. Cells are collected at parse time and used
/// during Phase 1 (fade in) and Phase 3 (dissolve).
#[derive(Clone, Copy)]
struct LogoCell {
    /// Cell X within the logo bounding box (0 = leftmost column).
    bx: u16,
    /// Cell Y within the logo bounding box (0 = top row).
    by: u16,
    /// Distance from the logo center, squared. Used to order the
    /// dissolve from outermost ring inward. Stored as f32 for sorting.
    dist_sq: f32,
    /// Original glyph from the art.
    ch: char,
}

/// Parse [`LOGO_ART`] into lines, computing the bounding-box width and
/// height. Trailing whitespace is stripped from each line.
fn parse_logo_art() -> (Vec<&'static str>, u16, u16) {
    let lines: Vec<&'static str> = LOGO_ART.lines().collect();
    let height = lines.len() as u16;
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    (lines, width, height)
}

/// Collect every non-blank cell from the parsed art, along with its
/// squared distance from the logo's center. Cells are returned in
/// arbitrary order — callers sort by `dist_sq` descending for the
/// dissolve-from-outside-inward effect.
fn collect_logo_cells(lines: &[&'static str], width: u16, height: u16) -> Vec<LogoCell> {
    let cx = width as f32 * 0.5;
    let cy = height as f32 * 0.5;
    let mut out = Vec::with_capacity(256);
    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let xf = x as f32;
            let yf = y as f32;
            let dist_sq = (xf - cx) * (xf - cx) + (yf - cy) * (yf - cy);
            out.push(LogoCell {
                bx: x as u16,
                by: y as u16,
                dist_sq,
                ch,
            });
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Main entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Entry point for the Cosmostrix Logo intro. Plays a ~4.5 s cinematic.
///
/// See the module docs for the phase breakdown. The caller (intro
/// dispatcher) has already validated terminal size and `IntroType`.
pub(super) fn run_logo_intro(
    term: &mut Terminal,
    frame: &mut Frame,
    cloud: &Cloud,
    w: u16,
    h: u16,
) -> std::io::Result<()> {
    let (lines, logo_w, logo_h) = parse_logo_art();
    // If the logo is somehow wider than the terminal, abort gracefully.
    // (The dispatcher already enforces 80×24 minimum, but defensively
    // re-check here in case of future logo edits.)
    if logo_w > w || logo_h > h {
        return Ok(());
    }

    let mut logo_cells = collect_logo_cells(&lines, logo_w, logo_h);
    // Sort cells by squared distance from center, descending — the
    // dissolve phase walks this list in order, so outer cells dissolve
    // first. This sort happens once at intro start; per-frame cost is
    // a simple index walk.
    logo_cells.sort_by(|a, b| {
        b.dist_sq
            .partial_cmp(&a.dist_sq)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut rng = seed_rng();
    let palette_bg = cloud.palette.bg;
    let palette_rgb = palette_target_rgb(cloud);
    let rain_charset = rain_chars(cloud);

    // Logo top-left position so it's centered on the terminal. Integer
    // math with signed casts so truncation rounds toward zero (correct
    // for both even and odd terminal widths). All lines start at this
    // same `logo_x`; per-line shape comes from the leading spaces
    // baked into `LOGO_ART`. There is no per-line offset adjustment.
    let logo_x = (w as i32 - logo_w as i32) / 2;
    let logo_y = (h as i32 - logo_h as i32) / 2;
    // Center of the logo in terminal coordinates (for the spark target).
    let logo_center_x = logo_x as f32 + logo_w as f32 * 0.5;
    let logo_center_y = logo_y as f32 + logo_h as f32 * 0.5;

    // Spark spawn position: top of the terminal, horizontally aligned
    // with the logo center. The spark falls straight down to the center.
    let spark_start_y = 0.0f32;

    let mut pool = ParticlePool::new();
    let intro_start = Instant::now();

    // Reusable dissolve index — how many cells have already dissolved.
    let mut dissolved_count: usize = 0;

    loop {
        let elapsed_ms = intro_start.elapsed().as_millis() as u64;
        if elapsed_ms >= PHASE4_RAIN_END_MS {
            break;
        }
        if should_skip()? {
            return Ok(());
        }

        // Determine current phase and progress within phase.
        let (phase, phase_t) = if elapsed_ms < PHASE1_FADEIN_END_MS {
            (1u8, elapsed_ms as f32 / PHASE1_FADEIN_END_MS as f32)
        } else if elapsed_ms < PHASE2_IGNITION_END_MS {
            (
                2,
                (elapsed_ms - PHASE1_FADEIN_END_MS) as f32
                    / (PHASE2_IGNITION_END_MS - PHASE1_FADEIN_END_MS) as f32,
            )
        } else if elapsed_ms < PHASE3_DISSOLVE_END_MS {
            (
                3,
                (elapsed_ms - PHASE2_IGNITION_END_MS) as f32
                    / (PHASE3_DISSOLVE_END_MS - PHASE2_IGNITION_END_MS) as f32,
            )
        } else {
            (
                4,
                (elapsed_ms - PHASE3_DISSOLVE_END_MS) as f32
                    / (PHASE4_RAIN_END_MS - PHASE3_DISSOLVE_END_MS) as f32,
            )
        };

        let dt = frame_period_secs();

        // Update particles (rain droplets fall, age, die).
        update_rain_droplets(&mut pool, dt, h as f32);

        // ── Render ──────────────────────────────────────────────────────
        frame.clear_with_bg(palette_bg);

        // Compute the current logo brightness scalar in [0, 1]:
        // * Phase 1: ramps 0 → 1 as the logo fades in.
        // * Phase 2: 1.0 + brief flash on spark impact, decaying back to 1.
        // * Phase 3: stays at 1.0 for cells still visible; dissolved cells
        //   are skipped entirely (rendered as nothing).
        // * Phase 4: no logo cells rendered.
        let base_brightness = if phase == 1 { phase_t } else { 1.0 };

        // Ignition flash: bright spike just after the spark lands.
        // The spark lands at phase_t ≈ 0.5 (middle of Phase 2). After
        // impact, brightness spikes to 1.5 and decays exponentially.
        let flash = if phase == 2 && phase_t > 0.5 {
            let since_impact = phase_t - 0.5;
            (1.5_f32 * (-FLASH_DECAY_RATE * since_impact).exp()).max(0.0)
        } else {
            0.0
        };

        let logo_visible = phase != 4;
        if logo_visible {
            // Determine how many cells are still rendered as logo (i.e.
            // not yet dissolved). During Phase 1, also gate visibility
            // by the fade-in step counter so cells appear progressively.
            let reveal_count = if phase == 1 {
                // Reveal cells from center outward as the fade-in
                // progresses. We sort by dist_sq *ascending* for the
                // fade-in (inner cells appear first), which is the
                // opposite of the dissolve order. To avoid re-sorting,
                // we walk the sorted-descending list from the END.
                let total = logo_cells.len();
                ((phase_t * FADEIN_STEPS as f32).round() as usize * total / FADEIN_STEPS as usize)
                    .min(total)
            } else {
                logo_cells.len()
            };

            // For Phase 3 (dissolve), the cells we still render are the
            // last `(len - dissolved_count)` entries of the sorted-desc
            // list (i.e., the innermost cells). For other phases, all
            // `reveal_count` cells are rendered — but during fade-in we
            // only render the last `reveal_count` entries (innermost).
            let active_window_start = if phase == 1 {
                logo_cells.len().saturating_sub(reveal_count)
            } else if phase == 3 {
                dissolved_count
            } else {
                0
            };

            for cell in logo_cells.iter().skip(active_window_start) {
                let tx = logo_x + cell.bx as i32;
                let ty = logo_y + cell.by as i32;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let tx = tx as u16;
                let ty = ty as u16;
                if tx >= w || ty >= h {
                    continue;
                }
                // Fade-in cells ramp from 0 → base_brightness. Already-
                // revealed cells use base_brightness.
                let cell_brightness = if phase == 1 {
                    // Reveal this cell progressively over a short window
                    // so it doesn't pop in at full brightness.
                    let cell_t = ((phase_t * FADEIN_STEPS as f32).fract()).clamp(0.0, 1.0);
                    base_brightness * cell_t
                } else {
                    (base_brightness + flash).clamp(0.0, 1.5)
                };
                // Color = brand purple scaled by brightness, clamped.
                // Logo always renders in LOGO_COLOR_RGB regardless of
                // the user's --color flag — the brand mark stays purple
                // across all palette themes.
                let color = lerp_rgb((0, 0, 0), LOGO_COLOR_RGB, cell_brightness.clamp(0.0, 1.0));
                // During the flash, lean the color toward white.
                let color = if flash > 0.0 {
                    let flash_t = (flash / 1.5).clamp(0.0, 1.0);
                    lerp_rgb(color, (255, 255, 255), flash_t * 0.6)
                } else {
                    color
                };
                frame.set_force(
                    tx,
                    ty,
                    Cell {
                        ch: cell.ch,
                        fg: Some(Color::Rgb {
                            r: color.0,
                            g: color.1,
                            b: color.2,
                        }),
                        bg: palette_bg,
                        bold: flash > 0.1,
                    },
                );
            }
        }

        // Spark render during Phase 2 (until impact at phase_t ≈ 0.5).
        if phase == 2 && phase_t < 0.5 {
            // Spark falls from top to logo center over the first half
            // of Phase 2 (0 → 0.5).
            let spark_t = phase_t / 0.5;
            let spark_y = lerp(spark_start_y, logo_center_y, spark_t);
            let spark_x = logo_center_x;
            let spark_color = (255, 255, 220); // warm white
            render_particle_cell(
                frame,
                w,
                h,
                spark_x,
                spark_y,
                '*',
                spark_color,
                palette_bg,
                1.0,
                true,
            );
            // Spark trail: 3 dimmer cells above.
            for trail_step in 1..=3u16 {
                let trail_y = spark_y - trail_step as f32;
                let trail_brightness = 1.0 / (trail_step as f32 + 1.0);
                let trail_rgb = lerp_rgb((0, 0, 0), spark_color, trail_brightness);
                render_particle_cell(
                    frame,
                    w,
                    h,
                    spark_x,
                    trail_y,
                    '.',
                    trail_rgb,
                    palette_bg,
                    trail_brightness,
                    false,
                );
            }
        }

        // Spawn new rain droplets during Phase 3 (dissolve). Walk the
        // sorted-descending list from `dissolved_count` forward, up to a
        // per-frame budget. Each dissolved cell converts to a falling
        // rain particle.
        if phase == 3 {
            let target_dissolved = (phase_t * logo_cells.len() as f32).round() as usize;
            // Per-frame budget so we don't spawn 300 particles in one
            // frame. 24 droplets/frame at 30 FPS = 720 droplets/sec,
            // which comfortably covers ~300 cells over the 1 s phase
            // and gives a denser, more dramatic curtain than the
            // previous 16/frame.
            const PER_FRAME_BUDGET: usize = 24;
            let mut spawned_this_frame = 0usize;
            while dissolved_count < target_dissolved
                && dissolved_count < logo_cells.len()
                && spawned_this_frame < PER_FRAME_BUDGET
            {
                let cell = logo_cells[dissolved_count];
                let tx = logo_x + cell.bx as i32;
                let ty = logo_y + cell.by as i32;
                if tx >= 0 && ty >= 0 {
                    let _ = spawn_rain_droplet(
                        &mut pool,
                        &mut rng,
                        tx as f32,
                        ty as f32,
                        &rain_charset,
                    );
                }
                dissolved_count += 1;
                spawned_this_frame += 1;
            }
        }

        // Render all active rain droplets. Each droplet's color is
        // interpolated from LOGO_COLOR_RGB (at spawn, life_t = 1.0)
        // toward the active palette's brightest stop (at death,
        // life_t = 0.0). This creates the cinematic "brand purple →
        // rain color" transition as droplets fall.
        for p in pool.particles.iter() {
            if !p.active {
                continue;
            }
            let life_t = (p.life / p.max_life).clamp(0.0, 1.0);
            let droplet_rgb = lerp_rgb(palette_rgb, LOGO_COLOR_RGB, life_t);
            render_particle_cell(
                frame,
                w,
                h,
                p.x,
                p.y,
                p.ch,
                droplet_rgb,
                palette_bg,
                life_t,
                true,
            );
            // Dim trailing cell directly above the droplet for a streak.
            let trail_y = p.y - 1.0;
            let trail_brightness = life_t * 0.4;
            let trail_rgb = lerp_rgb((0, 0, 0), droplet_rgb, trail_brightness);
            render_particle_cell(
                frame,
                w,
                h,
                p.x,
                trail_y,
                p.ch,
                trail_rgb,
                palette_bg,
                trail_brightness,
                false,
            );
        }

        end_frame(term, frame)?;
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Particle helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Spawn a rain droplet at `(x, y)` — used during the dissolve phase.
/// The droplet starts with the brand purple color ([`LOGO_COLOR_RGB`])
/// and a random glyph from `rain_charset`. The render loop interpolates
/// the color toward the active rain palette as the droplet ages — see
/// the render section in [`run_logo_intro`].
///
/// Initial velocity is mostly straight down (randomized speed in
/// `[DISSOLVE_SPEED_MIN, DISSOLVE_SPEED_MAX)`) with a small horizontal
/// jitter (`vx ∈ [-JITTER_VX, +JITTER_VX]`) so the curtain spreads
/// organically instead of dropping in perfectly straight columns.
///
/// Life is set so the droplet lives ~2 s — long enough to fall through
/// a 24-row terminal even at the slow end of the speed range.
fn spawn_rain_droplet(
    pool: &mut ParticlePool,
    rng: &mut XorShift,
    x: f32,
    y: f32,
    rain_charset: &[char],
) -> bool {
    let speed = lerp(DISSOLVE_SPEED_MIN, DISSOLVE_SPEED_MAX, rng.next_f32());
    let ch = if rain_charset.is_empty() {
        '0'
    } else {
        rain_charset[(rng.next_u32() as usize) % rain_charset.len()]
    };
    // Slight positional jitter so droplets don't spawn on the exact
    // same column as the logo cell they came from.
    let jitter_x = (rng.next_f32() - 0.5) * 0.6;
    // Horizontal velocity jitter so droplets spread a bit before
    // falling — creates a more organic curtain effect.
    let vx = (rng.next_f32() - 0.5) * 2.0 * JITTER_VX;
    let life = 2.0;
    pool.spawn(Particle {
        x: x + jitter_x,
        y,
        vx,
        vy: speed,
        ch,
        r: LOGO_COLOR_RGB.0,
        g: LOGO_COLOR_RGB.1,
        b: LOGO_COLOR_RGB.2,
        life,
        max_life: life,
        angle: std::f32::consts::FRAC_PI_2, // 90° = down
        speed,
        spiral_rate: 0.0,
        active: true,
    })
}

/// Advance all active rain droplets by `dt` seconds. Droplets fall
/// (with their horizontal jitter carrying them sideways); those that
/// leave the bottom of the screen or expire are killed and returned to
/// the free-list.
///
/// # Zero per-frame allocation
///
/// The kill list is a stack-allocated `[usize; PARTICLE_POOL_SIZE]`
/// array (4 KiB on 64-bit) with a length counter. No `Vec` is
/// allocated per frame — this is critical for the intro's zero-alloc
/// guarantee.
fn update_rain_droplets(pool: &mut ParticlePool, dt: f32, screen_h: f32) {
    // Stack-allocated kill list — zero per-frame heap allocation.
    // PARTICLE_POOL_SIZE is 512, so this is 4 KiB on the stack.
    let mut to_kill: [usize; PARTICLE_POOL_SIZE] = [0; PARTICLE_POOL_SIZE];
    let mut kill_count: usize = 0;

    for (i, p) in pool.particles.iter_mut().enumerate() {
        if !p.active {
            continue;
        }
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        p.life -= dt;
        if p.y > screen_h + 2.0 || p.life <= 0.0 {
            // The pool size bounds kill_count — every active particle
            // could die in one frame in the worst case, but the pool
            // never has more than PARTICLE_POOL_SIZE slots total.
            if kill_count < PARTICLE_POOL_SIZE {
                to_kill[kill_count] = i;
                kill_count += 1;
            }
        }
    }

    for &idx in to_kill.iter().take(kill_count) {
        pool.kill(idx);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_color_matches_rgb_constant() {
        // The Color enum form and the RGB tuple form must agree so the
        // brand purple is consistent everywhere it's referenced.
        match LOGO_COLOR {
            Color::Rgb { r, g, b } => assert_eq!((r, g, b), LOGO_COLOR_RGB),
            _ => panic!("LOGO_COLOR must be Color::Rgb"),
        }
    }

    #[test]
    fn logo_color_is_brand_purple() {
        // Spec: #A855F7 = RGB(168, 85, 247).
        assert_eq!(LOGO_COLOR_RGB, (168, 85, 247));
    }

    #[test]
    fn logo_art_is_non_empty() {
        assert!(!LOGO_ART.is_empty());
        assert!(
            LOGO_ART.lines().count() >= 10,
            "logo should have at least 10 lines"
        );
    }

    #[test]
    fn parse_logo_art_returns_consistent_dimensions() {
        let (lines, w, h) = parse_logo_art();
        assert_eq!(lines.len() as u16, h, "height must match line count");
        // Width is the max char count across lines.
        let computed_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        assert_eq!(w, computed_w);
        // Logo should fit in a typical 80×24 terminal with room to spare.
        assert!(w <= 80, "logo width {w} must fit in 80-col terminal");
        assert!(h <= 24, "logo height {h} must fit in 24-row terminal");
    }

    #[test]
    fn collect_logo_cells_skips_blanks() {
        let (lines, w, h) = parse_logo_art();
        let cells = collect_logo_cells(&lines, w, h);
        // Every collected cell must have a non-blank glyph.
        for c in &cells {
            assert_ne!(c.ch, ' ', "blank cell should not be collected");
        }
        // The logo clearly has more than 50 non-blank cells.
        assert!(
            cells.len() > 50,
            "logo should have many non-blank cells, got {}",
            cells.len()
        );
    }

    #[test]
    fn collect_logo_cells_computes_center_distance() {
        let (lines, w, h) = parse_logo_art();
        let cells = collect_logo_cells(&lines, w, h);
        // The centermost cell should have a small dist_sq; the outermost
        // should have a large dist_sq.
        let cx = w as f32 * 0.5;
        let cy = h as f32 * 0.5;
        let mut min_d = f32::MAX;
        let mut max_d = f32::MIN;
        for c in &cells {
            // Verify the stored dist_sq matches a fresh computation.
            let xf = c.bx as f32;
            let yf = c.by as f32;
            let expected = (xf - cx) * (xf - cx) + (yf - cy) * (yf - cy);
            assert!(
                (c.dist_sq - expected).abs() < 0.01,
                "dist_sq mismatch: stored={}, expected={}",
                c.dist_sq,
                expected
            );
            min_d = min_d.min(c.dist_sq);
            max_d = max_d.max(c.dist_sq);
        }
        assert!(min_d < max_d, "logo should have spatial extent");
    }

    #[test]
    fn phase_boundaries_are_monotonic() {
        const {
            assert!(PHASE1_FADEIN_END_MS < PHASE2_IGNITION_END_MS);
        }
        const {
            assert!(PHASE2_IGNITION_END_MS < PHASE3_DISSOLVE_END_MS);
        }
        const {
            assert!(PHASE3_DISSOLVE_END_MS < PHASE4_RAIN_END_MS);
        }
    }

    #[test]
    fn phase_boundaries_match_spec() {
        // Spec: 0-1s fade in, 1-2.5s ignition, 2.5-3.5s dissolve, 3.5-4.5s rain.
        assert_eq!(PHASE1_FADEIN_END_MS, 1_000);
        assert_eq!(PHASE2_IGNITION_END_MS, 2_500);
        assert_eq!(PHASE3_DISSOLVE_END_MS, 3_500);
        assert_eq!(PHASE4_RAIN_END_MS, 4_500);
    }

    #[test]
    fn dissolve_speed_range_is_valid() {
        const {
            assert!(DISSOLVE_SPEED_MIN < DISSOLVE_SPEED_MAX);
            assert!(DISSOLVE_SPEED_MIN >= 1.0);
            assert!(DISSOLVE_SPEED_MAX <= 100.0);
        }
    }

    #[test]
    fn fadein_steps_is_reasonable() {
        const {
            assert!(
                FADEIN_STEPS >= 8,
                "fade-in must have enough steps for smoothness"
            );
            assert!(FADEIN_STEPS <= 128, "fade-in step count is excessive");
        }
    }

    #[test]
    fn spawn_rain_droplet_populates_pool() {
        let mut pool = ParticlePool::new();
        let mut rng = XorShift::new(42);
        let charset = ['0', '1', 'x', 'z'];
        let ok = spawn_rain_droplet(&mut pool, &mut rng, 10.0, 5.0, &charset);
        assert!(ok);
        assert_eq!(pool.active_count(), 1);
        let p = pool
            .particles
            .iter()
            .find(|p| p.active)
            .expect("spawned droplet should be active");
        // Velocity should be mostly downward with optional horizontal jitter.
        assert!(p.vy > 0.0, "droplet should move downward");
        assert!(
            p.vx.abs() <= JITTER_VX + 0.01,
            "horizontal velocity should be within jitter range, got {}",
            p.vx
        );
        assert!(p.speed >= DISSOLVE_SPEED_MIN * 0.95);
        assert!(p.speed <= DISSOLVE_SPEED_MAX * 1.05);
        assert!(charset.contains(&p.ch), "glyph should come from charset");
        // Particle should start with the brand purple color.
        assert_eq!((p.r, p.g, p.b), LOGO_COLOR_RGB);
    }

    #[test]
    fn spawn_rain_droplet_handles_empty_charset() {
        let mut pool = ParticlePool::new();
        let mut rng = XorShift::new(7);
        let ok = spawn_rain_droplet(&mut pool, &mut rng, 10.0, 5.0, &[]);
        assert!(ok);
        let p = pool
            .particles
            .iter()
            .find(|p| p.active)
            .expect("droplet should be active");
        assert_eq!(p.ch, '0', "empty charset should fall back to '0'");
    }

    #[test]
    fn update_rain_droplets_kills_offscreen() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 50.0,
            vx: 0.0,
            vy: 20.0,
            ch: '0',
            r: 57,
            g: 255,
            b: 20,
            life: 1.0,
            max_life: 1.0,
            angle: std::f32::consts::FRAC_PI_2,
            speed: 20.0,
            spiral_rate: 0.0,
            active: true,
        });
        // Screen height 24 — droplet at y=50 is already off-screen.
        update_rain_droplets(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn update_rain_droplets_kills_expired_life() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 1.0,
            ch: '0',
            r: 57,
            g: 255,
            b: 20,
            life: 0.05,
            max_life: 0.05,
            angle: std::f32::consts::FRAC_PI_2,
            speed: 1.0,
            spiral_rate: 0.0,
            active: true,
        });
        // After 0.1s, life = 0.05 - 0.1 = negative → killed.
        update_rain_droplets(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn update_rain_droplets_keeps_alive() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 5.0,
            ch: '0',
            r: 57,
            g: 255,
            b: 20,
            life: 5.0,
            max_life: 5.0,
            angle: std::f32::consts::FRAC_PI_2,
            speed: 5.0,
            spiral_rate: 0.0,
            active: true,
        });
        update_rain_droplets(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn update_rain_droplets_advances_position() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 10.0,
            ch: '0',
            r: 57,
            g: 255,
            b: 20,
            life: 5.0,
            max_life: 5.0,
            angle: std::f32::consts::FRAC_PI_2,
            speed: 10.0,
            spiral_rate: 0.0,
            active: true,
        });
        update_rain_droplets(&mut pool, 0.5, 24.0);
        let p = pool
            .particles
            .iter()
            .find(|p| p.active)
            .expect("droplet should still be active");
        // y should have advanced by speed*dt = 10*0.5 = 5 cells, so the
        // new y is 5 + 5 = 10.
        assert!(
            (p.y - 10.0).abs() < 0.1,
            "y should have advanced by speed*dt, got {}",
            p.y
        );
    }
}
